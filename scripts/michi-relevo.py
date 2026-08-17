#!/usr/bin/env python3
"""michi-relevo.py — el relevo de MichiClaude para Linux (VPS, y algún día WSL).

Réplica del crate `relevo/` (michi.exe) con SOLO la biblioteca estándar: en
Linux la PTY es nativa (`pty`, `termios`) y no hace falta compilar nada — el
script viaja embebido en la app y se re-sube por SSH igual que
meter-export.py. MANTENER EN SINCRONÍA con relevo/src/main.rs (espíritu del
invariante #1): mismo esquema de estado, mismos códigos ERR_RELAY_*, mismas
constantes. Lo que NO se replica: el decodificador win32-input-mode (eso es
ConPTY de Windows; aquí las teclas llegan como bytes normales).

Uso:
  michi-relevo.py claude [args...]   abre Claude Code con relevo (TERMINAL)
  michi-relevo.py wrap [claude] [args...]
                                     relevo del CHAT: proxy de stream-json,
                                     para claudeCode.claudeProcessWrapper
  michi-relevo.py status [--debug]   sesiones con relevo vivas
  michi-relevo.py inject [pid] /compact
  michi-relevo.py inject [pid] /clear --export
                                     /clear con red: guarda antes una copia
                                     con /export; sin copia no borra

Privacidad, igual que en Windows: el relevo ve cada tecla porque está en
medio del cable, pero JAMÁS escribe lo tecleado en disco — del tecleo solo
salen un booleano, relojes y cuentas.
"""
import fcntl
import json
import os
import select
import signal
import struct
import subprocess
import sys
import termios
import threading
import time
import tty
import uuid

# 2 = este relevo sabe hacer la red /export antes de un /clear (el panel
# decide por este número; uno viejo ignoraría la marca y borraría sin copia)
STATE_V = 2
ALLOWED = ("/compact", "/clear")
# ÚNICA ampliación con argumento (2026-08-17, ruteo etapa 5b — el guardián
# escala solo): `/model <alias>` con alias de LISTA CERRADA. Nada más pasa:
# ni texto libre, ni un modelo con id raro. Sigue siendo una lista.
MODEL_ALIASES = ("haiku", "sonnet", "opus", "fable")
# Un `/model` llega en el instante en que el guardián FRENA un prompt: el
# relevo aún ve el turno "en curso" (el result del bloqueo llega un pelo
# después). Para ese comando —y SOLO para ese, que no destruye nada— se
# espera a que el relevo quede libre en vez de rechazar (medido 2026-08-17:
# ERR_RELAY_BUSY con la orden escrita 0.2 s después del bloqueo).
MODEL_WAIT_S = 8.0
MODEL_TICK_S = 0.2


def is_model_cmd(text):
    return text.split()[:1] == ["/model"]


def allowed(text):
    """¿Es un comando que este relevo puede teclear? Los dos exactos, o
    `/model <alias>` con el alias en la lista cerrada."""
    if text in ALLOWED:
        return True
    parts = text.split()
    return len(parts) == 2 and parts[0] == "/model" and parts[1] in MODEL_ALIASES

CALM_MS = 8_000       # R3: calma de teclado
QUIET_MS = 2_000      # R2: silencio de la PTY ("Claude generando" se INFIERE)
COOLDOWN_MS = 15_000  # tras inyectar
SUBMIT_WAIT_MS = 3_000  # un Enter no limpia hasta ver si Claude REACCIONA
# la red del /clear: espera máxima a que /export escriba su archivo, y
# silencio de la PTY exigido tras verlo aparecer (el REPL asentado)
EXPORT_WAIT_MS = 12_000
EXPORT_SETTLE_MS = 1_500
# Pausa entre el texto y su Enter. NO es cosmética: la TUI de Claude Code
# trata texto+Enter en la MISMA ráfaga como un pegado y NO ejecuta la línea
# — se queda escrita en el prompt. Con /compact (9 bytes) colaba; con
# /export <ruta> (~110) el Enter se lo tragaba SIEMPRE (cazado en vivo,
# 2026-08-09). Se separan para todos: el fallo dependía del largo.
ENTER_GAP_S = 0.25
HANDOFF_KEEP_DAYS = 90  # una copia más vieja ya cumplió su papel de red
TICK_S = 0.25
STATE_EVERY_S = 0.5
FRESH_S = 15          # viva = estado con menos de esto (MISMA regla que el panel)

# El mensaje que se inyecta en modo chat. Es el MISMO protocolo que usa la
# extensión para tus mensajes: una línea JSON completa por stdin.
def user_line(text):
    return json.dumps({"type": "user", "message": {
        "role": "user", "content": [{"type": "text", "text": text}]}}) + "\n"


# El eco hacia el CHAT necesita la forma COMPLETA del replay del CLI —
# session_id, uuid, parent_tool_use_id, timestamp, isReplay — porque la
# extensión descarta en silencio lo que no case con la sesión (medido
# 2026-08-10 contra el binario real: la forma corta no se pinta). El hijo,
# en cambio, recibe siempre la forma corta de user_line().
def replay_line(text, sid):
    return json.dumps({
        "type": "user",
        "message": {"role": "user",
                    "content": [{"type": "text", "text": text}]},
        "session_id": sid, "parent_tool_use_id": None,
        "uuid": str(uuid.uuid4()),
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S",
                                   time.gmtime()) + ".000Z",
        "isReplay": True}) + "\n"


# El MISMO texto en terminal y chat: si alguna vez cambia, cambia en los dos.
def banner_text(pid):
    return (f"michi · relevo activo (sesión {pid}) — MichiClaude puede "
            "aplicar /compact y /clear en esta ventana")


def state_dir():
    d = os.path.expanduser("~/.michiclaude/relevo")
    os.makedirs(d, exist_ok=True)
    return d


def write_atomic(path, data):
    # el temporal AÑADE .tmp al nombre ENTERO: con la extensión reemplazada,
    # <pid>.json y <pid>.cmd compartirían <pid>.tmp y se pisarían
    tmp = path + ".tmp"
    try:
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(data)
        os.replace(tmp, path)
    except OSError:
        pass


def now_ms():
    return int(time.monotonic() * 1000)


def now_epoch():
    return int(time.time())


# ---------- la red del /clear: /export verificado antes de borrar ----------
# La ruta de la copia la genera el RELEVO — jamás viene del canal, así que
# el canal sigue sin poder dictar ni un byte fuera de la lista blanca.

def handoff_path(ext="md"):
    d = os.path.expanduser("~/.michiclaude/handoff")
    os.makedirs(d, exist_ok=True)
    return os.path.join(d, f"handoff-{os.getpid()}-{now_epoch()}.{ext}")


def session_jsonl(sid):
    """El JSONL de la sesión `sid`: <config>/projects/<carpeta>/<sid>.jsonl.
    El nombre del archivo ES el session_id (UUID, único), así que se busca
    por nombre en vez de reproducir la transformación de carpetas de Claude
    Code — menos frágil ante sus cambios. Solo lo usa el modo chat (2026-08-13):
    la extensión NO tiene /export y la copia de la red la hace el relevo."""
    if not sid:
        return None
    base = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    try:
        for e in os.scandir(os.path.join(base, "projects")):
            p = os.path.join(e.path, sid + ".jsonl")
            if os.path.isfile(p):
                return p
    except OSError:
        pass
    return None


def prune_handoffs():
    """Al arrancar: las copias de hace más de HANDOFF_KEEP_DAYS se van.
    Best-effort — un fallo aquí no frena nada."""
    d = os.path.expanduser("~/.michiclaude/handoff")
    now = time.time()
    try:
        for n in os.listdir(d):
            p = os.path.join(d, n)
            try:
                if now - os.path.getmtime(p) > HANDOFF_KEEP_DAYS * 24 * 3600:
                    os.unlink(p)
            except OSError:
                pass
    except OSError:
        pass


def ack_row(rid, text, ok, err, export=None):
    """El acuse con la misma forma en todos los caminos. `export` = ruta de
    la copia si la hubo (un TYPED tras exportar deja copia pero no /clear,
    y las dos cosas tienen que poder decirse)."""
    a = {"id": rid, "ok": ok, "err": err, "text": text, "ts": now_epoch()}
    if export:
        a["export"] = export
    return a


# ---------- lector de teclas: de dónde salen `typed` y `user_cmd` ----------
# Réplica de KeyWatch (main.rs) sin la rama win32: aquí una tecla es un byte.
# Reglas que vienen de fallos reales de la 3a, no perderlas:
#  - los avisos del terminal (foco ESC[I/O, cursor R, DA c, DSR n, ventana t)
#    NO son teclas y no reinician la calma;
#  - "hay texto" se DERIVA del buffer — jamás un booleano al lado;
#  - un Enter no limpia el modelo: aparta la línea a `pending` y espera a ver
#    si Claude reacciona (bytes por la PTY). Sin reacción en SUBMIT_WAIT_MS,
#    la línea VUELVE. Mientras se decide, cuenta como texto vivo (fail-closed).
class KeyWatch:
    def __init__(self):
        self.line = bytearray()
        self.esc = 0          # 0 fuera · 1 vi ESC · 2 CSI · 3 OSC
        self.pending = None   # (línea, ms) de un Enter aún sin veredicto
        self.k_print = 0
        self.k_enter = 0
        self.k_esc = 0
        self.k_other = 0
        self.user_cmd = None  # (epoch, texto) si el usuario tecleó uno permitido

    def feed(self, data, now):
        """Devuelve True si hubo actividad HUMANA (mueve el reloj de calma)."""
        human = False
        for b in data:
            if self.esc == 0:
                if b == 0x1B:
                    self.esc = 1
                    self.k_esc += 1
                elif b in (13, 10):
                    self.k_enter += 1
                    human = True
                    self._enter(now)
                elif b in (8, 127):
                    self.k_other += 1
                    human = True
                    # borra un CARÁCTER, no un byte (UTF-8 multibyte)
                    while self.line and (self.line.pop() & 0xC0) == 0x80:
                        pass
                elif b in (3, 21):  # Ctrl+C / Ctrl+U
                    self.k_other += 1
                    human = True
                    self.line.clear()
                    self.pending = None
                elif b >= 32 or (b & 0x80):
                    self.k_print += 1
                    human = True
                    if len(self.line) < 65536:
                        self.line.append(b)
                else:
                    self.k_other += 1
                    human = True
            elif self.esc == 1:
                if b == 0x5B:      # [
                    self.esc = 2
                elif b == 0x5D:    # ]
                    self.esc = 3
                else:              # ESC suelto + algo: humano (Alt+tecla…)
                    self.esc = 0
                    self.k_other += 1
                    human = True
            elif self.esc == 2:
                if 0x40 <= b <= 0x7E:
                    self.esc = 0
                    # I/O foco · R cursor · c DA · n DSR · t ventana: avisos
                    # del TERMINAL, no del humano
                    if b not in (0x49, 0x4F, 0x52, 0x63, 0x6E, 0x74):
                        human = True
            else:  # OSC: respuesta del terminal, hasta BEL o ST
                if b == 0x07:
                    self.esc = 0
                elif b == 0x1B:
                    self.esc = 4
                if self.esc == 4 and b == 0x5C:
                    self.esc = 0
        return human

    def _enter(self, now):
        if self.line.endswith(b"\\"):
            return  # línea de continuación, sigue siendo texto vivo
        txt = self.line.decode("utf-8", "replace").strip()
        for a in ALLOWED:
            if txt == a or txt.startswith(a + " "):
                self.user_cmd = (now_epoch(), a)
        old = bytes(self.line)
        self.line.clear()
        if self.pending:
            old = self.pending[0] + old
        self.pending = (old, now)

    def resolve(self, now, last_out):
        """El veredicto del Enter apartado: si la PTY escupió bytes después,
        se envió; si pasó la espera sin reacción, la línea VUELVE."""
        if not self.pending:
            return
        line, at = self.pending
        if last_out > at:
            self.pending = None          # Claude reaccionó: se envió
        elif now - at >= SUBMIT_WAIT_MS:
            self.line = bytearray(line) + self.line
            self.pending = None          # no se envió: el texto sigue ahí

    def has_text(self):
        return bool(self.line) or self.pending is not None


# ---------- la marca del título (misma máquina que TitleMark en Rust) ----------
# Única excepción al paso transparente: antepone «michi · carpeta · » al
# título que escriba Claude Code (OSC 0/1/2). Fail-open con tope: lo peor
# posible es quedarse sin marca, jamás comerse la salida.
class TitleMark:
    MAX = 1024

    def __init__(self, mark):
        self.mark = mark.encode()
        self.st = 0
        self.buf = bytearray()

    def _rewrite(self):
        i = self.buf.find(b";")
        if i < 0:
            return bytes(self.buf)
        head, rest = bytes(self.buf[: i + 1]), bytes(self.buf[i + 1:])
        if rest.startswith(self.mark):
            return bytes(self.buf)
        return head + self.mark + rest

    def feed(self, data):
        out = bytearray()
        for b in data:
            if self.st == 0:
                if b == 0x1B:
                    self.st = 1
                    self.buf = bytearray([b])
                else:
                    out.append(b)
            elif self.st == 1:
                self.buf.append(b)
                if b == 0x5D:
                    self.st = 2
                else:
                    out += self.buf
                    self.buf.clear()
                    self.st = 0
            elif self.st == 2:
                # el NÚMERO entero, no el primer dígito: ESC]10; es color
                self.buf.append(b)
                if 0x30 <= b <= 0x39 and len(self.buf) < 8:
                    pass
                elif b == 0x3B:
                    if bytes(self.buf[2:-1]) in (b"0", b"1", b"2"):
                        self.st = 3
                    else:
                        out += self.buf
                        self.buf.clear()
                        self.st = 0
                else:
                    out += self.buf
                    self.buf.clear()
                    self.st = 0
            else:
                self.buf.append(b)
                done = b == 0x07 or (
                    b == 0x5C and len(self.buf) >= 2 and self.buf[-2] == 0x1B
                )
                if done:
                    out += self._rewrite()
                    self.buf.clear()
                    self.st = 0
                elif len(self.buf) > self.MAX:
                    out += self.buf
                    self.buf.clear()
                    self.st = 0
        return bytes(out)


# ---------- el relevo ----------

def base_name(path):
    b = path.rstrip("/").rsplit("/", 1)[-1]
    return b or path


def run_relevo(extra):
    cwd = os.getcwd()
    pid = os.getpid()
    claude = os.environ.get("MICHI_CLAUDE_BIN", "claude")

    # SIN terminal no hay relevo: es una invocación no interactiva (la
    # extensión de VS Code, un script, un pipe). FAIL-OPEN al claude real,
    # con MICHI_RELEVO=0 para que un futuro shim de PATH no re-entre en bucle.
    if os.environ.get("MICHI_RELEVO") or not (
        os.isatty(0) and os.isatty(1)
    ):
        os.environ["MICHI_RELEVO"] = "0"
        try:
            os.execvp(claude, [claude] + list(extra))
        except OSError as e:
            print(f"michi: no pude lanzar `claude`: {e}", file=sys.stderr)
            sys.exit(127)

    print(banner_text(pid))

    master, slave = os.openpty()

    # tamaño inicial y reenvío de los cambios
    def sync_size(*_):
        try:
            c = os.get_terminal_size()
            fcntl.ioctl(master, termios.TIOCSWINSZ,
                        struct.pack("HHHH", c.lines, c.columns, 0, 0))
        except OSError:
            pass

    sync_size()
    signal.signal(signal.SIGWINCH, sync_size)

    def preexec():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    env = dict(os.environ, MICHI_RELEVO=str(pid))
    try:
        child = subprocess.Popen(
            [claude] + list(extra), stdin=slave, stdout=slave, stderr=slave,
            preexec_fn=preexec, env=env, cwd=cwd, close_fds=True,
        )
    except OSError as e:
        os.close(master)
        os.close(slave)
        print(f"michi: no encontré `claude` en el PATH: {e}", file=sys.stderr)
        sys.exit(127)
    os.close(slave)

    saved = termios.tcgetattr(0)
    tty.setraw(0)

    kw = KeyWatch()
    tm = TitleMark(f"michi · {base_name(cwd)} · ")
    started = now_epoch()
    base = now_ms()
    last_in = base
    last_out = base
    inject_at = 0
    last_ack = None
    last_state = 0.0
    # secuencia /export+/clear en curso: el relevo está ocupado (corre en su
    # hilo para que este bucle siga bombeando pantalla y estado)
    hand = {"on": False}
    pend = {"cmd": None}   # /model a la espera de que el relevo quede libre
    prune_handoffs()
    d = state_dir()
    state_path = os.path.join(d, f"{pid}.json")
    cmd_path = os.path.join(d, f"{pid}.cmd")
    # marca de arranque, para que el título salga aunque Claude no ponga uno
    os.write(1, f"\x1b]0;michi · {base_name(cwd)}\x07".encode())

    def why_not():
        now = now_ms()
        if child.poll() is not None:
            return "ERR_RELAY_GONE"
        if hand["on"]:
            return "ERR_RELAY_BUSY"
        kw.resolve(now, last_out)
        if kw.has_text():
            return "ERR_RELAY_TYPED"
        if now - last_out < QUIET_MS:
            return "ERR_RELAY_BUSY"
        if now - last_in < CALM_MS:
            return "ERR_RELAY_NOISY"
        if inject_at and now - inject_at < COOLDOWN_MS:
            return "ERR_RELAY_COOLDOWN"
        return ""

    def snapshot():
        now = now_ms()
        kw.resolve(now, last_out)
        why = why_not()
        uc = kw.user_cmd
        return json.dumps({
            "v": STATE_V,
            "pid": pid,
            "started": started,
            "cwd": cwd,
            "ts": now_epoch(),
            "alive": child.poll() is None,
            "typed": kw.has_text(),
            "idle_in": (now - last_in) // 1000,
            "idle_out": (now - last_out) // 1000,
            "ready": why == "",
            "why": why,
            # cuentas, nunca contenido — para diagnosticar sin ver lo escrito
            "diag": {
                "line_len": len(kw.line),
                "pending": kw.pending is not None,
                "k_print": kw.k_print,
                "k_enter": kw.k_enter,
                "k_esc": kw.k_esc,
                "k_other": kw.k_other,
                "k_win32": 0,   # no existe fuera de ConPTY; el esquema se conserva
            },
            "user_cmd": uc and uc[1],
            "user_cmd_ts": uc and uc[0],
            "last": last_ack,
        })

    def type_line(txt):
        """Teclea una línea: el texto, una pausa, y el Enter APARTE (ver
        ENTER_GAP_S). R5 intacta: solo se AÑADE, ni un borrado."""
        os.write(master, txt.encode())
        time.sleep(ENTER_GAP_S)
        os.write(master, b"\r")

    def do_handoff(rid, text):
        """La red del /clear, en su hilo: `/export <copia>` → verificar que
        la copia existe con contenido y el REPL se calló → re-verificar que
        el usuario no tecleó → /clear. Sin copia NO hay /clear
        (ERR_RELAY_EXPORT): antes perder la limpieza que la conversación."""
        nonlocal inject_at, last_ack
        path = handoff_path()
        try:
            type_line(f"/export {path}")
        except OSError:
            last_ack = ack_row(rid, text, False, "ERR_RELAY_WRITE")
            hand["on"] = False
            return
        inject_at = now_ms()
        t0 = time.monotonic()
        while True:
            time.sleep(0.25)
            if child.poll() is not None:
                last_ack = ack_row(rid, text, False, "ERR_RELAY_GONE")
                hand["on"] = False
                return
            try:
                there = os.path.getsize(path) > 0
            except OSError:
                there = False
            if there and now_ms() - last_out >= EXPORT_SETTLE_MS:
                ok = True
                break
            if (time.monotonic() - t0) * 1000 >= EXPORT_WAIT_MS:
                ok = there
                break
        if not ok:
            last_ack = ack_row(rid, text, False, "ERR_RELAY_EXPORT")
            hand["on"] = False
            return
        # R4 otra vez: si tecleó durante la espera, el /clear pierde (la
        # copia queda hecha y el acuse lo dice — sus manos ganan siempre).
        kw.resolve(now_ms(), last_out)
        if kw.has_text():
            last_ack = ack_row(rid, text, False, "ERR_RELAY_TYPED", path)
            hand["on"] = False
            return
        try:
            type_line(text)
        except OSError:
            last_ack = ack_row(rid, text, False, "ERR_RELAY_WRITE", path)
            hand["on"] = False
            return
        inject_at = now_ms()
        last_ack = ack_row(rid, text, True, "", path)
        hand["on"] = False

    def attend(raw):
        """R4: se re-verifica TODO en el instante de escribir. Que el panel
        haya terminado su countdown no es un permiso. R5: solo se AÑADE.
        Devuelve None si el /clear con red quedó corriendo en su hilo (el
        acuse lo publica el hilo al terminar)."""
        nonlocal inject_at
        try:
            v = json.loads(raw)
        except ValueError:
            return {"ok": False, "err": "ERR_RELAY_BADCMD"}
        rid = v.get("id") or ""
        text = (v.get("text") or "").strip()
        # la red solo acompaña a /clear: /compact no destruye nada
        export = bool(v.get("export")) and text == "/clear"
        if not allowed(text):
            return ack_row(rid, text, False, "ERR_RELAY_BADCMD")
        w = why_not()
        if w and is_model_cmd(text):
            # /model: no se rechaza, se deja PENDIENTE y el bucle de la PTY
            # lo reintenta en cada vuelta hasta MODEL_WAIT_S (esperar aquí
            # dentro congelaría la pantalla: este attend corre en el bucle)
            pend["cmd"] = (rid, text, time.time() + MODEL_WAIT_S)
            return None
        if w:
            return ack_row(rid, text, False, w)
        if export:
            hand["on"] = True
            threading.Thread(target=do_handoff, args=(rid, text),
                             daemon=True).start()
            return None
        try:
            type_line(text)
        except OSError:
            return ack_row(rid, text, False, "ERR_RELAY_WRITE")
        inject_at = now_ms()
        return ack_row(rid, text, True, "")

    code = 1
    try:
        while True:
            try:
                r, _, _ = select.select([0, master], [], [], TICK_S)
            except (OSError, ValueError):
                break
            if 0 in r:
                try:
                    data = os.read(0, 8192)
                except OSError:
                    data = b""
                if not data:
                    break
                if kw.feed(data, now_ms()):
                    last_in = now_ms()
                try:
                    os.write(master, data)
                except OSError:
                    break
            if master in r:
                try:
                    data = os.read(master, 8192)
                except OSError:
                    data = b""     # EIO = el hijo cerró su lado
                if not data:
                    if child.poll() is not None:
                        break
                else:
                    last_out = now_ms()
                    os.write(1, tm.feed(data))
            # un /model pendiente (guardián): se teclea en cuanto haya calma
            if pend["cmd"]:
                prid, ptext, pfin = pend["cmd"]
                pw = why_not()
                if not pw:
                    pend["cmd"] = None
                    try:
                        type_line(ptext)
                        inject_at = now_ms()
                        last_ack = ack_row(prid, ptext, True, "")
                    except OSError:
                        last_ack = ack_row(prid, ptext, False, "ERR_RELAY_WRITE")
                    last_state = 0.0
                elif time.time() > pfin:
                    pend["cmd"] = None
                    last_ack = ack_row(prid, ptext, False, pw)
                    last_state = 0.0
            # órdenes del panel (o de `inject`): el buzón se borra al leer
            if os.path.exists(cmd_path):
                try:
                    with open(cmd_path, encoding="utf-8") as f:
                        raw = f.read()
                    os.unlink(cmd_path)
                except OSError:
                    raw = ""
                if raw.strip():
                    # None = la secuencia /export+/clear corre en su hilo y
                    # publicará el acuse ella misma
                    res = attend(raw)
                    if res is not None:
                        last_ack = res
                    last_state = 0.0
            if time.monotonic() - last_state >= STATE_EVERY_S:
                last_state = time.monotonic()
                write_atomic(state_path, snapshot())
            if child.poll() is not None and master not in r:
                break
        code = child.wait() if child.poll() is not None else 0
    finally:
        termios.tcsetattr(0, termios.TCSADRAIN, saved)
        # el título deja de anunciar un relevo que ya no existe
        try:
            os.write(1, f"\x1b]0;{base_name(cwd)}\x07".encode())
        except OSError:
            pass
        for p in (state_path, cmd_path):
            try:
                os.unlink(p)
            except OSError:
                pass
    sys.exit(code if isinstance(code, int) else 1)


# ---------- relevo del CHAT: proxy de stream-json (paso 2 de la etapa 4) ----------
# El chat de la extensión de VS Code NO es una terminal: lanza `claude` con
# --input-format stream-json y le habla por stdin/stdout con UNA LÍNEA JSON
# por mensaje. Aquí el relevo no envuelve una PTY, envuelve ese tubo — se pone
# en medio con `claudeCode.claudeProcessWrapper`, el enganche OFICIAL de la
# extensión ("Executable path used to launch the Claude process").
#
# Por qué este modo es MÁS seguro que el de terminal, no menos:
#  - R1 (no teclear encima del usuario) se cumple por CONSTRUCCIÓN: no hay
#    buffer compartido ni teclas a medias. Cada mensaje es una línea atómica;
#    si el usuario envía el suyo un instante después, son dos mensajes
#    separados y mezclarlos es imposible. Su borrador en el cuadro ni se toca.
#  - R2 ("Claude está generando") deja de INFERIRSE del silencio: el propio
#    protocolo lo dice — un `user` entra, un `result` sale. Certeza.
#  - Con --replay-user-messages (lo que usa la extensión) el /compact
#    inyectado APARECE en el chat: auditable a simple vista.
#
# Riesgo residual, dicho sin adornos: no vemos lo que el usuario está
# escribiendo en el cuadro. Si inyectamos mientras redacta, su mensaje llega
# DESPUÉS y se evalúa con el contexto ya compactado. No se corrompe nada y es
# justo para lo que sirve la función, pero no es invisible: por eso el
# countdown y el candado de calma siguen existiendo aquí.


def find_claude(args):
    """La ruta del claude real. Se aceptan las DOS convenciones posibles del
    wrapper (que la extensión pase el binario como primer argumento, o que no
    lo pase y el envoltorio deba encontrarlo), porque cuál usa no está
    documentado y no se puede adivinar sin romperle el chat a alguien."""
    rest = list(args)
    if rest and os.path.isfile(rest[0]) and os.access(rest[0], os.X_OK):
        return rest[0], rest[1:]
    env = os.environ.get("MICHI_CLAUDE_BIN")
    if env and os.path.isfile(env):
        return env, rest
    # el binario que trae la propia extensión (lo normal en Remote-SSH)
    import glob
    pats = sorted(glob.glob(os.path.expanduser(
        "~/.vscode-server/extensions/anthropic.claude-code-*/resources/native-binary/claude")))
    if pats:
        return pats[-1], rest
    for d in (os.environ.get("PATH") or "").split(os.pathsep):
        c = os.path.join(d, "claude")
        if os.path.isfile(c) and os.access(c, os.X_OK):
            return c, rest
    return None, rest


def run_wrap(args):
    claude, rest = find_claude(args)
    if not claude:
        print("michi: no encontré el binario de Claude Code", file=sys.stderr)
        sys.exit(127)
    # Ya dentro de un relevo, o sin el protocolo esperado: paso directo. El
    # wrapper NUNCA puede dejar sin Claude Code (misma regla que el shim).
    if os.environ.get("MICHI_RELEVO") or "--input-format" not in rest:
        os.environ["MICHI_RELEVO"] = "0"
        try:
            os.execv(claude, [claude] + rest)
        except OSError as e:
            print(f"michi: no pude lanzar claude: {e}", file=sys.stderr)
            sys.exit(127)

    pid = os.getpid()
    cwd = os.getcwd()
    env = dict(os.environ, MICHI_RELEVO=str(pid))
    child = subprocess.Popen(
        [claude] + rest, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=None, env=env, cwd=cwd, bufsize=0,
    )

    started = now_epoch()
    d = state_dir()
    state_path = os.path.join(d, f"{pid}.json")
    cmd_path = os.path.join(d, f"{pid}.cmd")

    st = {
        "last_in": now_ms(),    # último mensaje del usuario hacia Claude
        "last_out": now_ms(),   # última salida de Claude
        "busy": False,          # turno en curso (CERTEZA, no inferencia)
        "inject_at": 0,
        "sid": "",              # session_id del log: casado EXACTO con el panel
        "user_cmd": None,
        "ack": None,
        "alive": True,
        "hand": False,          # secuencia /export+/clear en curso
        "banner": False,        # el aviso de arranque ya se pintó en el chat
    }
    lock = threading.Lock()      # escribir al hijo
    olock = threading.Lock()     # escribir al chat
    prune_handoffs()

    def why_not():
        now = now_ms()
        if not st["alive"] or child.poll() is not None:
            return "ERR_RELAY_GONE"
        if st["hand"]:
            return "ERR_RELAY_BUSY"
        if st["busy"]:
            return "ERR_RELAY_BUSY"
        if now - st["last_out"] < QUIET_MS:
            return "ERR_RELAY_BUSY"
        if now - st["last_in"] < CALM_MS:
            return "ERR_RELAY_NOISY"
        if st["inject_at"] and now - st["inject_at"] < COOLDOWN_MS:
            return "ERR_RELAY_COOLDOWN"
        return ""

    def snapshot():
        now = now_ms()
        why = why_not()
        uc = st["user_cmd"]
        return json.dumps({
            "v": STATE_V, "pid": pid, "started": started, "cwd": cwd,
            "ts": now_epoch(), "alive": child.poll() is None,
            # en el chat no hay borrador visible: `typed` es SIEMPRE false y no
            # se finge otra cosa. R1 se cumple por construcción (líneas atómicas)
            "typed": False,
            "idle_in": (now - st["last_in"]) // 1000,
            "idle_out": (now - st["last_out"]) // 1000,
            "ready": why == "", "why": why,
            "mode": "chat",          # el panel lo enseña distinto de "terminal"
            "sid": st["sid"],        # casado EXACTO por sesión, no por carpeta
            "diag": {"line_len": 0, "pending": st["busy"], "k_print": 0,
                     "k_enter": 0, "k_esc": 0, "k_other": 0, "k_win32": 0},
            "user_cmd": uc and uc[1], "user_cmd_ts": uc and uc[0],
            "last": st["ack"],
        })

    def emit_banner():
        """El banner del chat: el gemelo del de la terminal. Va SOLO al chat
        (el hijo no lo recibe, el JSONL no lo registra), UNA vez por
        conversación — se re-arma cuando cambia el session_id. NO se emite
        pegado al init: en el arranque la interfaz aún no pinta y la línea
        se pierde (medido 2026-08-10); se emite delante de la PRIMERA
        actividad de usuario, cuando el chat seguro está mirando."""
        # sin init visto aún no hay sesión donde pintarlo: se deja pasar SIN
        # gastar el turno — la siguiente actividad lo reintenta
        if st["banner"] or not st["sid"]:
            return
        st["banner"] = True
        try:
            with olock:
                sys.stdout.buffer.write(
                    replay_line(banner_text(pid), st["sid"]).encode())
                sys.stdout.buffer.flush()
        except (OSError, ValueError):
            pass

    def send_user(text, echo=True):
        """Una línea `user` al hijo Y su eco al chat. El eco es obligatorio:
        --replay-user-messages replica los mensajes normales pero NO los
        comandos — la CLI los intercepta antes (medido 2026-08-08). Sin él,
        la conversación se compactaría/borraría sin que nada en pantalla
        dijera quién lo pidió — justo el "Michi actuando a tus espaldas" que
        este proyecto no permite. Se emite la MISMA forma que la CLI usa al
        replicar un mensaje de usuario, así que la extensión ya sabe
        pintarla. Va solo al chat: el JSONL lo escribe la CLI y no se toca,
        así que esto no falsea el registro. `echo=False` para un mensaje
        NORMAL (el reenvío del 5c): ese sí lo replica la CLI y con el eco
        salía dos veces en el chat (medido 2026-08-17)."""
        emit_banner()
        with lock:
            child.stdin.write(user_line(text).encode())
            child.stdin.flush()
        st["inject_at"] = now_ms()
        st["busy"] = True
        if not echo:
            return
        try:
            with olock:
                sys.stdout.buffer.write(
                    replay_line(text, st["sid"]).encode())
                sys.stdout.buffer.flush()
        except (OSError, ValueError):
            pass

    def wrap_handoff(rid, text):
        """La red del /clear en el chat. El chat NO tiene /export (medido en
        vivo 2026-08-13: "/export isn't available in this environment" — un
        año de ERR_RELAY_EXPORT si dependiéramos de él), así que la copia la
        hace el RELEVO: el init regaló el session_id exacto y el JSONL de la
        sesión es el registro FIEL — se copia ese archivo (tmp+rename, el
        estilo de la casa) y la verificación sigue siendo un hecho del
        disco: la copia existe con contenido o no hay /clear. La ruta
        origen la resuelve session_jsonl() por nombre; la de destino la
        genera el relevo. Nada nuevo viaja por el canal."""
        src = session_jsonl(st["sid"])
        if not src:
            st["ack"] = ack_row(rid, text, False, "ERR_RELAY_EXPORT")
            st["hand"] = False
            return
        path = handoff_path("jsonl")   # honesto: el contenido ES jsonl
        try:
            with open(src, "rb") as f:
                data = f.read()
            if not data:
                raise OSError("jsonl vacío")
            tmp = path + ".tmp"        # .tmp sobre el nombre ENTERO
            with open(tmp, "wb") as f:
                f.write(data)
            os.replace(tmp, path)
        except OSError:
            st["ack"] = ack_row(rid, text, False, "ERR_RELAY_EXPORT")
            st["hand"] = False
            return
        # la verificación de siempre: el archivo en su sitio, con contenido
        try:
            ok = os.path.getsize(path) > 0
        except OSError:
            ok = False
        if not ok:
            st["ack"] = ack_row(rid, text, False, "ERR_RELAY_EXPORT")
            st["hand"] = False
            return
        # R4: ¿entró un turno del usuario mientras copiábamos? Sus manos
        # ganan — el /clear pierde, la copia queda (y el acuse lo dice).
        if child.poll() is not None:
            st["ack"] = ack_row(rid, text, False, "ERR_RELAY_GONE", path)
            st["hand"] = False
            return
        if st["busy"]:
            st["ack"] = ack_row(rid, text, False, "ERR_RELAY_BUSY", path)
            st["hand"] = False
            return
        try:
            send_user(text)
        except (OSError, ValueError):
            st["ack"] = ack_row(rid, text, False, "ERR_RELAY_WRITE", path)
            st["hand"] = False
            return
        st["ack"] = ack_row(rid, text, True, "", path)
        st["hand"] = False

    def attend(raw):
        """Devuelve el acuse, o None si el /clear con red quedó corriendo en
        su hilo (el acuse lo publica el hilo al terminar)."""
        try:
            v = json.loads(raw)
        except ValueError:
            return {"ok": False, "err": "ERR_RELAY_BADCMD"}
        rid = v.get("id") or ""
        text = (v.get("text") or "").strip()
        export = bool(v.get("export")) and text == "/clear"
        if not allowed(text):
            return ack_row(rid, text, False, "ERR_RELAY_BADCMD")
        w = why_not()
        if w and is_model_cmd(text):
            # /model: esperar a que el relevo quede libre (ver MODEL_WAIT_S)
            fin = time.time() + MODEL_WAIT_S
            while w and time.time() < fin:
                time.sleep(MODEL_TICK_S)
                w = why_not()
        if w:
            return ack_row(rid, text, False, w)
        if export:
            st["hand"] = True
            threading.Thread(target=wrap_handoff, args=(rid, text),
                             daemon=True).start()
            return None
        # REENVÍO (ruteo 5c): tras un /model, el guardián puede pedir que se
        # reenvíe el prompt frenado (`then`). SOLO detrás de un /model y SOLO
        # aquí (chat: mensaje JSON atómico, el multilínea viaja entero). El
        # texto NO se persiste: ni acuse, ni estado, ni log — vive en esta
        # variable y se va con el hilo.
        then = v.get("then") if is_model_cmd(text) else None
        if isinstance(then, str) and then.strip():
            try:
                send_user(text)
            except (OSError, ValueError):
                return ack_row(rid, text, False, "ERR_RELAY_WRITE")

            def resend():
                # el /model es un comando local: su result llega en <1 s. Se
                # espera a que el turno cierre y se reenvía el prompt.
                fin = time.time() + MODEL_WAIT_S
                while st["busy"] and time.time() < fin:
                    time.sleep(MODEL_TICK_S)
                ok = False
                if not st["busy"]:
                    try:
                        send_user(then, echo=False)
                        ok = True
                    except (OSError, ValueError):
                        ok = False
                a = ack_row(rid, text, True, "")
                a["resent"] = ok
                st["ack"] = a
            threading.Thread(target=resend, daemon=True).start()
            return None
        try:
            send_user(text)
        except (OSError, ValueError):
            return ack_row(rid, text, False, "ERR_RELAY_WRITE")
        return ack_row(rid, text, True, "")

    def pump_in():
        """Del chat hacia Claude. Cada línea se reenvía INTACTA."""
        try:
            for line in iter(sys.stdin.buffer.readline, b""):
                try:
                    v = json.loads(line)
                    if v.get("type") == "user":
                        emit_banner()   # delante del primer mensaje: la
                                        # pestaña toma el nombre del banner
                        st["last_in"] = now_ms()
                        st["busy"] = True
                        # ¿el usuario aplicó él mismo uno de los dos? cuenta
                        # para el desbloqueo igual que en la terminal
                        c = v.get("message", {}).get("content")
                        txt = ""
                        if isinstance(c, str):
                            txt = c
                        elif isinstance(c, list):
                            txt = " ".join(b.get("text", "") for b in c
                                           if isinstance(b, dict))
                        txt = txt.strip()
                        for a in ALLOWED:
                            if txt == a or txt.startswith(a + " "):
                                st["user_cmd"] = (now_epoch(), a)
                except ValueError:
                    pass            # no es JSON: se reenvía igual, sin tocar
                with lock:
                    child.stdin.write(line)
                    child.stdin.flush()
        except (OSError, ValueError):
            pass
        finally:
            st["alive"] = False
            try:
                child.stdin.close()
            except OSError:
                pass

    def pump_out():
        """De Claude hacia el chat. Igual de intacta."""
        try:
            for line in iter(child.stdout.readline, b""):
                st["last_out"] = now_ms()
                try:
                    v = json.loads(line)
                    t = v.get("type")
                    if t == "result":
                        st["busy"] = False       # fin de turno: CERTEZA
                    if t == "system" and v.get("session_id"):
                        # sesión NUEVA en el mismo proceso (conversación
                        # nueva o /clear): el banner se re-arma para que
                        # cada conversación estrene el suyo
                        if st["sid"] and v["session_id"] != st["sid"]:
                            st["banner"] = False
                        st["sid"] = v["session_id"]
                except ValueError:
                    pass
                with olock:
                    sys.stdout.buffer.write(line)
                    sys.stdout.buffer.flush()
        except (OSError, ValueError):
            pass
        finally:
            st["alive"] = False

    for fn in (pump_in, pump_out):
        threading.Thread(target=fn, daemon=True).start()

    try:
        while child.poll() is None:
            if os.path.exists(cmd_path):
                try:
                    with open(cmd_path, encoding="utf-8") as f:
                        raw = f.read()
                    os.unlink(cmd_path)
                except OSError:
                    raw = ""
                if raw.strip():
                    res = attend(raw)
                    if res is not None:
                        st["ack"] = res
            write_atomic(state_path, snapshot())
            time.sleep(STATE_EVERY_S)
    finally:
        for p in (state_path, cmd_path):
            try:
                os.unlink(p)
            except OSError:
                pass
    sys.exit(child.returncode or 0)


# ---------- subcomandos (validar sin panel, igual que en Windows) ----------

def live_sessions():
    out = []
    now = now_epoch()
    d = state_dir()
    try:
        names = os.listdir(d)
    except OSError:
        return out
    for n in sorted(names):
        if not n.endswith(".json"):
            continue
        try:
            with open(os.path.join(d, n), encoding="utf-8") as f:
                v = json.load(f)
        except (OSError, ValueError):
            continue
        if now - (v.get("ts") or 0) <= FRESH_S and v.get("alive"):
            out.append(v)
    return out


def cmd_status(args):
    lst = live_sessions()
    if not lst:
        print("Ninguna sesión con relevo. Abre una con:  michi-relevo.py claude")
        return
    if "--debug" in args:
        for v in lst:
            print(json.dumps(v, indent=2, ensure_ascii=False))
        return
    print(f"{'sesión':8} {'listo':6} {'texto':6} {'quieta':7} {'motivo':18} carpeta")
    for v in lst:
        why = (v.get("why") or "").replace("ERR_RELAY_", "")
        print(f"{v.get('pid'):<8} {'sí' if v.get('ready') else 'no':6} "
              f"{'sí' if v.get('typed') else 'no':6} "
              f"{str(v.get('idle_in')) + 's':7} {why:18} {v.get('cwd')}")


def cmd_inject(args):
    # `--export` (solo con /clear): la red de seguridad, igual que la pide
    # el panel. Sirve para validar la secuencia sin la app en medio.
    export = "--export" in args
    args = [a for a in args if a != "--export"]
    if len(args) == 2:
        pid, text = args
    elif len(args) == 1:
        lst = live_sessions()
        if not lst:
            print("michi: no hay ninguna sesión con relevo abierta",
                  file=sys.stderr)
            sys.exit(1)
        pid, text = str(lst[0]["pid"]), args[0]
    else:
        print("uso: michi-relevo.py inject [sesión] /compact | /clear [--export]",
              file=sys.stderr)
        sys.exit(2)
    if text not in ALLOWED:
        print(f"michi: solo puedo aplicar {' o '.join(ALLOWED)}",
              file=sys.stderr)
        sys.exit(2)
    d = state_dir()
    rid = f"cli-{int(time.time() * 1000)}"
    write_atomic(os.path.join(d, f"{pid}.cmd"),
                 json.dumps({"id": rid, "op": "inject", "text": text,
                             "export": export and text == "/clear"}))
    state = os.path.join(d, f"{pid}.json")
    # con red, la secuencia espera a la copia: hasta ~15 s más de margen
    for _ in range(120 if export else 40):
        time.sleep(0.2)
        try:
            with open(state, encoding="utf-8") as f:
                v = json.load(f)
        except (OSError, ValueError):
            continue
        last = v.get("last") or {}
        if last.get("id") == rid:
            copia = f" (copia: {last['export']})" if last.get("export") else ""
            if last.get("ok"):
                print(f"aplicado: {text}{copia}")
            else:
                print(f"no se aplicó: {last.get('err') or '?'}{copia}")
            return
    print("sin respuesta del relevo (¿sigue abierto?)")


def main():
    args = sys.argv[1:]
    if args and args[0] == "claude":
        run_relevo(args[1:])
    elif args and args[0] == "wrap":
        run_wrap(args[1:])
    elif args and args[0] == "status":
        cmd_status(args[1:])
    elif args and args[0] == "inject":
        cmd_inject(args[1:])
    else:
        print(__doc__.strip().split("\n\n")[2])


if __name__ == "__main__":
    main()
