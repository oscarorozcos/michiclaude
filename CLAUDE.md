# CLAUDE.md — MichiClaude (antes Claude Code Meter)

Contexto del proyecto para Claude Code. **Léelo completo antes de modificar
nada.** Aquí vive solo lo VIGENTE: reglas, invariantes y pendientes. El
HISTORIAL completo (jornadas, validaciones, bugs con autopsia, decisiones
con su porqué) está en `docs/bitacora.md` — buscar ahí antes de rediscutir
una decisión vieja. Al cerrar una jornada: validaciones nuevas a la
bitácora; aquí solo se actualizan reglas y pendientes. REGLA DURA: este
archivo debe quedar por debajo de 40k caracteres — Claude Code corta lo que
sobre (pasó el 2026-08-04 con 118.8k y dos tercios del archivo sin leerse).

## Qué es esta app

Widget de bandeja para Windows 11 hecho con **Tauri 2** que mide en tiempo
real el uso de Claude (suscripción):

- **Cuota real del plan** (sesión de 5 h + límites semanales con buckets por
  modelo) — la misma de claude.ai → Configuración → Uso. Compartida entre
  claude.ai, Claude Code e IDEs.
- **Marcador de ritmo** y **proyección de burn rate** ("al 100% en X min").
- **Gasto por proyecto** (equivalente API) y modelo más usado, desde los
  logs locales. Nota `spend_only_cc`: los $ son SOLO de Claude Code; lo de
  claude.ai gasta cuota pero no es medible en dinero.
- **Icono de bandeja dinámico** (% de sesión dibujado en canvas).
- **Analizador de fugas de tokens** (pestaña Hallazgos) y **coach**
  (pestaña Consejos) — ver sus secciones.
- **Modo HUB** multi-máquina y **avisos al celular** (ntfy).

## Arquitectura

```
src/index.html          # Frontend completo: HTML+CSS+JS vanilla, un archivo,
                        # sin frameworks, sin bundler, sin deps npm de runtime.
src/pill.html pcard.html cat.html card.html notif.html   # ventanas del widget
src-tauri/src/main.rs   # Entry point (windows_subsystem = "windows")
src-tauri/src/lib.rs    # Backend: comandos, tray, ventanas, Win32
scripts/meter-export.py # Exportador remoto (VPS vía SSH; solo stdlib)
docs/                   # bitacora.md + diseños (leer antes de tocar su área)
.github/workflows/release.yml  # compila y publica instalador en tags v*
```

### Fuentes de datos

**A) Cuota real — `get_quota` (Rust):** token OAuth de
`~/.claude/.credentials.json` (respeta `CLAUDE_CONFIG_DIR`); si venció,
respaldo WSL y luego máquinas de `remotes.json` (el PRIMER token vigente
gana; viaja por SSH, solo vive en memoria). NUNCA llamar a la API con token
vencido (provoca 429). `GET https://api.anthropic.com/api/oauth/usage` con
`anthropic-beta: oauth-2025-04-20`. Endpoint NO oficial: el frontend extrae
buckets de forma recursiva y dinámica (`extractBuckets()` busca
`utilization`/`resets_at`) y pinta los que existan. El endpoint NO envía el
plan (verificado con quota_debug.json real). Respuesta cruda a
`quota_debug.json` para diagnóstico.

**B) Detalle local — `get_local_stats` (Rust):** parsea
`~/.claude/projects/**/*.jsonl`. "**/*" incluye
`<sesión>/subagents/agent-*.jsonl` vía `project_jsonls()` (2026-08-04):
Claude Code moderno (v2.1.221+) pone ahí los transcripts de subagentes —
sin entrar a esa subcarpeta ni el costo ni el detector los ven.
Deduplicación por `message.id + requestId` (los duplicados TAMBIÉN cruzan
archivos — la dedup global es imprescindible). Tokens "de trabajo" = input
+ output + cache_write; **cache_read excluido** (infla ~100×) salvo para el
coste (a 10% del precio de input). `<synthetic>` fuera. Fuente WSL:
`wsl.exe -l -q` (UTF-16LE) + `\\wsl.localhost\<distro>\{home/*,root}\.claude`,
sufijo `wsl-<distro>`. Lectura incremental: archivos más viejos que la
ventana ni se abren; de los recientes se cachea el PARSEO por tamaño+mtime
(`scan_cache.json`), nunca el coste. Agrega por proyecto (ventana 1/7/30,
`by_model`), por modelo, coste hoy/ventana y serie `daily` de 30 días. Los
proyectos remotos llevan el sufijo del nombre que el usuario dio al server.

**Precios Y TECHO DE CONTEXTO:** la misma tabla y la misma cascada
(LiteLLM → models.dev → OpenRouter, caché 24 h en `prices_cache.json`,
RESPALDO no verificación cruzada). `price_for()` cae a la embebida
`price_table()` y `ctx_for()` a `ctx_table()`; las dos deciden por VERSIÓN,
no familia (Opus 4.5+ $5/$25 vs Opus 3/4.0/4.1 $15/$75; Fable/Mythos
$10/$50; caché 1.25x y 0.1x). Modelo sin tarifa → `estimated`, la UI marca
"~". Viajan al exportador por STDIN (`--prices-stdin`). Descarga fallando
>1 semana: aviso ⚠ junto a "costo estimado", no toast. La sección de
Ajustes informa de AMBAS cosas (`ctx_count` = modelos con techo): si una
fuente deja de publicarlo, el número baja a la vista.
`price_key()` unifica PUNTO→GUIÓN entre dígitos: OpenRouter escribe
`claude-opus-4.8` donde el resto y los logs escriben `claude-opus-4-8`, y
sin eso la 3.ª fuente casaba 6 de 14 modelos —ocho vigentes se quedaban sin
precio y sin techo en silencio— (auditoría 2026-08-08: las 3 fuentes
coinciden al céntimo en precios; el único techo discrepante es
sonnet-4-5, 200k de base con beta de 1M).

**C) Remotas (dentro de `get_local_stats`):** `remotes.json` en
`%APPDATA%\com.oscarorozco.michiclaude\`; por fuente, `ssh -o BatchMode=yes
<host> <command>`. `meter-export.py` replica la MISMA agregación
(**mantener AMBOS lados en sincronía** — invariante #1). Fusión: totales
sumados, proyectos etiquetados. SSH falla → se ignora en silencio. El alta
sube el exportador EMBEBIDO (include_str!, saltos normalizados a LF) a
`~/.michiclaude/meter-export.py` y lo re-sube al arrancar — editar el .py
en el VPS NO tiene efecto, hay que recompilar. `install_remote(host,python)`
verifica el binario de Python (`verify_python`); sin Python debe fallar con
ERR_NO_PYTHON. El nombre de un servidor se edita con clic en la lista.

## Ventanas

- **Panel** (`main`, 446x660): flyout sin decoraciones, transparente, alwaysOnTop,
  skipTaskbar. Clic en tray abre; se oculta al perder foco (salvo drag);
  ✕ oculta a bandeja; arrastrable desde el encabezado. 5 pestañas
  (Principal · Fuentes de datos · Hallazgos · Consejos · Ajustes —
  "Preferencias" se renombró a "Ajustes" en ES el 2026-08-05; los otros 7
  idiomas ya decían Settings/Einstellungen/設定… y no cambian),
  encabezado+pestañas sticky en `.p-top` (el padding superior vive AHÍ, no
  en `.panel` — devolverlo abre una rendija transparente al hacer scroll).
  Pie Hoy/Semana solo en Principal. El panel es el ÚNICO que llama al
  endpoint; el tray se actualiza desde su ciclo (`updateTray`).
- **Pastilla** (`pill`, 280x54) + **detalle** (`pcard`, 280x300): cápsula
  de cristal con asa ⠿, sticker del gatito como MARCA, "Sesión X%", hueco
  semanal (calendario) y hueco semanal POR MODELO (destellos, VARIABLE: si
  el endpoint no lo reporta no se pinta). Clic en cápsula = desplegar
  detalle; clic en la MARCA = abrir panel; ⠿ arrastra (pliega antes); clic
  derecho oculta. NO robar foco (WS_EX_NOACTIVATE). NUNCA llama al
  endpoint: el panel emite `quota:update` y cada ventana pide el último
  dato con `pill:ready` al cargar (toda ventana nueva del widget DEBE
  emitirlo). El detalle son DOS ventanas (mostrar/ocultar, parece que
  crece); `toggle_pill_card()` elige pose (hacia abajo si cabe; si no,
  `body.up` invierte). Cabecera del detalle = geometría IDÉNTICA a la
  cápsula (el margen de 6 px en ambas es lo que las alinea; sin él el halo
  del box-shadow se corta en recto);
  con el detalle abierto la cabecera esconde números (CSS del pcard). La
  cabecera es funcional: gatito abre panel, asa arrastra vía
  `drag_pill_from_card`. SIN tooltips nativos en la cápsula. El hover para
  desplegar se probó y se DEVOLVIÓ a clic — no reintroducir. El % en
  color: acento en "todo bien", pero ÁMBAR y ROJO se conservan. Los
  tamaños de estas ventanas se definen en `ensure_widget_windows`, NO en
  el json. Indicadores: campana roja (hallazgos) y foco ámbar (consejos),
  ambos SVG inline (la CSP no permite fuentes externas).
- **Gatito** (estilo `cat`): 4 ventanas — `cat` (gif + cápsula "Sesión X%"
  + zona `.head`), `card` (globo resumen al hover), `notif` (globo de
  alarma), pastilla oculta. Estados por gravedad (`mascotState()`):
  cat-zzz (`hit:week`) / cat-break (`hit:session`) / cat-fire
  (`ackPending:alarm`) / normal; los banderines `hit:*` los limpia
  `trackResets()` con ventana nueva. Cápsula nace OCULTA (`body.nodata`)
  hasta tener lectura real; sin arte de un estado, cae al gif normal. Zona `.head` en vars CSS `--hx:50% --hy:52% --hw:37% --hh:36%`
  (RECALIBRADA 2026-08-04 midiendo el gif: cabeza real x[50%,86.5%]
  y[53%,87.5%]; para recalibrar, pintar .head de rojo). El HOVER del globo resumen vive SOLO en `.head` (la laptop no lo
  despliega; salir de la ventana pliega; rozar <300ms cancela el
  temporizador). Laptop y márgenes arrastran. Post-its en la tapa:
  pilita ROJA de hallazgos (`.fstack`, vars `--bx/--by/--bs`, rojo FIJO
  sin tinte por severidad) y pilita TURQUESA del coach (`.tstack`,
  #128097 profundo para que el número blanco dé ~4.7:1, tamaño .95bs,
  offset 1.8bs). Clic en post-it = panel directo en su pestaña
  (`panel:findings` / `panel:tips`). Gifs 400² transparentes en variantes
  -black/-white por tema, recortados por CSS EN PORCENTAJES — NO editar
  los archivos; `cat-break-black.gif` vino en lienzo distinto y se
  recoloca con `.cat.odd-canvas` (borrarla si se reexporta). Arte del
  gatito y piel de los globos se eligen POR SEPARADO (`catArt`/`catSkin`,
  viajan como `artTheme`/`skinTheme` en el resumen); la cápsula del % va
  con los globos.
- **Globos** (`notif`): REGLA ÚNICA — el globo se queda hasta ✕ o abrir el
  panel, y no vuelve. NADA de auto-cierre por temporizador. Hover lo
  oculta pero NO cuenta como leído (`notif:ready` lo restaura). Un globo a
  la vez; gana el primero de `ACK_KINDS`. SEGUNDA REGLA: cerrar el globo
  NO cambia el dibujo del gatito (refleja el estado REAL; solo la alarma
  lo calma). NINGÚN aviso va a toast de Windows con widget; el toast queda
  SOLO sin widget (y ahí se repite cada 5 min).
  Con la pastilla el globo es POPOVER (`body.cap`): severidad en `--sev`
  (la calcula `balloonMeta()`), fondo OPACO a propósito, cola pequeña;
  sigue al tema del panel. Si notif.html se ve "sin estilo", verificar que
  sigan `*{box-sizing}`, `.box`, `.msg`, `.x` — `body.cap` las ESPECIALIZA. `place_balloon()` ancla al widget (cola
  62% gato / 50% pastilla), pose automática multi-monitor; la punta se
  mete 40 px en el gatito y 8 en la cápsula (`notif_overlap`). Globo y
  detalle de la pastilla NUNCA a la vez (el globo gana y pliega).
- **Capa** (`PillConfig.layer`): top/normal/bottom. `apply_layer()` +
  `reassert_layers()` cada ciclo + `win_taskbar::force_topmost()`
  (SetWindowPos HWND_TOPMOST con SWP_NOACTIVATE — Windows degrada el
  always-on-top y la llamada de Tauri se vuelve no-op). REGLA: widget y
  globos SIEMPRE en la misma capa; el panel no participa. Si el bug de
  hundirse volviera: SetWinEventHook (EVENT_SYSTEM_FOREGROUND).
- **CRÍTICO — ventanas transparentes:** NUNCA redimensionar en vivo
  (`set_size`): WebView2 deja de pintar. Patrón correcto: ventanas de
  tamaño fijo que se muestran/ocultan.
- **Creación en caliente:** `pill`/`pcard`/`cat`/`card` NO están en
  tauri.conf.json — las crea `ensure_widget_windows()` (solo el par del
  estilo elegido; al cambiar de widget se crea el nuevo y se DESTRUYE el
  viejo — ahorra ~115 MB). Sus tamaños se tocan en Rust. Las capabilities
  siguen listando las 6 etiquetas (los permisos van por etiqueta).
- **Tray dinámico:** número a 24 px con contorno de 4 px (legible en barra
  clara/oscura sin detectar tema) + barrita semanal. Con cuota en error:
  "–" gris, nunca datos inventados. El menú lo construye Rust pero el
  panel se lo manda TRADUCIDO vía `set_tray_menu` desde `applyI18n()` —
  todo texto que Rust dibuje llega así. Windows CORTA el tooltip a 128
  chars: si el motivo no cabe, solo la primera frase (`firstSentence`).

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
   VENTANA vive en la cabecera de "gasto por proyecto" desde 2026-08-05
   (es la suma de esa lista y cambia con el selector; en el pie obligaba a
   bajar hasta abajo para ver el efecto del filtro) y el pie queda solo
   con "Hoy"; con ventana de 1 día ese total de la cabecera se OCULTA —
   sería el mismo número que "Hoy" con otro nombre.
9. No tocar `README.md`, `.github/workflows/release.yml` ni
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

- `resets_at` trae JITTER: detección de ventana nueva SIEMPRE con
  tolerancia (`windowChanged`, 10 min sesión / 360 semana), nunca
  comparación exacta (re-disparaba alarmas cada ciclo).
- Alarmas de sesión configurables (chips, localStorage `alarms`): el aviso
  se REPITE cada 5 min hasta abrir el panel; varios umbrales de golpe →
  solo el más alto. Límite semanal al 100%: un aviso por ventana. Avisos
  de restablecimiento solo si la anterior llegó al 100% (`hit:*`); con
  confirmación (abrir/enfocar el panel limpia `ackPending:*`). Sin banners
  dentro de la app. Nunca quitar el mecanismo de confirmación.
- 429: espera 5 min respetando Retry-After (backoff rápido solo para
  errores de red; NUNCA reintentar rápido un rate-limit); cuerpo a
  quota_debug.json; cadencia de cuota 3 min (60 s disparaba 429); el
  gauge conserva el último dato bueno hasta 15 min. OJO: muchos arranques
  seguidos (compilar-probar) acaban en 429 de 60 MINUTOS.
- Instancia única (tauri-plugin-single-instance, registrado primero).
- Si se toca algo que `emitPill()` calcula, se llama
  `emitPill(...lastPillArgs)` — NUNCA parchear un campo suelto de
  `lastPill` (dejaba el tema del ciclo pasado).
- Export CSV/JSON: UNA fila por hecho (fecha × proyecto × modelo ×
  origen); BOM en el CSV; campos entre comillas; filas solo al exportar
  (`want_rows`); el ORIGEN remoto lo pone quien lee; sin fila de totales;
  periodo propio (1/7/15/30). Un export es una foto, no un cierre.
- Presupuesto semanal: se compara contra la suma de los últimos 7 días de
  la serie diaria, no contra la ventana elegida.
- Autostart solo release, una única vez (marker); si el usuario lo apaga,
  se respeta.

## Analizador de fugas (pestaña Hallazgos)

Diseño completo en `docs/analizador-fugas.md` — LEERLO antes de tocar.
Tres piezas en sincronía (invariante #1): motor en `meter-export.py`
(`scan_findings`, `--findings`), réplica Rust (`scan_local_findings` +
`get_findings`, async doble), pestaña con severidad por costo (rojo ≥$10,
ámbar ≥$1 o MCP), Ignorar persistente (`fndIgnore`) y ventana propia.

**Detectores y umbrales** (constantes; el detalle, en el doc): reread (≥3
lecturas y ~2k tok — MIDE chars devueltos, no tamaño de archivo), inflate
(+50k y 10+ turnos), cachebreak (≥300k reescritos; excluye isSidechain y
compactaciones ±120 s), mech (≥5; git/pytest/cargo/npm), subagents (≥50k
tok de sidechain), hooks_noise (≥15 disparos y ≥10k tok; mira attachments
hook_success, no texto), mcp_unused (resta de conjuntos), skills_unused y
claudemd (solo 7d+; identificadores por línea contra el texto crudo, gris
sin identificadores, rojo solo si NINGUNA mención; costo PISO chars/4 ×
sesiones, NUNCA líneas × turnos), y claudemdsize (CLAUDE.md >
CLAUDEMD_LOAD_LIMIT 40k: lo que sobra Claude Code NO lo carga; tarjeta de
estado costo 0, solo 7d+; nos pasó con 118.8k).
Tope 12 por costo en el backend. REGLA: los de "lo instalado" señalan lo
que NO se usa y lo que cuesta cargarlo — nunca califican si algo que sí
se usa "gastó de más".

**Orden:** por `ts` desc (última actividad) y luego costo. Llevan ts los
de sesión (reread/inflate/cachebreak) Y los agregados con actividad
(hooks_noise/subagents/mech); solo los de estado puro (mcp, skills,
claudemd) van abajo por costo. En Python `parse_ts` da datetime — va
`int(ts.timestamp())`.

**Subagentes:** sus turnos llevan el sessionId de la sesión MADRE y NO
tocan el estado de sesión (turns/first_cr/last_cr/cr_cost/cb) — solo
suman a su tarjeta; sus tool_use SÍ cuentan (MCP usado es usado). El
coach queda plano a propósito. `proj` (carpeta de logs, para casar con
claudemd) y `disp` (cwd real, para enseñar) van SEPARADOS — unificarlos
dejaría claudemd en costo 0 en silencio.

**Tarjetas:** contraíbles con clic (pose en `fndMin`, guard !simFnd;
Ignorar lleva stopPropagation). Primera apertura: enseña lo guardado al
instante con "Analizando…" mientras corre el fresco; se refresca al abrir
la pestaña si tiene >5 min. Precarga de fondo a los 15 s.

**Avisos (sin globo — se eliminó 2026-08-04):** post-it rojo / campana /
contador de pestaña encienden cada vez que hay hallazgos NO VISTOS.
Pasada ligera 1d compartida `fndPass()`: al NACER UN RECIBO (cierre de
sesión local; freno 15 min `fndEventLast`, marcado ANTES) y periódica
cada 3 h como respaldo (era 20 h: los nacidos en el VPS no disparan
cierre local y quedaban invisibles un día entero, 2026-08-06). "LEÍDO" =
CLIC en la tarjeta, estilo Gmail (Oscar 2026-08-07): abrir la pestaña o
el post-it NO marca nada; contador y post-it descuentan tarjeta por
tarjeta al clicarla (plegar/desplegar marca; Ignorar apaga la suya;
restaurar ignorados revive las no leídas). Esto ENTERRÓ la TRAMPA DEL
VIGILANTE (4 mordidas): ya no existe "nace vista por estar mirando la
pestaña". Los hallazgos
NUNCA van al celular (privacidad ntfy). El interruptor de Preferencias
("Avisarme en el widget — hallazgos y consejos") apaga SOLO el widget;
los contadores de pestaña quedan siempre. Para re-armar en pruebas:
borrar fndSeen y fndAutoLast.

## Coach (pestaña Consejos)

Diseño en `docs/consejos-coach.md` — LEERLO antes de tocar. Fichas
estáticas curadas (sin IA, sin red, `tip_<id>_*` ×8) + motor de sesión
activa: `get_coach` (Rust, lectura incremental por offset, sesiones
tocadas en 30 min). Desde 2026-08-05 MULTI-FUENTE: local + WSL + cada
servidor SSH — el exportador replica el motor bajo `--coach` (invariante
#1; estado incremental en `~/.cache/michiclaude/coach_state.json` del
servidor, reconstruible; subagentes fuera, plano como en Rust) y
`get_coach` fusiona poniendo `origin` (vacío = local; el panel lo enseña
en fichas, recibos y pushes). Regla `press` (2026-08-07, manómetro de
remediación etapa 1): un hit por sesión con contexto y quieta <10 min
(`PRESS_QUIET_MAX`), `value` = tokens de contexto crudos, campos
aditivos `quiet` (min quieta) + señales del clasificador `topen/ttotal`
(último TodoWrite), `cont` (Jaccard % archivos, últimos 10 vs 10
previos del rastro `trail` tope 20) y `gclean` (git commit sin
ediciones después); NO es ficha ni aviso — coachPoll la aparta
(como done/ask), elige la más fresca y emitPill la monta como campo
`press` en quota:update (umbrales 60/85). EL TECHO NO ES CONSTANTE
(corregido 2026-08-08): el hit trae `full` = techo del modelo de esa
sesión y `pressFull()/pressPct()` son el ÚNICO sitio que divide. Sale
de `ctx_for()` (Rust y exportador en sincronía): tabla DESCARGADA —la
cascada de precios ya trae `max_input_tokens`— y si no, respaldo por
VERSIÓN `ctx_table` (Opus/Sonnet 4.6+ y Fable 1M, resto 200k; `[1m]`
manda y se mira ANTES de price_key, que lo recorta). En la duda 200k. Y
si lo MEDIDO supera a la tabla, manda lo medido: `ctx_full` sube al
siguiente escalón de `CTX_LADDER` (devolver lo visto a secas dejaría el
manómetro clavado en 100%). Autopsia en la bitácora. Gauge SVG en pastilla y gatito; número+proyecto en pcard y
en el globo del hover. Nunca viaja a ntfy ni al hub. El motor manda
HECHOS crudos: el veredicto Alive/Boundary/Uncertain vive UNA sola vez
en JS (`intentVerdict`, reina = topen>0). Con presión ≥80
(`INTENT_PCT`) coachPoll sintetiza el hit LOCAL `intent` → tarjeta de
intención en Consejos (exenta del tope diario, una por sesión vía
tipSeen, se refresca sin renacer, ✕/"Ahora no" no resucitan): dos
opciones en llano con comando al lado, insignia "Recomendado" solo con
veredicto (unsure = sin insignia), advertencia si hay pendientes, botón
"Copiar comando" → `plugin:clipboard-manager|write_text` invocado
directo (dep `tauri-plugin-clipboard-manager`, capability
`clipboard-manager:allow-write-text`, sin wrapper npm). Exportador viejo: ignora --coach → cero hits, se
degrada solo (validado en vivo en el VPS, sondeo ~80 ms). Reglas: ctx≥120k → compact;
pausa≥6 min con ctx≥30k → cache; mismo archivo leído ≥3 → attach; `ask`
(tool_use sin tool_result ≥3 min) y `done` (quieta 5 min, 5+ turnos) son
SOLO push al celular, no fichas; `sum` (quieta 10 min) = recibo con
título AI, min/comandos/archivos, `· ~$X` y ⚠ de `coach_leaks()` (kinds
attach/compact/cache; ctx y cache EXCLUYENTES; cerrar con ctx≥30k es fuga
al cierre). Anti-spam: tope diario 10 (`tipDay`, sum EXENTO), una tarjeta
viva por regla (la nueva reemplaza), `tipSeen` se marca al ENTRAR al
almacén. Almacén `coachCards` (tope 12): ✕, contraer recordado (`min`),
leído (`v`) apaga el aviso sin despachar, caducidad 24 h (TIP_TTL).
"LEÍDO" = CLIC en la tarjeta, estilo Gmail (Oscar 2026-08-07): abrir la
pestaña no marca nada; cada tarjeta descuenta al clicarla (el ✕ además
la despacha). Las tarjetas vivas
(recibos y fichas calientes) se pintan en UNA corriente por `born` desc —
la más reciente arriba; las frías del catálogo abajo. PENDIENTE FANTASMA
(blindado): un turno nuevo del hilo principal LIMPIA pending_tool; los
tool_use de subagentes no lo tocan. El nombre del proyecto va RESUELTO
desde Rust (`pname`, cwd real). Aviso en widget: post-it turquesa /
foco ámbar, campo `coach` en quota:update, mismo interruptor. El recibo
NO manda push propio (su push fue el "terminó"). Al depurar "no llegó
X": LEER PRIMERO `coach_debug.json` (compuertas por sesión en cada
sondeo) y la bitácora `flowLog` (botón 📜 en dev: clic copia, Mayús+clic
vacía). coachHits queda SOLO para el simulador.

## Avisos al celular (ntfy)

Diseño en `docs/avisos-ntfy.md` — LEERLO antes de tocar. Opt-in APAGADO
por defecto; `ntfy_config.json` (topic = CONTRASEÑA del canal, CSPRNG,
"michi-"+12). REGLA DE PRIVACIDAD: por ntfy viajan SOLO porcentajes,
horas de reset, conteos y frases del diccionario — nunca proyectos,
rutas ni dólares (los topics son públicos). El nombre del proyecto es
casilla aparte (`names`, apagada, con advertencia). Rust no redacta
avisos (códigos, invariante #10). Publicación JSON a la raíz (headers no
aguantan UTF-8). Al 100%: aviso inmediato + "ya volvió" PROGRAMADO (header delay +120 s de
colchón) que llega con la PC APAGADA; si el reset no cabe en los 3 días
del servidor público, no se promete. Un push por ventana (notifS/notifW).
El simulador NUNCA manda pushes (guard simRunning). "Canal nuevo"
regenera el topic en dos pasos. ntfy NO viaja en los ajustes compartidos
del hub (esa pantalla promete no guardar contraseñas). Dedup de done/ask:
ntfyDone/ntfyAsked, máx 3 por sondeo. Fallos a ntfy_debug.json sin
bloquear nada.

## Modo HUB (multi-máquina)

TERMINADO y verificado. Análisis en `docs/hub-modo-equipo.md` — LEERLO
antes de tocar código del hub. Cada ciclo sube la foto LOCAL A SECAS
(antes de fusionar — subir lo fusionado haría eco) a
`~/.michiclaude/hosts/<máquina>.json` por SSH; identidad en
`hub_identity.json`, guard por id EN el servidor (código 3 si otro id).
UNA FOTO POR VENTANA (`HUB_WINDOWS` = 1/7/15/30, DEBE coincidir con el
selector del panel); quien lee no puede recortar un resumen ajeno.
`fetch_remote` pasa `--exclude-host <id>` y Rust re-filtra por id
(recibir lo propio = contarlo doble). Nada se descarta por antigüedad.
Config compartida: MANUAL a propósito (dos botones en Fuentes de datos);
al guardar escribe en TODOS los servidores, al traer gana el primero;
los servidores se FUSIONAN por host; NO viajan posición del widget,
identidad, llaves SSH ni ntfy. Traer va en dos
pasos con la fecha de lo guardado.

## Auto-updater

Implementado, SIN probar (falta publicar un tag). Comandos propios Rust
(`check_update`/`install_update`/`open_releases`) — sin API JS del
plugin (invariante #4). Franja en cabecera + globo persistente. Fallo al
instalar → "descárgala a mano" con botón a `RELEASES_URL`, CONSTANTE en
Rust y que jamás sale de un archivo descargado. Llave pública en
tauri.conf.json; la privada en secretos del repo y copias de Oscar (si se
pierde: llave nueva + instalar a mano UNA vez). El workflow ya firma.
BLOQUEADO: el repo es PRIVADO y las releases privadas dan 404 sin auth.

## Estado / pendientes

- [ ] HUB + RANGOS DE FECHA (2026-08-05; NO hacer hasta que Oscar tenga
      una segunda máquina con MichiClaude — hoy no aporta nada).
      Problema: la foto del hub son cuatro TOTALES cocinados (HUB_WINDOWS
      1/7/15/30) y un total no se descompone, así que con rango esas
      máquinas quedan fuera (`hub_skipped`). Solución: que la foto lleve
      el DESGLOSE POR DÍA (fecha × proyecto × modelo), que ya existe —es
      lo del export CSV, `want_rows`/ExportRow—; con eso cualquier rango
      se suma igual que en local. Coste: la foto pasa de pocos KB a
      50-150 KB → subirla SOLO cuando cambie. Tres piezas en sincronía y
      verificar con la prueba de rangos: dos periodos contiguos suman
      exactamente el total. Límite: solo los 30 (o 90) días subidos.

- [x] REDISEÑO UX/UI del panel: TERMINADO Y VALIDADO (2026-08-05, tag de
      respaldo `pre-rediseno-20260805`; el detalle, en la bitácora
      §"Ronda de rediseño UX/UI"). Cayeron además: coach MULTI-FUENTE,
      calendario de rango, filtro de proyectos, "cuándo pasó" en
      hallazgos, Acerca de con versión real y el panel llenando su
      ventana (margen de sombra 1 px, Oscar lo quiere casi invisible;
      scrollbar fina global vía ::-webkit-scrollbar).
      DECISIONES VIGENTES: tipografía EMBEBIDA (`src/fonts/`, OFL, sin
      CDN — una fuente remota rompería CSP y privacidad); `.sect` es
      TARJETA con fondo; toda tarjeta con fondo propio redefine
      `--txt-mut`/`--txt-dim` en vez de repintar hijos; en filas con
      elementos de dos líneas, FLEX antes que grid; panel a 446 px; nada
      de `color-mix()` (demasiado reciente para WebView2); al MOVER un
      bloque de pestaña, buscar qué inicialización dependía de abrir la
      pestaña vieja. El widget CONSERVA su estética propia — el rediseño
      fue solo del panel; armonizarlo sería otra ronda.
- [ ] VALIDACIÓN PASIVA (con el uso normal): alarmas reales (cruzar
      umbral, 100%, ventana nueva por trackResets/windowChanged), camino
      ntfy completo (alarma real, 100%, el programado con PC apagada) y
      el aviso de hallazgos al cierre de sesión SIN re-armar nada (el
      mecanismo se validó el 2026-08-05 re-armando fndSeen; falta verlo
      nacer natural, con una fuga nueva y el panel cerrado).
- [ ] Updater: decidir repo público + publicar tag v* y probar completo.
- [ ] Capturas para el README (las hace Oscar).
- [ ] MÉTRICAS DE RENDIMIENTO Y REPORTE EJECUTIVO (diseño en
      `docs/presion-y-rendimiento.md` — LEERLO antes de tocar). CERRADO
      HASTA DONDE ESTÁ (Oscar, 2026-08-07): fases 1 y 2 implementadas y
      funcionando; queda como pendiente SOLO por si al usarlo falta algo
      o pide ajustes — no hay obra activa. La fase 1 (motor de datos) se
      implementó el 2026-08-06, pendiente de
      `cargo check` en Windows y de validación en vivo. Qué existe ya:
      (a) TURNOS ÚTILES `uturns` en LocalStats/proyectos/daily (mensajes
      HUMANOS: fuera meta, sidechain, tool_result, comandos locales e
      inyecciones `<ide_…`; `is_user_turn` réplica exacta Rust/Python,
      invariante #1; caché v2 se reconstruye solo); 0 turnos = "sin
      datos", NUNCA dividir (invariante #8); un exportador viejo manda 0
      y se degrada honesto. (b) HISTÓRICO DE CUOTA `quota_history.json`
      (90 días, una foto por ciclo, `log_quota`/`get_quota_history`;
      solo lecturas BUENAS, nunca simulador; local, no viaja a hub ni
      ntfy). (c) MARCAS DE ARREGLO (`fndHist`/`fndMarks`, solo hallazgos
      de estado, escaneos ≥7d sin rango; visto ≥3 días + desaparecido ≥2
      = arreglado). FASE 2 pestaña Reporte (`rep_tab`): IMPLEMENTADA
      2026-08-06, pulida 2026-08-07 sobre maqueta de Oscar — chips
      Semana/Mes/Personalizado, héroe EFICIENCIA/VOLUMEN, "¿te duró más
      o menos?", gráfica 4 semanas tokens/$ con detalle (repSel), deltas
      por proyecto, "qué lo encareció". REGLAS VIGENTES (detalle en el
      doc): nunca pintar con uturns=0; mínimo 20 fotos de cuota o
      "juntando datos"; "1M tok ≈ $X" con la tarifa REAL del periodo,
      jamás fija; el $ SIEMPRE pegado a su dato de tokens (.as-money);
      caché POR PERIODO, render PROGRESIVO y re-render al cambiar
      idioma; sin candado de carga (sello `repStamp`). ~72 claves i18n
      ×8. SI SE RETOMA, lo primero: fase 3, export HTML del mockup A.
      También en el doc y sin arrancar: detectores de
      auto-compacts y de pegado masivo. DESCARTADO con porqué en
      el doc: sesión contaminada, score único, modelo local, telemetría
      colectiva (choca con invariante #3).
- [ ] REMEDIACIÓN (diseño en `docs/remediacion.md` — LEERLO antes de
      tocar; ahí está lo que chocaba con invariantes y su porqué, y los
      prompts de maquetas en `docs/prompts-diseno-remediacion.md`).
      Etapas: 1 consejero con intención, 2 automático out-of-band, 3
      relevo ConPTY `michi claude`, 4 relevo en WSL/SSH. ETAPA 1
      COMPLETA Y VALIDADA EN VIVO (2026-08-07). ETAPA 2 IMPLEMENTADA y
      VALIDADA EN VIVO el mismo día (con el go explícito de Oscar —
      matar procesos era decisión suya; autopsia y receta del zombie de
      laboratorio en la bitácora): zombies MCP por
      PowerShell/CIM sin deps nuevas (firma = arg más largo de cada MCP
      stdio de ~/.claude.json; huérfano = padre muerto o PID de padre
      reciclado; kill re-verifica PID+exe+arranque), archivado ≥365d a
      `%APPDATA%\<app>\archive`, registro `actions_log.json` (tope 200,
      d1/d2 crudos y el panel traduce), desbloqueo progresivo
      (`remCfg`/`remFirst`: zombie ON / archive OFF, primera vez SIEMPRE
      manual), sección en Ajustes + tarjeta de zombies en Consejos
      (clave zombie|arranque-más-nuevo) y sondeo horario `remPoll`. SOLO
      LOCAL: WSL/SSH quedan para la etapa 4. De ahí: barras
      normalizadas a `/` al casar firmas, y todo script de PowerShell
      escrito desde Rust con saltos de línea REALES (en una línea muere
      en el parser: hacía fallar TODO cierre); el veredicto del kill
      sale de re-consultar el PID, nunca de `$?`, y lo raro deja
      `rem_debug.json`. ETAPA 3 partida en 3a/3b/3c; **3a COMPLETA Y
      VALIDADA EN VIVO 2026-08-08** (seis pruebas; autopsia de los tres
      fallos en la bitácora): crate APARTE `relevo/` (paquete `michi`,
      FUERA de src-tauri — la app no gana deps, invariante #4), ConPTY
      transparente, canal por ARCHIVOS en
      `%APPDATA%\<app>\relevo\<pid>.json|.cmd` (tmp+rename con `.tmp`
      sobre el nombre ENTERO, si no estado y orden se pisan), viva =
      estado <15 s, LISTA BLANCA de dos textos (/compact, /clear) como
      límite duro, R2 INFERIDA del silencio de la PTY. REGLAS DURAS:
      **ConPTY negocia `win32-input-mode` a espaldas del que está en
      medio** — las teclas llegan como `ESC[Vk;Sc;Uc;Kd;Cs;Rc_`, hay que
      decodificarlas o no se cuenta ni una; los avisos del terminal
      (foco, cursor) NO son teclas; UNA fuente de verdad para "hay
      texto" (derivada del buffer); un Enter no limpia hasta ver si
      Claude REACCIONA. JAMÁS escribe lo tecleado — `michi status
      --debug` son CUENTAS; con eso y `michi inject` se valida sin panel. **3b IMPLEMENTADA 2026-08-08** (falta `cargo check` y
      validación en vivo): `get_relays` (async, lee la carpeta del
      relevo, misma regla de frescura, borra los de >24 h) y el CASADO
      sesión↔relevo por el `cwd` COMPLETO — para eso el hit `press` lleva
      el campo aditivo `scwd` (replicado en el exportador, invariante #1).
      FAIL-CLOSED: con dos relevos en la misma carpeta no se afirma nada,
      y dos sesiones en la misma carpeta (una con relevo, otra sin) es
      límite asumido — la hora de arranque no sirve, `--resume` la rompe.
      Se ve en Ajustes → Remediación (proyecto · pid · % de la sesión
      casada · listo/motivo; sondeo 5 s SOLO con esa pestaña visible) y
      como insignia "relevo" en la tarjeta de intención. El % de la fila
      ES la prueba visible del casado. **3c-1
      COMPLETA Y VALIDADA EN VIVO 2026-08-08** (manual): `relay_inject`
      escribe la orden y espera acuse 8 s; lista blanca en LOS DOS
      lados; countdown de
      5 s donde el propio botón es el de parar; `relayBusy` impide
      repintar con una cuenta viva (si no, el botón se va y la orden se
      aplica a ciegas); botón en la tarjeta de intención y en Ajustes
      (ahí SOLO /compact — `/clear` necesita su contexto); todo al
      registro (`kind:"relay"`); desbloqueo en
      `relayDone` (/compact 2, /clear 3). El motivo del rechazo va en
      línea propia, NUNCA dentro del botón (lo estiraba fuera del
      panel). ATAJO DEL PATH
      (`set_relay_alias`): un `claude.cmd` en `%APPDATA%\<app>\bin`
      DELANTE del PATH de usuario — resuelve Windows, no el shell, así
      que vale para cualquier terminal/editor sin integrarlos uno a uno;
      NO alcanza WSL/SSH ni rutas absolutas. El shim NUNCA puede dejarte
      sin Claude Code: con `MICHI_RELEVO` puesto o sin michi.exe ejecuta
      el real (ruta resuelta al instalar). PATH por
      `[Environment]::SetEnvironmentVariable`, JAMÁS `setx` (trunca a
      1024); copia en `path_backup.txt` y el interruptor quita EXACTA su
      entrada. Falta 3c-2, el AUTOMÁTICO: el countdown va a la cápsula
      del widget (un clic lo para) y con el widget oculto NO actúa. Y
      `michi.exe` debe viajar en el instalador (workflow, invariante #9).
- APUESTA #2 pendiente de arrancar: tarjeta semanal compartible del
  gatito (marketing) y gamificación ligera. NO hacer: rastrear otras
  herramientas, base de datos de historial, modo equipo/empresa.

## Consumo de recursos (medido en release)

Instalador 5.8 MB · exe 21.7 MB · RAM privada real **276 MB**
(`WorkingSetPrivate`; sumar WorkingSet64 cuenta doble lo compartido y da
695). Release NO baja la RAM (el peso son los ~9 procesos WebView2); el
gatito NO es el culpable (dos veces lo pareció); cada ventana WebView2
tiene piso ~57 MB — por eso los pares de widget se crean/destruyen al
cambiar de estilo.

## Retención de logs

Claude Code borra los .jsonl a los 30 días y el analizador necesita
historial: `cleanupPeriodDays: 365` en VPS y Windows (2026-07-29).

## Comandos

```powershell
npm install        # CLI de Tauri (solo devDependency)
npm run icons      # regenera iconos desde app-icon.png
npm run dev        # desarrollo
npm run build      # release: NSIS en src-tauri/target/release/bundle/nsis/
cd src-tauri; cargo check   # verificación rápida del backend
cd relevo; cargo build --release   # el relevo (michi.exe), crate aparte
```

Verificación obligatoria al terminar cualquier cambio en Rust: `cargo
check` limpio y listar archivos tocados con motivo. En el VPS NO hay
toolchain de Rust (espejo de código; `cargo check` corre en el Windows de
Oscar) — al cambiar la FIRMA de una función, grep de TODOS sus usos antes
de subir: el compilador no está para avisar.

**Simulador** (solo dev, `is_dev`): "🐱 Simular estados" (gatito) / "🔔
Simular avisos" (pastilla, uno por severidad) / "🧪 Simular hallazgos"
(tarjetas+post-its y fichas+resumen). `simRunning` es la bandera (NO
simMascot). NUNCA tocan localStorage ni mandan pushes; al parar,
`processAcks()` restaura lo real. Pausa `simMin` (mínimo 5 s). Único
control sin `t()`.

## Flujo de trabajo del repo

- Remoto: `https://github.com/oscarorozcos/michiclaude` — **PRIVADO**.
- Desarrollo y pruebas en el Windows de Oscar
  (`C:\Users\oscar\Claude\MichiClaude`); en el VPS un clon espejo
  (`/opt/projects/michiclaude`). Al mover un clon en Windows: `target/`
  guarda rutas absolutas → `cargo clean`.
- Antes de trabajar en cualquier lado: `git pull`. Al terminar y
  verificar: commit (Conventional Commits en español) y push.
- La parte de negocio del analizador vive FUERA del repo
  (`~/.michiclaude/notas-negocio-analizador.md`): el historial de git se
  publica con el repo.

## Contexto de producto

- Usuario objetivo: suscriptores Pro/Max de Claude Code que quieren saber
  cuánto les queda, cuándo se les acaba al ritmo actual y qué proyecto/
  modelo consume más.
- El coste en $ es NOCIONAL (equiv. API) para suscriptores; la UI lo
  etiqueta así.
- Diferenciadores vs ccusage/claudeusagewin: cuota real + costo por
  proyecto + multi-máquina + gatito (nadie tiene mascota). GPL-3.0 con
  excepción de assets Bongo Cat; releases automáticas por tag. La
  confianza es prioridad: transparencia total sobre el token y el
  endpoint no oficial.
