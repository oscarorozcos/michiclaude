// michi — el relevo de MichiClaude (etapa 3 de docs/remediacion.md).
//
// QUÉ ES. El usuario teclea `michi claude` en su terminal de siempre y en
// su carpeta de siempre. Por dentro, Claude Code arranca dentro de una
// ConPTY que este programa controla y todo se reenvía tal cual: teclas,
// pantalla, colores, resize, Ctrl+C. Claude Code ni se entera. A cambio,
// MichiClaude gana un canal por el que puede pedir un /compact o un
// /clear sin que nadie teclee encima del usuario.
//
// POR QUÉ UN CRATE APARTE y no un binario del crate de Tauri:
//   - la app no gana ni una dependencia (invariante #4): portable-pty
//     vive solo aquí;
//   - si el relevo no compila, la app sigue compilando y publicándose.
//
// EL CANAL — archivos, no un named pipe (corrección al diseño original;
// el porqué está en docs/remediacion.md §"Decisiones de la etapa 3"):
//   %APPDATA%\com.oscarorozco.michiclaude\relevo\
//     <pid>.json   estado, lo escribe el relevo cada ~500 ms
//     <pid>.cmd    una orden, la escribe la app y el relevo la borra al leerla
// Ambos se escriben con tmp+rename para que nadie lea un archivo a medias.
//
// PRIVACIDAD (regla dura). El relevo VE todo lo que el usuario teclea
// porque está en medio del cable, pero NUNCA lo escribe en disco ni lo
// manda a ningún sitio. Del tecleo solo salen del proceso: un booleano
// ("hay texto sin enviar"), relojes de inactividad, y —si el usuario
// escribió él mismo /compact o /clear— cuál de esos dos comandos fue.
// Nada más. Ni una letra del contenido.
//
// SEGURIDAD. El canal solo acepta DOS textos, comparados literalmente:
// "/compact" y "/clear". No hay forma de que este programa teclee otra
// cosa dentro de la sesión del usuario, venga la orden de donde venga.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

/// Versión del formato del archivo de estado. Si cambia la forma, sube —
/// el panel debe poder ignorar un relevo viejo sin romperse.
const STATE_V: u32 = 1;

/// Los ÚNICOS textos que el relevo acepta inyectar. Lista cerrada a
/// propósito: es el límite duro de lo que puede pasarle a la sesión.
const ALLOWED: [&str; 2] = ["/compact", "/clear"];

// --- Reglas anti-choque (R1-R5 de docs/remediacion.md) -------------------
// R3: ventana de calma. Ni una tecla en este tiempo.
const CALM_MS: u64 = 8_000;
// R2: silencio de salida. Si la PTY sigue escupiendo bytes, Claude está
// generando su turno. Es la señal honesta que tenemos (ver el doc).
const QUIET_MS: u64 = 2_000;
// Tras inyectar, nada más durante este rato.
const COOLDOWN_MS: u64 = 15_000;
// Cada cuánto se refresca el estado y se mira si hay una orden.
const TICK_MS: u64 = 250;
const STATE_EVERY_MS: u64 = 500;

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Carpeta del canal. Es la MISMA carpeta de datos de la app
/// (%APPDATA%\com.oscarorozco.michiclaude), donde ya viven
/// actions_log.json y compañía.
fn state_dir() -> PathBuf {
    if let Ok(a) = std::env::var("APPDATA") {
        if !a.is_empty() {
            return PathBuf::from(a)
                .join("com.oscarorozco.michiclaude")
                .join("relevo");
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".michiclaude").join("relevo")
}

/// Escritura atómica: nadie debe poder leer un JSON a medio escribir.
/// El temporal AÑADE ".tmp" al nombre entero en vez de sustituir la
/// extensión — con `with_extension` el estado (<pid>.json) y la orden
/// (<pid>.cmd) compartirían el mismo <pid>.tmp y se pisarían entre sí.
fn write_atomic(path: &PathBuf, data: &str) {
    let Some(name) = path.file_name().and_then(|x| x.to_str()) else {
        return;
    };
    let tmp = path.with_file_name(format!("{name}.tmp"));
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

// ---------- estado compartido entre hilos ----------

/// Lo que el relevo sabe de la sesión. Todo son HECHOS que ve pasar por el
/// cable, no adivinanzas — salvo `last_out`, que es la señal de "Claude
/// está generando" (ver el doc: es fail-closed, no certeza).
struct Shared {
    base: Instant,
    /// ms (desde `base`) del último byte tecleado por el usuario
    last_in: AtomicU64,
    /// ms del último byte que escupió la PTY
    last_out: AtomicU64,
    /// R1: hay texto del usuario desde su último Enter
    typed: AtomicBool,
    /// ms de la última inyección (0 = ninguna)
    inject_at: AtomicU64,
    /// código de salida del hijo (-1 = sigue vivo)
    exit: AtomicI64,
    /// el usuario aplicó él mismo un comando de la lista: (epoch, cuál)
    user_cmd: Mutex<Option<(i64, String)>>,
    /// eco de la última orden atendida, para que la app sepa qué pasó
    ack: Mutex<Option<serde_json::Value>>,
}

impl Shared {
    fn ms(&self) -> u64 {
        self.base.elapsed().as_millis() as u64
    }
    fn since_in(&self) -> u64 {
        self.ms().saturating_sub(self.last_in.load(Ordering::Relaxed))
    }
    fn since_out(&self) -> u64 {
        self.ms().saturating_sub(self.last_out.load(Ordering::Relaxed))
    }
    fn since_inject(&self) -> u64 {
        let at = self.inject_at.load(Ordering::Relaxed);
        if at == 0 {
            u64::MAX
        } else {
            self.ms().saturating_sub(at)
        }
    }
    /// ¿Se puede inyectar AHORA MISMO? R1 + R2 + R3 + enfriamiento + hijo vivo.
    /// Se vuelve a llamar en el instante de inyectar (R4): el countdown del
    /// panel no es un permiso, es solo un aviso.
    fn ready(&self) -> bool {
        self.exit.load(Ordering::Relaxed) < 0
            && !self.typed.load(Ordering::Relaxed)
            && self.since_in() >= CALM_MS
            && self.since_out() >= QUIET_MS
            && self.since_inject() >= COOLDOWN_MS
    }
    /// Motivo del NO, en código (el panel lo traduce — invariante #10).
    fn why_not(&self) -> &'static str {
        if self.exit.load(Ordering::Relaxed) >= 0 {
            "ERR_RELAY_GONE"
        } else if self.typed.load(Ordering::Relaxed) {
            "ERR_RELAY_TYPED"
        } else if self.since_out() < QUIET_MS {
            "ERR_RELAY_BUSY"
        } else if self.since_in() < CALM_MS {
            "ERR_RELAY_NOISY"
        } else if self.since_inject() < COOLDOWN_MS {
            "ERR_RELAY_COOLDOWN"
        } else {
            ""
        }
    }
}

// ---------- consola en modo crudo (solo Windows) ----------

#[cfg(windows)]
mod console {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::*;

    pub struct Restore {
        inp: HANDLE,
        outp: HANDLE,
        in_mode: CONSOLE_MODE,
        out_mode: CONSOLE_MODE,
        in_cp: u32,
        out_cp: u32,
    }

    /// Modo crudo: sin eco, sin líneas, sin traducción de Ctrl+C (llega como
    /// byte 0x03 y se reenvía tal cual), con secuencias VT en ambos sentidos
    /// y páginas de código en UTF-8. Devuelve lo que había para restaurarlo.
    pub fn enter_raw() -> Option<Restore> {
        unsafe {
            let inp = GetStdHandle(STD_INPUT_HANDLE).ok()?;
            let outp = GetStdHandle(STD_OUTPUT_HANDLE).ok()?;
            let mut im = CONSOLE_MODE(0);
            let mut om = CONSOLE_MODE(0);
            GetConsoleMode(inp, &mut im).ok()?;
            GetConsoleMode(outp, &mut om).ok()?;
            let saved = Restore {
                inp,
                outp,
                in_mode: im,
                out_mode: om,
                in_cp: GetConsoleCP(),
                out_cp: GetConsoleOutputCP(),
            };
            let _ = SetConsoleCP(65001);
            let _ = SetConsoleOutputCP(65001);
            let new_in = (im
                & !(ENABLE_ECHO_INPUT
                    | ENABLE_LINE_INPUT
                    | ENABLE_PROCESSED_INPUT
                    | ENABLE_MOUSE_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT;
            let new_out = om | ENABLE_VIRTUAL_TERMINAL_PROCESSING | ENABLE_PROCESSED_OUTPUT;
            let _ = SetConsoleMode(inp, new_in);
            let _ = SetConsoleMode(outp, new_out);
            Some(saved)
        }
    }

    pub fn restore(r: &Restore) {
        unsafe {
            let _ = SetConsoleMode(r.inp, r.in_mode);
            let _ = SetConsoleMode(r.outp, r.out_mode);
            let _ = SetConsoleCP(r.in_cp);
            let _ = SetConsoleOutputCP(r.out_cp);
        }
    }

    /// Tamaño VISIBLE de la ventana (no el del búfer, que suele ser más alto).
    pub fn size() -> Option<(u16, u16)> {
        unsafe {
            let outp = GetStdHandle(STD_OUTPUT_HANDLE).ok()?;
            let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
            GetConsoleScreenBufferInfo(outp, &mut info).ok()?;
            let cols = (info.srWindow.Right - info.srWindow.Left + 1).max(1) as u16;
            let rows = (info.srWindow.Bottom - info.srWindow.Top + 1).max(1) as u16;
            Some((cols, rows))
        }
    }
}

// El relevo es de Windows por ahora (etapa 4 lleva WSL y SSH), pero el
// crate compila en cualquier sitio para no atarse de manos.
#[cfg(not(windows))]
mod console {
    pub struct Restore;
    pub fn enter_raw() -> Option<Restore> {
        None
    }
    pub fn restore(_r: &Restore) {}
    pub fn size() -> Option<(u16, u16)> {
        None
    }
}

// ---------- lector de teclas: de dónde salen `typed` y `user_cmd` ----------

/// Lo que sacamos de un puñado de bytes que llegaron por el teclado.
struct Keys {
    /// el usuario envió él mismo uno de los comandos de la lista blanca
    cmd: Option<String>,
    /// hubo actividad HUMANA de verdad. Por el mismo cable llegan avisos
    /// del TERMINAL que nadie tecleó (cambio de foco al pasar a otra
    /// ventana, respuestas a consultas del programa…) y esos NO pueden
    /// reiniciar la ventana de calma: si contaran, bastaría con hacer clic
    /// en el panel de MichiClaude para que nunca se pudiera inyectar.
    human: bool,
}

/// Máquina de estados mínima sobre el flujo de teclas. Solo mira la FORMA
/// de lo tecleado (¿hay algo escrito?, ¿se envió?); el contenido se queda
/// en memoria y muere con el proceso.
struct KeyWatch {
    /// bytes de la línea en curso, para saber si quedó vacía y para casar
    /// /compact y /clear cuando el usuario los escribe él mismo
    line: Vec<u8>,
    /// dentro de una secuencia de escape (flechas, Alt+tecla, pegado…)
    esc: u8, // 0 = no, 1 = acaba de ver ESC, 2 = dentro de CSI, 3 = dentro de OSC
}

impl KeyWatch {
    fn new() -> Self {
        KeyWatch {
            line: Vec::new(),
            esc: 0,
        }
    }

    fn feed(&mut self, buf: &[u8], sh: &Shared) -> Keys {
        let mut r = Keys {
            cmd: None,
            human: false,
        };
        for &b in buf {
            match self.esc {
                // --- dentro de una secuencia de escape ---
                1 => {
                    // ESC + '[' → CSI ; ESC + ']' → OSC ; ESC + otra = Alt+tecla.
                    // OJO: ESC seguido de CR (Shift+Enter en varias terminales)
                    // cae aquí y NO se toma como envío — que es justo lo que
                    // queremos: el texto sigue ahí.
                    self.esc = match b {
                        b'[' => 2,
                        b']' => 3,
                        _ => {
                            r.human = true;
                            0
                        }
                    };
                }
                2 => {
                    if (0x40..=0x7e).contains(&b) {
                        // Estos finales son RESPUESTAS DEL TERMINAL, no teclas:
                        // I/O foco ganado o perdido, R posición del cursor,
                        // c identificación, n estado, t medidas de la ventana.
                        // Todo lo demás (flechas, inicio/fin, pegado, F1…) sí
                        // lo tecleó una persona.
                        if !matches!(b, b'I' | b'O' | b'R' | b'c' | b'n' | b't') {
                            r.human = true;
                        }
                        self.esc = 0;
                    }
                }
                3 => {
                    // OSC en dirección entrante = el terminal contestando algo.
                    if b == 0x07 || b == 0x1b {
                        self.esc = 0;
                    }
                }
                // --- flujo normal ---
                _ => match b {
                    // El ESC solo se marca como humano al ver qué venía
                    // detrás (o al acabar el bloque: ver más abajo).
                    0x1b => self.esc = 1,
                    b'\r' | b'\n' => {
                        r.human = true;
                        // Una línea que acaba en "\" es continuación: Claude
                        // Code no la envía, así que el texto SIGUE ahí.
                        if self.line.last() == Some(&b'\\') {
                            continue;
                        }
                        let txt = String::from_utf8_lossy(&self.line).trim().to_string();
                        for a in ALLOWED {
                            if txt == a || txt.starts_with(&format!("{a} ")) {
                                r.cmd = Some(a.to_string());
                            }
                        }
                        self.line.clear();
                        sh.typed.store(false, Ordering::Relaxed);
                    }
                    0x08 | 0x7f => {
                        r.human = true;
                        self.line.pop();
                        sh.typed.store(!self.line.is_empty(), Ordering::Relaxed);
                    }
                    // Ctrl+C y Ctrl+U dejan el prompt limpio
                    0x03 | 0x15 => {
                        r.human = true;
                        self.line.clear();
                        sh.typed.store(false, Ordering::Relaxed);
                    }
                    _ if b >= 0x20 => {
                        r.human = true;
                        // tope defensivo: una pegada enorme no debe comerse RAM
                        if self.line.len() < 64 * 1024 {
                            self.line.push(b);
                        }
                        sh.typed.store(true, Ordering::Relaxed);
                    }
                    _ => r.human = true,
                },
            }
        }
        // Un ESC suelto (la tecla Escape, que en Claude Code cancela) llega
        // solo en su propio bloque: si al acabar seguimos esperando lo que
        // venía detrás, es que no venía nada y fue una tecla de verdad.
        if self.esc == 1 {
            self.esc = 0;
            r.human = true;
        }
        r
    }
}

// ---------- el relevo ----------

fn run_relevo(extra: &[String]) -> ! {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let pid = std::process::id();
    let started = now_epoch();

    // Una línea de cortesía ANTES del modo crudo: el usuario tiene que saber
    // que hay alguien en medio del cable. Después de esto, la pantalla es de
    // Claude Code y de nadie más.
    println!("michi · relevo activo (sesión {pid}) — MichiClaude puede aplicar /compact y /clear en esta ventana");

    let restore = console::enter_raw();
    let (cols, rows) = console::size().unwrap_or((100, 30));

    let pty = native_pty_system();
    let pair = match pty.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(p) => p,
        Err(e) => bail(restore.as_ref(), &format!("no pude abrir la consola virtual: {e}"), 1),
    };
    // `mut` a propósito aunque resize/take_writer tomen &self: si alguna
    // versión de portable-pty pide &mut, esto sigue compilando (un aviso de
    // "mut innecesario" es barato; un error de compilación cuesta una ronda
    // entera con Oscar, que es quien tiene el compilador).
    #[allow(unused_mut)]
    let portable_pty::PtyPair { mut master, slave } = pair;

    // Programa a lanzar: `claude` con los argumentos que venían detrás.
    let build = |prog: &str, via_cmd: bool| {
        let mut c = CommandBuilder::new(prog);
        if via_cmd {
            c.arg("/c");
            c.arg("claude");
        }
        for a in extra {
            c.arg(a);
        }
        c.cwd(&cwd);
        // Marca para que la propia sesión (y quien mire su entorno) sepa que
        // va por el relevo. No lleva ningún dato del usuario.
        c.env("MICHI_RELEVO", pid.to_string());
        c
    };

    // Primero directo; si `claude` es un .cmd de npm y CreateProcess no lo
    // traga, se reintenta a través de cmd.exe. Directo es preferible: cmd.exe
    // en medio se queda con el Ctrl+C ("¿Terminar trabajo por lotes?").
    let mut child = match slave.spawn_command(build("claude", false)) {
        Ok(c) => c,
        Err(_) if cfg!(windows) => match slave.spawn_command(build("cmd.exe", true)) {
            Ok(c) => c,
            Err(e) => bail(
                restore.as_ref(),
                &format!("no encontré `claude` en el PATH: {e}"),
                127,
            ),
        },
        Err(e) => bail(restore.as_ref(), &format!("no pude lanzar `claude`: {e}"), 127),
    };
    drop(slave); // si no, la PTY nunca ve el cierre del hijo

    let sh = Arc::new(Shared {
        base: Instant::now(),
        last_in: AtomicU64::new(0),
        last_out: AtomicU64::new(0),
        typed: AtomicBool::new(false),
        inject_at: AtomicU64::new(0),
        exit: AtomicI64::new(-1),
        user_cmd: Mutex::new(None),
        ack: Mutex::new(None),
    });

    let writer = match master.take_writer() {
        Ok(w) => Arc::new(Mutex::new(w)),
        Err(e) => bail(restore.as_ref(), &format!("no pude escribir en la consola virtual: {e}"), 1),
    };
    let reader = match master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => bail(restore.as_ref(), &format!("no pude leer de la consola virtual: {e}"), 1),
    };

    // Hilo 1 — de la PTY a la pantalla. Tal cual, byte a byte.
    {
        let sh = sh.clone();
        let mut reader = reader;
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let out = std::io::stdout();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        sh.last_out.store(sh.ms(), Ordering::Relaxed);
                        let mut lock = out.lock();
                        if lock.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = lock.flush();
                    }
                }
            }
        });
    }

    // Hilo 2 — del teclado a la PTY. Además cuenta teclas (R1) sin guardar
    // jamás lo tecleado.
    {
        let sh = sh.clone();
        let writer = writer.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut watch = KeyWatch::new();
            let inp = std::io::stdin();
            loop {
                let n = match inp.lock().read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let keys = watch.feed(&buf[..n], &sh);
                // La ventana de calma solo la reinicia una PERSONA: los avisos
                // del terminal (foco, respuestas a consultas) llegan por aquí
                // y no cuentan.
                if keys.human {
                    sh.last_in.store(sh.ms(), Ordering::Relaxed);
                }
                if let Some(cmd) = keys.cmd {
                    // El usuario se adelantó: que el panel cancele lo suyo.
                    *sh.user_cmd.lock().unwrap() = Some((now_epoch(), cmd));
                }
                let mut w = writer.lock().unwrap();
                if w.write_all(&buf[..n]).is_err() {
                    break;
                }
                let _ = w.flush();
            }
        });
    }

    // Hilo 3 — esperar al hijo.
    {
        let sh = sh.clone();
        std::thread::spawn(move || {
            let code = child.wait().map(|s| s.exit_code() as i64).unwrap_or(0);
            sh.exit.store(code, Ordering::Relaxed);
        });
    }

    // Hilo principal — resize, estado y órdenes.
    let dir = state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let state_path = dir.join(format!("{pid}.json"));
    let cmd_path = dir.join(format!("{pid}.cmd"));
    let mut last_size = (cols, rows);
    let mut last_state = 0u64;

    loop {
        if sh.exit.load(Ordering::Relaxed) >= 0 {
            break;
        }

        if let Some(sz) = console::size() {
            if sz != last_size {
                last_size = sz;
                let _ = master.resize(PtySize {
                    rows: sz.1,
                    cols: sz.0,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
        }

        if let Ok(raw) = std::fs::read_to_string(&cmd_path) {
            let _ = std::fs::remove_file(&cmd_path);
            let ack = attend(&raw, &sh, &writer);
            *sh.ack.lock().unwrap() = Some(ack);
            last_state = 0; // que el estado salga YA con el acuse
        }

        let now = sh.ms();
        if last_state == 0 || now.saturating_sub(last_state) >= STATE_EVERY_MS {
            last_state = now.max(1);
            write_atomic(&state_path, &snapshot(&sh, pid, started, &cwd));
        }

        std::thread::sleep(Duration::from_millis(TICK_MS));
    }

    // Cierre limpio: el archivo de estado desaparece con la sesión.
    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_file(&cmd_path);
    if let Some(r) = restore.as_ref() {
        console::restore(r);
    }
    let code = sh.exit.load(Ordering::Relaxed);
    std::process::exit(code as i32)
}

/// La foto que lee el panel. Solo hechos y relojes — ni una letra tecleada.
fn snapshot(sh: &Shared, pid: u32, started: i64, cwd: &PathBuf) -> String {
    let uc = sh.user_cmd.lock().unwrap().clone();
    let ack = sh.ack.lock().unwrap().clone();
    serde_json::json!({
        "v": STATE_V,
        "pid": pid,
        "started": started,
        "cwd": cwd.to_string_lossy(),
        "ts": now_epoch(),
        "alive": sh.exit.load(Ordering::Relaxed) < 0,
        "typed": sh.typed.load(Ordering::Relaxed),
        "idle_in": sh.since_in() / 1000,
        "idle_out": sh.since_out() / 1000,
        "ready": sh.ready(),
        "why": sh.why_not(),
        // último comando de la lista que el usuario aplicó por su cuenta
        "user_cmd": uc.as_ref().map(|(_, c)| c.clone()),
        "user_cmd_ts": uc.as_ref().map(|(t, _)| *t),
        "last": ack,
    })
    .to_string()
}

/// Atiende una orden del panel. Aquí se re-verifican R1-R3 en el INSTANTE
/// de actuar (R4): que el countdown haya terminado no es un permiso.
fn attend(
    raw: &str,
    sh: &Shared,
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
) -> serde_json::Value {
    let v: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return serde_json::json!({"ok": false, "err": "ERR_RELAY_BADCMD"}),
    };
    let id = v["id"].as_str().unwrap_or("").to_string();
    let text = v["text"].as_str().unwrap_or("").trim().to_string();
    let done = |ok: bool, err: &str| {
        serde_json::json!({"id": id, "ok": ok, "err": err, "text": text, "ts": now_epoch()})
    };

    // Lista blanca: es el límite duro de lo que el relevo puede teclear.
    if !ALLOWED.contains(&text.as_str()) {
        return done(false, "ERR_RELAY_BADCMD");
    }
    if !sh.ready() {
        return done(false, sh.why_not());
    }

    // R5 (sagrada): solo se AÑADE texto. Ni un backspace, ni un Ctrl+U, ni
    // nada que pueda borrar lo del usuario. Si hubiera texto suyo, ya
    // habríamos salido por ERR_RELAY_TYPED.
    let mut w = writer.lock().unwrap();
    if w.write_all(format!("{text}\r").as_bytes()).is_err() {
        return done(false, "ERR_RELAY_WRITE");
    }
    let _ = w.flush();
    sh.inject_at.store(sh.ms(), Ordering::Relaxed);
    done(true, "")
}

/// Salida temprana con la consola devuelta a como estaba.
fn bail(restore: Option<&console::Restore>, msg: &str, code: i32) -> ! {
    if let Some(r) = restore {
        console::restore(r);
    }
    eprintln!("michi: {msg}");
    std::process::exit(code)
}

// ---------- subcomandos de prueba (validar sin tocar el panel) ----------

/// Sesiones vivas: las que han refrescado su estado hace menos de 15 s.
/// El panel usará exactamente esta regla — un relevo muerto de golpe deja
/// su archivo, y la frescura es lo único fiable.
fn live_sessions() -> Vec<serde_json::Value> {
    let mut out = vec![];
    let now = now_epoch();
    let Ok(rd) = std::fs::read_dir(state_dir()) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if now - v["ts"].as_i64().unwrap_or(0) <= 15 && v["alive"].as_bool().unwrap_or(false) {
            out.push(v);
        }
    }
    out.sort_by_key(|v| v["pid"].as_i64().unwrap_or(0));
    out
}

fn cmd_status() {
    let list = live_sessions();
    if list.is_empty() {
        println!("Ninguna sesión con relevo. Abre una con:  michi claude");
        return;
    }
    // El motivo va en columna propia: metido dentro de "listo" descuadraba
    // la tabla en cuanto el código era largo (ERR_RELAY_NOISY, visto en la
    // primera prueba de Oscar).
    println!(
        "{:<8} {:<6} {:<6} {:<7} {:<18} {}",
        "sesión", "listo", "texto", "quieta", "motivo", "carpeta"
    );
    let mut motivos = false;
    for v in &list {
        let why = v["why"].as_str().unwrap_or("");
        if !why.is_empty() {
            motivos = true;
        }
        println!(
            "{:<8} {:<6} {:<6} {:<7} {:<18} {}",
            v["pid"].as_i64().unwrap_or(0),
            if v["ready"].as_bool().unwrap_or(false) { "sí" } else { "no" },
            if v["typed"].as_bool().unwrap_or(false) { "sí" } else { "no" },
            format!("{}s", v["idle_in"].as_i64().unwrap_or(0)),
            why,
            v["cwd"].as_str().unwrap_or("")
        );
    }
    if motivos {
        println!(
            "\nTYPED = hay texto tuyo sin enviar · BUSY = Claude está generando\n\
             NOISY = acabas de teclear (faltan segundos de calma) · COOLDOWN = se\n\
             inyectó hace poco · GONE = la sesión ya terminó"
        );
    }
}

fn cmd_inject(args: &[String]) {
    let (pid, text) = match args {
        [p, t] => (p.clone(), t.clone()),
        [t] => match live_sessions().first() {
            Some(v) => (v["pid"].as_i64().unwrap_or(0).to_string(), t.clone()),
            None => {
                eprintln!("michi: no hay ninguna sesión con relevo abierta");
                std::process::exit(1)
            }
        },
        _ => {
            eprintln!("uso: michi inject [sesión] /compact");
            std::process::exit(2)
        }
    };
    if !ALLOWED.contains(&text.as_str()) {
        eprintln!("michi: solo puedo aplicar {}", ALLOWED.join(" o "));
        std::process::exit(2);
    }
    let dir = state_dir();
    let id = format!("cli-{}", now_epoch_ms());
    write_atomic(
        &dir.join(format!("{pid}.cmd")),
        &serde_json::json!({"id": id, "op": "inject", "text": text}).to_string(),
    );
    // Esperar el acuse en el archivo de estado (el relevo mira cada 250 ms).
    let state = dir.join(format!("{pid}.json"));
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(200));
        let Ok(raw) = std::fs::read_to_string(&state) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if v["last"]["id"].as_str() == Some(id.as_str()) {
            if v["last"]["ok"].as_bool().unwrap_or(false) {
                println!("aplicado: {text}");
            } else {
                println!("no se aplicó: {}", v["last"]["err"].as_str().unwrap_or("?"));
            }
            return;
        }
    }
    println!("sin respuesta del relevo (¿sigue abierto?)");
}

fn usage() {
    println!(
        "michi {} — relevo de MichiClaude\n\
         \n\
           michi claude [...]   abre Claude Code con relevo (todo funciona igual)\n\
           michi status         sesiones con relevo abiertas ahora\n\
           michi inject /compact\n\
                                aplica un comando a la sesión con relevo\n\
         \n\
         El relevo solo puede aplicar {} — nada más.",
        env!("CARGO_PKG_VERSION"),
        ALLOWED.join(" y ")
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("claude") => run_relevo(&args[1..]),
        Some("status") => cmd_status(),
        Some("inject") => cmd_inject(&args[1..]),
        Some("--version") | Some("-v") => println!("michi {}", env!("CARGO_PKG_VERSION")),
        _ => usage(),
    }
}
