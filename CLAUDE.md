# CLAUDE.md — MichiClaude (antes Claude Code Meter)

Contexto del proyecto para Claude Code. **Léelo completo antes de modificar
nada.** Aquí vive solo lo VIGENTE: reglas, invariantes y pendientes; el
HISTORIAL (jornadas, validaciones, bugs con autopsia, decisiones con su
porqué) está en `docs/bitacora.md` — buscar ahí antes de rediscutir una
decisión vieja; al cerrar la jornada, entrada con SU PLANTILLA (cabecera
del archivo). `docs/README.md` = índice de docs + "dónde mirar cuando
algo falla" (rastro por área). REGLA DURA: este archivo bajo 40k —
Claude Code corta lo que sobre (pasó con 118.8k: dos tercios sin leerse).

## Qué es esta app

Widget de bandeja para Windows 11 (**Tauri 2**) que mide en tiempo real el
uso de Claude por suscripción:

- **Cuota real del plan** (sesión de 5 h + semanales con buckets por
  modelo) — la de claude.ai → Configuración → Uso, compartida entre
  claude.ai, Claude Code e IDEs.
- **Marcador de ritmo** y **burn rate** ("al 100% en X min").
- **Gasto por proyecto** (equiv. API) y modelo más usado, de los logs
  locales. Nota `spend_only_cc`: los $ son SOLO de Claude Code; claude.ai gasta
  cuota pero no es medible en dinero.
- **Icono de bandeja** dinámico (% de sesión dibujado en canvas).
- **Analizador de fugas** (Hallazgos) y **coach** (Consejos).
- **Modo HUB** multi-máquina y **avisos al celular** (ntfy).

## Arquitectura

```
src/index.html          # Frontend completo: HTML+CSS+JS vanilla, un archivo
src/pill.html pcard.html cat.html card.html notif.html   # ventanas del widget
src-tauri/src/main.rs   # Entry point (windows_subsystem = "windows")
src-tauri/src/lib.rs    # Backend: comandos, tray, ventanas, Win32
scripts/meter-export.py # Exportador remoto (VPS vía SSH; solo stdlib)
docs/                   # README.md (índice) + bitacora.md + diseños + img/
.github/workflows/release.yml  # compila y publica instalador en tags v*
```

### Fuentes de datos

**A) Cuota real — `get_quota` (Rust):** token OAuth de
`~/.claude/.credentials.json` (respeta `CLAUDE_CONFIG_DIR`); si venció,
respaldo WSL y luego `remotes.json` (el PRIMER vigente gana; viaja por
SSH, solo vive en memoria). NUNCA llamar a la API con token vencido
(429). `GET https://api.anthropic.com/api/oauth/usage` con
`anthropic-beta: oauth-2025-04-20`. Endpoint NO oficial: el frontend extrae
buckets recursiva y dinámicamente (`extractBuckets()` busca
`utilization`/`resets_at`) y pinta los que existan. El endpoint NO envía el
plan (verificado con quota_debug.json real). Respuesta cruda a
`quota_debug.json` para diagnóstico.

**B) Detalle local — `get_local_stats` (Rust):** parsea
`~/.claude/projects/**/*.jsonl`. "**/*" incluye
`<sesión>/subagents/agent-*.jsonl` vía `project_jsonls()` (2026-08-04):
Claude Code v2.1.221+ pone ahí los transcripts de subagentes — sin entrar
ahí ni el costo ni el detector los ven.
Dedup por `message.id + requestId` (los duplicados TAMBIÉN cruzan
archivos: la dedup global es imprescindible). Tokens "de trabajo" =
input + output + cache_write; **cache_read excluido** (infla ~100×) salvo
para el coste (a 10% del input). `<synthetic>` fuera. Fuente WSL:
`wsl.exe -l -q` (UTF-16LE) + `\\wsl.localhost\<distro>\{home/*,root}\.claude`,
sufijo `wsl-<distro>`. Incremental: lo más viejo que la ventana ni se abre; de lo reciente
cachea el PARSEO por tamaño+mtime (`scan_cache.json`), nunca el coste. Agrega por proyecto (ventana 1/7/30,
`by_model`), por modelo, coste hoy/ventana y serie `daily` de 30 días.
Los proyectos remotos llevan el sufijo del nombre del server.

**Precios Y TECHO DE CONTEXTO:** misma tabla y cascada (LiteLLM →
models.dev → OpenRouter, caché 24 h en `prices_cache.json`, RESPALDO, no
verificación cruzada). `price_for()` cae a la embebida
`price_table()` y `ctx_for()` a `ctx_table()`; las dos deciden por VERSIÓN,
no familia (Opus 4.5+ $5/$25 vs Opus 3/4.0/4.1 $15/$75; Fable/Mythos
$10/$50; caché 1.25x y 0.1x). Modelo sin tarifa → `estimated`, la UI marca "~". Viajan al exportador
por STDIN (`--prices-stdin`). Descarga fallando >1 semana: aviso ⚠ junto
a "costo estimado", no toast. La sección de Ajustes informa de AMBAS
cosas (`ctx_count` = modelos con techo): si una fuente deja de publicarlo,
el número baja a la vista.
`price_key()` unifica PUNTO→GUIÓN entre dígitos (OpenRouter escribe
`claude-opus-4.8`, el resto `claude-opus-4-8`): sin eso la 3.ª fuente
casaba 6 de 14, ocho modelos sin precio ni techo EN SILENCIO. Auditadas
coinciden al céntimo; el techo discrepante es sonnet-4-5 (200k / 1M beta).

**C) Remotas (dentro de `get_local_stats`):** `remotes.json` en
`%APPDATA%\com.oscarorozco.michiclaude\`; por fuente, `ssh -o BatchMode=yes
<host> <command>`. `meter-export.py` replica la MISMA agregación
(**AMBOS lados en sincronía** — invariante #1). Fusión: totales sumados,
proyectos etiquetados. SSH falla → se ignora en silencio. El alta sube el exportador
EMBEBIDO (include_str!, saltos a LF) a `~/.michiclaude/meter-export.py` y
lo re-sube al arrancar: editar el .py en el VPS NO tiene efecto, hay que
recompilar. `install_remote(host,python)`
verifica el binario de Python (`verify_python`); sin Python debe fallar con
ERR_NO_PYTHON. El nombre de un servidor se edita con clic en la lista.

## Ventanas

- **Panel** (`main`, 446x660): sin decoraciones, transparente,
  alwaysOnTop, skipTaskbar. Clic en tray abre/enfoca; SOLO el ✕ (y el
  menú del tray) ocultan — NO se cierra al perder foco (Oscar 2026-08-14;
  antes era flyout y estorbaba). Se arrastra del encabezado. SIN borde
  perimetral ni rendija (Oscar 2026-08-14): body padding 0 y `.panel`
  sin outline/ring — la "línea de la orilla" era el propio borde
  --stroke; el panel es solo fondo. Pestañas
  (Principal · Fuentes de datos · Hallazgos · Consejos · Reporte ·
  Ajustes), con encabezado sticky en `.p-top` (el padding superior vive
  AHÍ, no en `.panel`: si no, rendija al scroll).
  Pie Hoy/Semana solo en Principal. El panel es el ÚNICO que llama al endpoint;
  el tray se actualiza desde su ciclo (`updateTray`).
- **Pastilla** (`pill`, 280x54) + **detalle** (`pcard`, 280x300): cápsula
  de cristal con asa ⠿, gatito como MARCA, "Sesión X%" y huecos semanales,
  global y POR MODELO (si el endpoint no los reporta, no se pintan).
  Clic en cápsula = desplegar detalle; clic en la MARCA = abrir panel; ⠿
  arrastra (pliega antes); clic derecho oculta. NO roba foco
  (WS_EX_NOACTIVATE). NUNCA llama al endpoint: el panel emite
  `quota:update` y cada ventana pide el último dato con `pill:ready` al
  cargar (toda ventana nueva DEBE emitirlo). El detalle son DOS ventanas y
  `toggle_pill_card()` elige pose (abajo si cabe; si no, `body.up` invierte). Cabecera del detalle = geometría IDÉNTICA a la cápsula (el margen de 6 px
  las alinea; sin él el halo del box-shadow se corta en recto); con el
  detalle abierto esconde números. Es funcional: el gatito abre
  panel y el asa arrastra vía `drag_pill_from_card`. SIN tooltips. El hover
  para desplegar se probó y se DEVOLVIÓ a clic: no reintroducir. El % en color: acento en "todo bien", ÁMBAR y ROJO se
  conservan. Los tamaños se definen en `ensure_widget_windows`, NO en el
  json. Indicadores: campana roja (hallazgos) y foco ámbar (consejos), SVG inline
  (la CSP no permite fuentes externas).
- **Gatito** (estilo `cat`, 210x157): 4 ventanas — `cat` (gif + BOMBILLA +
  cápsula "Sesión X%" + zona `.head`), `card` (globo resumen al hover),
  `notif` (globo de alarma), pastilla oculta. COLUMNA: gato → BOMBILLA
  (4 niveles animados; sin hit `press` no se pinta) → cápsula, que sube con
  `body.hasidea` y vuelve a su sitio sin ella. Hover en la bombilla = ficha de
  contexto en la MISMA ventana (`body.showtip`); el globo del resumen ya no la
  lleva. El gato cuelga de `.stage`, anclado abajo.
  Estados por gravedad (`mascotState()`):
  cat-zzz (`hit:week`) / cat-break (`hit:session`) / cat-fire
  (`ackPending:alarm`) / normal; los banderines `hit:*` los limpia
  `trackResets()` con ventana nueva. Cápsula nace OCULTA (`body.nodata`)
  hasta tener lectura real; sin arte, cae al gif normal.
  Zona `.head` en vars CSS (para recalibrar, pintarla de rojo). El HOVER del globo resumen vive SOLO
  en `.head` (salir de la ventana pliega; rozar <300ms lo cancela).
  Laptop y márgenes arrastran. Post-its en la tapa:
  pilita ROJA de hallazgos (`.fstack`, vars `--bx/--by/--bs`, rojo FIJO)
  y pilita TURQUESA del coach (`.tstack`, #128097 para que el número
  blanco dé ~4.7:1). Clic en post-it = panel directo en su pestaña
  (`panel:findings` / `panel:tips`). Gifs 400² transparentes en variantes
  -black/-white por tema, recortados por CSS EN PORCENTAJES — NO editar
  los archivos; `cat-break-black.gif` vino en lienzo distinto y se
  recoloca con `.cat.odd-canvas` (borrarla si se reexporta). Arte del
  gatito y piel de los globos se eligen POR SEPARADO (`catArt`/`catSkin`,
  viajan como `artTheme`/`skinTheme` en el resumen); la cápsula del % va
  con los globos.
- **Globos** (`notif`): REGLA ÚNICA — se queda hasta ✕ o abrir el panel, y
  no vuelve. NADA de auto-cierre. Hover lo oculta pero NO cuenta como
  leído (`notif:ready` lo restaura). Un globo a la vez;
  gana el primero de `ACK_KINDS`. SEGUNDA REGLA: cerrar el globo
  NO cambia el dibujo del gatito (refleja el estado REAL; solo la alarma
  lo calma). NINGÚN aviso va a toast de Windows con widget; el toast queda
  SOLO sin widget (y ahí se repite cada 5 min). Con la pastilla el globo
  es POPOVER (`body.cap`): severidad en `--sev`, fondo OPACO, cola
  pequeña; sigue al tema del panel. Si notif.html se ve "sin estilo",
  verificar `*{box-sizing}`, `.box`, `.msg`, `.x` — `body.cap` las
  ESPECIALIZA. `place_balloon()` ancla al widget (cola 62% gato / 50%
  pastilla), pose automática multi-monitor; la punta se mete 40 px en el
  gatito y 8 en la cápsula (`notif_overlap`). Globo y detalle de la
  pastilla NUNCA a la vez (el globo gana y pliega).
- **Capa** (`PillConfig.layer`): top/normal/bottom. `apply_layer()` +
  `reassert_layers()` cada ciclo + `win_taskbar::force_topmost()`
  (SetWindowPos HWND_TOPMOST con SWP_NOACTIVATE — Windows degrada el
  always-on-top y la llamada de Tauri es no-op). REGLA: widget y
  globos SIEMPRE en la misma capa; el panel no participa. Si el bug de
  hundirse volviera: SetWinEventHook (EVENT_SYSTEM_FOREGROUND).
- **CRÍTICO — ventanas transparentes:** NUNCA redimensionar en vivo
  (`set_size`): WebView2 deja de pintar. Tamaño fijo que se muestra u oculta.
- **Creación en caliente:** `pill`/`pcard`/`cat`/`card` NO están en
  tauri.conf.json — las crea `ensure_widget_windows()` (solo el par del
  estilo elegido; al cambiar se DESTRUYE el viejo, ahorra ~115 MB). Las capabilities
  siguen con las 6 etiquetas (los permisos van por etiqueta).
- **Tray dinámico:** número a 24 px con contorno de 4 px (legible en barra
  clara u oscura sin detectar el tema) + barrita semanal. Con cuota en error:
  "–" gris, nunca datos inventados. El menú lo construye Rust pero el
  panel se lo manda TRADUCIDO vía `set_tray_menu` desde `applyI18n()` —
  todo texto que Rust dibuje llega así. Windows CORTA el tooltip a 128 chars:
  si no cabe, solo la primera frase (`firstSentence`).

## INVARIANTES — no romper nunca

1. `get_quota` y `get_local_stats`: no cambiar firmas (`days: Option<u32>`,
   clamp 1..90); no eliminar dedup ni exclusión de cache_read. Campo nuevo
   en LocalStats → replicar en `meter-export.py` y `#[serde(default)]`
   (ExportRow.origin y Finding.ts ya mordieron por esto). AMPLIACIÓN
   ADITIVA 2026-08-05: `get_local_stats` acepta además `end: Option<i64>`
   (epoch) y el exportador `--end EPOCH` — mueven el FINAL de la ventana
   al pasado, que es como se sirve un rango de fechas: [end-days, end].
   Sin ese argumento todo se comporta EXACTAMENTE igual que antes
   (verificado con regresión byte a byte). NO añadir un camino paralelo
   por fechas: el motor solo entiende ancho + final.
2. `demo()` del frontend existe SOLO para abrir index.html suelto en un
   navegador. PROHIBIDO que datos de demo lleguen a la app real.
3. Seguridad: el token nunca se loggea/muestra/viaja a otro dominio que
   api.anthropic.com. CSP restrictiva. Sin telemetría. MATIZ OBLIGATORIO:
   `security` lleva `"dangerousDisableAssetCspModification": ["style-src"]`
   y NO se puede quitar sin romper la app COMPILADA: Tauri inyecta nonces
   al compilar y el estándar CSP ignora `'unsafe-inline'` cuando hay
   nonce — en release se bloqueaban todos los estilos al vuelo (barras,
   tendencia, globos) mientras dev se veía perfecto. `script-src` intacto.
   AL DIAGNOSTICAR: si algo se ve bien con `npm run dev` y mal con `npm
   run build`, sospechar de la CSP ANTES que del código.
4. Frontend vanilla: sin frameworks, bundlers ni deps npm de runtime.
   Deps Rust nuevas: solo imprescindibles, features mínimas.
5. Porcentajes SIEMPRE redondeados a entero en UI (`Math.round`).
6. Buckets de cuota: render dinámico, nunca hardcodear modelos. Lo mismo
   `prettyModel()`: separa familia y números sin listas fijas — NO volver
   a un regex con familias ni exigir versión de dos dígitos.
7. Tag del plan: el que reporte el endpoint; si no viene, "Suscripción".
   No inventar "MAX 5×".
8. NUNCA poner una cifra donde no se puede calcular. La fila "claude.ai /
   otros" se ELIMINÓ (el desglose no es calculable: gasto local en $ y
   cuota en %); en su lugar la nota `spend_only_cc`. El total de la
   VENTANA vive en la cabecera de "gasto por proyecto" (suma de esa
   lista, cambia con el selector); el pie queda solo con "Hoy"; con
   ventana de 1 día el total de cabecera se OCULTA (= "Hoy").
9. No tocar `README*.md`, `.github/workflows/release.yml` ni
   `app-icon.png` salvo petición explícita. (El token de este entorno no
   puede tocar workflows — eso lo hace Oscar desde la web.)
10. UI multiidioma: diccionario `I18N` (8 idiomas, EN default,
    autodetección, persistido). Todo texto visible pasa por `t()`. El
    backend devuelve códigos `ERR_*` que el frontend traduce.
10bis. `[hidden]{display:none !important}` en index.html: NO quitarlo
    (cualquier regla con display lo anula en silencio; sin él, el
    simulador de dev se veía en RELEASE).
10ter. Todo comando Rust que ESPERE (SSH, red, disco largo) es `async fn`
    + `spawn_blocking` (síncrono congela el panel: test_remote,
    install_remote, get_local_stats, export_data, save/load_hub_config,
    get_findings, get_coach). NO envolver los que tocan ventanas — PERO
    todo comando que CREE ventanas tiene que ser async (`set_pill_style`:
    crear ventana desde comando síncrono congela la app entera — el bucle
    de eventos y el comando se esperan mutuamente; en `setup()` sí puede
    ser síncrono).
11. Tema claro/oscuro: variables CSS + override `body.light`, toggle ◐
    persistido. `color-scheme` en body para controles nativos. Texto
    SOBRE el acento usa `--accent-ink` (tinta oscura en tema oscuro,
    blanco en claro) — nunca blanco fijo sobre acento claro (~2:1).

## Reglas de comportamiento — no regresionar

- `resets_at` trae JITTER: ventana nueva SIEMPRE con tolerancia
  (`windowChanged`, 10 min sesión / 360 semana).
- Alarmas de sesión configurables (chips, `alarms`): el aviso se REPITE
  cada 5 min hasta abrir el panel; varios umbrales de golpe → solo el más
  alto. Semanal al 100%: uno por ventana. Restablecimiento solo si la
  anterior llegó al 100% (`hit:*`), con confirmación (abrir/enfocar limpia
  `ackPending:*`). Sin banners. NUNCA quitar la confirmación.
- 429: espera 5 min respetando Retry-After (backoff rápido solo para
  red; NUNCA reintentar rápido un rate-limit); cuerpo a quota_debug.json;
  cadencia de cuota 3 min (60 s disparaba 429); el gauge conserva el
  último dato bueno 15 min. Muchos arranques seguidos = 429 de 60 MIN.
- Instancia única.
- Tocar algo que `emitPill()` calcula: `emitPill(...lastPillArgs)`, NUNCA
  parchear un campo suelto de `lastPill` (dejaba el tema viejo).
- Export CSV/JSON: UNA fila por hecho (fecha × proyecto × modelo ×
  origen); BOM en CSV; campos entre comillas; filas solo al exportar
  (`want_rows`); el ORIGEN lo pone quien lee; sin totales; periodo propio
  (1/7/15/30). Un export es una foto.
- Presupuesto semanal: contra la suma de los últimos 7 días de la serie
  diaria, no la ventana elegida.
- Autostart solo en release, una vez (marker); apagado se respeta.

## Analizador de fugas (pestaña Hallazgos)

Diseño en `docs/analizador-fugas.md` — LEERLO antes de tocar. Tres piezas
en sincronía (invariante #1): motor en `meter-export.py`
(`scan_findings`, `--findings`), réplica Rust (`scan_local_findings` +
`get_findings`), pestaña con severidad por costo (rojo ≥$10, ámbar ≥$1 o
MCP), Ignorar persistente (`fndIgnore`) y ventana propia.

**Detectores y umbrales** (constantes; detalle en el doc): reread (≥3
lecturas del MISMO archivo+RANGO, ~2k tok — MIDE chars devueltos), inflate (+50k, 10+ turnos),
cachebreak (≥300k reescritos; fuera isSidechain y compactaciones ±120 s),
mech (≥5; git/pytest/cargo/npm), subagents (≥50k de sidechain),
acompact (≥3 auto-compacts POR PROYECTO; trigger≠manual, dedup uuid;
costo PISO preTokens×input "~" — sin usage no se factura; NO entra al
waste), paste (≥3 mensajes humanos ≥5k chars y ≥10k tok por proyecto;
piso chars/4; base user_turn_text; NO waste),
hooks_noise (≥15 disparos, ≥10k tok; attachments hook_success),
mcp_unused (resta de conjuntos), skills_unused, claudemd (solo 7d+;
identificadores por línea contra texto crudo, rojo solo si NINGUNA
mención; costo PISO chars/4 × sesiones, NUNCA líneas × turnos) y
claudemdsize (>40k `CLAUDEMD_LOAD_LIMIT`: lo que sobra no se carga;
costo 0, solo 7d+).
Tope 12 por costo en backend. REGLA: los de "lo instalado" señalan lo que
NO se usa y su costo de carga, nunca si algo usado "gastó de más".

**Orden:** `ts` desc, luego costo. Llevan ts los de sesión
(reread/inflate/cachebreak) y los agregados con actividad
(hooks_noise/subagents/mech); los de estado puro abajo por costo. En
Python `parse_ts` da datetime — va `int(ts.timestamp())`.

**Subagentes:** sus turnos llevan el sessionId de la MADRE y NO tocan
el estado de sesión (turns/first_cr/last_cr/cr_cost/cb); solo suman a su
tarjeta; sus tool_use SÍ cuentan. `proj` (carpeta de logs, casa con
claudemd) y `disp` (cwd real) SEPARADOS — unificarlos deja claudemd en
costo 0 en silencio.

**Tarjetas:** contraíbles con clic (pose en `fndMin`, guard !simFnd;
Ignorar lleva stopPropagation). Primera apertura: lo guardado al instante
con "Analizando…" mientras corre el fresco; refresco al abrir la pestaña
si tiene >5 min. Precarga a los 15 s.

**Avisos (sin globo):** post-it rojo / campana / contador encienden con
hallazgos NO VISTOS. Pasada ligera 1d compartida `fndPass()`: al NACER UN
RECIBO (cierre local; freno 15 min `fndEventLast`, marcado ANTES) y cada
3 h de respaldo (20 h era mucho para los nacidos en el VPS). "LEÍDO" =
CLIC en la tarjeta, estilo Gmail: abrir pestaña o post-it NO marca;
contador y post-it descuentan al clicar cada tarjeta (plegar/desplegar
marca; Ignorar apaga la suya; restaurar ignorados revive las no leídas).
TRAMPA DEL VIGILANTE (4 mordidas): nada nace visto por mirar la pestaña.
Hallazgos NUNCA al celular (privacidad ntfy). El interruptor de Ajustes
apaga SOLO el widget; los contadores de pestaña quedan. Re-armar en
pruebas: borrar fndSeen y fndAutoLast.

## Coach (pestaña Consejos)

Diseño en `docs/consejos-coach.md` — LEERLO antes de tocar. Fichas
curadas (sin IA ni red, `tip_<id>_*` ×8) + motor de sesión activa:
`get_coach` (Rust, incremental por offset, sesiones tocadas en 30 min). Desde 2026-08-05 MULTI-FUENTE: local + WSL + cada
servidor SSH — el exportador replica el motor bajo `--coach` (invariante
#1; estado incremental en `~/.cache/michiclaude/coach_state.json` del
servidor, reconstruible; subagentes fuera, plano como en Rust) y
`get_coach` fusiona poniendo `origin` (vacío = local; el panel lo enseña
en fichas, recibos y pushes). Regla `press` (manómetro): un hit por sesión con
contexto y quieta <10 min (`PRESS_QUIET_MAX`), `value` = tokens de
contexto crudos, campos aditivos `quiet` + señales del clasificador
`topen/ttotal` (último TodoWrite), `cont` (Jaccard % archivos, últimos 10 vs 10
previos del rastro `trail` tope 20) y `gclean` (commit sin ediciones
después); NO es ficha ni aviso: coachPoll la aparta (como
done/ask), elige la más fresca y emitPill la monta como `press` en
quota:update (umbrales 60/85). EL TECHO NO ES CONSTANTE: el hit trae
`full` = techo del modelo de esa sesión; `pressFull()/pressPct()` son el
ÚNICO sitio que divide. Sale de `ctx_for()` (ver Arquitectura; `[1m]` manda y se mira ANTES de
price_key, que lo recorta; en la duda 200k). Y
si lo MEDIDO supera a la tabla, manda lo medido: `ctx_full` sube al
siguiente escalón de `CTX_LADDER` (devolver lo visto a secas dejaría el
manómetro clavado en 100%). Autopsia en la bitácora. Arco en la pastilla y BOMBILLA en el gatito;
número+proyecto en pcard y en la ficha de la bombilla. Nunca viaja a ntfy ni al hub. El motor manda HECHOS crudos: el veredicto
Alive/Boundary/Uncertain vive UNA vez en JS (`intentVerdict`, reina =
topen>0). Con presión ≥80 (`INTENT_PCT`)
coachPoll sintetiza el hit LOCAL `intent` → tarjeta de intención en
Consejos (exenta del tope diario, una por sesión vía tipSeen, se refresca
sin renacer, ✕/"Ahora no" no resucitan): dos
opciones en llano con comando al lado, insignia "Recomendado" solo con
veredicto (unsure = sin insignia), advertencia si hay pendientes, botón
"Copiar comando" → `plugin:clipboard-manager|write_text` invocado
directo (capability `clipboard-manager:allow-write-text`, sin wrapper
npm). Exportador viejo: ignora --coach → cero hits, se
degrada solo (validado en vivo, sondeo ~80 ms). ANÁLISIS LOCAL (IA),
`docs/analisis-local.md` — LEERLO: con veredicto unsure, `ai_intent`
(llama-server BAJO DEMANDA en 127.0.0.1, gramática por
`response_format`, se MATA al terminar) pinta insignia PROPIA punteada; JAMÁS toca compuertas del
automático; evidencia = `title`+`msgs` del press (3 mensajes humanos ×300
chars; `user_turn_text` = ÚNICO filtro, el bool lo envuelve, réplica en
exportador); `msgs` NO se persiste (solo `c.ai`); UNA invocación por
sesión aunque falle; AUTOMÁTICO POR INFERENCIA: `relayClearAi` (OFF, bajo
relayClear) = 2.ª razón del auto-/clear (`unsure`+`tema_nuevo`, `topen==0`,
30 s), resto IGUAL, red incluida, espera el veredicto; fail-quiet; interruptor nace OFF; Probar = la misma
tubería. ETAPA 2 HECHA (2026-08-13):
peldaño de EMBEDDINGS (`ai_emb_verdict`, EmbeddingGemma-300M q8_0
~319 MB, GGUF OFICIAL ggml-org — los e5 comunitarios están ROTOS, banco
en bitácora) ANTES del 2B — coseno tema↔reciente SIN prefijos (calibrado:
separan mejor), <0.45 clear·tema_nuevo / >0.65 compact·tema_cruzado /
banda media al 2B; fail-quiet total (sin GGUF = v1 exacta) con rastro
PROPIO `emb_debug.txt` + `emb_server.log`; `via`/`sim` al flowLog y al
botón Probar, tarjeta solo {rec,reason}. DESCARGA GUIADA `ai_setup`: URLs y SHA-256 en 9 CONSTANTES
(original + ESPEJO `modelos-v1` por archivo — PRERELEASE y tag sin `v`,
o rompe updater/workflow; `ai_fetch` cae al espejo por fallo de red O de
huella; REEMPLAZAR un binario = constantes juntas + release `modelos-v2`;
AÑADIR un asset nuevo al v1 está bien, detalle en el doc); única conexión
fuera de api.anthropic.com, opt-in y anunciada; respeta rutas manuales. Regla `acomp`:
`compact_boundary` con trigger≠manual y <30 min → ficha con los preTokens
(los manuales no avisan: los hiciste tú; los INYECTADOS por el relevo
entran como manual y se auditan solos). TODO `compact_boundary` —de quien
sea— pone `last_ctx = 0`: el contexto se vació y hasta el próximo turno no
hay medida (`press` exige >0 y no sale, invariante #8); sin eso el
manómetro mentía 10 min y el automático inyectaba un /compact redundante.
`ctx_seen` intacto. La auto-compactación de Claude Code (~94% de su
ventana) NO se toca ni se sugiere apagar: es la red cuando MichiClaude no
está, y apagarla desactiva su `precomputeCompactionEnabled`. Entramos al
80% (`INTENT_PCT`): se gana por diseño, no por carrera. Y la compactación
NO lleva `usage`: no se puede facturar, solo se ve en cuota.
Reglas: ctx ≥60% del techo (`COACH_CTX_PCT`×`ctx_full`, antes 120k
fijos; el ⚠ "ctx" de `coach_leaks` usa el MISMO umbral) → compact;
pausa≥6 min con ctx≥30k → cache; mismo
archivo+RANGO leído ≥3 → attach (SOLO texto; imágenes → `shots` ≥10, ficha
propia; ambos hits llevan `file`, la línea "Ahora:" dice QUÉ leyó Claude —
2026-08-15); `ask` (tool_use sin tool_result ≥3 min) y
`done` (quieta 5 min, 5+ turnos) son SOLO push, no fichas; `sum` (quieta 10 min) = recibo con
título AI, min/comandos/archivos, `· ~$X` y ⚠ de `coach_leaks()` (kinds
attach/compact/cache; ctx y cache EXCLUYENTES; cerrar con ctx≥30k es fuga
al cierre). Anti-spam: tope diario 10 (`tipDay`, sum EXENTO), una tarjeta viva por
regla, `tipSeen` se marca al ENTRAR al almacén. La ficha CALIENTE se REFRESCA
cada sondeo sin renacer (misma sesión, conserva born/min/v) y lleva `ts`
("medido hace X min" si la regla calla); `sum`/`acomp` NO: son fotos. Almacén `coachCards` (tope 12): ✕, contraer recordado (`min`), leído (`v`)
apaga el aviso sin despachar, caducidad 24 h (TIP_TTL). "LEÍDO" = CLIC en
la tarjeta (regla Gmail, ver Hallazgos); el ✕ además la despacha. Las
vivas (recibos y fichas calientes) van en UNA corriente por `born` desc —
la más reciente arriba—; las frías del catálogo, abajo. PENDIENTE FANTASMA
(blindado): un turno nuevo del hilo principal LIMPIA pending_tool; los
tool_use de subagentes no lo tocan. El nombre del proyecto va RESUELTO desde Rust (`pname`, cwd real). Aviso
en widget: post-it turquesa / foco ámbar, campo `coach` en quota:update,
mismo interruptor. El recibo NO manda push (su push fue el "terminó"). Depurar "no llegó X": PRIMERO `coach_debug.json` y la bitácora `flowLog`
(📜 en dev). coachHits queda SOLO
para el simulador. COMPÁS ADAPTATIVO (2026-08-13): `coachPoll` se
auto-agenda (`coachSched`) — 3 min en reposo, 60 s con sesión activa,
20 s ≥55%, 10 s ≥70% o salto ≥15k tok entre sondeos (rampa) — porque el
fijo de 3 min perdía las rampas (pico de 197k invisible, autopsia en
bitácora). NO tocar la cadencia de CUOTA (3 min, 429): el coach no habla
con la API. Y `relayAutoCheck` exige `rly.ready` ANTES de arrancar la
cuenta: con Claude generando, el rechazo quemaba el reintento de 10 min
y el auto-compact del ~94% ganaba la carrera.

## Avisos al celular (ntfy)

Diseño en `docs/avisos-ntfy.md` — LEERLO antes de tocar. Opt-in APAGADO;
`ntfy_config.json` (topic = CONTRASEÑA del canal, CSPRNG, "michi-"+12). REGLA DE PRIVACIDAD: por ntfy viajan SOLO porcentajes,
horas de reset, conteos y frases del diccionario — nunca proyectos,
rutas ni dólares (los topics son públicos). El nombre del proyecto es
casilla aparte (`names`, apagada, con advertencia). Rust no redacta
avisos (códigos, invariante #10). Publicación JSON a la raíz (headers no
aguantan UTF-8). Al 100%: aviso inmediato + "ya volvió" PROGRAMADO (header delay +120 s)
que llega con la PC APAGADA; si el reset no cabe en los 3 días del
servidor público, no se promete. Un push por
ventana (notifS/notifW). El simulador NUNCA manda pushes (guard
simRunning). "Canal nuevo" regenera el topic en dos pasos. ntfy NO viaja
en los ajustes compartidos del hub (esa pantalla promete no guardar
contraseñas). Dedup done/ask: ntfyDone/ntfyAsked, máx 3 por sondeo.
Fallos a ntfy_debug.json sin bloquear nada.

## Modo HUB (multi-máquina)

TERMINADO. Análisis en `docs/hub-modo-equipo.md` — LEERLO antes de
tocarlo. Cada ciclo sube la foto LOCAL A SECAS (subir lo fusionado haría eco) a
`~/.michiclaude/hosts/<máquina>.json` por SSH; identidad en
`hub_identity.json`, guard por id EN el servidor (código 3 si otro id). UNA FOTO POR VENTANA (`HUB_WINDOWS` = 1/7/15/30, DEBE
coincidir con el selector); quien lee no puede recortar un resumen
ajeno. `fetch_remote` pasa `--exclude-host <id>` y Rust re-filtra
(recibir lo propio = contarlo doble). Nada se descarta por antigüedad.
Config compartida: MANUAL a propósito (dos botones en Fuentes de datos);
al guardar escribe en TODOS los servidores, al traer gana el primero;
los servidores se FUSIONAN por host; NO viajan posición del widget,
identidad, llaves SSH ni ntfy. Traer va en dos pasos con su fecha.

## Auto-updater

PROBADO DE PUNTA A PUNTA (2026-08-12; autopsias de los 3 releases en la
bitácora). Comandos propios Rust (`check_update`/`install_update`/
`open_releases`), sin API JS del plugin (inv. #4). Check al arrancar (8 s)
y cada 12 h; guarda `v===updVer` (el globo cerrado NO vuelve). REGLAS:
`createUpdaterArtifacts: true` OBLIGATORIO (sin él no hay .sig ni
latest.json); iconos COMMITEADOS; al re-etiquetar borrar release+tag y el
tag SIEMPRE tras `git pull`. Fallo al instalar → botón a `RELEASES_URL`
(constante Rust). Llave pública en tauri.conf.json, privada en secretos +
copias de Oscar (si se pierde: llave nueva + instalar a mano UNA vez).

## Estado / pendientes

FOTO COMPLETA: bitácora §"cierre 2026-08-08/09"; métricas:
presion-y-rendimiento §"Qué queda vivo".

- [ ] HUB + RANGOS DE FECHA: NO sin una SEGUNDA máquina con MichiClaude
      (`docs/hub-modo-equipo.md` §"Rangos de fecha").

- [x] REDISEÑO UX/UI del panel: HECHO Y VALIDADO (2026-08-05; bitácora
      §"Ronda de rediseño UX/UI", tag `pre-rediseno-20260805`). VIGENTE:
      tipografía EMBEBIDA (`src/fonts/`, OFL, sin CDN — una fuente remota
      rompe CSP y privacidad); `.sect` es TARJETA con fondo; toda tarjeta
      con fondo redefine `--txt-mut`/`--txt-dim`, no repinta hijos; filas
      con elementos de dos líneas: FLEX antes que grid; panel a 446 px;
      nada de `color-mix()` (WebView2); al MOVER un bloque de pestaña,
      buscar qué init dependía de abrir la vieja. El widget conserva su
      estética propia (armonizarlo sería otra ronda).
- [ ] VALIDACIÓN PASIVA (con el uso): alarmas reales (umbral, 100%,
      ventana nueva), camino ntfy completo (PC apagada) y el aviso de
      hallazgos naciendo natural. CERRADO el bloque del relevo
      (2026-08-17: auto-/compact y auto-/clear 4/4, globo y registro
      post-/clear; el chat copia ÉL el jsonl — remediacion.md).
- [x] PURGA DEL ARCHIVO (2026-08-15; reglas COMPLETAS en remediacion.md
      §"Purga del archivo" — LEERLO; aquí solo lo que no se puede
      olvidar): el archivador MUEVE (≥365d), la purga BORRA
      solo lo archivado, allowlist canónica (JAMÁS `~/.claude`) y el VPS
      SOLO INFORMA (`--du`) — nunca se borra por SSH.
- [ ] ANÁLISIS LOCAL: v1 validada en vivo (5/5 tema_nuevo) y ETAPA 2
      OPERATIVA (2026-08-13, ver §Coach y analisis-local.md §"Etapa 2 —
      HECHA"): EmbeddingGemma descargado y Probar en el Windows de Oscar
      dio "embeddings 0.36" — el número CLAVADO con el banco del VPS.
      EN VALIDACIÓN PASIVA con el uso diario (Oscar, desde 2026-08-13):
      falta el primer `via:emb` en sesión REAL al 80% y la muestra
      natural antes de afinar umbrales (EMB_NEW/EMB_CROSS constantes a
      propósito); cualquier rareza de clear/compact que Oscar vea, se
      revisa con flowLog + emb_debug.txt.
      ETAPA 3 DISEÑADA (2026-08-16, analisis-local.md §"Etapa 3" —
      LEERLO antes de tocar): TEMAS sobre `inflate` — embeddings (nunca
      el 2B) parten la sesión en tramos y CALCULAN el ahorro por
      frontera; capa ADITIVA sobre el hallazgo (fndKey intacto,
      fail-quiet); local/WSL primero, VPS manda `umsgs` por SSH y el
      Windows embebe (el modelo no va al VPS). APARCADA tras el ruteo.
- [ ] MÉTRICAS Y REPORTE EJECUTIVO (`docs/presion-y-rendimiento.md` —
      LEERLO antes de tocar). CERRADO HASTA DONDE ESTÁ (Oscar,
      2026-08-07): fases 1 y 2 hechas. FILA 18 (% de desperdicio
      estructural) HECHA 2026-08-14: `waste` en las 3 piezas (§fórmula
      del doc), tarjeta en Reporte; cargo check limpio (2026-08-16). Qué existe: (a) TURNOS ÚTILES `uturns`
      en LocalStats/proyectos/daily (mensajes HUMANOS: fuera meta,
      sidechain, tool_result, comandos locales, inyecciones `<ide_…` y
      resúmenes de compactación isCompactSummary — 2026-08-14, caché v3;
      `is_user_turn` réplica exacta Rust/Python, invariante #1); 0 turnos
      = "sin datos", NUNCA dividir (invariante #8). (b) HISTÓRICO DE
      CUOTA `quota_history.json` (90 días, una foto por ciclo; solo
      lecturas BUENAS, nunca simulador; local, no viaja). (c) MARCAS DE
      ARREGLO (`fndHist`/`fndMarks`, solo hallazgos de estado; visto ≥3
      días + desaparecido ≥2 = arreglado). FASE 2 = pestaña Reporte
      (`rep_tab`). REGLAS: nunca pintar con uturns=0; mínimo 20 fotos de
      cuota o "juntando datos"; "1M tok ≈ $X" con la tarifa REAL del
      periodo, jamás fija; el $ pegado a su dato de tokens (.as-money);
      caché POR PERIODO y render PROGRESIVO. Fase 3 y lo DESCARTADO, en
      el doc.
- [x] REMEDIACIÓN — 4 ETAPAS COMPLETAS Y VALIDADAS EN VIVO (2026-08-07/10):
      zombies+archivado, relevo (terminal/chat/SSH/WSL), automáticos.
      TODAS las reglas duras viven en `docs/remediacion.md` §"REGLAS
      VIGENTES" — LEERLO ANTES de tocar cualquier cosa del relevo, los
      automáticos o la remediación; aquí solo lo transversal:
      crate APARTE `relevo/` (la app no gana deps); LISTA BLANCA
      (/compact, /clear + `/model <alias>` de lista cerrada, 2026-08-17 —
      remediacion.md §"La ÚNICA ampliación") en LOS DOS lados; /clear
      automático SOLO con copia `/export` VERIFICADA en disco
      (fail-closed) y por dos razones — Boundary (hecho) o análisis
      local `tema_nuevo` (inferencia, `relayClearAi` OFF, `topen==0`);
      cuenta atrás 15 s (30 por inferencia) que DICE el comando, widget
      A LA VISTA, una vez por sesión, cualquier toque para; el AUTOMÁTICO
      espera el veredicto del análisis (`aiPending`); michi.exe viaja en
      el instalador SIN tocar el workflow (invariante #9).
- [ ] RUTEO INTELIGENTE (etapas 0-6; TODAS las reglas vigentes en
      `docs/ruteo-inteligente.md` §10-11 — LEERLO antes de tocar).
      ETAPAS 0-5 HECHAS (0-2 validadas en vivo VPS+Windows; 3-5 el
      2026-08-17, la 5 ADELANTADA por Oscar: es el error caro). Piezas:
      nota `router_state.json` (local+WSL+SSH; apagado = no se escribe;
      >10 min = ausente; lleva `guard`/`ctx`/`esc`); Hook B `router-hook.py/
      .ps1` (exploración→haiku, implementación→sonnet, análisis baja solo
      con cuota ≥70; anota `parent`); Hook A `guard-hook.py/.ps1`
      (UserPromptSubmit: frena prompt pesado en haiku/sonnet ANTES de
      gastar; insistencia pasa; `~` escotilla; JAMÁS el texto al log); UN
      alta para los dos hooks (respaldo+atómico); registro visible en
      Ajustes; medición `scan_ruteo` (Rust+exportador `--ruteo`, réplicas:
      agent-*.jsonl reales × tarifa padre−impuesto; sin casar no se
      factura) en Reporte; consejero `light` (motor del coach, réplicas:
      racha de turnos ligeros que se reinicia con código; compuerta en el
      panel: modelo caro + gauge ≥70 + sin 3 «no») → tarjeta →
      `set_default_model` (SOLO sesiones nuevas, lista cerrada). ESCALAR
      SOLO (5b, `esc`, exige guardián): al frenar, el hook deja `/model
      <peldaño>` al relevo SIN esperar acuse (abrazo mortal medido); el
      relevo espera ≤20 s SOLO para /model; globo del gatito.
      5c `rs` (exige esc): el relevo reenvía el prompt (`then`, jamás
      persistido) — chat: mensaje JSON; terminal: `type_model` (Enter al
      diálogo + RESTAURAR el default que /model guarda) y `type_paste`.
      Validados en vivo. FALTA: cargo check relevo/, .ps1, consejero, WSL.
- APUESTA #2 sin arrancar: tarjeta semanal compartible del gatito. NO:
  rastrear otras herramientas, BD de historial, modo equipo.

## Consumo de recursos (medido en release)

Instalador 5.8 MB · exe 21.7 MB · RAM **276 MB** (~9 procesos WebView2,
~57 MB por ventana: por eso los pares de widget se crean y destruyen).

## Integridad de las fuentes (los .jsonl no son nuestros)

Un limpiador o el usuario los recortan y el panel diría "bajó el
consumo". 4 piezas; diseño y validación en
`docs/adr-multiharness-y-persistencia.md` §"LAS 4 PIEZAS" — LEERLO:
(1) DETECTOR sobre el caché de escaneo (archivo que ENCOGIÓ o
DESAPARECIÓ) → `integrity.json` local, no viaja; réplica en el
exportador (inv. #1), Rust pone el origen; guardas: solo raíces LEÍBLES
(WSL apagado ≠ borrado) y solo si no existe (envejecer ≠ borrarse).
(2) NO CONCLUYENTE: hecho en el tramo comparado o día del cuadernito que
ya no se ve → "no comparable", nunca una mejora. (3)
`daily_history.json` (serie FUSIONADA, 400 d): RESPALDO, NO JEFE — manda
lo vivo, si no un arreglo retroactivo quedaría fosilizado. (4) Las
marcas congelan su "antes" (`m.b`) al nacer. NADA de SQLite (inv. #4).

## Retención de logs

Claude Code borra a los 30 días y el analizador necesita historial:
`cleanupPeriodDays: 365` (VPS y Windows).

## Comandos

```powershell
npm install        # CLI de Tauri (solo devDependency)
npm run icons      # iconos desde app-icon.png
npm run dev        # desarrollo
npm run build      # release: NSIS en target/release/bundle/nsis/
cd src-tauri; cargo check          # verificación rápida del backend
cd relevo; cargo build --release   # el relevo; dev/build ya lo compilan
```

Verificación obligatoria tras cualquier cambio en Rust: `cargo check`
limpio y listar archivos tocados con motivo. En el VPS NO hay
toolchain de Rust (espejo de código; `cargo check` corre en el Windows de
Oscar) — al cambiar la FIRMA de una función, grep de TODOS sus usos antes
de subir: el compilador no está para avisar.

LECTURA DE ARCHIVOS GRANDES: no abrir entero uno de miles de líneas
(`index.html` ≈ 137k tokens): grep y leer SOLO ese rango — lo leído viaja
en cada turno (medido: 7 relecturas, sesión de $96.86).

TRAS `git pull`, LA HORA DEL BINARIO: pull en el MISMO MINUTO que la
última compilación = Cargo ve empate y NO recompila (`Finished` en 0.1 s,
sin `Compiling`) — corre el exe viejo y el bug "no se arregla". Arreglo:
`(Get-Item src\main.rs).LastWriteTime = Get-Date` y recompilar;
`cargo clean -p` NO basta.

**Simulador** (solo dev): gatito / avisos / hallazgos / contexto /
intención. Bandera `simRunning` (NO simMascot). NUNCA toca localStorage ni
manda pushes; al parar, `processAcks()` restaura lo real. Pausa `simMin`
(mín. 5 s). Único control sin `t()`.

## Flujo de trabajo del repo

- Remoto: `github.com/oscarorozcos/michiclaude` — **PÚBLICO**; CLA ligero
  en CONTRIBUTING.md (abrir un PR lo acepta).
- Windows de Oscar (`C:\Users\oscar\Claude\MichiClaude`) desarrolla y
  prueba; el VPS es clon espejo (`/opt/projects/michiclaude`). Al mover un
  clon en Windows: `target/` guarda rutas absolutas → `cargo clean`.
- `git pull` antes de trabajar; al terminar y verificar, commit
  (Conventional Commits en español) y push.
- La parte de negocio del analizador vive FUERA del repo
  (`~/.michiclaude/notas-negocio-analizador.md`): el git se publica.

## Contexto de producto

- Usuario objetivo: suscriptores Pro/Max que quieren saber cuánto les
  queda, cuándo se acaba y qué consume más.
- El coste en $ es NOCIONAL (equiv. API); la UI lo etiqueta así.
- Diferenciadores vs ccusage/claudeusagewin: cuota real + costo por
  proyecto + multi-máquina + gatito. GPL-3.0 con excepción de assets
  Bongo Cat. La confianza es prioridad: transparencia sobre el token y el
  endpoint no oficial.
