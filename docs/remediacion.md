# Remediación — de consejero a plomero que también arregla

Diseño acordado el 2026-08-07 a partir de una propuesta externa (handoff
de otra IA + mockups). Este documento es la versión DESTILADA Y CORREGIDA:
la propuesta original traía contexto falso del proyecto y varias piezas
que chocan con invariantes — aquí queda solo lo viable, con su porqué y
sus etapas. LEER ANTES de tocar cualquier cosa de remediación.
Los prompts para generar las maquetas con otra IA están guardados en
`prompts-diseno-remediacion.md` (referencia visual, no código).

## La idea en una frase

MichiClaude ya diagnostica (Hallazgos) y aconseja (Consejos). La
evolución: que además APLIQUE remediaciones — las que controla directo,
en automático opt-in; las que viven dentro de la terminal de Claude Code,
primero a un clic (clipboard) y después de verdad, vía el relevo.

## Principios de diseño (no negociables)

1. **Intención, no comando:** Michi nunca pregunta "¿/compact o /clear?";
   pregunta "¿sigues trabajando o ya terminaste?" y ÉL pone el comando.
   El nombre del comando aparece pequeño al lado — el usuario aprende el
   mapeo sin que sea requisito.
2. **Regla de oro:** en la duda, Michi pregunta, no actúa. El peor caso
   permitido del modo automático es "me preguntó de más" (recuperable),
   nunca "borró mi contexto".
3. **Honestidad de UI (espíritu del invariante #8):** jamás prometer lo
   que no se puede hacer ni afirmar lo que no se puede saber. Sin relevo
   no existe "Aplicar /compact" — existe "Copiar comando". "Michi aplicó
   X" solo se afirma si de verdad lo inyectó (o si lo VIO aplicado en el
   JSONL después).
4. **Confianza progresiva:** cada acción automática se gana. Candado con
   contador de aplicaciones manuales, primera vez de cada tipo SIEMPRE
   manual, re-bloqueo automático tras 2 cancelaciones seguidas del
   countdown, ventana de 30 días para que las manuales cuenten.
5. **Evidencia visible:** toda tarjeta de intención enseña POR QUÉ
   ("lista 8/10 · mismos archivos en 20 msgs · último msg hace 4 min").
   Mostrar evidencia = diagnóstico, no adivinanza.
6. **Las manos del usuario siempre ganan** (ver reglas anti-choque del
   relevo).

## Dos clases de remediación

**Out-of-band** — Michi las ejecuta directo, sin tocar la terminal:
matar MCPs zombies, archivar JSONL viejos. Riesgo bajo real, reversibles.
Candidatas al modo automático desde la etapa 2.

**In-band** — `/compact`, `/clear`: viven DENTRO del REPL de Claude Code
y MichiClaude no tiene canal de escritura. Sin relevo: modo
semi-automático (tarjeta + clipboard al clic). Con relevo: inyección
real. La inyección de teclado simulado (SendInput) queda DESCARTADA como
mecanismo (frágil, puede teclear encima del usuario); como mucho, un
opt-in experimental futuro.

## El relevo (`michi claude`) — el "tmux nativo" sin tmux

Un mini-ejecutable relevo que viaja con la app. El usuario teclea
`michi claude` en su terminal de siempre (Windows Terminal, VS Code,
la que sea) en su carpeta de siempre — cero configuración, nada se
mueve, `~/.claude` intacto. El relevo:

1. Lanza Claude Code dentro de una **ConPTY** que él controla (API
   nativa de Windows; en Rust, crates tipo `portable-pty` — escribir
   contra la abstracción portable desde el día 1: en Mac/Linux la PTY es
   nativa y el relevo portaría casi gratis).
2. Reenvía TODO transparente (teclas, pantalla, resize, Ctrl+C, modo
   raw). Claude Code ni se entera.
3. Abre un canal local (named pipe) por donde MichiClaude le pide
   inyectar un comando.
4. Como ve pasar cada tecla y cada byte de salida, SABE (no adivina) si
   el prompt está vacío y si Claude está generando.

**Reglas anti-choque de la inyección (todas obligatorias):**
- R1: NUNCA inyectar si hay texto del usuario desde su último Enter (el
  relevo cuenta las teclas — es certeza, no heurística).
- R2: NUNCA inyectar mientras Claude genera su turno.
- R3: ventana de calma (5-10 s sin una sola tecla) antes de actuar.
- R4: countdown visible primero; al vencer se RE-verifican R1-R3 en ese
  instante; si fallan, se aborta y degrada a clipboard.
- R5 (sagrada): Michi JAMÁS borra texto del usuario (nada de backspaces
  ni Ctrl+U para "limpiar" la línea). Si hay texto, no se inyecta, punto.
- Si el usuario aplica su propio comando primero, el relevo lo ve pasar
  y cancela el suyo (y lo anota en el registro).

**Degradación:** sesiones no lanzadas por el relevo funcionan como hoy
(detección por logs + modo clipboard). Nunca un error; solo "esta sesión
la puedo remediar / esta te dejo el comando listo". Alias opcional
(un clic en Ajustes, reversible) para que `claude` pase por el relevo
sin cambiar el hábito.

**Los 3 modos:** el relevo no sabe qué hay al otro lado del tubo —
local directo; WSL envolviendo `wsl claude`; SSH envolviendo el cliente
`ssh` local (la inyección viaja por el tubo como tecleo). Matiz SSH: los
DATOS de esa sesión llegan por el exportador con retraso de sondeo, y
casar "esta terminal" con "esta sesión de los logs remotos" necesita
heurística (carpeta + hora de inicio) — es el modo que más pruebas pide.

## Clasificador de tarea viva (señales del JSONL)

`Alive` / `Boundary` / `Uncertain`, determinista (nivel 1):

| Señal | Peso | Nota |
|---|---|---|
| TodoWrite: items con status != completed | Alta (la reina) | último TodoWrite de la sesión |
| Actividad: último evento <15 min / >60 min | Alta | viva / probable frontera |
| Continuidad de archivos (Jaccard últimos 10 vs 10 anteriores >0.4) | Media | misma tarea |
| `git commit` reciente sin ediciones después | Media | señal de cierre |
| Densidad de tool calls | Baja | ráfaga de edits = faena |

La señal de "lenguaje de cierre" por regex ("listo", "ya quedó") queda
FUERA: es solo-español y la app es de 8 idiomas.

Mapeo: `Alive` → recomendar /compact, /clear con advertencia (y en
automático, auto-/clear JAMÁS); `Boundary` → recomendar /clear;
`Uncertain` → SIEMPRE preguntar (la propuesta original resolvía
Uncertain con un modelo local — descartado, ver abajo). Los todos
abiertos alimentan la advertencia contextual del /clear ("tienes 2
pendientes — esto los borraría de la memoria de Claude").

## Desbloqueo progresivo por acción

| Acción | Default | Desbloqueo del automático |
|---|---|---|
| Matar MCPs zombies | on | inmediato (riesgo bajo real) |
| Archivar JSONL ≥365d | off | inmediato |
| /compact (con relevo) | candado | 2 aplicaciones manuales |
| /clear (con relevo) | candado | 3 manuales; solo en Boundary |

Microcopy del candado: "Michi no automatiza lo que no entiendes" —
restricción convertida en argumento. Persistencia: contador por acción
con timestamps (solo cuentan los últimos 30 días). Tarjeta educativa en
la primera vez de cada comando (qué hace / qué vas a ver / qué NO se
pierde — la línea "qué vas a ver" es la clave anti-susto).

## Etapas (cada una útil por sí sola)

1. **Consejero con intención** (sin relevo, TODAS las sesiones):
   manómetro de presión en pastilla/gatito (puntos 9-10 de
   presion-y-rendimiento.md — dato ya existe, viaja en quota:update)
   [HECHO 2026-08-07 y VALIDADO en vivo por Oscar],
   parser de TodoWrite + clasificador (TRES piezas: Rust +
   meter-export.py + panel, invariante #1) [HECHO 2026-08-07: el motor
   manda HECHOS crudos en el hit press (topen/ttotal/cont/gclean) y el
   veredicto vive UNA vez en JS (`intentVerdict`); Python validado con
   logs reales del VPS], tarjeta de intención con
   evidencia y botón "Copiar comando" (clipboard SOLO al clic — pisar el
   clipboard sin pedirlo es invasivo; dep nueva justificada:
   tauri-plugin-clipboard-manager) [HECHO 2026-08-07: regla sintética
   `intent` con presión ≥80, tarjeta en Consejos con dos opciones,
   evidencia, "Recomendado" por veredicto y "Ahora no"; VALIDADA en vivo
   el mismo día: nació sola con la sesión real del VPS al 100%,
   veredicto frontera (0/5 todos + commit limpio) → /clear Recomendado,
   y el botón "Copiar comando" probado por Oscar].
2. **Automático out-of-band:** matar zombies (con re-verificación
   anti-reciclaje de PID: nombre de ejecutable + hora de inicio antes
   del taskkill) + archivar JSONL **≥365 días** + registro de acciones +
   desbloqueo progresivo. Todo async + spawn_blocking (invariante
   10ter). SOLO LOCAL — nada de matar procesos ni mover archivos por
   SSH; las tarjetas de origen remoto no ofrecen botón.
   [IMPLEMENTADA 2026-08-07 con el go explícito de Oscar; pendiente de
   `cargo check` en Windows y de validación en vivo. Decisiones de la
   implementación en §"Decisiones de la etapa 2".]
3. **El relevo** (`michi claude`) [3a IMPLEMENTADA 2026-08-08: crate
   `relevo/`, paso transparente por ConPTY, canal por archivos, reglas
   R1-R5 y subcomandos `status`/`inject` para validar sin panel; pendiente
   de `cargo build` y prueba en el Windows de Oscar. Decisiones en
   §"Decisiones de la etapa 3a". Faltan 3b (descubrimiento en el panel) y
   3c (countdown + desbloqueo progresivo)]: inyección real de /compact//clear con
   countdown, solo sesiones del relevo; los checks "Aplicar" APARECEN
   solo cuando existen sesiones inyectables. El countdown va en una
   SUPERFICIE PROPIA (tarjeta del panel o ventana nueva) — NUNCA
   reutilizar los globos (regla única: ningún globo se cierra solo) ni
   toast de Windows con widget vivo.
4. **Relevo en WSL y SSH** — el modo automático completo en los 3 modos.

## Decisiones de la etapa 2 (implementación 2026-08-07)

- **Foto de procesos por PowerShell/CIM** (`Get-CimInstance Win32_Process`
  → JSON), no una crate de procesos: cero dependencias nuevas (invariante
  #4) y en Windows 11 PowerShell siempre está. `[Console]::OutputEncoding`
  forzado a UTF-8 (redirigido, PS 5.1 emite OEM y una ruta con acentes
  rompía el parseo). Cuesta ~1 s de CPU → sondeo `remPoll` cada 60 min y
  todo en spawn_blocking (10ter). OJO: `ConvertTo-Json` devuelve OBJETO
  suelto si hay un solo elemento — el parser lo envuelve.
- **Qué es zombie:** proceso que casa con la FIRMA de un MCP stdio
  configurado en `~/.claude.json` (global + por proyecto, misma fuente
  que mcp_unused) Y cuyo padre ya no existe — o el PID del padre lo
  recicló un proceso MÁS NUEVO que el hijo (un padre no nace después que
  su hijo). La firma es el argumento más largo del comando (paquete o
  ruta de script, mínimo 5 chars; "npx"/"node"/"python" solos NO firman
  nada). Un MCP con sesión viva tiene padre presente y más viejo: jamás
  se marca. Los MCP http/sse no son procesos hijos y quedan fuera.
- **Kill con anti-reciclaje:** `kill_zombie(pid, name, start, server,
  auto)` re-consulta ESE pid justo antes y exige mismo ejecutable y misma
  hora de arranque (±2 s). Ya no está → "gone" (no es error, no va al
  registro); cambió → ERR_ZOMBIE_CHANGED y no se toca nada.
- **Archivado:** mueve (rename, y si el volumen no deja, copia+borra) los
  .jsonl con mtime ≥365 d de `~/.claude/projects/**` (subagentes
  incluidos) a `%APPDATA%\<app>\archive\` conservando estructura —
  archivar, no borrar. WSL queda FUERA (mover por `\\wsl.localhost` es
  lento y falible — etapa 4); SSH ni se considera.
- **Registro:** `actions_log.json` en el dir de la app, tope 200. Rust
  guarda datos crudos (`kind`, `auto`, `ok`, `d1`, `d2`) y el panel los
  traduce con `t()` (invariante #10) — el registro cambia de idioma con
  la app. Nunca viaja a ntfy ni al hub (lleva nombres).
- **Desbloqueo progresivo (frontend):** `remCfg` (interruptores: zombie
  ON por defecto — riesgo bajo real —, archive OFF) y `remFirst` (sello
  de la primera aplicación MANUAL por tipo). Automático = interruptor ON
  **y** primera manual hecha; el candado de Ajustes solo se enseña
  cuando está pedido pero no ganado. Sin contador de N aplicaciones ni
  ventana de 30 días: eso es de los candados in-band (/compact, /clear)
  de la etapa 3.
- **Superficies:** sección "Remediación automática" en Ajustes (toggles +
  candado + revisar/cerrar/archivar a mano + registro) y TARJETA de
  zombies en Consejos por el pipeline normal de tarjetas (leído Gmail,
  ✕, TTL 24 h, "Ahora no"). La tarjeta nace solo cuando el automático
  NO puede actuar (apagado o sin desbloquear): su botón "Cerrar todos"
  ES la primera manual. Clave `zombie|<arranque del zombie más nuevo>`:
  un lote nuevo re-avisa aunque el anterior se haya despachado; el mismo
  lote no resucita (tipSeen). El archivado no tiene tarjeta: vive entero
  en Ajustes (apagado por defecto; quien lo enciende tiene ahí mismo el
  botón de la primera vez). Sin globos ni toasts (regla única intacta).
- **Cadencias:** zombies cada 60 min (+ primer sondeo a los 90 s);
  archivado automático una pasada al día (`remArchDay`, marcado ANTES de
  intentar, estilo fndEventLast).

### Lo que cazó la validación en vivo (2026-08-07)

Dos bugs que solo aparecían en Windows real, ninguno visible en revisión
de código:

- **Barras.** El config trae el paquete con barra normal
  (`@modelcontextprotocol/server-x`) y la línea de comando del proceso ya
  resuelto lleva barra invertida
  (`…\node_modules\@modelcontextprotocol\server-x\dist\index.js`): NINGÚN
  MCP lanzado con npx casaba. Se normalizan los dos lados a `/` antes de
  comparar.
- **El script del kill no compilaba en PowerShell.** Iba en UNA sola
  línea, y PowerShell no acepta el `}` de un bloque seguido de otra
  sentencia sin separador: el script moría en el parser, stdout salía
  vacío y TODO cierre acababa en ERR_ZOMBIE_KILL ("No se pudo cerrar")
  mientras `Stop-Process` a mano funcionaba. Ahora el script lleva saltos
  de línea reales. El escaneo nunca lo sufrió porque es una tubería de
  una sola sentencia — al escribir scripts de PowerShell desde Rust,
  saltos reales SIEMPRE.
- De ahí salió `rem_debug.json` (foto cruda de stdout/stderr cuando el
  veredicto no se reconoce): sin él, un fallo del kill es indistinguible
  desde la UI. Y el veredicto ya no se decide por `$?` —que con
  `-ErrorAction SilentlyContinue` no distingue "no pude" de "ya no
  estaba"— sino re-consultando el PID.

**Receta para fabricar un zombie de prueba** (los MCP bien educados como
`server-memory` se cierran solos en cadena cuando su cliente muere, así
que no sirven): un `mcp-fantasma.js` con `setInterval(function(){},
1000000)`, registrado con `claude mcp add fantasma -- node <ruta>`, y
lanzado con `powershell -Command "Start-Process node -ArgumentList
'<ruta>' -WindowStyle Hidden"` — ese powershell intermedio muere en el
acto y deja al node huérfano de nacimiento.

## Decisiones de la etapa 3a (el relevo, 2026-08-08)

La etapa 3 se parte en tres para que cada trozo se valide solo:
**3a** el relevo que envuelve Claude Code sin que se note (ESTO),
**3b** el panel descubre sesiones con relevo y las enseña,
**3c** countdown, inyección desde la UI y desbloqueo progresivo
(/compact 2 manuales, /clear 3 y solo en `Boundary`).

- **Crate APARTE** (`relevo/`, paquete `michi`, binario `michi.exe`), no un
  binario del crate de Tauri y **fuera de `src-tauri/`**. Tres motivos: la
  app no gana ni una dependencia (invariante #4 — `portable-pty` vive solo
  ahí); si el relevo no compila, la app sigue compilando y publicándose; y
  dentro de `src-tauri/` sería un paquete Cargo anidado que el vigilante de
  `npm run dev` recompilaría sin motivo.
- **El canal son ARCHIVOS, no un named pipe** (corrección al diseño). Un
  pipe con nombre exige `CreateNamedPipeW` + `ConnectNamedPipe` a mano:
  código `unsafe` que nadie puede compilar en el VPS. Los archivos viven en
  `%APPDATA%\com.oscarorozco.michiclaude\relevo\` — misma frontera de
  seguridad (el perfil del usuario), cero `unsafe`, y sobreviven a que la
  app se reinicie. `<pid>.json` = estado (lo escribe el relevo cada 500 ms),
  `<pid>.cmd` = una orden (la escribe la app, el relevo la borra al leerla).
  Ambos con tmp+rename; el temporal AÑADE `.tmp` al nombre entero, porque
  con `with_extension` estado y orden compartirían `<pid>.tmp` y se
  pisarían.
- **Sesión viva = estado con menos de 15 s.** Un relevo matado de golpe deja
  su archivo; la frescura es lo único fiable. Misma regla en el panel.
- **Lista blanca de DOS textos** (`/compact`, `/clear`), comparados
  literalmente. Es el límite duro: aunque algo escribiera en esa carpeta, no
  hay forma de que el relevo teclee otra cosa dentro de la sesión.
- **Privacidad:** el relevo ve cada tecla porque está en medio del cable,
  pero NUNCA la escribe en disco. Del tecleo solo salen del proceso un
  booleano (`typed`), relojes de inactividad y —si el usuario escribió él
  mismo uno de los dos comandos— cuál de los dos fue. Ni una letra del
  contenido.
- **R1 se cuenta con una máquina de estados sobre el flujo de teclas**
  (secuencias de escape aparte, para que una flecha no cuente como texto).
  Dos trampas ya cubiertas: una línea que acaba en `\` es continuación y NO
  se toma como enviada, y `ESC`+`CR` (Shift+Enter en varias terminales) cae
  dentro del estado de escape, así que tampoco. Riesgo residual honesto: si
  alguna terminal envía un salto que Claude Code no ejecuta y que no encaja
  en esos dos casos, el relevo creería la línea enviada. Lo peor posible
  sigue siendo que el texto del usuario y el comando salgan juntos — R5
  garantiza que **jamás se borra nada**.
- **Por el cable del teclado no solo llegan teclas** (cazado en la primera
  prueba en Windows, 2026-08-08): el terminal mete sus propios avisos —
  cambio de foco (`ESC [ I` / `ESC [ O`), posición del cursor (`R`),
  identificación (`c`), estado (`n`), medidas de ventana (`t`)— y si
  contaran como actividad, la ventana de calma se reiniciaría CADA VEZ que
  el usuario sale de la terminal… que es exactamente lo que hace para ir al
  panel de MichiClaude. Nunca se podría inyectar. `KeyWatch::feed` devuelve
  ahora `human` y solo eso mueve el reloj de calma. Todo lo demás de una
  secuencia CSI (flechas, inicio/fin, pegado, F1…) sí es humano, y un `ESC`
  suelto también (llega solo en su propio bloque de lectura).
- **R2 es la única señal que NO es certeza** y hay que decirlo: "Claude está
  generando" se deduce de que la PTY siga escupiendo bytes (`QUIET_MS` 2 s).
  El diseño original prometía saberlo; en realidad se infiere. Es
  fail-closed y se combina con la calma de teclado (`CALM_MS` 8 s) y un
  enfriamiento de 15 s tras inyectar.
- **R4 vive repartido:** el countdown lo pinta el panel (etapa 3c), pero
  quien decide es el relevo — `attend()` vuelve a comprobar R1-R3 en el
  instante de escribir. Que el countdown termine no es un permiso.
- **Se valida sin panel.** El binario trae `michi status` (sesiones vivas y
  por qué están o no listas) y `michi inject /compact`. Con eso la etapa 3a
  se prueba entera desde la terminal, antes de escribir una línea de UI.
- Motivos del "no" en código (`ERR_RELAY_TYPED`, `_BUSY`, `_NOISY`,
  `_COOLDOWN`, `_GONE`, `_BADCMD`, `_WRITE`): los traduce el panel,
  invariante #10.
- Arranque de `claude`: primero directo y, si falla, a través de
  `cmd.exe /c` (el `claude.cmd` de npm). Directo es preferible — cmd.exe en
  medio se queda con el Ctrl+C ("¿Terminar trabajo por lotes?").
- El hijo recibe `MICHI_RELEVO=<pid>` en el entorno; servirá en 3b para
  casar "esta terminal" con "esta sesión de los logs" junto a cwd + hora.

## Correcciones sobre la propuesta original (para no rediscutir)

- **Contexto falso que traía el handoff:** licencia "MIT/Apache
  open-core" (somos GPL-3.0), tier "Pro" con precios (el negocio vive
  FUERA del repo), tipografías/colores inventados a medias, iconos
  Tabler por icon-font (rompería la CSP — aquí todo icono es SVG
  inline).
- **"L2: modelo local" para Uncertain:** DESCARTADO — ya estaba
  descartado con porqué en presion-y-rendimiento.md §Lo descartado
  (peso, distribución, invariante #4). Uncertain = preguntar.
- **"Archivar JSONL >30 días" etiquetado riesgo bajo:** era la acción
  MÁS peligrosa — el analizador, el Reporte y las marcas de arreglo
  necesitan ese historial (cleanupPeriodDays 365). Umbral corregido a
  ≥365 días.
- **"Handoff Pro" (resumen de traspaso + /clear):** APLAZADO — necesita
  una IA que la app no tiene (coach sin IA por diseño, modelo local
  descartado, y usar el token OAuth gastaría la cuota del usuario).
  Decisión de negocio de Oscar, no técnica.
- **Podar CLAUDE.md / editar settings.json del usuario:** APLAZADO — el
  mayor salto de confianza; decidir QUÉ reglas sobran no lo sabe una
  regla determinista (nos pasó con el corte de 118.8k: solo Oscar sabía
  qué sobraba). El detector claudemdsize que avisa ya existe y basta.
- **Wrapper tmux/WSL:** INNECESARIO — el relevo ConPTY cubre WSL
  envolviendo `wsl claude`.
- **Todo el microcopy va por I18N** (códigos desde Rust, t() en el
  panel, invariante #10): son ~30-40 claves × 8 idiomas, y hay que
  validar que el tono seco de Michi sobreviva la traducción.
- **"Telemetría local"** (ajustar heurística si el usuario corrige 2
  veces): válida SOLO como contadores en localStorage — nada viaja
  (invariante #3). Y nada del registro de acciones va a ntfy con rutas
  ni nombres de proyecto.

## Decisión de arranque (histórico)

- Matar procesos es una CLASE NUEVA de capacidad (hasta la etapa 1 la
  app solo leía logs y llamaba un endpoint) — por eso las etapas 2-4 no
  se colaron en un PR: Oscar dio el GO EXPLÍCITO el 2026-08-07 y la
  etapa 2 se implementó ese día.
- Las etapas 3-4 (el relevo) arrancan cuando la 2 pase `cargo check` en
  Windows y se valide en vivo: el relevo construye SOBRE el registro de
  acciones y el desbloqueo progresivo que nacen aquí, y validar por
  etapas es la misma disciplina que funcionó con la 1.
