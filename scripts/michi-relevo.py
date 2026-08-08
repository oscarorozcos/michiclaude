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
  michi-relevo.py claude [args...]   abre Claude Code con relevo
  michi-relevo.py status [--debug]   sesiones con relevo vivas
  michi-relevo.py inject [pid] /compact

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
import time
import tty

STATE_V = 1
ALLOWED = ("/compact", "/clear")
CALM_MS = 8_000       # R3: calma de teclado
QUIET_MS = 2_000      # R2: silencio de la PTY ("Claude generando" se INFIERE)
COOLDOWN_MS = 15_000  # tras inyectar
SUBMIT_WAIT_MS = 3_000  # un Enter no limpia hasta ver si Claude REACCIONA
TICK_S = 0.25
STATE_EVERY_S = 0.5
FRESH_S = 15          # viva = estado con menos de esto (MISMA regla que el panel)


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

    print(
        f"michi · relevo activo (sesión {pid}) — MichiClaude puede aplicar "
        "/compact y /clear en esta ventana"
    )

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
    d = state_dir()
    state_path = os.path.join(d, f"{pid}.json")
    cmd_path = os.path.join(d, f"{pid}.cmd")
    # marca de arranque, para que el título salga aunque Claude no ponga uno
    os.write(1, f"\x1b]0;michi · {base_name(cwd)}\x07".encode())

    def why_not():
        now = now_ms()
        if child.poll() is not None:
            return "ERR_RELAY_GONE"
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

    def attend(raw):
        """R4: se re-verifica TODO en el instante de escribir. Que el panel
        haya terminado su countdown no es un permiso. R5: solo se AÑADE."""
        nonlocal inject_at
        try:
            v = json.loads(raw)
        except ValueError:
            return {"ok": False, "err": "ERR_RELAY_BADCMD"}
        rid = v.get("id") or ""
        text = (v.get("text") or "").strip()
        if text not in ALLOWED:
            return {"id": rid, "ok": False, "err": "ERR_RELAY_BADCMD",
                    "text": text, "ts": now_epoch()}
        w = why_not()
        if w:
            return {"id": rid, "ok": False, "err": w,
                    "text": text, "ts": now_epoch()}
        try:
            os.write(master, (text + "\r").encode())
        except OSError:
            return {"id": rid, "ok": False, "err": "ERR_RELAY_WRITE",
                    "text": text, "ts": now_epoch()}
        inject_at = now_ms()
        return {"id": rid, "ok": True, "err": "", "text": text,
                "ts": now_epoch()}

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
            # órdenes del panel (o de `inject`): el buzón se borra al leer
            if os.path.exists(cmd_path):
                try:
                    with open(cmd_path, encoding="utf-8") as f:
                        raw = f.read()
                    os.unlink(cmd_path)
                except OSError:
                    raw = ""
                if raw.strip():
                    last_ack = attend(raw)
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
        print("uso: michi-relevo.py inject [sesión] /compact", file=sys.stderr)
        sys.exit(2)
    if text not in ALLOWED:
        print(f"michi: solo puedo aplicar {' o '.join(ALLOWED)}",
              file=sys.stderr)
        sys.exit(2)
    d = state_dir()
    rid = f"cli-{int(time.time() * 1000)}"
    write_atomic(os.path.join(d, f"{pid}.cmd"),
                 json.dumps({"id": rid, "op": "inject", "text": text}))
    state = os.path.join(d, f"{pid}.json")
    for _ in range(40):
        time.sleep(0.2)
        try:
            with open(state, encoding="utf-8") as f:
                v = json.load(f)
        except (OSError, ValueError):
            continue
        last = v.get("last") or {}
        if last.get("id") == rid:
            if last.get("ok"):
                print(f"aplicado: {text}")
            else:
                print(f"no se aplicó: {last.get('err') or '?'}")
            return
    print("sin respuesta del relevo (¿sigue abierto?)")


def main():
    args = sys.argv[1:]
    if args and args[0] == "claude":
        run_relevo(args[1:])
    elif args and args[0] == "status":
        cmd_status(args[1:])
    elif args and args[0] == "inject":
        cmd_inject(args[1:])
    else:
        print(__doc__.strip().split("\n\n")[2])


if __name__ == "__main__":
    main()
