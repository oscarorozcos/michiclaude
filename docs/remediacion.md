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
automático, auto-/clear JAMÁS — matizado el 2026-08-09: existe auto-/clear,
pero SOLO en Boundary y con la red /export verificada, ver §El auto-/clear
con red); `Boundary` → recomendar /clear;
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
3. **El relevo** (`michi claude`) [3a COMPLETA Y VALIDADA EN VIVO
   2026-08-08 en el Windows de Oscar, seis pruebas: transparencia (con
   `/login` y navegador incluidos), `michi status` desde otra terminal,
   inyección real de `/compact` y el candado negándose con texto vivo en
   el prompt. Crate `relevo/`, ConPTY, canal por archivos, reglas R1-R5 y
   subcomandos `status`/`inject` para validar sin panel. Decisiones y los
   TRES fallos que cayó por el camino en §"Decisiones de la etapa 3a";
   autopsia completa en la bitácora. 3b IMPLEMENTADA 2026-08-08
   (descubrimiento y casado en el panel; pendiente de `cargo check` en
   Windows y de validación en vivo — §"Decisiones de la etapa 3b").
   Falta 3c (countdown + inyección desde la UI + desbloqueo
   progresivo)]: inyección real de /compact//clear con
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
- **R1 FALLÓ en la primera validación en vivo (2026-08-08) y así se
  arregló.** Con `hola` sin enviar en el prompt, `michi status` decía
  `texto: no` y la inyección se aplicó: salió `hola/compact` como un solo
  mensaje. R5 aguantó (no se borró nada — el peor caso previsto), pero el
  guardián no hizo su trabajo. Dos causas de fondo, las dos de diseño:
  1. **Dos fuentes de verdad.** `typed` era un `AtomicBool` aparte del
     buffer de la línea; en cuanto se desincronizaron, mandó el booleano.
     Ahora `typed` se DERIVA del buffer (`KeyWatch::has_text`), fuente
     única.
  2. **El Enter limpiaba el modelo a ciegas.** Un Enter que Claude Code no
     ejecuta (Shift+Enter según terminal, un modo del REPL, lo que sea)
     dejaba el modelo vacío con el texto todavía en pantalla. Ahora un
     Enter no limpia: aparta la línea a `pending` y espera a ver si Claude
     REACCIONA — si salen bytes por la PTY después del Enter, se envió; si
     en `SUBMIT_WAIT_MS` (3 s) no sale nada, no se envió y la línea VUELVE.
     Mientras está sin decidir cuenta como texto vivo (fail-closed).
  De ahí salió `michi status --debug`, que enseña `line_len` y las CUENTAS
  de teclas (imprimibles/Enter/escapes/controles) — nunca el contenido —
  para poder diagnosticar sin ver lo que el usuario escribió.
- **Y el diagnóstico destapó la causa REAL, que no era ninguna de las dos:
  `k_print: 0`, `k_esc: 38` con `hola` escrito.** El relevo no había
  contado una sola tecla en su vida. En Windows Terminal, **ConPTY pide
  `win32-input-mode` (`ESC [ ? 9001 h`) al arrancar y el terminal se lo
  concede a TODA la ventana** — incluida la nuestra, que es quien reenvía
  esa petición sin saberlo. Con ese modo, cada tecla viaja como
  `ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _`: ni un carácter suelto. Las letras
  llegaban a Claude porque el relevo reenvía los bytes intactos, pero para
  el contador eran ruido — y encima el terminador `_` cae dentro de
  `0x40..0x7e`, así que las secuencias se cerraban limpiamente y nada
  chirriaba. `KeyWatch::win32_key` las decodifica: `Uc` es el carácter en
  decimal, y solo cuenta con `Kd` = pulsación (soltar una tecla no
  escribe). El camino de bytes sueltos SE QUEDA: el modo solo está activo
  mientras el terminal lo concede.
  Lecciones: **(a)** envolver una terminal no es solo reenviar bytes — hay
  un protocolo negociado a tus espaldas entre el terminal y la ConPTY, y
  quien se mete en medio hereda esa negociación; **(b)** un guardián que
  cuenta cosas necesita EXPONER sus cuentas: aquí `k_print: 0` valía más
  que tres rondas de teoría.
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

## El techo del manómetro NO es una constante (corregido 2026-08-08)

La etapa 1 dividía la presión entre 200k fijos. Opus/Sonnet 4.6+ y
Fable/Mythos son de **1M**, así que sesiones reales de 998k marcaban 100%
y la tarjeta de intención saltaba al 16% del depósito. Autopsia completa
en la bitácora; lo VIGENTE:

- El hit `press` trae `full` = techo del modelo de ESA sesión. El motor
  manda el denominador junto al dato porque solo él sabe qué modelo corrió.
- `ctx_for()` (Rust y `meter-export.py` en sincronía, invariante #1) lo
  resuelve: tabla DESCARGADA primero —la cascada de precios ya publica el
  techo (`max_input_tokens` / `limit.context` / `context_length`), así que
  no hay ni una descarga ni una dependencia nuevas— y si no, `ctx_table()`,
  respaldo embebido que decide por VERSIÓN, no por lista (invariante #6).
- **En la duda, 200k.** Quedarse corto avisa de más; pasarse no avisa
  nunca. El fallo seguro de un avisador es avisar de más.
- El sufijo `[1m]` del id manda sobre todo y se mira ANTES de `price_key()`,
  que lo recorta al normalizar.
- Se guarda el MODELO en el estado de sesión, no el techo ya resuelto: una
  tabla nueva corrige la cuenta al siguiente sondeo.
- En el panel el denominador vive en UN sitio (`pressFull`/`pressPct`).
  Repartir la división fue lo que dejó vivir el bug tanto tiempo.
- Esto va ANTES de la 3c a propósito: la 3c ACTÚA sobre este porcentaje.

**Auditoría de las tres fuentes (2026-08-08, a petición de Oscar).** Si el
techo lo decide "la primera que responda", conviene saber si dicen lo mismo:

- **Precios: coinciden al céntimo** en todos los modelos compartidos.
- **Techo: una sola discrepancia**, `claude-sonnet-4-5` — LiteLLM 200k,
  models.dev 1M. Las dos aciertan a medias: es de 200k con un beta de 1M.
  Por eso existe `ctx_full()`: sin evidencia se queda en 200k (conservador),
  y si una sesión de esa máquina supera esa cifra, manda lo medido.
- **Fallo real que venía de antes:** OpenRouter escribe la versión con
  PUNTO (`claude-opus-4.8`) donde LiteLLM, models.dev y los logs usan
  GUIÓN. La tercera fuente casaba 6 de sus 14 modelos: si las otras dos
  caían, ocho modelos vigentes se quedaban sin precio Y sin techo, en
  silencio. `price_key()` unifica punto→guión entre dígitos (solo entre
  dígitos: `anthropic.claude-opus-5` no se toca), en el ÚNICO punto por el
  que pasan guardar y buscar, así que los dos lados siguen casando.
- Contra el error contrario —una fuente que INFLE el techo y silencie el
  aviso— la respuesta buena es el detector de auto-compacts (pendiente en
  `presion-y-rendimiento.md`): Claude Code comprime cerca del límite real,
  y eso es la medida más honesta que hay. No se hizo ahora.
- **Regla para la 3c:** el automático se gana con certeza. Si el techo no
  es de fiar, la 3c aconseja pero NO actúa.
- El panel informa de las dos cosas en la misma sección de Ajustes
  (`ctx_count` = cuántos modelos traen techo): si una fuente deja de
  publicarlo, el número baja a la vista en vez de degradarse callando.

## Decisiones de la etapa 3b (el panel descubre el relevo, 2026-08-08)

La 3b SOLO MIRA. No hay un solo camino por el que el panel pueda pedirle
nada al relevo todavía: eso es la 3c. Se separó así para poder validar el
descubrimiento y el casado por su cuenta, que es donde estaba el riesgo
real (señalar la sesión equivocada).

- **`get_relays`** (Rust, async + `spawn_blocking`, invariante #10ter) lee
  `%APPDATA%\<app>\relevo\*.json` y devuelve las sesiones VIVAS. Viva =
  estado con menos de 15 s Y `alive` — la MISMA regla que `michi status`;
  si cambia, cambia en los dos lados. No devuelve el bloque `diag` (las
  cuentas de teclas son para diagnosticar en la terminal, no para la UI).
- **Basura:** un archivo que lleva 24 h sin tocarse es de un relevo muerto
  de golpe (uno vivo escribe cada 500 ms); se borra al pasar. Es nuestra
  propia carpeta de datos y así no crece para siempre.
- **Casar sesión y relevo se hace por el `cwd` COMPLETO, no por el nombre
  de la carpeta.** Dos proyectos distintos pueden llamarse igual, y en la
  3c eso sería teclear en la terminal equivocada. Para eso el hit `press`
  lleva un campo ADITIVO `scwd` (el cwd normalizado a `/`), replicado en
  `meter-export.py` (invariante #1). Verificado antes de subir: el export
  normal y `--findings` salen IDÉNTICOS byte a byte contra la versión
  anterior, y `--coach` solo añade la clave nueva.
- **Fail-closed en la ambigüedad,** igual que en el relevo: si dos relevos
  comparten carpeta, no hay forma de saber cuál es esta sesión y no se
  afirma nada. LÍMITE ASUMIDO: dos sesiones de Claude Code en la MISMA
  carpeta, una con relevo y otra sin él, el cwd no las desempata. La
  alternativa (hora de arranque del relevo vs primer turno del log) falla
  con `--continue`/`--resume`, así que no se usa: mejor no emparejar que
  emparejar mal.
- **Solo LOCAL:** un hit con `origin` (VPS) no casa nunca, aunque las
  rutas coincidieran. El relevo por SSH es la etapa 4.
- **Dónde se ve:** Ajustes → Remediación, lista "Sesiones con relevo"
  (proyecto · pid · % de contexto de la sesión casada · listo o el
  motivo). Sondeo de 5 s SOLO con esa pestaña a la vista: el estado
  caduca en 15 s y enseñarlo viejo es peor que no enseñarlo. Fuera de ahí
  basta el compás del coach (3 min), que es quien necesita saber si la
  sesión bajo presión tiene relevo.
- **El % de la fila es la prueba visible del casado.** Se puede validar el
  emparejamiento sin esperar a una sesión al 80%: si la fila enseña el
  contexto de la sesión correcta, relevo y log están casados.
- **Tarjeta de intención:** insignia "relevo" cuando la sesión bajo
  presión tiene uno. Nada más — prometer ahí un botón que no existe sería
  vender la 3c antes de tiempo.
- Los motivos `ERR_RELAY_*` se traducen en el panel (`rly_e_*`,
  invariante #10); un código desconocido se enseña CRUDO antes que
  inventarle una frase bonita.
- **Pendiente que destapó la 3b:** cómo llega `michi.exe` al usuario. Hoy
  se compila aparte a mano (`cd relevo; cargo build --release`) y no va en
  el instalador. Decidirlo es parte de la 3c — si el panel va a ofrecer
  "aplicar por ti", el binario tiene que existir en la máquina y estar en
  el PATH.

## Decisiones de la etapa 3c (aplicar desde la interfaz, 2026-08-08)

Va en dos pasadas. **3c-1 (ESTO): manual con countdown**, que es lo que se
puede probar y validar entero hoy. **3c-2: el automático**, que necesita una
superficie visible con el panel cerrado — ver "lo que falta" al final.

- **`relay_inject(pid, text, auto)`** (Rust, async + `spawn_blocking`):
  escribe `<pid>.cmd` con tmp+rename —`.tmp` sobre el nombre ENTERO, misma
  regla que el relevo— y ESPERA el acuse en `<pid>.json` hasta 8 s. Devuelve
  el `ERR_RELAY_*` del relevo sin traducir (invariante #10); `ERR_RELAY_NOACK`
  es nuestro: la orden se escribió y nadie contestó.
- **La lista blanca se comprueba en los DOS lados.** Aquí para no escribir
  una orden imposible; en el relevo porque es el límite duro y no se fía de
  quien le escriba.
- **El panel pide, el relevo decide.** `attend()` vuelve a comprobar R1-R3
  en el instante de escribir: si el usuario se puso a teclear durante la
  cuenta atrás, la orden se rechaza y el botón dice por qué. Que el countdown
  termine NO es un permiso — eso es R4.
- **El countdown es de 5 s y el propio botón es el de parar.** Un solo
  control, imposible confundirse. Cancelar no escribe nada en ninguna parte.
- **NADIE repinta mientras hay una cuenta atrás viva** (`relayBusy`): un
  re-render se llevaría el botón, el temporizador seguiría sobre un nodo
  huérfano y la orden se aplicaría sin que el usuario viera nada. El
  countdown es su única ventana para parar; si desaparece de la pantalla,
  deja de ser una ventana.
- **Dónde está el botón.** En la tarjeta de intención, junto a cada comando
  (donde ya está el veredicto y el aviso de pendientes), y en la lista de
  Ajustes → Sesiones con relevo, pero **ahí solo `/compact`**: `/clear` borra
  la memoria de la conversación y esa decisión necesita el contexto de la
  tarjeta, no una fila suelta. El de Ajustes hace además de banco de
  pruebas: no hay que esperar a una sesión al 80% para ejercitar
  el camino entero.
- **El motivo del rechazo NUNCA va dentro del botón.** Metido ahí, un
  "No se aplicó: tienes texto sin enviar" estiraba el botón hasta salirse
  del panel (visto por Oscar en la primera validación). Va en `.int-msg`,
  una línea propia a lo ancho de la fila; el botón conserva su etiqueta
  corta. Regla general: **un control de tamaño fijo no es un sitio donde
  poner texto de longitud desconocida.**
- **Todo lo aplicado va al registro de acciones** (`kind: "relay"`, d1 =
  comando, d2 = proyecto, crudos y traducidos por el panel). Si Michi teclea
  en tu terminal, queda escrito.
- **Lo que TECLEAS TÚ cuenta para el desbloqueo** (2026-08-08, lo cazó
  Oscar: aplicó un `/clear` a mano y el contador no se movió). El candado
  dice "aplícalo tú una vez", no "pulsa mi botón una vez" — y teclearlo en
  la terminal es la aplicación manual por antonomasia. El relevo ya lo veía
  pasar y lo publicaba en `user_cmd`; el panel lo tiraba. Ahora `rlyPoll` lo
  cuenta, una sola vez por (pid, momento), con sello en localStorage para
  que ni un sondeo repetido ni un reinicio lo dupliquen. NO va al registro
  de acciones: ese es de lo que aplica MICHI, no tú.
- **Desbloqueo progresivo** en `localStorage.relayDone`: `/compact` 2
  aplicaciones manuales, `/clear` 3 —una más porque borra memoria y no se
  deshace—. El marcador se enseña en Ajustes para que se vea acumular, en vez
  de que un día aparezca un automático de la nada.

## La etapa 3c-2: el automático (2026-08-08)

- **Cuatro condiciones, todas duras:** interruptor encendido (nace APAGADO),
  desbloqueo ganado a mano (2 aplicaciones de `/compact`), relevo casado sin
  ambigüedad, y **widget A LA VISTA**. Esta última es la que manda: la cuenta
  atrás vive en la cápsula, así que con el widget oculto NO se actúa — una
  cuenta atrás que nadie puede ver no es una cuenta atrás, es Michi tecleando
  a tus espaldas con un adorno.
- **`/clear` no se automatiza**, aunque se desbloquee. Borra la memoria de la
  conversación y no se deshace; queda para cuando el automático tenga
  kilómetros. (Superado el 2026-08-09 a petición de Oscar, con condiciones
  extra: ver §El auto-/clear con red.)
- **15 s de cuenta**, el triple que la manual: esto no lo pediste tú.
- **Cualquier toque en la cápsula (o en el gatito) la para.** En el gatito el
  manejador va en fase de CAPTURA para ganarle a todos los demás: recuperar
  el control no puede depender de acertar la zona correcta.
- **Una vez por sesión, y se marca ANTES de empezar** (`relayAuto` en
  localStorage). Si algo falla, no puede convertirse en un bucle que teclee
  en la terminal del usuario. Consecuencia asumida: si cancelas, esa sesión
  no vuelve a intentarlo — la tarjeta de intención sigue ahí para hacerlo a
  mano.
- El relevo **vuelve a comprobar R1-R3 al escribir** (R4), así que si el
  usuario se puso a teclear durante la cuenta, la orden se rechaza. Sus manos
  siempre ganan, y no hace falta que cancele nada para ello.
- Todo lo aplicado en automático va al registro marcado como `auto`.
- **Dos fallos que salieron en la PRIMERA prueba real** (Oscar, 2026-08-08,
  con el automático disparando sobre su sesión de chat del VPS):
  1. **El rechazo del candado quemaba la sesión.** `autoMark` sellaba ANTES y
     para siempre, así que un `ERR_RELAY_BUSY` —Claude estaba generando, un
     estado que dura segundos— dejaba esa sesión sin automático de por vida.
     Ahora la memoria distingue: `"done"` solo tras aplicar de verdad; un
     fallo guarda el MOMENTO del intento y se reintenta a los 10 min. El
     sello antes de empezar se conserva (evita el bucle), pero deja de ser
     una condena.
  2. **La cuenta atrás terminaba en silencio.** El usuario veía el segundero
     y después nada: ni aplicado, ni por qué no. Ahora la cuenta CIERRA con
     veredicto en la propia cápsula — ✓ verde si se aplicó, ✕ rojo si el
     relevo se negó — durante 4 s. Regla que queda: **una cuenta atrás que
     acaba sin decir qué pasó es peor que no haber avisado**, porque deja al
     usuario adivinando si actuaste.
  El resto de la cadena se validó sola en esa prueba: panel en Windows → SSH
  → relevo del VPS → candado → rechazo correcto, con el acuse guardado
  (`app-…`, `ERR_RELAY_BUSY`).
- **Mordida del invariante 10bis, otra vez** (2026-08-08): `pill.html` y
  `cat.html` ocultaban POR CLASE (`.m[hidden]`, `.pgauge[hidden]`), no con la
  regla global que sí tiene `index.html`. La cuenta atrás traía `display`
  propio escrito después, le ganaba por orden a igual especificidad, y se
  quedaba pegada como un círculo ámbar vacío en la cápsula. Arreglado
  metiendo `[hidden]{display:none !important}` en las dos ventanas: mientras
  se oculte por clase, el siguiente que añada un elemento con `display`
  vuelve a pisarlo.

**Lo que falta después de la 3c-2, y por qué no se hizo de una:** el modo automático
necesita que el countdown se vea con el panel CERRADO, que es como está el
panel casi siempre. Una cuenta atrás que nadie puede ver no es una cuenta
atrás. La superficie correcta es el widget (pastilla/gatito), que ya recibe
`press` en `quota:update` y está siempre a la vista. Hasta entonces no hay
interruptor de automático: sería prometer algo que no se puede vigilar.
Y la regla que sale de la auditoría de fuentes: **si el techo de contexto no
es de fiar, la 3c aconseja pero no actúa.**

## Saber si ESTA terminal tiene relevo (2026-08-08)

Lo levantó Oscar validando el atajo: el indicador estaba en el panel, que
es donde NO tienes los ojos. Trabajas en la terminal, y enterarte de si hay
relevo exigía abrir el panel y comparar pids — o descubrir al final que no
lo había, que es el peor final posible.

- **Título de la pestaña** (`michi · <carpeta>`, OSC 0 al arrancar, quitado
  al salir). Es la única marca que sobrevive al borrado de pantalla de
  Claude Code, y no ocupa ni una línea. **Best-effort declarado:** si Claude
  Code cambia el título después, gana él; el relevo NO lo reimpone en bucle
  —parpadearía y rompería el paso transparente, que es sagrado—.
- El título se **quita al cerrar**: una pestaña que siga diciendo «michi ·»
  con la sesión muerta es un indicador que miente.
- **El plan A NO sobrevivió** (probado el mismo día): Claude Code pone
  «Claude Code» al arrancar y la marca desaparece. PLAN B en marcha —
  `TitleMark` reescribe al vuelo el título que escriba Claude y le antepone
  la marca. Es la ÚNICA excepción al paso transparente, y va acotada al
  hueso: solo `ESC ] 0|1|2 ;` (se lee el NÚMERO ENTERO, no el primer dígito
  — `ESC]10;` es color de primer plano y tratarlo como título lo
  destrozaría), no re-marca lo ya marcado, y con tope de 1024 bytes suelta
  lo retenido tal cual. FAIL-OPEN: lo peor que puede pasar es quedarse sin
  marca, jamás comerse la salida. Diez casos probados con un puerto de la
  máquina de estados antes de compilar.
- **Segunda superficie, HECHA:** marca en el widget. Dice algo DISTINTO del
  título y por eso hacen falta las dos — el título habla de la TERMINAL que
  tienes delante, el widget de la SESIÓN que Michi mide. Con una sola sesión
  coinciden; con varias, no, y confundirlas es volver a adivinar.
  El campo `relay` viaja en `press` dentro de `quota:update`. En las cápsulas
  (pastilla y gatito) es un punto relleno en el centro del arco del manómetro
  —dentro y no al lado porque ahí no hay un píxel libre, y relleno para que
  se lea a 24 px—. En el detalle y en el globo del gatito, donde SÍ hay
  sitio, se dice con palabras («proyecto · relevo»): un punto solo puede
  insinuar, y una insinuación que no se entiende no sirve de nada.

## El atajo del PATH (2026-08-08): que `claude` pase por el relevo

Sin esto el relevo depende de un hábito, y un automático que depende de un
hábito no es un automático: se trabaja media hora y luego se descubre que la
sesión no tenía relevo (lo planteó Oscar así).

- **Un shim en el PATH, NO un alias por shell.** Las terminales y los editores
  —Windows Terminal, VS Code, Cursor, Warp, Alacritty, WezTerm, Hyper…— no
  interpretan `claude`: ejecutan un SHELL, y el shell resuelve el comando. Ir
  por shells serían cuatro mecanismos (PowerShell 7, 5.1, cmd, Git Bash) y aun
  así quedarían fuera los que salgan mañana. Un `claude.cmd` propio primero en
  el PATH lo resuelve WINDOWS, así que vale para todos de una vez.
- **Alcance honesto:** cubre cualquier terminal o editor que resuelva `claude`
  por PATH. NO cubre WSL desde dentro ni SSH (cruzan la frontera — etapa 4) ni
  una integración que llame al binario por ruta absoluta.
- **El atajo nunca puede dejarte sin Claude Code.** Dos salidas: si
  `MICHI_RELEVO` ya está puesto (estamos dentro de un relevo, no re-envolver) o
  si falta `michi.exe`, ejecuta el Claude Code de verdad, cuya ruta se resuelve
  al instalar el atajo —antes de que nuestra carpeta entre al PATH, o `where`
  se encontraría a sí mismo—.
- **PATH con cinturón:** se lee el de USUARIO, se guarda copia en
  `path_backup.txt`, se añade UNA entrada delante (el PATH efectivo es máquina
  + usuario, y el claude de npm vive en el tramo de usuario: detrás no lo
  taparía) y el interruptor quita exactamente esa. Se usa
  `[Environment]::SetEnvironmentVariable` y NO `setx`, que trunca a 1024
  caracteres y puede cargarse el PATH entero.
- **Sin `michi.exe` no se ofrece el interruptor**, se explica por qué
  (invariante #8). El binario se busca junto al ejecutable de la app, en el
  `target` del relevo (desarrollo) y en el PATH — cuando viaje en el
  instalador, la primera ruta acierta sola.
- El PATH nuevo solo lo ven procesos que arranquen DESPUÉS, y hay un matiz
  que despistó en la primera prueba (Oscar, 2026-08-08): **una PESTAÑA nueva
  no basta**. Windows Terminal heredó su entorno al arrancar y se lo pasa a
  cada pestaña, así que hay que cerrar la VENTANA entera. El aviso del panel
  lo dice con esas palabras; "abre una terminal nueva" era engañoso porque
  una pestaña parece una terminal nueva y no lo es.
- **El shim se escribe en ASCII PURO.** Un `.cmd` no declara codificación y
  cmd.exe lo lee con la página de códigos que toque: la raya del comentario
  salió como `â€”`. En un `rem` es cosmético, pero un archivo de órdenes con
  bytes que se reinterpretan es una bomba de relojería.
- **`.cmd` y no `.exe`:** cubre a quien teclea en un shell, que es el caso
  real, sin añadir una segunda compilación. Un programa que haga
  `CreateProcess("claude")` sin extensión no lo vería.
- PENDIENTE que esto destapa: `michi.exe` tiene que viajar en el instalador
  como recurso de Tauri, y eso toca el workflow de release (invariante #9: lo
  edita Oscar).

## Etapa 4 — el relevo fuera de Windows (arrancada 2026-08-08)

Lo pidió Oscar con las dos preguntas correctas: "¿podemos cubrir ambos
casos?" (terminal SSH y desatendido) y, antes, su forma real de trabajar:
**el chat de la extensión de VS Code**. Eso parte la cobertura en tres casos
y hay que decirlos sin adornos:

| Caso | ¿Relevo? |
|---|---|
| Terminal (local, integrada de VS Code, SSH al VPS) | ✅ es el territorio del relevo |
| Desatendido (`claude` interactivo lanzado por relevo/alias, tmux…) | ✅ mismo mecanismo |
| **Chat de la extensión de VS Code** | ❌ NO ES UNA TERMINAL: no hay teclado ni pantalla que envolver. Michi queda en modo consejero (tarjeta + copiar comando). Decirlo es invariante #8; prometer otra cosa sería mentir. |

- **FAIL-OPEN nuevo en michi.exe (el riesgo que destapó la pregunta):** si
  algo NO interactivo invoca `claude` a través del atajo del PATH —la
  extensión, un script, un pipe—, envolverlo en ConPTY le rompería el
  protocolo. Ahora, sin consola (`enter_raw` = None), michi ejecuta el claude
  real tal cual con `MICHI_RELEVO=0` (el 0 = "sin relevo"; y sin esa marca,
  el reintento vía `cmd.exe /c claude` podría resolver a NUESTRO shim y
  entrar en bucle infinito). La misma regla de siempre: lo peor permitido es
  quedarse sin relevo, jamás sin Claude Code.
- **4a: el relevo del VPS es PYTHON (`scripts/michi-relevo.py`), no un
  binario.** En Linux la PTY vive en la stdlib (`pty`, `termios`) y el VPS no
  tiene toolchain de Rust — un michi-linux exigiría cross-compilar o tocar el
  workflow (invariante #9). El script viaja EXACTAMENTE como el exportador:
  embebido (`include_str!`), subido en el alta y re-subido al arrancar
  (`upload_script`; el del relevo es cortesía — si falla no tumba el alta).
  Réplica de main.rs con las MISMAS constantes, esquema de estado y códigos
  `ERR_RELAY_*`; sin la rama win32-input-mode (eso es ConPTY). Estado en
  `~/.michiclaude/relevo/<pid>.json`.
- **4a VALIDADA EN EL PROPIO VPS el mismo día, sin gastar ronda de Oscar:**
  banco de pruebas con PTY real y `cat` de falso claude — 12 comprobaciones:
  estado y esquema, calma, inyección aplicada y RECIBIDA por el hijo, candado
  TYPED con `k_print=4` exacto, Enter con reacción limpia el texto, los
  avisos de foco no reinician la calma, y el estado se borra al salir. Un
  fallo del banco por el camino que es lección de PTY: un Ctrl+D con bytes
  pendientes en la línea no es EOF, es "enviar línea" — hacen falta dos.
## El chat de la extensión SÍ se puede relevar (2026-08-08) — corrección

**El veredicto de más abajo era incorrecto y queda anulado.** Se dictó con la
investigación a medias: se miró CÓMO arranca la extensión (ruta absoluta,
sockets) y se concluyó "imposible" sin mirar si la extensión ofrecía un
enganche ni cómo se comporta su protocolo. Las dos cosas cambian la
respuesta. Se conserva el texto original abajo porque el error tiene valor:
**"no se puede" es una conclusión que exige tanta evidencia como "sí se
puede", y aquí se dio por buena con la mitad.**

Lo que se encontró al investigar de verdad:

- **La extensión tiene un enganche OFICIAL:**
  `claudeCode.claudeProcessWrapper` — *"Executable path used to launch the
  Claude process"*. Ajuste soportado, de scope `machine`, presente en 2.1.226.
  No hay que parchear nada ni pelearse con actualizaciones.
- **`/compact` se ejecuta por el protocolo:** enviando por stdin la línea
  `{"type":"user","message":{...,"content":[{"type":"text","text":"/compact"}]}}`
  la CLI lo INTERCEPTA como comando — turno `<synthetic>`, `num_turns: 0`,
  coste $0. No llega al modelo.
- **Y el modo chat es MÁS seguro que el de terminal, no menos.** R1 (jamás
  teclear encima del usuario) se cumple por CONSTRUCCIÓN: no hay buffer
  compartido ni teclas a medias, cada mensaje es una línea atómica y mezclarse
  es imposible. R2 deja de inferirse del silencio: el protocolo lo DICE (`user`
  entra → `result` sale). Certeza donde en la terminal había deducción.
- **Casado EXACTO por `session_id`**: el evento `system init` trae el id que
  da nombre al `.jsonl`, así que aquí no hace falta la heurística del `cwd`.

Implementación (`michi-relevo.py wrap`, paso 1) y activación (`michi-wrap.sh`
en el ajuste, paso 2):

- **`michi-wrap.sh`** es lo que se pone en el ajuste; solo antepone `wrap` y
  hace `exec`. Con tres caminos de emergencia —binario como primer argumento,
  el `native-binary` de la extensión, el del PATH— porque **el chat de alguien
  es su trabajo del día y una función de más no vale romperlo**. Validado: sin
  python3 y sin relevo, arranca igual.
- **Las dos convenciones posibles del wrapper están cubiertas** (que la
  extensión pase el binario real como primer argumento o que no lo pase),
  porque cuál usa NO está documentado y adivinar rompería chats ajenos.
- **El `/compact` inyectado se ve en el chat, y hubo que construirlo.**
  `--replay-user-messages` replica los mensajes normales pero NO los comandos:
  la CLI los intercepta antes (medido — el harness lo cazó y desmintió lo que
  yo había prometido una hora antes). El relevo emite él mismo la línea de
  replay, con la MISMA forma que usa la CLI, así que la extensión ya sabe
  pintarla. Va solo al chat: el JSONL lo escribe la CLI y no se toca, así que
  esto NO falsea el registro.
- **Riesgo residual, dicho claro:** no vemos el borrador del cuadro de chat.
  Si se inyecta mientras el usuario redacta, su mensaje llega después y se
  evalúa con el contexto ya compactado. No se corrompe nada —y es justo para
  lo que sirve la función—, pero no es invisible: por eso el countdown y la
  ventana de calma siguen existiendo también aquí.
- **VALIDADO 12/12 contra el binario REAL de la extensión** (2.1.226), con un
  cliente que simula al chat, incluida la invocación tal cual la hará VS Code
  (a través del lanzador y SIN pasarle el binario).

### El veredicto original, conservado como error documentado



Es el día a día de Oscar y pidió "hazlo compatible o ve la manera". Se
investigó EN SU PROPIA MÁQUINA (el VPS), no en teoría:

- La extensión lanza claude por **ruta absoluta**
  (`~/.vscode-server/extensions/anthropic.claude-code-*/resources/native-binary/claude`)
  con `--input-format stream-json` y **stdin/stdout conectados a sockets
  privados** del host de la extensión (verificado en /proc). Ni PATH, ni PTY,
  ni canal alcanzable: el shim no lo ve (bien: tampoco puede romperlo) y el
  relevo no tiene dónde engancharse.
- **Y aunque hubiera canal, no se inyectaría.** Michi no puede ver el
  borrador del cuadro de chat, así que R1 (jamás teclear encima del usuario)
  es INVERIFICABLE ahí. Regla de oro: en la duda no se actúa. Este es el
  motivo de principio; el técnico es solo el segundo.
- Lo que SÍ funciona en la extensión, demostrado hoy con la sesión real de
  Oscar al 78%: todo el circuito consejero — manómetro, tarjeta de intención,
  copiar comando (un pegado en el mismo cuadro donde ya escribe).
- **La compensación construida el mismo día: el detector de auto-compacts**
  (regla `acomp`, adelantada de presion-y-rendimiento.md). Claude Code se
  compacta SOLO al llegar al límite — ese es el airbag de la extensión — y
  deja en el log un `compact_boundary` con `trigger` (manual/auto) y
  `preTokens`. Michi avisa SOLO de las automáticas (una manual la hiciste tú;
  avisar sería ruido), explica por qué el manómetro bajó de golpe y enseña a
  elegir el momento con /compact en una pausa natural. Tres piezas en
  sincronía (Rust + exportador, invariante #1; ficha `tip_acomp_*` ×8).
  Ventana de 30 min para no revivir compactaciones viejas si el estado se
  reconstruye desde cero. Verificado en vivo: los compactos manuales de hoy
  NO dispararon.
- La propia extensión tiene un **modo "Terminal experience"** (banner
  "Prefer the Terminal experience?"). Si corre claude en la terminal
  integrada resolviendo por PATH, el atajo y el relevo aplicarían — POR
  VALIDAR, no prometido.

### 4b y 4c: el panel alcanza las sesiones de otra máquina (2026-08-08)

- **4b (leer):** `scan_relays_remote()` lee `~/.michiclaude/relevo/*.json` por
  SSH con UNA sola conexión por servidor (un `for … cat` que emite una línea
  JSON por sesión), la misma tubería del exportador — sin protocolo nuevo. El
  `Relay` gana `origin` (lo etiqueta quien lee, como en el hub), `mode`
  (terminal/chat) y `sid`.
- **Coste y cadencia:** una conexión SSH por servidor, así que las remotas
  SOLO se piden en el compás del coach (3 min). El sondeo de 5 s de la pestaña
  mira lo local y **conserva** las remotas ya conocidas: mejor un dato de hace
  dos minutos que una pestaña que se congela cada cinco segundos.
- **4c (escribir):** `relay_inject_remote()` deja el `.cmd` con **tmp+rename
  en el servidor** (si no, el relevo puede leer un archivo a medias) y espera
  el acuse releyendo su estado. El comando viaja por **stdin, jamás
  interpolado en la línea de shell**: hoy sale de una lista blanca de dos
  elementos, pero alguien la ampliará algún día y no quiero que ese día una
  comilla se convierta en ejecución remota. La puerta se cierra antes de que
  exista.
- **El casado, ahora en dos niveles.** Primero `sid` (EXACTO — solo lo trae el
  modo chat, donde el protocolo lo regala); si no, el `cwd` de siempre. Y
  ambos exigen la MISMA máquina: un hit del VPS solo puede casar con un relevo
  del VPS. `origin` vacío significa "esta máquina" en los dos lados.
- **VALIDADO 6/6 contra el relevo de chat vivo**, con las mismas órdenes de
  shell que ejecuta el Rust. El último caso vale por todos: la inyección
  remota devolvió `ERR_RELAY_BUSY` porque el modelo estaba generando en ese
  instante — el candado funcionando a través de SSH, no una simulación.

- **Falta:** el alias de `~/.bashrc` opt-in para las terminales del VPS
  (bloque con marcas, mismo espíritu que el shim) y WSL. El automático NO se
  extiende a remotas hasta que la 4b/4c tengan kilómetros: hoy el botón
  remoto es manual.

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

## La auto-compactación de Claude Code (investigada 2026-08-08)

Oscar vio en el chat de la extensión el circulito con "6% of context
remaining until auto-compact" y preguntó lo correcto: ¿es lo mismo que
hacemos nosotros?, ¿gasta?, ¿lo apagamos para dejar solo a MichiClaude?,
¿hay que ganarle por velocidad?

Se comprobó LEYENDO el binario instalado (v2.1.226), no de memoria:

- **Es la misma operación.** `autoCompactEnabled` se describe como
  "Automatically compact conversation when context fills". Nuestro
  `/compact` inyectado y el suyo terminan en la misma compactación.
- **Cuándo salta.** El umbral efectivo es `min(ventana − reserva,
  precompute)`, con reserva = `min(maxOutputTokens, 20 000)`. La
  ventana "auto" está afinada por modelo (los de 1M —opus-5, opus-4-8,
  opus-4-6, sonnet-4-6— llegan cerca del millón). En la práctica salta
  sobre el ~94% de su ventana: el "6% restante" que se vio.
- **Se puede apagar**, por tres vías: `/config` → "Auto-compact",
  `autoCompactEnabled:false` en settings.json, o `DISABLE_AUTO_COMPACT`.
  Y `/autocompact` mueve la ventana (100k–1M, o
  `CLAUDE_CODE_AUTO_COMPACT_WINDOW`).

**DECISIÓN: no se apaga, y MichiClaude no lo sugiere jamás.** Tres
razones, por orden de peso:

1. **Es la red de seguridad.** MichiClaude solo actúa si está abierto,
   con el widget a la vista, con el relevo enganchado y —si es remota—
   con el SSH vivo. La auto-compactación no depende de nada de eso. Un
   producto no apaga el airbag del coche porque él ya frena bien.
2. **Apagarla encarece nuestro propio /compact.** Con auto-compact ON,
   Claude Code precomputa el resumen en segundo plano
   (`precomputeCompactionEnabled`, descrito como "Only applies when
   auto-compact is on"). Apagarla nos quita ese adelanto a nosotros
   también.
3. **No hay carrera que ganar.** Nosotros entramos al 80%
   (`INTENT_PCT`, el mismo umbral de la tarjeta de intención) y él al
   ~94%: se le gana POR DISEÑO, con ~14 puntos de margen. El 2026-08-08
   quedó demostrado en vivo — nuestro `/compact` liberó 872 960 tokens y
   la auto-compactación nunca llegó a dispararse.

**Y el ahorro NO es la razón.** Medido sobre el log real: la
compactación resume lo que haya en contexto, así que entrar al 80%
(~800k) en vez del ~94% (~940k) ahorra ~140k de lectura cacheada,
céntimos. Vender MichiClaude como "te ahorra dinero compactando antes"
sería exagerar. Lo que aporta es el MOMENTO y el aviso.

**Cuidado — la compactación no se puede facturar desde el log.** Su
turno NO lleva `usage`: el resumen se guarda como un mensaje `user` con
`isCompactSummary` y nada más (verificado en el log del 2026-08-08). O
sea que ni MichiClaude ni ninguna herramienta que lea los .jsonl puede
poner un precio a una compactación — solo se ve en la CUOTA, que sí la
mide el endpoint. Si algún día se quiere enseñar ese coste, hay que
decir que es ESTIMADO (contexto × tarifa de caché + resumen × salida) o
callarlo (invariante #8). Hoy se calla.

Lo que sí aporta MichiClaude no es "compactar antes", es **compactar en
un momento elegido**: la suya salta cuando el contexto se llena, que es
casi siempre en mitad de una tarea; la nuestra exige sesión quieta y
avisa con cuenta atrás parable. Ese es el argumento honesto, y es el
único que hay que contar.

Efecto secundario verificado: **un `/compact` inyectado por el relevo se
registra con `trigger:"manual"`** (la CLI lo trata como comando del
usuario). O sea que la regla `acomp` —que solo avisa de las NO
manuales— nunca se avisa a sí misma. Se auditó sobre el log real: 8
`compact_boundary` en la sesión, las 8 manuales.

### El manómetro mentía después de compactar (arreglado el mismo día)

Oscar lo cazó: "aún en MichiClaude aparece alto contexto". No era
retraso, era un fallo. `last_ctx` solo se actualiza cuando llega un
turno nuevo con `usage`, así que entre la compactación y el siguiente
turno —hasta 10 minutos— el manómetro seguía marcando lo de ANTES.

Y no era solo cosmético: con la presión falsa por encima del 85%, el
automático podía disparar un `/compact` redundante sobre una sesión
recién compactada. Ese es el origen del "Error: No messages to compact"
que Oscar había visto días antes sin que le encontráramos la causa.

Arreglo, en Rust y en el exportador (invariante #1): **todo
`compact_boundary` pone `last_ctx = 0`**, lo compactara quien lo
compactara — fuera del `if trigger != manual`, porque el contexto se
vacía igual. Cero significa "sin medida", no "cero tokens": el hit
`press` exige `> 0` y sencillamente no se emite hasta el siguiente
turno real. Es invariante #8 aplicado: antes ningún manómetro que una
cifra que ya es mentira. `ctx_seen` NO se toca — es el máximo histórico
y la evidencia medida del techo real del modelo.

Probado con una sesión sintética que termina justo en la compactación:
antes daba `press 880000` + ficha `compact`; después, nada.

## El interruptor del chat (2026-08-09)

El relevo del chat funcionaba, pero activarlo era pegar UNA línea en el
`settings.json` de máquina del vscode-server — lo hice yo a mano en el
VPS de Oscar. Eso rompía la meta declarada del producto: el usuario no
configura, MichiClaude se configura. Un usuario normal habría tenido el
relevo subido al servidor y sin usarse jamás, sin forma de saberlo.

Ahora es un interruptor en Ajustes ("Relevar el chat de VS Code en los
servidores SSH"), pareja del atajo del PATH. Piezas:

- **`CHAT_WRAP_PY`** (guion embebido en lib.rs): corre EN el servidor
  vía `python3 -` con el guion por STDIN — jamás interpolado en el
  shell, la misma puerta cerrada que `relay_inject_remote`. Toca los
  `data/Machine/settings.json` de `.vscode-server`,
  `.vscode-server-insiders` y `.cursor-server` (los que existan) y
  responde con UNA palabra que el panel traduce (invariante #10).
- **Reglas de respeto, probadas caso por caso** (8 casos en banco):
  un wrapper AJENO no se pisa (OTHER); un archivo que no se entiende no
  se toca (MANUAL); antes de modificar un archivo que no escribimos,
  copia `.michi-backup` una sola vez; al apagar solo se quita NUESTRA
  clave (y el archivo entero solo si quedó vacío); escritura tmp+rename.
- **Fail-open otra vez**: encender con el lanzador ausente dejaría la
  clave apuntando a un archivo inexistente y el chat MUERTO. Por eso
  `set_chat_relay` re-sube `michi-relevo.py` y `michi-wrap.sh` ANTES de
  encender, y el guion además se niega (NOWRAP) si aun así no está.
- **VS Code acepta JSONC**: los comentarios de línea entera se quitan
  antes de parsear (`^\s*//`, que no casa con `https://…` dentro de una
  cadena). El archivo que dejé a mano en el VPS —comentarios en
  español— lo reconoce como propio y lo migra/quita limpio.
- El interruptor solo aparece si hay servidores dados de alta
  (invariante #8: nada de controles que no harían nada), y el estado se
  enseña POR SERVIDOR ("VPS-EU ✓ · otro: sin conexión").

Queda honestamente FUERA: el chat de VS Code contra el Windows LOCAL.
`michi-wrap.sh` es un guion de shell y el relevo de chat es Python —
en Windows no hay ninguno de los dos garantizado. Cubrirlo pide un modo
`wrap` en el michi.exe de Rust (stream-json proxy, factible: es el
mismo protocolo que ya habla el Python). Anotado en pendientes; para
Oscar no cambia nada porque su chat vive en el VPS vía Remote-SSH.

## Por qué la lista blanca se queda en 2 (analizado 2026-08-09)

Oscar preguntó si "los otros comandos" también se aplicarían en
automático, y si los nativos de Claude Code (/doctor y parecidos)
ayudarían. Investigado sobre el binario instalado (v2.1.226).

**No hay 10 comandos del relevo: hay 2.** `RELAY_ALLOWED = ["/compact",
"/clear"]`, idéntica en las tres piezas (main.rs del michi.exe,
michi-relevo.py, lib.rs del panel) y comprobada en LOS DOS lados de
cada inyección. El "10" que recordaba Oscar es otra cosa: el tope
diario de fichas del coach (10) o las ~8 fichas curadas.

De los 2, solo `/compact` se automatiza. `/clear` jamás, ni ganado el
desbloqueo: su descripción oficial es "Start a new session with empty
context" — borra la conversación de la vista. Recuperable con /resume,
pero una máquina no decide por ti cerrar tu sesión. Es el espíritu de
R5 (jamás borrar lo del usuario) aplicado a nivel de sesión.

**Los nativos de medición no entran, y la razón es de producto.** El
binario trae `/usage` ("Show session cost, plan usage, and activity
stats"), `/context` ("Visualize current context usage as a colored
grid") y `/cost` (alias de /usage). Son EXACTAMENTE lo que MichiClaude
ya enseña — pero para verlos tienes que interrumpir la sesión y
escribir un comando, y la respuesta se queda vieja al momento. El
widget lo enseña continuo, fuera de banda y sin gastar un turno. Es
decir: MichiClaude no compite con esos comandos, los VUELVE
INNECESARIOS. Inyectarlos sería imprimirle al usuario en su chat algo
que ya tiene flotando en la pantalla.

**`/doctor` no es de esta familia**: "Check the health of your Claude
Code installation" — diagnóstico de instalación (settings, npm/nativo,
permisos). Útil para un humano cuando algo va mal; inyectado no remedia
nada (no libera contexto ni cuota) y su reporte interrumpe en medio del
trabajo. Si algún día MichiClaude detecta una instalación rota, lo
correcto es una FICHA del coach que diga "corre /doctor", no teclearlo.

**Regla para el futuro**: un comando entra a la lista blanca solo si
(a) LIBERA un recurso (contexto, cuota, procesos), (b) no destruye
nada del usuario y (c) su efecto es verificable desde fuera. Hoy solo
/compact cumple las tres; /clear cumple a y c pero no b, por eso existe
pero no se automatiza. Candidato razonable si algún día se automatiza
/clear para ALGUIEN que lo pida: inyectar `/export` antes como red (la
arquitectura lo permite — misma lista en tres sitios), pero hoy es
alcance que nadie ha pedido.

(Al día siguiente, 2026-08-09, Oscar lo pidió con esas palabras. La
sección de abajo es exactamente ese candidato hecho realidad — y la
lista blanca del CANAL sigue siendo de 2: el /export no se puede pedir
desde fuera, lo genera el relevo como parte del /clear con red.)

## El auto-/clear con red (/export verificado) — 2026-08-09

Lo pidió Oscar tras ver el hallazgo de una conversación de 729 turnos:
"quiero que Michi decida el /clear como ya decide el /compact". La regla
(a)(b)(c) de arriba no se relaja — se le CONSTRUYE la (b): /clear no
destruye si antes existe una copia VERIFICADA de la conversación.

**La secuencia** (`handoff` en las tres piezas, invariante #1):

1. La orden llega con la marca `export:true` (solo válida con `/clear`;
   con /compact se ignora — no hay nada que respaldar).
2. El relevo genera ÉL la ruta de la copia
   (`<datos>/handoff/handoff-<pid>-<epoch>.md` en Windows,
   `~/.michiclaude/handoff/` en Linux). **La ruta JAMÁS viaja por el
   canal**: la lista blanca sigue siendo de 2 textos y nadie puede
   dictarle al relevo ni un byte fuera de ella (la puerta que
   relay_inject_remote cerró antes de que existiera, sigue cerrada).
3. Teclea `/export <ruta>` y espera la copia: el archivo EXISTE con
   contenido (esa es la verificación — un hecho del disco, no un texto
   en pantalla) y el REPL se calló (`EXPORT_SETTLE_MS` 1,5 s; en el
   chat el fin de turno es certeza: `result`). Tope `EXPORT_WAIT_MS`
   12 s.
4. Sin copia → `ERR_RELAY_EXPORT` y **CERO /clear** (fail-closed, el
   espíritu de R5: antes perder la limpieza que la conversación).
5. Con copia → re-verifica R1 (¿tecleó durante la espera? el /clear
   pierde, la copia queda) y teclea `/clear`. El acuse lleva la ruta.

**Decisiones que lo sostienen:**

- **La secuencia corre en SU hilo** (Rust y Python): esperar la copia
  tarda segundos y el bucle principal tiene que seguir bombeando
  pantalla y estado — si se bloqueara, el panel daría la sesión por
  muerta a los 15 s. Mientras dura, el relevo se declara ocupado
  (`ERR_RELAY_BUSY`) y no acepta otra orden.
- **`STATE_V` sube a 2** y es la compuerta de compatibilidad: el panel
  NO pide la red a un relevo v1 — la ignoraría y borraría sin copia,
  justo lo que la red existe para impedir. Manual con v1 = /clear a
  secas (lo de siempre); automático con v1 = no se dispara.
- **`/export` a secas abre un MENÚ interactivo** (verificado en el
  binario 2.1.226: "Export conversation — Select export method");
  inyectado sin argumento dejaría el REPL atrapado. Por eso SIEMPRE va
  con ruta, y con ruta escribe directo y responde "Conversation
  exported to:". La verificación nuestra es el archivo, no ese texto.
- **El automático del /clear** (relayAutoCheck) exige TODO lo del
  /compact (interruptor maestro, presión ≥ INTENT_PCT, relevo casado
  inequívoco, widget a la vista, una vez por sesión, cuenta de 15 s
  cancelable) MÁS: interruptor propio `remCfg.relayClear` (nace
  APAGADO), sus 3 manuales de `/clear` ganadas, veredicto **Boundary**
  del clasificador (todos al 100% o commit limpio — en la duda gana
  /compact, que no borra), y relevo v≥2. El manual (botón de la tarjeta
  de intención) también lleva la red cuando el relevo sabe (v2).
- **Las copias caducan a los 90 días** (`HANDOFF_KEEP_DAYS`, limpieza
  al arrancar el relevo). No viajan a ningún sitio: disco local de la
  máquina donde corre la sesión.
- `michi inject /clear --export` (y lo mismo en el .py) valida la
  secuencia sin la app en medio; el acuse enseña la ruta de la copia.

**Validado en banco de PTY real (VPS, 2026-08-09):** terminal 13/13
(regresión /compact intacta; /export ANTES de /clear con copia en disco;
claude sordo → ERR_RELAY_EXPORT y cero /clear) y chat 6/6 (sid casado,
orden de inyecciones, eco de AMBAS visibles en el chat).

**VALIDADO EN VIVO EN WINDOWS (2026-08-09, Oscar):** `cargo check`
limpio, y con una sesión real de Claude Code v2.1.225 bajo relevo:
`aplicado: /clear (copia: …\handoff\handoff-1948-1786286833.md)` con el
`/clear` visible en pantalla y la copia en disco. Tres intentos hicieron
falta y ninguno fue culpa del diseño: el primero destapó el Enter pegado
al texto (autopsia abajo) y el segundo, que el binario ni siquiera se
había recompilado (empate de mtime, bitácora del mismo día).

PENDIENTE menor: ver el `/export` del chat en vivo (en el banco lo
escribe el falso claude; si el real no lo escribiera, el fallo sería el
bueno — ERR_RELAY_EXPORT y nada borrado).

### El Enter NO puede ir pegado al texto (autopsia, 2026-08-09)

La primera prueba en vivo de Oscar devolvió `ERR_RELAY_EXPORT` con la
sesión en verde (`v:2`, `ready:true`, `idle_out:4`). La red hizo lo suyo
—no se borró nada— pero la copia no aparecía y en pantalla no salía ni
un error. Reproducido en el VPS contra Claude Code REAL:

**La línea se quedaba ESCRITA en el prompt, sin ejecutarse.** La TUI de
Claude Code trata el texto y el Enter que llegan en la MISMA ráfaga de
lectura como un PEGADO, y un pegado no se envía solo. Con `/compact`
(9 bytes) colaba; con `/export <ruta>` (~110 bytes) fallaba siempre.

Comprobado con dos sondas idénticas salvo en eso: `"/export <ruta>\r"`
de una vez → nada, la línea en el prompt; texto, pausa de 0,6 s, y `\r`
aparte → `Conversation exported to:` y archivo de 762 bytes en disco.

Arreglo: `type_line()` en las dos piezas de PTY (Rust y Python) escribe
el texto, duerme `ENTER_GAP_MS` (250 ms) y manda el Enter aparte. Se
aplica a TODOS los comandos, no solo al /export: el fallo dependía del
LARGO de la línea y de la velocidad de la máquina, o sea que el
/compact de la 3c-2 estaba vivo de suerte. El modo chat no lo necesita
(ahí un mensaje es una línea JSON, no teclas).

Validado end-to-end contra Claude Code real (relevo Python + sesión de
verdad): `aplicado: /clear (copia: …)` con copia de 912 bytes que
CONTIENE la conversación. Regresión del banco de falso claude: 13/13.

**Lección para la próxima integración con una TUI:** escribir en una PTY
no es "mandar bytes", es imitar a un humano — y un humano no teclea 110
caracteres y el Enter en el mismo instante. Cuando la TUI no reacciona a
algo que se ve escrito en pantalla, sospechar del RITMO antes que del
contenido.

**Residual honesto:** el veredicto Boundary se evalúa al ARMAR la cuenta
atrás; si en esos 15 s el usuario retoma la tarea, el clasificador no se
re-consulta — lo cubren R1-R3 (tecleó → se rechaza) y que la cuenta se
cancela con un toque. Y el título de la sesión exportada lleva el nombre
del archivo con pid+epoch, no el proyecto: a propósito, ni un dato del
usuario en el nombre.
