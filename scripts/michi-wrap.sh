#!/bin/sh
# michi-wrap.sh — lo que se pone en `claudeCode.claudeProcessWrapper` de VS
# Code para que el CHAT de la extensión pase por el relevo de MichiClaude.
#
# La extensión ejecuta esta ruta en lugar de `claude`, con los mismos
# argumentos. Aquí solo se le antepone el subcomando `wrap` y se cede el
# proceso (exec, sin dejar una capa de más).
#
# REGLA DURA, la misma del shim de Windows: esto NUNCA puede dejar a nadie sin
# Claude Code. Si falta python3 o falta el relevo, se arranca el claude real y
# ya está — se pierde el relevo, no la herramienta. El chat de alguien es su
# trabajo del día; una función de más no vale romperlo.
set -u

RELEVO="${HOME}/.michiclaude/michi-relevo.py"

if command -v python3 >/dev/null 2>&1 && [ -f "$RELEVO" ]; then
  exec python3 "$RELEVO" wrap "$@"
fi

# --- de aquí para abajo, solo el camino de emergencia ---

# a) la extensión pasa el binario real como primer argumento
if [ "$#" -gt 0 ] && [ -x "$1" ]; then
  exec "$@"
fi

# b) no lo pasa: se busca el que trae la propia extensión (Remote-SSH incluido)
for c in "${HOME}"/.vscode-server/extensions/anthropic.claude-code-*/resources/native-binary/claude \
         "${HOME}"/.vscode/extensions/anthropic.claude-code-*/resources/native-binary/claude; do
  [ -x "$c" ] && exec "$c" "$@"
done

# c) el del PATH, como último recurso
if command -v claude >/dev/null 2>&1; then
  exec claude "$@"
fi

echo "michi-wrap: no encuentro Claude Code. Quita claudeCode.claudeProcessWrapper de los ajustes." >&2
exit 127
