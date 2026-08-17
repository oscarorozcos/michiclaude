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

MICHI = os.path.join(os.path.expanduser("~"), ".michiclaude")
STATE = os.path.join(MICHI, "router_state.json")
LOG = os.path.join(MICHI, "ruteo_log.jsonl")
LAST = os.path.join(MICHI, "guard_last.json")
LOG_MAX = 512 * 1024

# rutas de archivo: algo/con/barras.ext o C:\...\x.ext (mín. 1 barra + ext)
RE_PATH = re.compile(r'(?:[A-Za-z]:\\|\.{0,2}/|~/)?(?:[\w.\-]+[\\/]){1,}[\w.\-]+\.[A-Za-z0-9]{1,6}\b')
RE_FENCE = re.compile(r'```')
RE_ERR = re.compile(r'(Traceback \(most recent|\bat [\w$.<>]+ \([^)]+:\d+:\d+\)|panicked at|error\[E\d+\]|Exception in thread)')


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


def senales(prompt):
    """Señales estructurales, no de vocabulario. Devuelve la lista y un
    peso: fence de código = 2 (fuerte); ≥2 rutas = 1; traza de error = 1;
    imperativo largo (≥60 palabras y no termina en «?») = 1."""
    sig = []
    peso = 0
    if len(RE_FENCE.findall(prompt)) >= 2:
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
            try:
                with open(LAST, "r", encoding="utf-8") as fh:
                    last = json.load(fh)
                insiste = (last.get("h") == h and last.get("tier") == tr
                           and time.time() - float(last.get("ts", 0)) < INSIST_S)
            except Exception:
                pass
            if insiste:
                apunta(dict(base, ev="insist", model=tr, sig=sig))
            else:
                try:
                    os.makedirs(MICHI, exist_ok=True)
                    with open(LAST, "w", encoding="utf-8") as fh:
                        json.dump({"h": h, "tier": tr, "ts": time.time()}, fh)
                except Exception:
                    pass
                apunta(dict(base, ev="block", model=tr, sig=sig))
                # el texto lo lee el USUARIO en su terminal, no Claude: va
                # bilingüe corto (el hook no tiene el diccionario del panel)
                razon = ("MichiClaude: this looks complex and you're on %s. "
                         "Run /model opus and resend, or prefix ~ to send as is. "
                         "/ Esto se ve complejo y vas en %s: /model opus y reenvia, "
                         "o antepon ~ para mandarlo tal cual." % (tr, tr))
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
