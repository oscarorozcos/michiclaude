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
// ÚNICA ampliación (2026-08-09): un /clear puede llegar con la marca
// `export`, y entonces el relevo teclea ANTES un `/export <archivo>` como
// red de seguridad. La ruta la genera ESTE programa — jamás viene del
// canal, así que el canal sigue sin poder dictar ni un byte fuera de la
// lista. Y sin copia verificada (el archivo existe y tiene contenido) NO
// hay /clear: fail-closed.

use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

/// Versión del formato del archivo de estado. Si cambia la forma, sube —
/// el panel debe poder ignorar un relevo viejo sin romperse.
/// 2 = este relevo sabe hacer la red /export antes de un /clear; el panel
/// mira este número antes de pedirla (un relevo viejo ignoraría la marca y
/// borraría sin copia — justo lo que la red existe para impedir).
const STATE_V: u32 = 2;

/// Los ÚNICOS textos que el relevo acepta inyectar. Lista cerrada a
/// propósito: es el límite duro de lo que puede pasarle a la sesión.
const ALLOWED: [&str; 2] = ["/compact", "/clear"];
/// ÚNICA ampliación con argumento (2026-08-17, ruteo etapa 5b — el guardián
/// escala solo): `/model <alias>` con alias de LISTA CERRADA. Nada más.
const MODEL_ALIASES: [&str; 4] = ["haiku", "sonnet", "opus", "fable"];

/// ¿Puede este relevo teclear ese texto? Los dos exactos, o `/model <alias>`.
fn allowed(text: &str) -> bool {
    if ALLOWED.contains(&text) {
        return true;
    }
    let parts: Vec<&str> = text.split_whitespace().collect();
    parts.len() == 2 && parts[0] == "/model" && MODEL_ALIASES.contains(&parts[1])
}

// --- Reglas anti-choque (R1-R5 de docs/remediacion.md) -------------------
// R3: ventana de calma. Ni una tecla en este tiempo.
const CALM_MS: u64 = 8_000;
// R2: silencio de salida. Si la PTY sigue escupiendo bytes, Claude está
// generando su turno. Es la señal honesta que tenemos (ver el doc).
const QUIET_MS: u64 = 2_000;
// Tras inyectar, nada más durante este rato.
const COOLDOWN_MS: u64 = 15_000;
// Cuánto se espera a que Claude REACCIONE a un Enter antes de dar por hecho
// que aquel Enter no envió nada y el texto sigue en el prompt. Ver
// `KeyWatch::resolve` — de aquí salió el fallo del 2026-08-08.
const SUBMIT_WAIT_MS: u64 = 3_000;
// La red del /clear: cuánto se espera a que `/export` escriba su archivo
// antes de rendirse (y sin archivo NO hay /clear), y cuánto silencio de la
// PTY se exige tras verlo aparecer para dar el REPL por asentado.
const EXPORT_WAIT_MS: u64 = 12_000;
const EXPORT_SETTLE_MS: u64 = 1_500;
// Pausa entre el texto y su Enter. NO es cosmética: la TUI de Claude Code
// trata el texto y el Enter que llegan en la MISMA ráfaga de lectura como
// un pegado y NO ejecuta la línea — se queda escrita en el prompt. Con
// `/compact` (9 bytes) colaba; con `/export <ruta>` (~110) el Enter se lo
// tragaba siempre y la red fallaba entera con ERR_RELAY_EXPORT sin que
// nada saliera por pantalla (cazado en vivo, 2026-08-09). Separarlos lo
// arregla, y se hace para TODOS los comandos: el fallo dependía del largo
// de la línea y de la máquina, o sea que el /compact vivía de suerte.
const ENTER_GAP_MS: u64 = 250;
// Las copias del /clear ya cumplieron su papel pasado este tiempo.
const HANDOFF_KEEP_DAYS: u64 = 90;
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

/// Donde viven las copias del /clear: la carpeta de datos de la app
/// (hermana de `relevo/`). La ruta de cada copia la genera SOLO el relevo.
fn handoff_dir() -> PathBuf {
    let d = state_dir();
    match d.parent() {
        Some(p) => p.join("handoff"),
        None => d.join("handoff"),
    }
}

/// El JSONL de la sesión `sid`: `<config>/projects/<carpeta>/<sid>.jsonl`.
/// El nombre del archivo ES el session_id (UUID, único), así que se busca
/// por nombre en vez de reproducir la transformación de carpetas de Claude
/// Code — menos frágil ante sus cambios. Solo lo usa el modo chat
/// (2026-08-13): la extensión NO tiene /export y la copia de la red la
/// hace el relevo. Réplica exacta de `session_jsonl` en michi-relevo.py.
fn session_jsonl(sid: &str) -> Option<PathBuf> {
    if sid.is_empty() {
        return None;
    }
    let base = match std::env::var("CLAUDE_CONFIG_DIR") {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()?;
            PathBuf::from(home).join(".claude")
        }
    };
    let rd = std::fs::read_dir(base.join("projects")).ok()?;
    for e in rd.flatten() {
        let p = e.path().join(format!("{sid}.jsonl"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Limpieza al arrancar: una copia de hace más de HANDOFF_KEEP_DAYS ya
/// cumplió su papel de red. Best-effort — un fallo aquí no frena nada.
fn prune_handoffs() {
    let Ok(rd) = std::fs::read_dir(handoff_dir()) else {
        return;
    };
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs() > HANDOFF_KEEP_DAYS * 24 * 3600)
            .unwrap_or(false);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
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
    /// R1 vive aquí: el modelo de lo que hay en el prompt. Es UNA sola
    /// fuente de verdad a propósito — cuando `typed` era un booleano
    /// aparte se desincronizó del buffer y dejó pasar una inyección con
    /// texto del usuario delante (2026-08-08).
    keys: Mutex<KeyWatch>,
    /// ms de la última inyección (0 = ninguna)
    inject_at: AtomicU64,
    /// 1 = hay una secuencia /export+/clear en curso (corre en su propio
    /// hilo para que el bucle principal siga refrescando el estado);
    /// mientras dura, el relevo está ocupado y no acepta otra orden
    handoff: AtomicU64,
    /// código de salida del hijo (-1 = sigue vivo)
    exit: AtomicI64,
    /// el usuario aplicó él mismo un comando de la lista: (epoch, cuál)
    user_cmd: Mutex<Option<(i64, String)>>,
    /// eco de la última orden atendida, para que la app sepa qué pasó
    ack: Mutex<Option<serde_json::Value>>,
    /// 1 = hay un turno en curso. SOLO lo usa el modo chat, y ahí es
    /// CERTEZA, no inferencia: el protocolo lo dice (entra un `user`, sale
    /// un `result`). En la terminal se queda en 0 y R2 sigue saliendo del
    /// silencio de la PTY, que es lo único que hay allí.
    busy: AtomicU64,
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
    /// ¿Hay texto del usuario en el prompt? Resuelve antes los Enter
    /// pendientes, que es lo que necesita el reloj de salida de la PTY.
    fn has_text(&self) -> bool {
        let mut k = self.keys.lock().unwrap();
        k.resolve(self.ms(), self.last_out.load(Ordering::Relaxed));
        k.has_text()
    }
    /// ¿Se puede inyectar AHORA MISMO? R1 + R2 + R3 + enfriamiento + hijo vivo.
    /// Se vuelve a llamar en el instante de inyectar (R4): el countdown del
    /// panel no es un permiso, es solo un aviso.
    fn ready(&self) -> bool {
        self.exit.load(Ordering::Relaxed) < 0
            && self.handoff.load(Ordering::Relaxed) == 0
            && self.busy.load(Ordering::Relaxed) == 0
            && !self.has_text()
            && self.since_in() >= CALM_MS
            && self.since_out() >= QUIET_MS
            && self.since_inject() >= COOLDOWN_MS
    }
    /// Motivo del NO, en código (el panel lo traduce — invariante #10).
    fn why_not(&self) -> &'static str {
        if self.exit.load(Ordering::Relaxed) >= 0 {
            "ERR_RELAY_GONE"
        } else if self.handoff.load(Ordering::Relaxed) != 0 {
            "ERR_RELAY_BUSY"
        } else if self.busy.load(Ordering::Relaxed) != 0 {
            "ERR_RELAY_BUSY"
        } else if self.has_text() {
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
    /// parámetros de la CSI en curso (los necesita win32-input-mode)
    csi: Vec<u8>,
    /// Un Enter VISTO pero todavía sin confirmar que Claude lo aceptó:
    /// (lo que había escrito, ms del Enter). Mientras no se resuelva
    /// cuenta como que SÍ hay texto. Ver `resolve`.
    pending: Option<(Vec<u8>, u64)>,
    /// Contadores para diagnóstico. Son CUENTAS, no contenido: cuántas
    /// teclas imprimibles, cuántos Enter, cuántas secuencias de escape y
    /// cuántos controles. Nada de lo tecleado sale de aquí.
    k_print: u64,
    k_enter: u64,
    k_esc: u64,
    k_other: u64,
    /// cuántas teclas llegaron codificadas en win32-input-mode. Si esto
    /// sube y `k_print` no, el decodificador se ha vuelto a quedar corto.
    k_win32: u64,
}

impl KeyWatch {
    fn new() -> Self {
        KeyWatch {
            line: Vec::new(),
            esc: 0,
            csi: Vec::new(),
            pending: None,
            k_print: 0,
            k_enter: 0,
            k_esc: 0,
            k_other: 0,
            k_win32: 0,
        }
    }

    /// Una tecla, ya sea un byte suelto o el carácter que venía dentro de una
    /// secuencia win32-input-mode. UN solo sitio donde se decide qué le pasa
    /// al modelo del prompt.
    fn key_char(&mut self, uc: u32, now: u64, r: &mut Keys) {
        match uc {
            13 | 10 => {
                self.k_enter += 1;
                self.on_enter(now, r);
            }
            8 | 127 => {
                self.k_other += 1;
                // Quitar el último CARÁCTER, no el último byte: con acentos
                // o emoji, un carácter ocupa varios.
                while self.line.pop().map_or(false, |b| b & 0xc0 == 0x80) {}
            }
            // Ctrl+C y Ctrl+U dejan el prompt limpio de verdad
            3 | 21 => {
                self.k_other += 1;
                self.line.clear();
                self.pending = None;
            }
            c if c >= 32 => {
                self.k_print += 1;
                if self.line.len() < 64 * 1024 {
                    if let Some(ch) = char::from_u32(c) {
                        let mut b = [0u8; 4];
                        self.line
                            .extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                    }
                }
            }
            _ => self.k_other += 1,
        }
    }

    /// Un Enter no limpia el modelo: aparta la línea y espera a ver si Claude
    /// reacciona (ver `resolve`).
    fn on_enter(&mut self, now: u64, r: &mut Keys) {
        // Una línea que acaba en "\" es continuación: Claude Code no la
        // envía, así que el texto SIGUE ahí.
        if self.line.last() == Some(&b'\\') {
            return;
        }
        let txt = String::from_utf8_lossy(&self.line).trim().to_string();
        for a in ALLOWED {
            if txt == a || txt.starts_with(&format!("{a} ")) {
                r.cmd = Some(a.to_string());
            }
        }
        let mut old = std::mem::take(&mut self.line);
        if let Some((prev, _)) = self.pending.take() {
            let mut m = prev;
            m.extend_from_slice(&old);
            old = m;
        }
        self.pending = Some((old, now));
    }

    /// win32-input-mode: `ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _`.
    /// Lo activa la propia consola virtual (ConPTY pide el modo al arrancar,
    /// y el terminal se lo concede a TODA la ventana, o sea también a
    /// nosotros). Con él, ni una sola tecla llega como carácter suelto: el
    /// relevo veía 38 secuencias de escape y CERO letras (2026-08-08).
    /// Solo interesa `Uc` (el carácter en decimal) cuando `Kd` dice que es
    /// una pulsación; soltar una tecla no escribe nada.
    fn win32_key(&mut self, now: u64, r: &mut Keys) {
        self.k_win32 += 1;
        let s = String::from_utf8_lossy(&self.csi).to_string();
        let f: Vec<&str> = s.split(';').collect();
        let num = |i: usize| -> u32 {
            f.get(i)
                .and_then(|x| x.trim().parse::<u32>().ok())
                .unwrap_or(0)
        };
        if num(3) == 0 {
            return; // soltar la tecla
        }
        r.human = true;
        let uc = num(2);
        if uc == 0 {
            return; // teclas sin carácter: flechas, Mayús, F1…
        }
        for _ in 0..num(5).clamp(1, 64) {
            self.key_char(uc, now, r);
        }
    }

    /// ¿Damos por hecho que hay texto del usuario en el prompt?
    fn has_text(&self) -> bool {
        !self.line.is_empty() || self.pending.is_some()
    }

    /// Decide qué pasó con el último Enter. No se puede saber tecleando: hay
    /// que mirar si Claude REACCIONÓ. Si salieron bytes por la PTY después
    /// del Enter, la línea se envió. Si en 3 s no salió nada, aquel Enter no
    /// hizo nada (Shift+Enter mal interpretado, un modo del REPL, lo que
    /// sea) y el texto SIGUE en el prompt — así que vuelve.
    ///
    /// Esto es exactamente lo que faltaba el 2026-08-08: el Enter limpiaba
    /// el modelo a ciegas y el relevo inyectaba encima de un `hola` vivo.
    fn resolve(&mut self, now: u64, last_out: u64) {
        let Some((line, at)) = self.pending.take() else {
            return;
        };
        if last_out > at {
            return; // Claude reaccionó: la línea se fue de verdad
        }
        if now.saturating_sub(at) >= SUBMIT_WAIT_MS {
            let mut back = line;
            back.extend_from_slice(&self.line);
            self.line = back;
            return;
        }
        self.pending = Some((line, at)); // todavía sin decidir → fail-closed
    }

    fn feed(&mut self, buf: &[u8], now: u64) -> Keys {
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
                    self.k_esc += 1;
                    self.csi.clear();
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
                        self.esc = 0;
                        if b == b'_' {
                            // una tecla de verdad, codificada por el terminal
                            self.win32_key(now, &mut r);
                        } else if !matches!(b, b'I' | b'O' | b'R' | b'c' | b'n' | b't') {
                            // Esos finales son RESPUESTAS DEL TERMINAL, no
                            // teclas: I/O foco ganado o perdido, R posición
                            // del cursor, c identificación, n estado, t
                            // medidas de la ventana. El resto (flechas,
                            // inicio/fin, pegado, F1…) sí lo tecleó alguien.
                            r.human = true;
                        }
                    } else if self.csi.len() < 64 {
                        self.csi.push(b);
                    }
                }
                3 => {
                    // OSC en dirección entrante = el terminal contestando algo.
                    if b == 0x07 || b == 0x1b {
                        self.esc = 0;
                    }
                }
                // --- flujo normal: bytes sueltos ---
                // Sigue haciendo falta: win32-input-mode solo está activo
                // mientras el terminal lo concede, y hay terminales (y
                // pegados) que mandan los caracteres a pelo.
                _ => match b {
                    // El ESC solo se marca como humano al ver qué venía
                    // detrás (o al acabar el bloque: ver más abajo).
                    0x1b => self.esc = 1,
                    _ => {
                        r.human = true;
                        self.key_char(b as u32, now, &mut r);
                    }
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

/// Última carpeta de una ruta, para el título. Sin depender de `file_name()`:
/// una raíz (`C:\`) devolvería None y el título quedaría vacío.
fn path_base_str(p: &std::path::Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let base = s.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if base.is_empty() { s.to_string() } else { base.to_string() }
}

/// Antepone la marca del relevo al título que escriba Claude Code.
///
/// PLAN B, y por qué existe: poner el título al arrancar no sobrevive —
/// Claude Code pone el suyo ("Claude Code") en cuanto arranca y la marca
/// desaparece (comprobado en vivo, 2026-08-08). Como el relevo ve pasar todos
/// los bytes, puede reescribir esa secuencia al vuelo.
///
/// Es la ÚNICA excepción al paso transparente, y por eso está acotada al
/// hueso:
/// - Solo toca `ESC ] 0|1|2 ; …` (título de ventana/icono). Cualquier otra
///   cosa —incluidas otras OSC— sale byte a byte.
/// - Si la secuencia ya empieza por la marca, no se toca: nada de marcas
///   apiladas cuando Claude reescribe su título.
/// - Tope de `MAX`: si no llega el terminador, se suelta lo retenido tal cual
///   y se vuelve al paso directo. FAIL-OPEN — ante la duda, transparencia; lo
///   peor que puede pasar es quedarse sin marca, jamás comerse la salida.
struct TitleMark {
    mark: String,
    /// 0 = fuera, 1 = vi ESC, 2 = vi ESC], 3 = dentro de un título
    st: u8,
    buf: Vec<u8>,
}

impl TitleMark {
    const MAX: usize = 1024;
    fn new(mark: String) -> Self {
        Self { mark, st: 0, buf: Vec::new() }
    }
    fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + self.mark.len());
        for &b in data {
            match self.st {
                0 => {
                    if b == 0x1b {
                        self.st = 1;
                        self.buf.clear();
                        self.buf.push(b);
                    } else {
                        out.push(b);
                    }
                }
                1 => {
                    self.buf.push(b);
                    if b == b']' {
                        self.st = 2;
                    } else {
                        // no era una OSC: se suelta lo retenido intacto
                        out.extend_from_slice(&self.buf);
                        self.buf.clear();
                        self.st = 0;
                    }
                }
                2 => {
                    // Se lee el NÚMERO entero, no el primer dígito: `ESC]10;`
                    // (color de primer plano) empieza por '1' y tratarlo como
                    // título le metería la marca dentro y lo destrozaría.
                    self.buf.push(b);
                    if b.is_ascii_digit() && self.buf.len() < 8 {
                        // sigue el número
                    } else if b == b';' {
                        // OSC de título: 0 (ventana+icono), 1 (icono), 2 (ventana)
                        let num = &self.buf[2..self.buf.len() - 1];
                        if matches!(num, b"0" | b"1" | b"2") {
                            self.st = 3;
                        } else {
                            out.extend_from_slice(&self.buf);
                            self.buf.clear();
                            self.st = 0;
                        }
                    } else {
                        out.extend_from_slice(&self.buf);
                        self.buf.clear();
                        self.st = 0;
                    }
                }
                _ => {
                    self.buf.push(b);
                    // BEL o ST (ESC \) cierran la secuencia
                    let done = b == 0x07
                        || (b == b'\\' && self.buf.len() >= 2 && self.buf[self.buf.len() - 2] == 0x1b);
                    if done {
                        out.extend_from_slice(&self.rewrite());
                        self.buf.clear();
                        self.st = 0;
                    } else if self.buf.len() > Self::MAX {
                        out.extend_from_slice(&self.buf);
                        self.buf.clear();
                        self.st = 0;
                    }
                }
            }
        }
        out
    }
    /// `ESC ] N ; <texto> <fin>` → `ESC ] N ; <marca><texto> <fin>`. Si algo
    /// no encaja con esa forma exacta, se devuelve sin tocar.
    fn rewrite(&self) -> Vec<u8> {
        let Some(semi) = self.buf.iter().position(|&c| c == b';') else {
            return self.buf.clone();
        };
        let (head, rest) = self.buf.split_at(semi + 1);
        if rest.starts_with(self.mark.as_bytes()) {
            return self.buf.clone();
        }
        let mut v = Vec::with_capacity(self.buf.len() + self.mark.len());
        v.extend_from_slice(head);
        v.extend_from_slice(self.mark.as_bytes());
        v.extend_from_slice(rest);
        v
    }
}

/// Título de la pestaña vía OSC 0. Se escribe directo a la salida real, no a
/// la PTY: es un mensaje para el TERMINAL, no para Claude Code.
fn set_title(title: &str) {
    // Los caracteres de control romperían la secuencia; se filtran.
    let clean: String = title.chars().filter(|c| !c.is_control()).collect();
    let mut out = std::io::stdout();
    let _ = out.write_all(format!("\x1b]0;{clean}\x07").as_bytes());
    let _ = out.flush();
}

/// `claude` tal cual, con la entrada/salida heredadas — para invocaciones sin
/// consola. `MICHI_RELEVO=0` SIEMPRE: si el intento directo falla y se pasa
/// por `cmd.exe /c claude`, cmd resuelve por PATH y puede encontrar NUESTRO
/// shim — que sin esa marca volvería a llamar a michi, y michi a cmd, en
/// bucle infinito. El 0 significa "sin relevo" (un pid real nunca es 0).
fn passthrough(extra: &[String]) -> i32 {
    let run = |prog: &str, via_cmd: bool| -> std::io::Result<std::process::ExitStatus> {
        let mut c = std::process::Command::new(prog);
        if via_cmd {
            c.arg("/c");
            c.arg("claude");
        }
        c.args(extra);
        c.env("MICHI_RELEVO", "0");
        c.status()
    };
    match run("claude", false) {
        Ok(s) => s.code().unwrap_or(1),
        Err(_) if cfg!(windows) => match run("cmd.exe", true) {
            Ok(s) => s.code().unwrap_or(1),
            Err(_) => {
                eprintln!("michi: no encontré `claude` en el PATH");
                127
            }
        },
        Err(e) => {
            eprintln!("michi: no pude lanzar `claude`: {e}");
            127
        }
    }
}

fn run_relevo(extra: &[String]) -> ! {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let pid = std::process::id();
    let started = now_epoch();

    // Una línea de cortesía ANTES del modo crudo: el usuario tiene que saber
    // que hay alguien en medio del cable. Después de esto, la pantalla es de
    // Claude Code y de nadie más.
    println!("michi · relevo activo (sesión {pid}) — MichiClaude puede aplicar /compact y /clear en esta ventana");

    // …y una marca PERSISTENTE en el título de la pestaña. La línea de arriba
    // se la lleva el primer borrado de pantalla de Claude Code, y entonces no
    // queda forma de saber si esta terminal tiene relevo sin ir al panel a
    // comparar pids — que es justo el "me entero al final de que no" que hay
    // que evitar (Oscar, 2026-08-08).
    //
    // OSC 0 pone título de ventana Y de icono; `\x07` (BEL) lo cierra, que es
    // lo que entienden todas las terminales de Windows. BEST-EFFORT a
    // propósito: si Claude Code decide cambiar el título después, gana él y
    // esta marca se pierde. No se re-impone en bucle — pelearse por el título
    // parpadearía y rompería el paso transparente, que es sagrado.
    set_title(&format!("michi · {}", path_base_str(&cwd)));

    // SIN consola no hay relevo posible: nadie teclea y nadie mira. Ese es el
    // caso de todo lo NO interactivo que invoque `claude` a través del atajo
    // del PATH — el chat de la extensión de VS Code, un script, un pipe.
    // Envolverlos en una ConPTY les rompería el protocolo (la CLI se creería
    // en una terminal interactiva), así que se ejecuta el claude real tal
    // cual. FAIL-OPEN, la misma regla del shim: lo peor permitido es quedarse
    // sin relevo, jamás sin Claude Code.
    let restore = console::enter_raw();
    if restore.is_none() {
        std::process::exit(passthrough(extra));
    }
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
        keys: Mutex::new(KeyWatch::new()),
        inject_at: AtomicU64::new(0),
        handoff: AtomicU64::new(0),
        exit: AtomicI64::new(-1),
        user_cmd: Mutex::new(None),
        ack: Mutex::new(None),
        busy: AtomicU64::new(0),
    });

    let writer = match master.take_writer() {
        Ok(w) => Arc::new(Mutex::new(Some(w))),
        Err(e) => bail(restore.as_ref(), &format!("no pude escribir en la consola virtual: {e}"), 1),
    };
    let reader = match master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => bail(restore.as_ref(), &format!("no pude leer de la consola virtual: {e}"), 1),
    };
    // En la terminal se habla TECLEANDO (sin `sid`: no hay protocolo que
    // casar). El chat usa el mismo `attend` con otro Speaker.
    let sp = Arc::new(Speaker {
        to_child: writer.clone(),
        sid: None,
    });

    // Hilo 1 — de la PTY a la pantalla. Byte a byte, con UNA excepción: el
    // título de la ventana (ver TitleMark).
    {
        let sh = sh.clone();
        let mut reader = reader;
        let mark = format!("michi · {} · ", path_base_str(&cwd));
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let out = std::io::stdout();
            let mut tm = TitleMark::new(mark);
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        sh.last_out.store(sh.ms(), Ordering::Relaxed);
                        let mut lock = out.lock();
                        if lock.write_all(&tm.feed(&buf[..n])).is_err() {
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
            let inp = std::io::stdin();
            loop {
                let n = match inp.lock().read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let now = sh.ms();
                let keys = sh.keys.lock().unwrap().feed(&buf[..n], now);
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
                let mut lock = writer.lock().unwrap();
                let Some(w) = lock.as_mut() else { break };
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
    prune_handoffs();
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
            // None = la secuencia /export+/clear quedó corriendo en su hilo
            // y publicará el acuse ella misma al terminar.
            if let Some(ack) = attend(&raw, &sh, &sp) {
                *sh.ack.lock().unwrap() = Some(ack);
                last_state = 0; // que el estado salga YA con el acuse
            }
        }

        let now = sh.ms();
        if last_state == 0 || now.saturating_sub(last_state) >= STATE_EVERY_MS {
            last_state = now.max(1);
            write_atomic(&state_path, &snapshot(&sh, pid, started, &cwd, "terminal", ""));
        }

        std::thread::sleep(Duration::from_millis(TICK_MS));
    }

    // Cierre limpio: el archivo de estado desaparece con la sesión.
    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_file(&cmd_path);
    // Y el título deja de anunciar un relevo que ya no existe: si se quedara
    // puesto, la pestaña seguiría diciendo "michi ·" con la sesión muerta —
    // exactamente la clase de indicador que miente.
    set_title(&path_base_str(&cwd));
    if let Some(r) = restore.as_ref() {
        console::restore(r);
    }
    let code = sh.exit.load(Ordering::Relaxed);
    std::process::exit(code as i32)
}

/// La foto que lee el panel. Solo hechos y relojes — ni una letra tecleada.
fn snapshot(
    sh: &Shared,
    pid: u32,
    started: i64,
    cwd: &PathBuf,
    mode: &str,
    sid: &str,
) -> String {
    let uc = sh.user_cmd.lock().unwrap().clone();
    let ack = sh.ack.lock().unwrap().clone();
    // Ojo al orden: has_text() toma el mismo cerrojo que el bloque de abajo,
    // así que primero se pregunta y DESPUÉS se lee el detalle.
    let typed = sh.has_text();
    let ready = sh.ready();
    let why = sh.why_not();
    let diag = {
        let k = sh.keys.lock().unwrap();
        serde_json::json!({
            "line_len": k.line.len(),
            "pending": k.pending.is_some(),
            "k_print": k.k_print,
            "k_enter": k.k_enter,
            "k_esc": k.k_esc,
            "k_other": k.k_other,
            "k_win32": k.k_win32,
        })
    };
    serde_json::json!({
        "v": STATE_V,
        "pid": pid,
        "started": started,
        "cwd": cwd.to_string_lossy(),
        "ts": now_epoch(),
        "alive": sh.exit.load(Ordering::Relaxed) < 0,
        "typed": typed,
        "idle_in": sh.since_in() / 1000,
        "idle_out": sh.since_out() / 1000,
        "ready": ready,
        "why": why,
        // Cuentas, nunca contenido. Para diagnosticar sin ver lo tecleado.
        "diag": diag,
        // último comando de la lista que el usuario aplicó por su cuenta
        "user_cmd": uc.as_ref().map(|(_, c)| c.clone()),
        "user_cmd_ts": uc.as_ref().map(|(t, _)| *t),
        "last": ack,
        // por qué vía: el panel enseña distinto un chat que una terminal, y
        // el casado sesión↔relevo del chat es por `sid` EXACTO
        "mode": mode,
        "sid": sid,
    })
    .to_string()
}

/// Teclea una línea en la sesión: el texto, una pausa, y el Enter aparte
/// (ver ENTER_GAP_MS — juntos, la TUI los toma por un pegado y no ejecuta).
/// R5 intacta: solo se AÑADE, ni un borrado.
fn type_line(writer: &Arc<Mutex<Option<Box<dyn Write + Send>>>>, text: &str) -> bool {
    {
        let mut lock = writer.lock().unwrap();
        let Some(w) = lock.as_mut() else { return false };
        if w.write_all(text.as_bytes()).is_err() {
            return false;
        }
        let _ = w.flush();
    }
    std::thread::sleep(Duration::from_millis(ENTER_GAP_MS));
    let mut lock = writer.lock().unwrap();
    let Some(w) = lock.as_mut() else { return false };
    if w.write_all(b"\r").is_err() {
        return false;
    }
    let _ = w.flush();
    true
}

/// Cómo se le habla a la sesión relevada. Hay dos terrenos y UNA sola
/// coreografía: la terminal TECLEA en la PTY (texto, pausa, Enter aparte) y
/// el chat MANDA una línea de protocolo por el tubo. Todo lo que decide
/// CUÁNDO se puede hablar (`attend`, `handoff`, R1-R5) es común a los dos —
/// que es justo lo que no se puede duplicar sin que un día diverjan.
struct Speaker {
    /// `None` = ya se cerró (el chat colgó). Cerrable a propósito: sin EOT
    /// el hijo se quedaría esperando una entrada que no va a llegar.
    to_child: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    /// Some = modo chat, con la sesión que la CLI anunció. El eco al chat la
    /// necesita: la extensión descarta en silencio lo que no case con ella.
    sid: Option<Arc<Mutex<String>>>,
}

impl Speaker {
    fn say(&self, sh: &Shared, text: &str) -> bool {
        let Some(sid) = &self.sid else {
            return type_line(&self.to_child, text);
        };
        let sid = sid.lock().unwrap().clone();
        {
            let mut lock = self.to_child.lock().unwrap();
            let Some(w) = lock.as_mut() else { return false };
            if w.write_all(user_line(text).as_bytes()).is_err() {
                return false;
            }
            let _ = w.flush();
        }
        // Turno en curso por CERTEZA: acaba de entrar un `user`, y hasta que
        // salga su `result` esta sesión está ocupada.
        sh.busy.store(1, Ordering::Relaxed);
        // Y su eco al chat, que es obligatorio: la CLI intercepta los comandos
        // antes de replicarlos, así que sin esto la conversación se
        // compactaría sin que nada en pantalla dijera quién lo pidió.
        let out = std::io::stdout();
        let mut lock = out.lock();
        let _ = lock.write_all(replay_line(text, &sid).as_bytes());
        let _ = lock.flush();
        true
    }
}

/// El acuse con la misma forma en todos los caminos. `export` = ruta de la
/// copia, si la hubo (además del ok/err — un TYPED tras exportar deja copia
/// pero no /clear, y las dos cosas tienen que poder decirse).
fn ack_json(id: &str, text: &str, ok: bool, err: &str, export: Option<&str>) -> serde_json::Value {
    let mut a =
        serde_json::json!({"id": id, "ok": ok, "err": err, "text": text, "ts": now_epoch()});
    if let Some(p) = export {
        a["export"] = serde_json::json!(p);
    }
    a
}

/// Atiende una orden del panel. Aquí se re-verifican R1-R3 en el INSTANTE
/// de actuar (R4): que el countdown haya terminado no es un permiso.
/// Devuelve None si la orden era un /clear con red: la secuencia corre en
/// su propio hilo (esperar la copia tarda segundos y el bucle principal
/// tiene que seguir refrescando el estado) y publica el acuse al acabar.
fn attend(raw: &str, sh: &Arc<Shared>, sp: &Arc<Speaker>) -> Option<serde_json::Value> {
    let v: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Some(serde_json::json!({"ok": false, "err": "ERR_RELAY_BADCMD"})),
    };
    let id = v["id"].as_str().unwrap_or("").to_string();
    let text = v["text"].as_str().unwrap_or("").trim().to_string();
    // La red solo acompaña a /clear: /compact no destruye nada que respaldar.
    let export = v["export"].as_bool().unwrap_or(false) && text == "/clear";

    // Lista blanca: es el límite duro de lo que el relevo puede teclear.
    if !allowed(&text) {
        return Some(ack_json(&id, &text, false, "ERR_RELAY_BADCMD", None));
    }
    if !sh.ready() {
        return Some(ack_json(&id, &text, false, sh.why_not(), None));
    }

    if export {
        sh.handoff.store(1, Ordering::Relaxed);
        let sh2 = sh.clone();
        let sp2 = sp.clone();
        std::thread::spawn(move || {
            let ack = handoff(&id, &text, &sh2, &sp2);
            *sh2.ack.lock().unwrap() = Some(ack);
            sh2.handoff.store(0, Ordering::Relaxed);
        });
        return None;
    }

    // R5 (sagrada): solo se AÑADE texto. Ni un backspace, ni un Ctrl+U, ni
    // nada que pueda borrar lo del usuario. Si hubiera texto suyo, ya
    // habríamos salido por ERR_RELAY_TYPED.
    if !sp.say(sh, &text) {
        return Some(ack_json(&id, &text, false, "ERR_RELAY_WRITE", None));
    }
    sh.inject_at.store(sh.ms(), Ordering::Relaxed);
    Some(ack_json(&id, &text, true, "", None))
}

/// La red de seguridad del /clear: primero `/export <archivo>` —la ruta la
/// genera el RELEVO, jamás viene del canal—, después se VERIFICA que la
/// copia existe y tiene contenido, y solo entonces se teclea el /clear.
/// Sin copia no hay /clear (ERR_RELAY_EXPORT): fail-closed, la misma
/// familia que R5 — antes perder la limpieza que perder la conversación.
fn handoff(id: &str, text: &str, sh: &Arc<Shared>, sp: &Arc<Speaker>) -> serde_json::Value {
    let dir = handoff_dir();
    let _ = std::fs::create_dir_all(&dir);
    // MODO CHAT: la extensión NO tiene /export ("isn't available in this
    // environment", medido en vivo 2026-08-13) — teclearlo era un
    // ERR_RELAY_EXPORT garantizado. La copia la hace el RELEVO: el init
    // regaló el session_id exacto y el JSONL de la sesión es el registro
    // FIEL — se copia (tmp+rename) y la verificación sigue siendo un hecho
    // del disco. La ruta origen la resuelve session_jsonl() por nombre; la
    // de destino la genera el relevo. Nada nuevo viaja por el canal.
    // Réplica exacta de wrap_handoff en michi-relevo.py.
    if let Some(sid) = &sp.sid {
        let sid = sid.lock().unwrap().clone();
        let Some(src) = session_jsonl(&sid) else {
            return ack_json(id, text, false, "ERR_RELAY_EXPORT", None);
        };
        let path = dir.join(format!("handoff-{}-{}.jsonl", std::process::id(), now_epoch()));
        let ps = path.to_string_lossy().to_string();
        let tmp = path.with_extension("jsonl.tmp"); // .tmp sobre el nombre ENTERO
        let copied = std::fs::copy(&src, &tmp).map(|n| n > 0).unwrap_or(false)
            && std::fs::rename(&tmp, &path).is_ok();
        if !copied {
            let _ = std::fs::remove_file(&tmp);
            return ack_json(id, text, false, "ERR_RELAY_EXPORT", None);
        }
        // la verificación de siempre: el archivo en su sitio, con contenido
        if !std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) {
            return ack_json(id, text, false, "ERR_RELAY_EXPORT", None);
        }
        // R4: ¿entró un turno del usuario mientras copiábamos? Sus manos
        // ganan — el /clear pierde, la copia queda (y el acuse lo dice).
        if sh.exit.load(Ordering::Relaxed) >= 0 {
            return ack_json(id, text, false, "ERR_RELAY_GONE", Some(&ps));
        }
        if sh.busy.load(Ordering::Relaxed) != 0 {
            return ack_json(id, text, false, "ERR_RELAY_BUSY", Some(&ps));
        }
        if !sp.say(sh, text) {
            return ack_json(id, text, false, "ERR_RELAY_WRITE", Some(&ps));
        }
        sh.inject_at.store(sh.ms(), Ordering::Relaxed);
        return ack_json(id, text, true, "", Some(&ps));
    }
    // MODO TERMINAL: la TUI sí tiene /export — el camino validado no se toca.
    let path = dir.join(format!("handoff-{}-{}.md", std::process::id(), now_epoch()));
    let ps = path.to_string_lossy().to_string();
    if !sp.say(sh, &format!("/export {ps}")) {
        return ack_json(id, text, false, "ERR_RELAY_WRITE", None);
    }
    sh.inject_at.store(sh.ms(), Ordering::Relaxed);
    // Esperar la copia: el archivo existe con contenido Y el REPL volvió a
    // callarse (el "Conversation exported to:" también es salida).
    let t0 = Instant::now();
    let ok = loop {
        std::thread::sleep(Duration::from_millis(250));
        if sh.exit.load(Ordering::Relaxed) >= 0 {
            return ack_json(id, text, false, "ERR_RELAY_GONE", None);
        }
        let there = std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false);
        if there && sh.since_out() >= EXPORT_SETTLE_MS && sh.busy.load(Ordering::Relaxed) == 0 {
            break true;
        }
        if t0.elapsed().as_millis() as u64 >= EXPORT_WAIT_MS {
            break there;
        }
    };
    if !ok {
        return ack_json(id, text, false, "ERR_RELAY_EXPORT", None);
    }
    // R4 otra vez: si el usuario tecleó durante la espera, el /clear pierde
    // (la copia queda hecha, y el acuse lo dice — no es un fallo, es que
    // sus manos ganan siempre).
    if sh.has_text() {
        return ack_json(id, text, false, "ERR_RELAY_TYPED", Some(&ps));
    }
    if !sp.say(sh, text) {
        return ack_json(id, text, false, "ERR_RELAY_WRITE", Some(&ps));
    }
    sh.inject_at.store(sh.ms(), Ordering::Relaxed);
    ack_json(id, text, true, "", Some(&ps))
}

// ---------------------------------------------------------------------------
// Modo wrap: el chat de VS Code de ESTA máquina (etapa 4e)
//
// El shim del PATH no alcanza al chat: la extensión no lanza `claude` por el
// PATH, lo lanza por una ruta. Pero deja un enganche OFICIAL para ponerse en
// medio, `claudeCode.claudeProcessWrapper` ("Executable path used to launch
// the Claude process"), y el ajuste apunta AQUÍ. La extensión y la CLI hablan
// stream-json —una línea JSON por mensaje—, así que este modo no envuelve una
// PTY: envuelve ese tubo.
//
// Por qué es MÁS seguro que el de terminal, no menos:
//   - R1 (no teclear encima) se cumple por CONSTRUCCIÓN: no hay buffer
//     compartido ni teclas a medias. Cada mensaje es una línea atómica.
//   - R2 deja de inferirse del silencio: lo dice el protocolo (entra un
//     `user`, sale un `result`). Certeza, no fail-closed.
// Residual, dicho sin adornos: no vemos el borrador del cuadro de texto. Por
// eso siguen existiendo la cuenta atrás y la ventana de calma.
//
// Es el gemelo Windows de `michi-relevo.py wrap`, que hace esto mismo en los
// servidores y en WSL. MISMO esquema de estado, MISMOS códigos, MISMA lista
// blanca — invariante #1: los dos lados en sincronía.
// ---------------------------------------------------------------------------

/// Una línea `user` como las que manda la extensión: es lo que la CLI ENTIENDE.
fn user_line(text: &str) -> String {
    serde_json::json!({
        "type": "user",
        "message": {"role": "user", "content": [{"type": "text", "text": text}]}
    })
    .to_string()
        + "\n"
}

/// El eco hacia el CHAT necesita la forma COMPLETA del replay de la CLI
/// —session_id, uuid, parent_tool_use_id, timestamp, isReplay—: la extensión
/// DESCARTA EN SILENCIO un `user` a secas que no case con la sesión (medido
/// contra el binario real, 2026-08-10). Al hijo, en cambio, se le manda la
/// forma corta de `user_line`. Confundirlas no rompe nada visible: el eco
/// simplemente no se pinta, que es peor.
fn replay_line(text: &str, sid: &str) -> String {
    serde_json::json!({
        "type": "user",
        "message": {"role": "user", "content": [{"type": "text", "text": text}]},
        "session_id": sid,
        "parent_tool_use_id": serde_json::Value::Null,
        "uuid": pseudo_uuid(),
        "timestamp": iso_utc_now(),
        "isReplay": true
    })
    .to_string()
        + "\n"
}

/// Un identificador con forma de uuid. NO es criptográfico y no lo necesita:
/// solo tiene que ser distinto del de al lado dentro de una conversación.
/// Traerse un crate para rellenar un campo rompería la dieta de dependencias
/// de este binario (invariante #4).
fn pseudo_uuid() -> String {
    let mut x = (now_epoch_ms() as u64) ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    let mut hex = String::with_capacity(32);
    for _ in 0..4 {
        // xorshift64: barato, determinista y suficiente para distinguir
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        hex.push_str(&format!("{x:016x}"));
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Fecha UTC en ISO-8601. A mano porque `chrono` no vive en este crate y no
/// va a entrar por un campo de texto.
fn iso_utc_now() -> String {
    let s = now_epoch();
    let (y, m, d) = civil_from_days(s.div_euclid(86_400));
    let r = s.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z",
        r / 3600,
        (r % 3600) / 60,
        r % 60
    )
}

/// Días desde 1970-01-01 → (año, mes, día). Algoritmo `civil_from_days` de
/// Howard Hinnant (dominio público): aritmética entera, sin tablas ni casos
/// especiales de bisiesto.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// El binario REAL de Claude Code, con las dos convenciones posibles del
/// wrapper cubiertas (que la extensión pase la ruta como primer argumento o
/// que no la pase), porque cuál usa NO está documentada y adivinar rompería
/// el chat de alguien. Después, el binario que trae la propia extensión, y
/// por último el PATH — descartando SIEMPRE nuestro propio atajo, que
/// volvería a llamarnos.
fn find_claude_bin(args: &[String]) -> (Option<PathBuf>, Vec<String>) {
    let rest: Vec<String> = args.to_vec();
    if let Some(first) = rest.first() {
        let p = PathBuf::from(first);
        if p.is_file() {
            return (Some(p), rest[1..].to_vec());
        }
    }
    if let Some(v) = std::env::var_os("MICHI_CLAUDE_BIN") {
        let p = PathBuf::from(v);
        if p.is_file() {
            return (Some(p), rest);
        }
    }
    if let Some(p) = ext_claude() {
        return (Some(p), rest);
    }
    (path_claude(), rest)
}

/// El binario que la extensión instala consigo (lo normal en Windows).
fn ext_claude() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").ok()?;
    let dir = PathBuf::from(home).join(".vscode").join("extensions");
    let mut cands: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("anthropic.claude-code-"))
                .unwrap_or(false)
        })
        .collect();
    cands.sort(); // la versión más nueva ordena la última
    for c in cands.into_iter().rev() {
        for n in ["claude.exe", "claude"] {
            let p = c.join("resources").join("native-binary").join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// `claude` del PATH, saltándose el atajo de MichiClaude: si lo cogiéramos,
/// nos llamaríamos a nosotros mismos.
fn path_claude() -> Option<PathBuf> {
    let ours = state_dir().parent().map(|p| p.join("bin"));
    let mut cmd = std::process::Command::new("where.exe");
    cmd.arg("claude");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    let txt = String::from_utf8_lossy(&out.stdout).to_string();
    let mut mejor: Option<PathBuf> = None;
    for l in txt.lines() {
        let p = PathBuf::from(l.trim());
        if !p.is_file() || p.parent().map(PathBuf::from) == ours {
            continue;
        }
        // un .exe se lanza directo; un .cmd necesita cmd.exe, así que se
        // prefiere el primero y el segundo queda de reserva
        let exe = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("exe"))
            .unwrap_or(false);
        if exe {
            return Some(p);
        }
        if mejor.is_none() {
            mejor = Some(p);
        }
    }
    mejor
}

/// El comando para lanzarlo. Un `.cmd` (instalación por npm) no lo puede
/// ejecutar Windows directamente: va por `cmd.exe /c`.
fn claude_command(bin: &PathBuf, rest: &[String]) -> std::process::Command {
    let bat = bin
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        .unwrap_or(false);
    if bat {
        let mut c = std::process::Command::new("cmd.exe");
        c.arg("/c").arg(bin).args(rest);
        c
    } else {
        let mut c = std::process::Command::new(bin);
        c.args(rest);
        c
    }
}

/// ¿Esto es la extensión lanzando Claude Code? Se mira el protocolo, que es
/// el hecho: sin `--input-format` no hay tubo que envolver. Con esto el
/// ajuste de VS Code puede apuntar a michi.exe A SECAS, sin lanzador
/// intermedio (un .cmd en medio se lo tragaría el spawn de Node).
fn looks_like_chat_launch(args: &[String]) -> bool {
    args.iter().any(|a| a == "--input-format")
        || args
            .first()
            .map(|f| PathBuf::from(f).is_file())
            .unwrap_or(false)
}

/// Rastro del wrapper. Aquí no hay pantalla donde quejarse —la extensión se
/// come stderr— así que sin esto un fallo del enganche es indistinguible de
/// "no pasa nada": la conversación funciona igual y el relevo no aparece.
/// Solo la LLAMADA (argumentos y decisión), nunca contenido de la charla.
fn wrap_trace(linea: &str) {
    let Some(d) = state_dir().parent().map(|p| p.to_path_buf()) else {
        return;
    };
    let p = d.join("wrap_debug.txt");
    // se trunca solo: esto es para diagnosticar, no un histórico
    let previo = std::fs::read_to_string(&p).unwrap_or_default();
    let previo = if previo.len() > 20_000 { String::new() } else { previo };
    let _ = std::fs::write(&p, format!("{previo}{} · {linea}\n", now_epoch()));
}

fn run_wrap(args: &[String]) -> ! {
    wrap_trace(&format!("llamada: {}", args.join(" ")));
    let (bin, rest) = find_claude_bin(args);
    let Some(bin) = bin else {
        wrap_trace("sin binario de Claude Code: no puedo hacer nada");
        eprintln!("michi: no encontré el binario de Claude Code");
        std::process::exit(127);
    };

    // Ya dentro de un relevo, o sin el protocolo esperado: paso directo. Este
    // wrapper NUNCA puede dejar a alguien sin Claude Code — misma regla que
    // el shim del PATH.
    if std::env::var_os("MICHI_RELEVO").is_some() || !rest.iter().any(|a| a == "--input-format") {
        wrap_trace(&format!(
            "paso directo (anidado={}, protocolo={}) → {}",
            std::env::var_os("MICHI_RELEVO").is_some(),
            rest.iter().any(|a| a == "--input-format"),
            bin.to_string_lossy()
        ));
        let code = claude_command(&bin, &rest)
            .env("MICHI_RELEVO", "0")
            .status()
            .map(|s| s.code().unwrap_or(1))
            .unwrap_or(127);
        std::process::exit(code);
    }
    wrap_trace(&format!("relevando el chat con {}", bin.to_string_lossy()));

    let pid = std::process::id();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let started = now_epoch();
    let mut child = match claude_command(&bin, &rest)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .env("MICHI_RELEVO", pid.to_string())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("michi: no pude lanzar claude: {e}");
            std::process::exit(127);
        }
    };

    let sh = Arc::new(Shared {
        base: Instant::now(),
        last_in: AtomicU64::new(0),
        last_out: AtomicU64::new(0),
        // en el chat no hay teclas que vigilar: R1 se cumple por
        // construcción y `typed` es SIEMPRE false, sin fingir otra cosa
        keys: Mutex::new(KeyWatch::new()),
        inject_at: AtomicU64::new(0),
        handoff: AtomicU64::new(0),
        exit: AtomicI64::new(-1),
        user_cmd: Mutex::new(None),
        ack: Mutex::new(None),
        busy: AtomicU64::new(0),
    });
    let sid = Arc::new(Mutex::new(String::new()));
    // 0 = el aviso de "esta ventana va relevada" aún no se ha pintado en la
    // conversación en curso
    let banner = Arc::new(AtomicU64::new(0));

    let to_child: Arc<Mutex<Option<Box<dyn Write + Send>>>> = match child.stdin.take() {
        Some(s) => Arc::new(Mutex::new(Some(Box::new(s)))),
        None => {
            eprintln!("michi: la sesión no me dejó escribirle");
            std::process::exit(1);
        }
    };
    let sp = Arc::new(Speaker {
        to_child: to_child.clone(),
        sid: Some(sid.clone()),
    });

    prune_handoffs();
    let dir = state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let state_path = dir.join(format!("{pid}.json"));
    let cmd_path = dir.join(format!("{pid}.cmd"));

    // Hilo 1 — del chat hacia Claude. Cada línea se reenvía INTACTA.
    {
        let sh = sh.clone();
        let sp = sp.clone();
        std::thread::spawn(move || {
            let inp = std::io::stdin();
            let mut linea = String::new();
            loop {
                linea.clear();
                match inp.lock().read_line(&mut linea) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(linea.trim()) {
                    if v["type"].as_str() == Some("user") {
                        sh.last_in.store(sh.ms(), Ordering::Relaxed);
                        sh.busy.store(1, Ordering::Relaxed);
                        // ¿aplicó el usuario uno de los dos por su cuenta?
                        // cuenta para el desbloqueo igual que en la terminal
                        let txt = match &v["message"]["content"] {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Array(a) => a
                                .iter()
                                .filter_map(|b| b["text"].as_str())
                                .collect::<Vec<_>>()
                                .join(" "),
                            _ => String::new(),
                        };
                        let txt = txt.trim();
                        for a in ALLOWED {
                            if txt == a || txt.starts_with(&format!("{a} ")) {
                                *sh.user_cmd.lock().unwrap() = Some((now_epoch(), a.to_string()));
                            }
                        }
                    }
                }
                let mut lock = sp.to_child.lock().unwrap();
                let Some(w) = lock.as_mut() else { break };
                if w.write_all(linea.as_bytes()).is_err() {
                    break;
                }
                let _ = w.flush();
            }
            // El chat colgó: se le da EOF al hijo soltando el tubo. Sin esto
            // se quedaría esperando una entrada que ya no va a llegar.
            *sp.to_child.lock().unwrap() = None;
        });
    }

    // Hilo 2 — de Claude hacia el chat. Igual de intacta.
    {
        let sh = sh.clone();
        let sid = sid.clone();
        let banner = banner.clone();
        let salida = child.stdout.take();
        std::thread::spawn(move || {
            let Some(salida) = salida else { return };
            let mut rd = std::io::BufReader::new(salida);
            let mut linea = String::new();
            loop {
                linea.clear();
                match rd.read_line(&mut linea) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                sh.last_out.store(sh.ms(), Ordering::Relaxed);
                let mut fin_de_turno = false;
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(linea.trim()) {
                    match v["type"].as_str() {
                        // fin de turno: CERTEZA, no silencio inferido
                        Some("result") => {
                            sh.busy.store(0, Ordering::Relaxed);
                            fin_de_turno = true;
                        }
                        Some("system") => {
                            if let Some(s) = v["session_id"].as_str() {
                                let mut cur = sid.lock().unwrap();
                                // sesión NUEVA en el mismo proceso
                                // (conversación nueva o /clear): el aviso se
                                // re-arma para que cada una estrene el suyo
                                if !cur.is_empty() && *cur != s {
                                    banner.store(0, Ordering::Relaxed);
                                }
                                *cur = s.to_string();
                            }
                        }
                        _ => {}
                    }
                }
                let out = std::io::stdout();
                let mut lock = out.lock();
                if lock.write_all(linea.as_bytes()).is_err() {
                    break;
                }
                // EL AVISO, al ACABAR el primer turno y no antes. Primero se
                // probó delante del primer mensaje (como en Linux) y en
                // Windows no se pintaba nunca — mientras que el eco de un
                // /compact inyectado, que es la MISMA línea de replay, sale
                // perfecto. La diferencia era el momento: el eco llega con el
                // chat en reposo y el aviso llegaba con el mensaje del usuario
                // en vuelo. Así que se emite donde ya sabemos que se pinta.
                if fin_de_turno && banner.load(Ordering::Relaxed) == 0 {
                    let s = sid.lock().unwrap().clone();
                    if !s.is_empty() {
                        banner.store(1, Ordering::Relaxed);
                        let msg = format!(
                            "michi · relevo activo (sesión {pid}) — MichiClaude puede aplicar /compact y /clear en esta ventana"
                        );
                        let _ = lock.write_all(replay_line(&msg, &s).as_bytes());
                        wrap_trace("aviso emitido al acabar el primer turno");
                    }
                }
                let _ = lock.flush();
            }
        });
    }

    let mut last_state = 0u64;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                sh.exit.store(st.code().unwrap_or(0) as i64, Ordering::Relaxed);
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }

        if let Ok(raw) = std::fs::read_to_string(&cmd_path) {
            let _ = std::fs::remove_file(&cmd_path);
            if let Some(ack) = attend(&raw, &sh, &sp) {
                *sh.ack.lock().unwrap() = Some(ack);
                last_state = 0;
            }
        }

        let now = sh.ms();
        if last_state == 0 || now.saturating_sub(last_state) >= STATE_EVERY_MS {
            last_state = now.max(1);
            let s = sid.lock().unwrap().clone();
            write_atomic(&state_path, &snapshot(&sh, pid, started, &cwd, "chat", &s));
        }

        std::thread::sleep(Duration::from_millis(TICK_MS));
    }

    let _ = std::fs::remove_file(&state_path);
    let _ = std::fs::remove_file(&cmd_path);
    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(0);
    std::process::exit(code);
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

fn cmd_status(args: &[String]) {
    let list = live_sessions();
    if list.is_empty() {
        println!("Ninguna sesión con relevo. Abre una con:  michi claude");
        return;
    }
    // `michi status --debug` saca la foto cruda, con las CUENTAS de teclas.
    // Es lo que hay que pedir cuando el guardián se equivoca: dice si las
    // teclas llegaron y cómo quedó el modelo del prompt, sin enseñar ni una
    // letra de lo escrito.
    if args.iter().any(|a| a == "--debug") {
        for v in &list {
            println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
        }
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
    // `--export` (solo con /clear): la red de seguridad, igual que la pide
    // el panel. Sirve para validar la secuencia sin la app en medio.
    let export = args.iter().any(|a| a == "--export");
    let rest: Vec<String> = args.iter().filter(|a| *a != "--export").cloned().collect();
    let (pid, text) = match rest.as_slice() {
        [p, t] => (p.clone(), t.clone()),
        [t] => match live_sessions().first() {
            Some(v) => (v["pid"].as_i64().unwrap_or(0).to_string(), t.clone()),
            None => {
                eprintln!("michi: no hay ninguna sesión con relevo abierta");
                std::process::exit(1)
            }
        },
        _ => {
            eprintln!("uso: michi inject [sesión] /compact  |  /clear [--export]");
            std::process::exit(2)
        }
    };
    if !allowed(&text) {
        eprintln!("michi: solo puedo aplicar {}", ALLOWED.join(" o "));
        std::process::exit(2);
    }
    let dir = state_dir();
    let id = format!("cli-{}", now_epoch_ms());
    write_atomic(
        &dir.join(format!("{pid}.cmd")),
        &serde_json::json!({"id": id, "op": "inject", "text": text,
            "export": export && text == "/clear"})
        .to_string(),
    );
    // Esperar el acuse en el archivo de estado (el relevo mira cada 250 ms).
    // Con red, la secuencia espera a la copia: hasta ~15 s más de margen.
    let state = dir.join(format!("{pid}.json"));
    let tries = if export { 120 } else { 40 };
    for _ in 0..tries {
        std::thread::sleep(Duration::from_millis(200));
        let Ok(raw) = std::fs::read_to_string(&state) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if v["last"]["id"].as_str() == Some(id.as_str()) {
            let copia = v["last"]["export"]
                .as_str()
                .map(|p| format!(" (copia: {p})"))
                .unwrap_or_default();
            if v["last"]["ok"].as_bool().unwrap_or(false) {
                println!("aplicado: {text}{copia}");
            } else {
                println!(
                    "no se aplicó: {}{copia}",
                    v["last"]["err"].as_str().unwrap_or("?")
                );
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
           michi wrap [...]     releva el CHAT de VS Code (es el ajuste\n\
                                claudeCode.claudeProcessWrapper; no se teclea\n\
                                a mano)\n\
           michi status         sesiones con relevo abiertas ahora\n\
           michi status --debug foto cruda (cuentas de teclas, sin contenido)\n\
           michi inject /compact\n\
                                aplica un comando a la sesión con relevo\n\
           michi inject /clear --export\n\
                                /clear con red: guarda antes una copia con\n\
                                /export y sin copia verificada no borra\n\
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
        Some("wrap") => run_wrap(&args[1..]),
        Some("status") => cmd_status(&args[1..]),
        Some("inject") => cmd_inject(&args[1..]),
        Some("--version") | Some("-v") => println!("michi {}", env!("CARGO_PKG_VERSION")),
        // Sin subcomando: puede ser la extensión de VS Code llamándonos como
        // wrapper (el ajuste apunta a este .exe y le pasa los argumentos de
        // Claude Code tal cual). Se reconoce por el protocolo, no por
        // adivinar, y si no lo es se enseña la ayuda de siempre.
        _ if looks_like_chat_launch(&args) => run_wrap(&args),
        _ => usage(),
    }
}
