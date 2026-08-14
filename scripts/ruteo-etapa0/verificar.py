#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ETAPA 0 — el veredicto: ¿con qué modelo CORRIÓ de verdad el subagente?
Solo LEE archivos. No cambia nada. (Linux / WSL / macOS; el gemelo de
verificar.ps1.)"""

import glob
import json
import os
import re
import sys
from collections import Counter
from datetime import datetime

raiz = os.path.join(os.path.expanduser("~"), ".claude", "projects")
patron = os.path.join(raiz, "*", "*", "subagents", "agent-*.jsonl")

archivos = sorted(glob.glob(patron), key=os.path.getmtime, reverse=True)
if not archivos:
    print("No hay transcripts de subagente todavía (%s)" % patron)
    sys.exit(0)

for ruta in archivos[:3]:
    cuando = datetime.fromtimestamp(os.path.getmtime(ruta)).strftime("%Y-%m-%d %H:%M:%S")
    print("\n=== %s   (%s)" % (os.path.basename(ruta), cuando))

    meta = ruta[:-6] + ".meta.json"
    if os.path.exists(meta):
        try:
            with open(meta, encoding="utf-8") as fh:
                print("    tipo de agente: %s" % json.load(fh).get("agentType"))
        except Exception:
            pass

    with open(ruta, encoding="utf-8", errors="replace") as fh:
        modelos = Counter(re.findall(r'"model":"([^"]+)"', fh.read()))
    if not modelos:
        print("    (sin campo model)")
    for nombre, veces in modelos.most_common():
        print("    modelo: %s   (%d mensajes)" % (nombre, veces))

log = os.path.join(os.path.expanduser("~"), ".michiclaude", "ruteo-etapa0.log")
print("\nLog del hook:")
if os.path.exists(log):
    with open(log, encoding="utf-8", errors="replace") as fh:
        for linea in fh.readlines()[-20:]:
            print("  " + linea.rstrip())
else:
    print("  (no existe %s — el hook nunca corrió)" % log)
