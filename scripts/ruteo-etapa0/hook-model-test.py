#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
ETAPA 0 DEL RUTEO INTELIGENTE — hook de juguete (Linux / WSL / macOS).
Ver scripts/ruteo-etapa0/README.md y docs/ruteo-inteligente.md §11.

Qué hace: se engancha a PreToolUse sobre la herramienta de subagentes
(Task/Agent) y REESCRIBE su input para imponer el modelo `haiku`.
Solo actúa si el input trae la marca RUTEO-TEST, así que jamás estorba
trabajo real. Todo lo que ve y lo que responde queda en el log.

Reglas que respeta (docs/ruteo-inteligente.md §10.1 y §5 "Principios"):
- Devuelve el objeto de input COMPLETO, no el campo suelto.
- Fallo silencioso: ante cualquier error sale con 0 y sin salida, para
  que Claude Code siga como si el hook no existiera.
"""

import json
import os
import sys
from datetime import datetime

MARCA = "RUTEO-TEST"          # sin ella, el hook no toca nada
MARCA_ALLOW = "RUTEO-TEST-ALLOW"  # variante B: añade permissionDecision
MODELO = "haiku"

LOG = os.path.join(os.path.expanduser("~"), ".michiclaude", "ruteo-etapa0.log")


def apunta(titulo, texto):
    """Deja rastro. Si no se puede escribir, ni modo: el hook no falla."""
    try:
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        with open(LOG, "a", encoding="utf-8") as fh:
            fh.write("[%s] %s: %s\n" % (
                datetime.now().strftime("%Y-%m-%d %H:%M:%S"), titulo, texto))
    except Exception:
        pass


def main():
    crudo = sys.stdin.read()
    apunta("ENTRA", crudo[:4000])

    try:
        evento = json.loads(crudo)
    except Exception as e:
        apunta("SALE", "no es JSON (%s) — no hago nada" % e)
        return

    entrada = evento.get("tool_input")
    if not isinstance(entrada, dict):
        apunta("SALE", "sin tool_input — no hago nada")
        return

    texto = json.dumps(entrada, ensure_ascii=False)
    if MARCA not in texto:
        apunta("SALE", "sin la marca %s — no hago nada" % MARCA)
        return

    # el objeto COMPLETO, con el modelo impuesto encima
    nuevo = dict(entrada)
    antes = nuevo.get("model", "(no venía)")
    nuevo["model"] = MODELO

    salida = {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": nuevo,
        }
    }
    if MARCA_ALLOW in texto:
        salida["hookSpecificOutput"]["permissionDecision"] = "allow"
        salida["hookSpecificOutput"]["permissionDecisionReason"] = \
            "ruteo etapa 0 (variante B)"

    respuesta = json.dumps(salida, ensure_ascii=False)
    apunta("MODELO", "antes=%s despues=%s" % (antes, MODELO))
    apunta("SALE", respuesta[:4000])
    sys.stdout.write(respuesta)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # fallo silencioso, pase lo que pase
        apunta("ERROR", repr(e))
    sys.exit(0)
