# michi — el relevo de MichiClaude

Etapa 3 de `docs/remediacion.md`. Envuelve a Claude Code en una consola
virtual (ConPTY) para que MichiClaude pueda aplicar `/compact` o `/clear`
sin teclear encima de nadie.

Es un **crate aparte a propósito**: la app de Tauri no gana ni una
dependencia (invariante #4) y, si esto no compila, la app sigue
compilando y publicándose.

## Compilar

```powershell
cd relevo
cargo build --release
# queda en relevo\target\release\michi.exe
```

## Usar

```powershell
michi claude          # abre Claude Code con relevo — todo funciona igual
michi status          # sesiones con relevo abiertas ahora mismo
michi inject /compact # aplica el comando a la sesión con relevo
```

`michi status` e `inject` existen para poder validar el relevo entero
desde la terminal, antes de que el panel sepa nada de él.

## Lo que este programa NO hace

- **No guarda lo que tecleas.** Ve cada tecla porque está en medio del
  cable, pero de ahí solo salen un booleano ("hay texto sin enviar"),
  relojes de inactividad y —si escribiste tú mismo `/compact` o
  `/clear`— cuál de los dos fue. Ni una letra del contenido.
- **No puede teclear otra cosa.** La lista de textos que acepta es de
  dos, comparados literalmente: `/compact` y `/clear`.
- **No borra nada tuyo.** Si hay texto sin enviar en el prompt, no
  inyecta. Nunca manda un backspace (regla R5 del diseño).
- **No habla con la red.** El canal son dos archivos en tu propio perfil
  (`%APPDATA%\com.oscarorozco.michiclaude\relevo\`).
