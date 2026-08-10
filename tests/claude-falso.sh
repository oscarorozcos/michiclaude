#!/bin/sh
# Claude Code de mentira. SOLO para probar el relevo donde no hay un Claude
# Code instalado: una distro de WSL recién estrenada, un contenedor, un
# servidor de pruebas. No habla con ninguna API, no lee credenciales y no
# escribe nada fuera de su propia salida.
#
# Por qué existe: el relevo solo necesita, del otro lado, una PTY viva que
# reaccione a lo que se le escribe. Con esto se puede comprobar de punta a
# punta que un /compact pedido desde el panel LLEGA hasta el programa
# relevado —que es lo único que el relevo promete— sin tener que instalar
# Claude Code ni gastar cuota.
#
#   $HOME/bin/claude   ← cópialo aquí y ponle chmod +x
#   claude             ← arráncalo desde una terminal interactiva
#
# Lo que se teclee (o lo que inyecte MichiClaude) sale con el prefijo
# RECIBIDO. Ctrl-D para salir.

echo "claude de mentira · aquí se imprime lo que llegue (Ctrl-D para salir)"
while IFS= read -r linea; do
  printf 'RECIBIDO: %s\n' "$linea"
done
echo "claude de mentira · fin"
