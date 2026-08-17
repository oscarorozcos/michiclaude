#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
MichiClaude — Hook B del ruteo inteligente (Linux / WSL / servidores SSH).
Diseño: docs/ruteo-inteligente.md §5 (pieza 2) y §11 (etapa 2). La pareja
Windows es router-hook.ps1 — LOS DOS EN SINCRONÍA, como el exportador.

Se engancha a PreToolUse sobre la herramienta de subagentes (Task|Agent)
y decide el modelo con el que nace CADA subagente, leyendo la nota que
MichiClaude deja en ~/.michiclaude/router_state.json:

  - exploración/búsqueda  -> haiku   (siempre: el error de subir es barato)
  - análisis profundo     -> sonnet SOLO si la cuota aprieta (>=70%)
  - lo demás (implementación) -> sonnet

Reglas duras (del diseño, no opcionales):
  - El hook NO piensa: lee un JSON pre-computado y actúa en microsegundos.
  - Estado ausente o >10 min de viejo = exit 0 sin tocar nada (fail-quiet):
    si MichiClaude no está corriendo, es como si el hook no existiera.
  - Se devuelve el objeto de input COMPLETO (updatedInput añade `model`).
  - `model` explícito en el input = se respeta (el padre ya decidió).
  - Cada decisión queda en ~/.michiclaude/ruteo_log.jsonl (JSON plano,
    nunca SQLite) — la etapa 3 la cruza con los transcripts reales.
"""

import json
import os
import sys
import time

STALE_S = 600          # estado con más de 10 min = viejo (regla del diseño)
PRESSURE = 70          # % de cuota desde el que el análisis baja a sonnet
MODEL_LIGHT = "haiku"
MODEL_MID = "sonnet"

# Clases por NOMBRE del tipo de subagente (subcadenas, en minúsculas).
# Señales estructurales, no keywords del prompt: funcionan igual en
# cualquier idioma porque el nombre del agente lo pone el harness.
LIGHT_WORDS = ("explore", "search", "scout", "locate", "lookup", "grep")
THINK_WORDS = ("plan", "review", "audit", "analy", "research", "judge",
               "verify", "architect", "security")

MICHI = os.path.join(os.path.expanduser("~"), ".michiclaude")
STATE = os.path.join(MICHI, "router_state.json")
LOG = os.path.join(MICHI, "ruteo_log.jsonl")
LOG_MAX = 512 * 1024   # al pasar de 512 KB rota a .1 (una sola generación)


def apunta(fila):
    """Deja la decisión en el cuaderno. Si no se puede, ni modo: jamás
    un fallo del rastro tumba el hook."""
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
    """La nota del refri, solo si es de hace <10 min. Cualquier problema
    (no existe, ilegible, vieja) = None y el hook no actúa."""
    try:
        with open(STATE, "r", encoding="utf-8") as fh:
            st = json.load(fh)
        if not isinstance(st, dict):
            return None
        ts = st.get("ts")
        if not isinstance(ts, (int, float)) or time.time() - ts > STALE_S:
            return None
        return st
    except Exception:
        return None


def modelo_padre(transcript):
    """El modelo con el que va la sesión MADRE, leído de la cola del
    transcript (0.4 ms medidos). Es lo que el subagente habría heredado:
    con él la medición (etapa 3) calcula el ahorro REAL sin adivinar. Si
    no se puede saber, "" — nunca se inventa."""
    try:
        if not transcript or not os.path.isfile(transcript):
            return ""
        size = os.path.getsize(transcript)
        with open(transcript, "rb") as fh:
            fh.seek(max(0, size - 65536))
            tail = fh.read().decode("utf-8", "replace")
        import re
        ms = re.findall(r'"model"\s*:\s*"(claude-[^"]+)"', tail)
        return ms[-1] if ms else ""
    except Exception:
        return ""


def clase(tipo):
    t = (tipo or "").lower()
    if any(w in t for w in LIGHT_WORDS):
        return "light"
    if any(w in t for w in THINK_WORDS):
        return "think"
    return "work"


def presion(st):
    """¿Aprieta la cuota? Con el peor de los dos números que haya; sin
    ninguno, no hay presión (no inventar cifras — invariante #8)."""
    vals = [v for v in (st.get("week_pct"), st.get("session_pct"))
            if isinstance(v, (int, float))]
    return bool(vals) and max(vals) >= PRESSURE


def main():
    try:
        evento = json.loads(sys.stdin.read())
    except Exception:
        return
    entrada = evento.get("tool_input")
    if not isinstance(entrada, dict):
        return

    st = estado_fresco()
    if st is None:
        return  # MichiClaude no está: como si el hook no existiera

    base = {
        "ts": int(time.time()),
        "type": entrada.get("subagent_type") or "",
        "before": entrada.get("model"),
        "sid": evento.get("session_id") or "",
        "cwd": evento.get("cwd") or "",
        "parent": modelo_padre(evento.get("transcript_path")),
    }

    # El padre ya eligió modelo: se respeta y se anota.
    if entrada.get("model"):
        apunta(dict(base, ev="skip", why="explicit"))
        return
    # Escotilla manual: un prompt que empiece por «~» no se toca.
    if str(entrada.get("prompt") or "").startswith("~"):
        apunta(dict(base, ev="skip", why="bypass"))
        return

    c = clase(base["type"])
    if c == "light":
        modelo, why = MODEL_LIGHT, "light"
    elif c == "think":
        if presion(st):
            modelo, why = MODEL_MID, "think-pressure"
        else:
            # cuota holgada: el análisis se queda con el modelo del padre
            apunta(dict(base, ev="skip", why="think-comfort"))
            return
    else:
        modelo, why = MODEL_MID, "work"

    nuevo = dict(entrada)
    nuevo["model"] = modelo
    apunta(dict(base, ev="route", why=why, after=modelo))
    sys.stdout.write(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": nuevo,
        }
    }, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        pass  # fallo silencioso: jamás estorbar el trabajo
    sys.exit(0)
