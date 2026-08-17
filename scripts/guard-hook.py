#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
MichiClaude — Hook A, el GUARDIÁN de escalada (Linux / WSL / servidores SSH).
Diseño: docs/ruteo-inteligente.md §5 (Hook A) y §11 (etapa 5). La pareja
Windows es guard-hook.ps1 — LOS DOS EN SINCRONÍA.

Se engancha a UserPromptSubmit. Protege del ÚNICO error caro: pedir algo
complejo estando en un modelo barato (haiku/sonnet). Si el prompt trae
señales ESTRUCTURALES pesadas —bloque de código, varias rutas de archivo,
imperativo largo; nada de keywords, así vale en cualquier idioma— y la
sesión va en un modelo barato, BLOQUEA el prompt ANTES de gastar un token
y dice cómo seguir: `/model opus` y reenviar, o anteponer `~`.

Reglas duras:
  - Lee la nota ~/.michiclaude/router_state.json; ausente, >10 min de
    vieja o con `guard` apagado = exit 0 sin tocar nada (fail-quiet).
  - El modelo de la sesión sale del TRANSCRIPT (`transcript_path`, cola de
    64 KB — 0.4 ms medidos), que lo escribe Claude Code; si no hay (turno
    1) cae al `model` de settings.json; si tampoco, no bloquea.
  - Memoria de insistencia: si el MISMO prompt vuelve en <10 min con el
    mismo modelo barato, PASA (el usuario ya decidió). Y `~` = escotilla.
  - Bloquear NUNCA cuesta tokens: el prompt no llegó a Anthropic.
  - Contexto inyectado (`ctx` en la nota, apagado por defecto): 2 líneas
    gruesas de estado para que el propio Claude sugiera el tier. Es lo
    ÚNICO del sistema que gasta (~60 tok/turno) y se AUTO-REPORTA.
  - Al log SOLO señales y conteos — JAMÁS el texto del prompt (privacidad).
"""

import hashlib
import json
import os
import re
import sys
import time

STALE_S = 600
INSIST_S = 600
CHEAP = ("haiku", "sonnet")           # tiers desde los que se escala
TAIL = 65536
# La ESCALERA de familias, de barata a cara. Es la ÚNICA lista de modelos
# del hook: se reconoce por nombre (subcadena), un modelo desconocido no
# es de ninguna familia y el guardián no actúa. Añadir una familia nueva =
# una palabra aquí (y en el .ps1). Nunca se escala solo a la última.
LADDER = ("haiku", "sonnet", "opus", "fable")
RELAY_DIR = os.path.join(os.path.expanduser("~"), ".michiclaude", "relevo")

MICHI = os.path.join(os.path.expanduser("~"), ".michiclaude")
STATE = os.path.join(MICHI, "router_state.json")
LOG = os.path.join(MICHI, "ruteo_log.jsonl")
LAST = os.path.join(MICHI, "guard_last.json")
LOG_MAX = 512 * 1024

# rutas de archivo: algo/con/barras.ext o C:\...\x.ext (mín. 1 barra + ext)
RE_PATH = re.compile(r'(?:[A-Za-z]:\\|\.{0,2}/|~/)?(?:[\w.\-]+[\\/]){1,}[\w.\-]+\.[A-Za-z0-9]{1,6}\b')
RE_FENCE = re.compile(r'```')
RE_ERR = re.compile(r'(Traceback \(most recent|\bat [\w$.<>]+ \([^)]+:\d+:\d+\)|panicked at|error\[E\d+\]|Exception in thread)')
# código SIN fences: el chat de VS Code se come los ``` al enviar (mordió
# 2026-08-17: el prompt llegó con el código pelado y no bloqueó). Se cuenta
# por FORMA de las líneas: cabeceras de código, llaves solas, o una línea
# indentada tras otra que termina en ":" o "{".
RE_CODEHEAD = re.compile(r'^\s*(def |fn |function |class |import |from \S+ import|const |let |var |public |private |#include|SELECT |async def |return |if \(|for \(|while \()')


def apunta(fila):
    try:
        os.makedirs(MICHI, exist_ok=True)
        try:
            if os.path.getsize(LOG) > LOG_MAX:
                os.replace(LOG, LOG + ".1")
        except OSError:
            pass
        with open(LOG, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(fila, ensure_ascii=False) + "\n")
    except Exception:
        pass


def estado_fresco():
    try:
        with open(STATE, "r", encoding="utf-8") as fh:
            st = json.load(fh)
        ts = st.get("ts") if isinstance(st, dict) else None
        if not isinstance(ts, (int, float)) or time.time() - ts > STALE_S:
            return None
        return st
    except Exception:
        return None


def modelo_sesion(transcript, cfg_dir):
    """El modelo con el que va la sesión: cola del transcript; si no hay
    aún, el `model` de settings.json; si tampoco, None (no se adivina)."""
    try:
        if transcript and os.path.isfile(transcript):
            size = os.path.getsize(transcript)
            with open(transcript, "rb") as fh:
                fh.seek(max(0, size - TAIL))
                tail = fh.read().decode("utf-8", "replace")
            ms = re.findall(r'"model"\s*:\s*"(claude-[^"]+)"', tail)
            if ms:
                return ms[-1]
    except Exception:
        pass
    try:
        with open(os.path.join(cfg_dir, "settings.json"), "r", encoding="utf-8") as fh:
            m = json.load(fh).get("model")
            if isinstance(m, str) and m:
                return m
    except Exception:
        pass
    return None


def tier(model):
    m = (model or "").lower()
    for t in ("haiku", "sonnet", "opus", "fable", "mythos"):
        if t in m:
            return t
    return None


def codigo_pelado(prompt):
    """≥2 líneas con forma de código, o una cabecera seguida de línea
    indentada. Estructural: no depende de que las fences sobrevivan."""
    lines = prompt.splitlines()
    heads = 0
    prev_open = False
    for ln in lines:
        if not ln.strip():
            continue
        if RE_CODEHEAD.match(ln) or ln.strip() in ("{", "}", "};", ");"):
            heads += 1
        elif prev_open and (ln.startswith("    ") or ln.startswith("\t")):
            heads += 1
        prev_open = ln.rstrip().endswith((":", "{", "(", "=>"))
        if heads >= 2:
            return True
    return False


def destino(tr, peso):
    """A qué peldaño subir: una señal = un peldaño; código o dos señales =
    a «opus» (o el peldaño más alto permitido si opus no está). Jamás al
    último peldaño (fable): ese lo elige el usuario."""
    try:
        i = LADDER.index(tr)
    except ValueError:
        return None
    top = len(LADDER) - 2                     # índice máximo automático
    j = min(top, i + 1) if peso < 2 else min(top, max(i + 1, LADDER.index("opus")))
    return LADDER[j] if j > i else None


def relevo_de(sid, cwd):
    """El relevo de ESTA sesión: por sid exacto; si el relevo aún no lo
    conoce, por cwd SOLO si es único. Fail-closed: en la duda, ninguno."""
    try:
        names = [n for n in os.listdir(RELAY_DIR) if n.endswith(".json")]
    except OSError:
        return None
    por_cwd = []
    for n in names:
        try:
            with open(os.path.join(RELAY_DIR, n), "r", encoding="utf-8") as fh:
                st = json.load(fh)
        except Exception:
            continue
        if not st.get("alive"):
            continue
        if sid and st.get("sid") == sid:
            return st
        if cwd and str(st.get("cwd") or "").replace("\\", "/").rstrip("/") == cwd.replace("\\", "/").rstrip("/"):
            por_cwd.append(st)
    return por_cwd[0] if len(por_cwd) == 1 else None


def escalar(sid, cwd, alias, then=None):
    """Le deja al relevo de la sesión la orden `/model <alias>` y SALE sin
    esperar el acuse: el relevo solo queda libre cuando Claude Code emite el
    `result` del bloqueo, y ese result espera a que ESTE hook termine —
    esperar aquí era un abrazo mortal (medido 2026-08-17: ERR_RELAY_BUSY
    durante toda la espera). El relevo teclea en cuanto pueda; el acuse
    queda en su estado y el panel lo lee. `then` = el prompt a REENVIAR
    tras el /model (5c): chat = mensaje JSON atómico, terminal = pegado
    entre marcas; el buzón se borra al leerlo y el texto no se anota en
    ningún sitio. Devuelve (escrito, err, reenvio_pedido)."""
    st = relevo_de(sid, cwd)
    if not st or not st.get("pid"):
        return False, "NORELAY", False
    # SOLO CHAT (medido 2026-08-17 en la TUI 2.1.233): en terminal `/model`
    # abre un diálogo («Switch model? … 1. Yes / 2. No») Y guarda el modelo
    # como DEFAULT de sesiones nuevas — justo lo que este sistema promete no
    # hacer en silencio. En terminal se queda el freno con su mensaje.
    if st.get("mode") != "chat":
        return False, "TERMINAL", False
    pid = st.get("pid")
    rid = "esc-%d" % int(time.time() * 1000)
    orden = {"id": rid, "op": "inject", "text": "/model " + alias, "export": False}
    reenvio = bool(then)   # chat: mensaje JSON atómico (multilínea entero)
    if reenvio:
        orden["then"] = then
    body = json.dumps(orden, ensure_ascii=False)
    path = os.path.join(RELAY_DIR, "%s.cmd" % pid)
    try:
        tmp = path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as fh:
            fh.write(body)
        os.replace(tmp, path)
    except OSError:
        return False, "WRITE", False
    return True, rid, reenvio


def senales(prompt):
    """Señales estructurales, no de vocabulario. Devuelve la lista y un
    peso: fence de código = 2 (fuerte); ≥2 rutas = 1; traza de error = 1;
    imperativo largo (≥60 palabras y no termina en «?») = 1."""
    sig = []
    peso = 0
    if len(RE_FENCE.findall(prompt)) >= 2 or codigo_pelado(prompt):
        sig.append("code"); peso += 2
    paths = set(RE_PATH.findall(prompt))
    if len(paths) >= 2:
        sig.append("paths"); peso += 1
    if RE_ERR.search(prompt):
        sig.append("trace"); peso += 1
    # «largo» por palabras O por caracteres: japonés/chino no separan con
    # espacios y contar solo palabras los dejaba ciegos (mordió en pruebas)
    words = len(prompt.split())
    if (words >= 60 or len(prompt) >= 300) and not prompt.rstrip().endswith(("?", "？")):
        sig.append("long"); peso += 1
    return sig, peso


def main():
    try:
        ev = json.loads(sys.stdin.read())
    except Exception:
        return
    prompt = ev.get("prompt")
    if not isinstance(prompt, str) or not prompt.strip():
        return
    st = estado_fresco()
    if st is None:
        return
    base = {"ts": int(time.time()), "sid": ev.get("session_id") or "",
            "cwd": ev.get("cwd") or "", "plen": len(prompt)}

    # los comandos de barra son de Claude Code, no prompts: ni se miran
    if prompt.lstrip().startswith("/"):
        return
    # escotilla: «~» al principio = déjame en paz este turno
    if prompt.lstrip().startswith("~"):
        apunta(dict(base, ev="bypass"))
        return

    cfg_dir = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(os.path.expanduser("~"), ".claude")
    model = modelo_sesion(ev.get("transcript_path"), cfg_dir)
    tr = tier(model)

    salida = {}
    # --- (a) el guardián ---
    if st.get("guard") and tr in CHEAP:
        sig, peso = senales(prompt)
        # con haiku basta una señal; con sonnet hace falta peso ≥2
        umbral = 1 if tr == "haiku" else 2
        if peso >= umbral:
            h = hashlib.sha1(prompt.strip().encode("utf-8")).hexdigest()[:16]
            insiste = False
            auto = False
            try:
                with open(LAST, "r", encoding="utf-8") as fh:
                    last = json.load(fh)
                insiste = (last.get("h") == h and last.get("tier") == tr
                           and time.time() - float(last.get("ts", 0)) < INSIST_S)
                auto = bool(last.get("auto"))
            except Exception:
                pass
            if insiste:
                # el mismo prompt vuelve: o insististe tú, o lo reenvió el
                # relevo por orden nuestra (5c) — se anota lo que fue
                apunta(dict(base, ev="resent" if auto else "insist", model=tr, sig=sig))
            else:
                dest = destino(tr, peso) or "opus"
                # ESCALAR SOLO (bandera `esc` en la nota): el relevo de esta
                # sesión teclea `/model <dest>`; el usuario solo reenvía —
                # o, con `rs` (5c) y relevo de chat, lo reenvía el relevo.
                # Sin relevo o sin acuse, se cae al freno de siempre.
                esc_ok, esc_err, reenvio = (False, "", False)
                if st.get("esc"):
                    esc_ok, esc_err, reenvio = escalar(
                        base["sid"], base["cwd"], dest,
                        prompt if st.get("rs") else None)
                    apunta(dict(base, ev="escalate", model=tr, to=dest,
                                ok=esc_ok, err=esc_err, resend=reenvio, sig=sig))
                else:
                    apunta(dict(base, ev="block", model=tr, sig=sig, to=dest))
                # la memoria de insistencia se escribe DESPUÉS de escalar: así
                # sabe si el próximo reenvío será del relevo (auto) o tuyo
                try:
                    os.makedirs(MICHI, exist_ok=True)
                    with open(LAST, "w", encoding="utf-8") as fh:
                        json.dump({"h": h, "tier": tr, "ts": time.time(),
                                   "auto": bool(esc_ok and reenvio)}, fh)
                except Exception:
                    pass
                # el texto lo lee el USUARIO en su terminal, no Claude: va
                # bilingüe corto (el hook no tiene el diccionario del panel)
                if esc_ok and reenvio:
                    razon = ("MichiClaude: this looked complex for %s, so I'm switching "
                             "this session to %s and resending it for you. Nothing to do. "
                             "/ Esto se veia complejo para %s: estoy subiendo la sesion a %s "
                             "y lo reenvio yo. No tienes que hacer nada." % (tr, dest, tr, dest))
                elif esc_ok:
                    razon = ("MichiClaude: this looked complex for %s, so I'm switching "
                             "this session to %s. Give it ~10 s and resend (Up + Enter). "
                             "/ Esto se veia complejo para %s: estoy subiendo la sesion a %s. "
                             "Dale ~10 s y reenvialo (flecha arriba + Enter)." % (tr, dest, tr, dest))
                else:
                    razon = ("MichiClaude: this looks complex and you're on %s. "
                             "Run /model %s and resend, or prefix ~ to send as is. "
                             "/ Esto se ve complejo y vas en %s: /model %s y reenvia, "
                             "o antepon ~ para mandarlo tal cual." % (tr, dest, tr, dest))
                salida = {"decision": "block", "reason": razon}
                sys.stdout.write(json.dumps(salida, ensure_ascii=False))
                return

    # --- (b) contexto inyectado (opcional, apagado por defecto) ---
    if st.get("ctx"):
        w, s = st.get("week_pct"), st.get("session_pct")
        partes = []
        if isinstance(w, (int, float)):
            partes.append("weekly quota ~%d%% used, resets in ~%sh" % (w, st.get("week_reset_h", "?")))
        if isinstance(s, (int, float)):
            partes.append("5h session ~%d%%" % s)
        if partes:
            ctx = ("[MichiClaude] Session model: %s. %s. If this request is trivial, "
                   "briefly suggest a cheaper /model; if it is architecture-level, "
                   "confirm the tier. Keep it to one line, only when relevant."
                   % (model or "unknown", "; ".join(partes)))
            apunta(dict(base, ev="ctx", model=tr))
            sys.stdout.write(json.dumps({"hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": ctx}}, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        pass
    sys.exit(0)
