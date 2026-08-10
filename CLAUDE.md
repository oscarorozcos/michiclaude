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

Widget de bandeja para Windows 11 con **Tauri 2** que mide en tiempo real
el uso de Claude (suscripción):

- **Cuota real del plan** (sesión de 5 h + semanales con buckets por
  modelo) — la misma de claude.ai → Configuración → Uso, compartida
  entre claude.ai, Claude Code e IDEs.
- **Marcador de ritmo** y **proyección de burn rate** ("al 100% en X min").
- **Gasto por proyecto** (equiv. API) y modelo más usado, de los logs
  locales. Nota `spend_only_cc`: los $ son SOLO de Claude Code; claude.ai
  gasta cuota pero no es medible en dinero.
- **Icono de bandeja dinámico** (% de sesión dibujado en canvas).
- **Analizador de fugas** (Hallazgos) y **coach** (Consejos).
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
respaldo WSL y luego `remotes.json` (el PRIMER token vigente gana; viaja
por SSH, solo vive en memoria). NUNCA llamar a la API con token vencido
(provoca 429). `GET https://api.anthropic.com/api/oauth/usage` con
`anthropic-beta: oauth-2025-04-20`. Endpoint NO oficial: el frontend extrae
buckets de forma recursiva y dinámica (`extractBuckets()` busca
`utilization`/`resets_at`) y pinta los que existan. El endpoint NO envía el
plan (verificado con quota_debug.json real). Respuesta cruda a
`quota_debug.json` para diagnóstico.

**B) Detalle local — `get_local_stats` (Rust):** parsea
`~/.claude/projects/**/*.jsonl`. "**/*" incluye
`<sesión>/subagents/agent-*.jsonl` vía `project_jsonls()` (2026-08-04):
Claude Code v2.1.221+ pone ahí los transcripts de subagentes — sin entrar
ahí ni el costo ni el detector los ven.
Dedup por `message.id + requestId` (los duplicados TAMBIÉN cruzan
archivos — la dedup global es imprescindible). Tokens "de trabajo" =
input + output + cache_write; **cache_read excluido** (infla ~100×) salvo
para el coste (a 10% del input). `<synthetic>` fuera. Fuente WSL:
`wsl.exe -l -q` (UTF-16LE) + `\\wsl.localhost\<distro>\{home/*,root}\.claude`,
sufijo `wsl-<distro>`. Incremental: lo más viejo que la ventana ni se abre; de lo reciente se
cachea el PARSEO por tamaño+mtime (`scan_cache.json`), nunca el coste. Agrega por proyecto (ventana 1/7/30,
`by_model`), por modelo, coste hoy/ventana y serie `daily` de 30 días.
Los proyectos remotos llevan el sufijo del nombre del server.

**Precios Y TECHO DE CONTEXTO:** misma tabla y misma cascada (LiteLLM →
models.dev → OpenRouter, caché 24 h en `prices_cache.json`, RESPALDO no
verificación cruzada). `price_for()` cae a la embebida
`price_table()` y `ctx_for()` a `ctx_table()`; las dos deciden por VERSIÓN,
no familia (Opus 4.5+ $5/$25 vs Opus 3/4.0/4.1 $15/$75; Fable/Mythos
$10/$50; caché 1.25x y 0.1x). Modelo sin tarifa → `estimated`, la UI marca "~". Viajan al exportador
por STDIN (`--prices-stdin`). Descarga fallando >1 semana: aviso ⚠ junto
a "costo estimado", no toast. La sección de
Ajustes informa de AMBAS cosas (`ctx_count` = modelos con techo): si una
fuente deja de publicarlo, el número baja a la vista.
`price_key()` unifica PUNTO→GUIÓN entre dígitos (OpenRouter escribe
`claude-opus-4.8`, el resto `claude-opus-4-8`): sin eso la 3.ª fuente
casaba 6 de 14, ocho modelos sin precio ni techo EN SILENCIO. Auditadas:
las 3 coinciden al céntimo; el techo discrepante es sonnet-4-5 (200k
base, beta de 1M).

**C) Remotas (dentro de `get_local_stats`):** `remotes.json` en
`%APPDATA%\com.oscarorozco.michiclaude\`; por fuente, `ssh -o BatchMode=yes
<host> <command>`. `meter-export.py` replica la MISMA agregación
(**AMBOS lados en sincronía** — invariante #1). Fusión: totales sumados,
proyectos etiquetados. SSH falla → se ignora en silencio. El alta sube el
exportador EMBEBIDO (include_str!, saltos a LF) a
`~/.michiclaude/meter-export.py` y lo re-sube al arrancar — editar el .py
en el VPS NO tiene efecto, hay que recompilar. `install_remote(host,python)`
verifica el binario de Python (`verify_python`); sin Python debe fallar con
ERR_NO_PYTHON. El nombre de un servidor se edita con clic en la lista.

## Ventanas

- **Panel** (`main`, 446x660): flyout sin decoraciones, transparente,
  alwaysOnTop, skipTaskbar. Clic en tray abre; se oculta al perder foco
  (salvo drag); ✕ oculta a bandeja; arrastrable del encabezado. Pestañas
  (Principal · Fuentes de datos · Hallazgos · Consejos · Reporte ·
  Ajustes), encabezado+pestañas sticky en `.p-top` (el padding superior
  vive AHÍ, no en `.panel` — si no, rendija al hacer scroll).
  Pie Hoy/Semana solo en Principal. El panel es el ÚNICO que llama al
  endpoint; el tray se actualiza desde su ciclo (`updateTray`).
- **Pastilla** (`pill`, 280x54) + **detalle** (`pcard`, 280x300): cápsula
  de cristal con asa ⠿, gatito como MARCA, "Sesión X%", hueco semanal y
  hueco semanal POR MODELO (si el endpoint no lo reporta, no se pinta).
  Clic en cápsula = desplegar detalle; clic en la MARCA = abrir panel; ⠿
  arrastra (pliega antes); clic derecho oculta. NO robar foco
  (WS_EX_NOACTIVATE). NUNCA llama al endpoint: el panel emite
  `quota:update` y cada ventana pide el último dato con `pill:ready` al
  cargar (toda ventana nueva DEBE emitirlo). El detalle son DOS ventanas;
  `toggle_pill_card()` elige pose (abajo si cabe; si no `body.up`
  invierte). Cabecera del detalle = geometría IDÉNTICA a la cápsula (el
  margen de 6 px las alinea; sin él el halo del box-shadow se corta en
  recto); con el detalle abierto esconde números. Es funcional: el gatito
  abre panel y el asa arrastra vía `drag_pill_from_card`. SIN tooltips. El hover para desplegar se probó y se DEVOLVIÓ a clic —
  no reintroducir. El % en color: acento en "todo bien", ÁMBAR y ROJO se
  conservan. Los tamaños se definen en `ensure_widget_windows`, NO en el
  json. Indicadores: campana roja (hallazgos) y foco ámbar (consejos),
  SVG inline (la CSP no permite fuentes externas).
- **Gatito** (estilo `cat`): 4 ventanas — `cat` (gif + cápsula "Sesión X%"
  + zona `.head`), `card` (globo resumen al hover), `notif` (globo de
  alarma), pastilla oculta. Estados por gravedad (`mascotState()`):
  cat-zzz (`hit:week`) / cat-break (`hit:session`) / cat-fire
  (`ackPending:alarm`) / normal; los banderines `hit:*` los limpia
  `trackResets()` con ventana nueva. Cápsula nace OCULTA (`body.nodata`)
  hasta tener lectura real; sin arte, cae al gif normal.
  Zona `.head` en vars CSS `--hx:50% --hy:52% --hw:37% --hh:36%` (para
  recalibrar, pintar .head de rojo). El HOVER del globo resumen vive SOLO
  en `.head` (salir de la ventana pliega; rozar <300ms lo cancela).
  Laptop y márgenes arrastran. Post-its en la tapa:
  pilita ROJA de hallazgos (`.fstack`, vars `--bx/--by/--bs`, rojo FIJO)
  y pilita TURQUESA del coach (`.tstack`, #128097 para que el número
  blanco dé ~4.7:1, tamaño .95bs, offset 1.8bs). Clic en post-it = panel directo en su pestaña
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
  estilo elegido; al cambiar se DESTRUYE el viejo, ahorra ~115 MB). Sus tamaños se tocan en Rust. Las capabilities
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

- `resets_at` trae JITTER: ventana nueva SIEMPRE con tolerancia
  (`windowChanged`, 10 min sesión / 360 semana), nunca exacta.
- Alarmas de sesión configurables (chips, `alarms`): el aviso se REPITE
  cada 5 min hasta abrir el panel; varios umbrales de golpe → solo el más
  alto. Semanal al 100%: un aviso por ventana. Avisos de restablecimiento
  solo si la anterior llegó al 100% (`hit:*`), con confirmación
  (abrir/enfocar limpia `ackPending:*`). Sin banners en la app. Nunca
  quitar la confirmación.
- 429: espera 5 min respetando Retry-After (backoff rápido solo para
  red; NUNCA reintentar rápido un rate-limit); cuerpo a quota_debug.json;
  cadencia de cuota 3 min (60 s disparaba 429); el gauge conserva el
  último dato bueno 15 min. OJO: muchos arranques seguidos
  (compilar-probar) acaban en 429 de 60 MINUTOS.
- Instancia única (single-instance, registrado primero).
- Si se toca algo que `emitPill()` calcula: `emitPill(...lastPillArgs)`,
  NUNCA parchear un campo suelto de `lastPill` (dejaba el tema viejo).
- Export CSV/JSON: UNA fila por hecho (fecha × proyecto × modelo ×
  origen); BOM en el CSV; campos entre comillas; filas solo al exportar
  (`want_rows`); el ORIGEN lo pone quien lee; sin fila de totales;
  periodo propio (1/7/15/30). Un export es una foto.
- Presupuesto semanal: contra la suma de los últimos 7 días de la serie
  diaria, no contra la ventana elegida.
- Autostart solo release, una vez (marker); si lo apagan, se respeta.

## Analizador de fugas (pestaña Hallazgos)

Diseño en `docs/analizador-fugas.md` — LEERLO antes de tocar. Tres piezas
en sincronía (invariante #1): motor en `meter-export.py`
(`scan_findings`, `--findings`), réplica Rust (`scan_local_findings` +
`get_findings`), pestaña con severidad por costo (rojo ≥$10, ámbar ≥$1 o
MCP), Ignorar persistente (`fndIgnore`) y ventana propia.

**Detectores y umbrales** (constantes; detalle en el doc): reread (≥3
lecturas y ~2k tok — MIDE chars devueltos), inflate (+50k y 10+ turnos),
cachebreak (≥300k reescritos; excluye isSidechain y compactaciones
±120 s), mech (≥5; git/pytest/cargo/npm), subagents (≥50k de sidechain),
hooks_noise (≥15 disparos y ≥10k tok; mira attachments hook_success),
mcp_unused (resta de conjuntos), skills_unused, claudemd (solo 7d+;
identificadores por línea contra el texto crudo, rojo solo si NINGUNA
mención; costo PISO chars/4 × sesiones, NUNCA líneas × turnos) y
claudemdsize (CLAUDE.md > 40k `CLAUDEMD_LOAD_LIMIT`: lo que sobra no se
carga; costo 0, solo 7d+).
Tope 12 por costo en el backend. REGLA: los de "lo instalado" señalan lo
que NO se usa y lo que cuesta cargarlo, nunca si algo usado "gastó de más".

**Orden:** `ts` desc y luego costo. Llevan ts los de sesión
(reread/inflate/cachebreak) y los agregados con actividad
(hooks_noise/subagents/mech); los de estado puro van abajo por costo. En
Python `parse_ts` da datetime — va `int(ts.timestamp())`.

**Subagentes:** sus turnos llevan el sessionId de la MADRE y NO tocan el
estado de sesión (turns/first_cr/last_cr/cr_cost/cb); solo suman a su
tarjeta, y sus tool_use SÍ cuentan. `proj` (carpeta de logs, para casar con
claudemd) y `disp` (cwd real) van SEPARADOS — unificarlos dejaría
claudemd en costo 0 en silencio.

**Tarjetas:** contraíbles con clic (pose en `fndMin`, guard !simFnd;
Ignorar lleva stopPropagation). Primera apertura: enseña lo guardado al
instante con "Analizando…" mientras corre el fresco; se refresca al abrir
la pestaña si tiene >5 min. Precarga a los 15 s.

**Avisos (sin globo):** post-it rojo / campana / contador encienden con
hallazgos NO VISTOS. Pasada ligera 1d compartida `fndPass()`: al NACER UN
RECIBO (cierre local; freno 15 min `fndEventLast`, marcado ANTES) y cada
3 h de respaldo (era 20 h: los nacidos en el VPS no disparan cierre local
y quedaban invisibles un día). "LEÍDO" = CLIC en la tarjeta, estilo
Gmail: abrir la pestaña o el post-it NO marca nada; contador y post-it
descuentan tarjeta por tarjeta al clicarla (plegar/desplegar marca;
Ignorar apaga la suya; restaurar ignorados revive las no leídas). Esto
ENTIERRA la TRAMPA DEL VIGILANTE (4 mordidas): nada nace visto por estar
mirando la pestaña. Los hallazgos NUNCA van al celular
(privacidad ntfy). El interruptor de Ajustes ("Avisarme en el widget")
apaga SOLO el widget; los contadores de pestaña quedan siempre. Para
re-armar en pruebas: borrar fndSeen y fndAutoLast.

## Coach (pestaña Consejos)

Diseño en `docs/consejos-coach.md` — LEERLO antes de tocar. Fichas
curadas (sin IA ni red, `tip_<id>_*` ×8) + motor de sesión activa:
`get_coach` (Rust, incremental por offset, sesiones tocadas en 30 min). Desde 2026-08-05 MULTI-FUENTE: local + WSL + cada
servidor SSH — el exportador replica el motor bajo `--coach` (invariante
#1; estado incremental en `~/.cache/michiclaude/coach_state.json` del
servidor, reconstruible; subagentes fuera, plano como en Rust) y
`get_coach` fusiona poniendo `origin` (vacío = local; el panel lo enseña
en fichas, recibos y pushes). Regla `press` (manómetro): un hit por sesión
con contexto y quieta <10 min (`PRESS_QUIET_MAX`), `value` = tokens de
contexto crudos, campos aditivos `quiet` + señales del clasificador
`topen/ttotal` (último TodoWrite), `cont` (Jaccard % archivos, últimos
10 vs 10 previos del rastro `trail` tope 20) y `gclean` (commit sin
ediciones después); NO es ficha ni aviso — coachPoll la aparta (como
done/ask), elige la más fresca y emitPill la monta como campo `press`
en quota:update (umbrales 60/85). EL TECHO NO ES CONSTANTE: el hit trae
`full` = techo del modelo de esa sesión y `pressFull()/pressPct()` son
el ÚNICO sitio que divide. Sale
de `ctx_for()` (ver Arquitectura; `[1m]` manda y se mira ANTES de
price_key, que lo recorta; en la duda 200k). Y
si lo MEDIDO supera a la tabla, manda lo medido: `ctx_full` sube al
siguiente escalón de `CTX_LADDER` (devolver lo visto a secas dejaría el
manómetro clavado en 100%). Autopsia en la bitácora. Gauge SVG en pastilla y gatito; número+proyecto en pcard y en el globo
del hover. Nunca viaja a ntfy ni al hub. El motor manda
HECHOS crudos: el veredicto Alive/Boundary/Uncertain vive UNA sola vez
en JS (`intentVerdict`, reina = topen>0). Con presión ≥80
(`INTENT_PCT`) coachPoll sintetiza el hit LOCAL `intent` → tarjeta de
intención en Consejos (exenta del tope diario, una por sesión vía
tipSeen, se refresca sin renacer, ✕/"Ahora no" no resucitan): dos
opciones en llano con comando al lado, insignia "Recomendado" solo con
veredicto (unsure = sin insignia), advertencia si hay pendientes, botón
"Copiar comando" → `plugin:clipboard-manager|write_text` invocado
directo (capability `clipboard-manager:allow-write-text`, sin wrapper
npm). Exportador viejo: ignora --coach → cero hits, se
degrada solo (validado en vivo, sondeo ~80 ms). Regla `acomp`:
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
Reglas: ctx≥120k → compact; pausa≥6 min con ctx≥30k → cache; mismo
archivo leído ≥3 → attach; `ask` (tool_use sin tool_result ≥3 min) y
`done` (quieta 5 min, 5+ turnos) son SOLO push, no fichas; `sum` (quieta 10 min) = recibo con
título AI, min/comandos/archivos, `· ~$X` y ⚠ de `coach_leaks()` (kinds
attach/compact/cache; ctx y cache EXCLUYENTES; cerrar con ctx≥30k es fuga
al cierre). Anti-spam: tope diario 10 (`tipDay`, sum EXENTO), una tarjeta viva por
regla, `tipSeen` se marca al ENTRAR al almacén. Almacén `coachCards` (tope 12): ✕, contraer recordado (`min`), leído (`v`)
apaga el aviso sin despachar, caducidad 24 h (TIP_TTL). "LEÍDO" = CLIC en
la tarjeta (regla Gmail, ver Hallazgos); el ✕ además la despacha. Las
vivas (recibos y fichas calientes) van en UNA corriente por `born` desc —
la más reciente arriba—; las frías del catálogo, abajo. PENDIENTE FANTASMA
(blindado): un turno nuevo del hilo principal LIMPIA pending_tool; los
tool_use de subagentes no lo tocan. El nombre del proyecto va RESUELTO desde Rust (`pname`, cwd real). Aviso
en widget: post-it turquesa / foco ámbar, campo `coach` en quota:update,
mismo interruptor. El recibo NO manda push (su push fue el "terminó"). Al depurar "no llegó X": LEER PRIMERO `coach_debug.json` (compuertas por
sesión en cada sondeo) y la bitácora `flowLog` (botón 📜 en dev).
coachHits queda SOLO para el simulador.

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

TERMINADO y verificado. Análisis en `docs/hub-modo-equipo.md` — LEERLO
antes de tocarlo. Cada ciclo sube la foto LOCAL A SECAS (subir lo fusionado haría eco) a
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

Implementado, SIN probar (falta publicar un tag). Comandos propios Rust
(`check_update`/`install_update`/`open_releases`), sin API JS del plugin
(invariante #4). Franja en cabecera + globo persistente. Fallo al instalar → "descárgala a mano" con botón a `RELEASES_URL`,
CONSTANTE en Rust y que jamás sale de un archivo descargado. Llave
pública en tauri.conf.json, privada en secretos del repo y copias de
Oscar (si se pierde: llave nueva + instalar a mano UNA vez). Firma el
workflow. BLOQUEADO: repo PRIVADO → releases dan 404 sin auth.

## Estado / pendientes

FOTO COMPLETA: bitácora §"cierre 2026-08-08/09"; métricas:
presion-y-rendimiento §"Qué queda vivo".

- [ ] HUB + RANGOS DE FECHA: NO hacer hasta que haya una SEGUNDA máquina
      con MichiClaude (hoy no aporta nada). Diseño en
      `docs/hub-modo-equipo.md` §"Rangos de fecha con hub".

- [x] REDISEÑO UX/UI del panel: TERMINADO Y VALIDADO (2026-08-05;
      bitácora §"Ronda de rediseño UX/UI", tag `pre-rediseno-20260805`).
      DECISIONES VIGENTES: tipografía EMBEBIDA (`src/fonts/`, OFL, sin
      CDN — una fuente remota rompería CSP y privacidad); `.sect` es
      TARJETA con fondo; toda tarjeta con fondo propio redefine
      `--txt-mut`/`--txt-dim` en vez de repintar hijos; en filas con
      elementos de dos líneas, FLEX antes que grid; panel a 446 px; nada
      de `color-mix()` (demasiado reciente para WebView2); al MOVER un
      bloque de pestaña, buscar qué inicialización dependía de abrir la
      pestaña vieja. El widget CONSERVA su estética propia — el rediseño
      fue solo del panel; armonizarlo sería otra ronda.
- [ ] VALIDACIÓN PASIVA (con el uso normal): alarmas reales (umbral,
      100%, ventana nueva), camino ntfy completo (con PC apagada) y el
      aviso de hallazgos naciendo natural (fuga nueva, panel cerrado,
      sin re-armar fndSeen). El auto-/compact y el /clear con red ya
      pasaron la suya; falta ver el AUTO-/clear disparándose solo.
- [ ] Updater: repo público + tag v* y probar completo.
- [ ] Capturas del README (Oscar).
- [ ] MÉTRICAS DE RENDIMIENTO Y REPORTE EJECUTIVO (diseño en
      `docs/presion-y-rendimiento.md` — LEERLO antes de tocar). CERRADO
      HASTA DONDE ESTÁ (Oscar, 2026-08-07): fases 1 y 2 hechas; queda
      por si al usarlo falta algo. Qué existe: (a) TURNOS ÚTILES `uturns`
      en LocalStats/proyectos/daily (mensajes HUMANOS: fuera meta,
      sidechain, tool_result, comandos locales e inyecciones `<ide_…`;
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
- [x] REMEDIACIÓN — LAS 4 ETAPAS COMPLETAS Y VALIDADAS EN VIVO
      (2026-08-07/10). Diseño, historia y lo descartado en
      `docs/remediacion.md` — LEERLO antes de tocar nada de esto. Aquí
      solo las REGLAS que no se pueden romper.
      ETAPA 2 (zombies MCP + archivado ≥365d a `%APPDATA%\<app>\archive`):
      firma = arg más largo del MCP stdio de ~/.claude.json y barras a `/`
      al casar; huérfano = padre muerto o PID reciclado; el kill
      re-verifica PID+exe+arranque y su veredicto sale de RE-CONSULTAR el
      PID, nunca de `$?`; PowerShell desde Rust con saltos de línea REALES
      (en una línea muere en el parser); `actions_log.json` (tope 200,
      crudo, el panel traduce); desbloqueo progresivo `remCfg`/`remFirst`
      (zombie ON / archive OFF, primera vez SIEMPRE manual); sondeo
      horario `remPoll`; lo raro deja `rem_debug.json`. SOLO LOCAL.
      ETAPA 3, REGLAS DURAS:
      crate APARTE `relevo/` (la app no gana deps, invariante #4);
      canal por ARCHIVOS `%APPDATA%\<app>\relevo\<pid>.json|.cmd`
      (tmp+rename con `.tmp` sobre el nombre ENTERO); viva = estado
      <15 s; LISTA BLANCA (/compact, /clear) comprobada en LOS DOS
      lados; R2 se INFIERE del silencio de la PTY (en el chat es CERTEZA:
      entra un `user`, sale un `result`). **ConPTY negocia
      `win32-input-mode`** — las teclas llegan como `ESC[…_`; los avisos
      del terminal (foco, cursor) NO son teclas; UNA fuente de verdad
      para "hay texto"; un Enter no limpia hasta ver si Claude REACCIONA;
      JAMÁS escribe lo tecleado. `TitleMark` antepone la marca al título:
      ÚNICA excepción al paso transparente, lee el número OSC ENTERO
      (`ESC]10;` es color), fail-open con tope. Casado sesión↔relevo por `cwd` COMPLETO
      (`scwd` del hit `press`), FAIL-CLOSED ante ambigüedad. DECIDE el
      relevo: `attend()` revuelve R1-R3 al escribir. `relayBusy` impide
      repintar con una cuenta viva. El motivo del rechazo va en línea
      propia, nunca en el botón.
      Desbloqueo en `relayDone` (/compact 2, /clear 3; lo que TECLEAS TÚ
      también cuenta). AUTO-/CLEAR CON RED (doc §El auto-/clear con red):
      solo Boundary + `relayClear` (nace OFF) + 3 manuales + relevo v≥2
      (a un v1 el panel NO pide la red — borraría sin copia). Red =
      `/export <ruta>` GENERADA por el relevo (jamás viaja por el canal),
      copia VERIFICADA en disco (`<datos>/handoff/`, 90 días) o NO hay
      /clear (`ERR_RELAY_EXPORT`); hilo propio; `/export` sin ruta abre
      MENÚ — jamás a secas. **El Enter va SEPARADO del texto** (`type_line`,
      ENTER_GAP_MS 250): juntos, la TUI los toma por PEGADO y la línea se
      queda escrita sin ejecutarse. El AUTOMÁTICO exige widget A LA VISTA —la
      cuenta atrás vive en la cápsula—, dura 15 s, cualquier toque la
      para (en el gatito el manejador va en CAPTURA) y se marca ANTES de
      empezar. Un rechazo del candado es TRANSITORIO: `done` solo tras
      aplicar; un fallo guarda el momento y reintenta a los 10 min. La
      cuenta CIERRA con veredicto ✓/✕: acabar en silencio deja al
      usuario adivinando. ATAJO DEL PATH (`set_relay_alias`): un
      `claude.cmd` en `%APPDATA%\<app>\bin` DELANTE del PATH de usuario
      — resuelve Windows, no el shell (cualquier terminal/editor); NO
      alcanza WSL/SSH ni rutas absolutas. NUNCA deja sin Claude Code
      (con `MICHI_RELEVO` o sin michi.exe ejecuta el real). PATH por
      `SetEnvironmentVariable`, JAMÁS `setx` (trunca a 1024); copia en
      `path_backup.txt` y quita EXACTA su entrada. FAIL-OPEN sin consola:
      michi ejecuta el claude real con `MICHI_RELEVO=0` (sin la marca,
      `cmd /c claude` puede resolver a NUESTRO shim y ciclar) — por ahí
      pasa el chat de la extensión, que no es una terminal.
      Y AUN ASÍ EL CHAT SE RELEVA, por otra puerta:
      `claudeCode.claudeProcessWrapper`
      es enganche OFICIAL y `michi-relevo.py wrap` proxea stream-json —
      `/compact` como línea `user` lo INTERCEPTA la CLI ($0, turno
      `<synthetic>`). Ahí R1 se cumple POR CONSTRUCCIÓN (líneas atómicas)
      y R2 es CERTEZA (`user` entra → `result` sale): más seguro que la
      terminal; casado por `session_id`. `michi-wrap.sh` va en el ajuste,
      con 3 caminos de emergencia. OJO: `--replay-user-messages` NO
      replica comandos: el relevo emite él la línea de replay para que
      la inyección SE VEA (solo al chat; JSONL intacto) CON LA FORMA
      COMPLETA del replay (`replay_line`: sid/uuid/isReplay) — el `user`
      a secas (`user_line`, la del hijo) lo DESCARTA en silencio. Igual
      el banner «michi · relevo activo», uno por conversación y delante
      del PRIMER mensaje (en el init aún no pinta).
      Residual: no vemos el borrador del cuadro.
      ETAPA 4: el relevo del VPS va en PYTHON (`michi-relevo.py`, stdlib
      pty; allí no hay Rust), embebido y re-subido como el exportador
      junto a `michi-wrap.sh`, con las MISMAS constantes/esquema/códigos
      que main.rs. `scan_relays_remote` lee por SSH con UNA conexión por
      servidor (solo en el compás del coach; el sondeo de 5 s conserva
      las remotas) y `relay_inject_remote` escribe el `.cmd` con
      tmp+rename EN el servidor, con el comando por STDIN y jamás
      interpolado en el shell. Casado en dos niveles: `sid` EXACTO (solo
      chat), si no el `cwd`; los dos exigen la MISMA máquina. El
      AUTOMÁTICO cruza a remotas (pasa `origin`). Los dos interruptores
      (chat `CHAT_WRAP_PY`, terminal `TERM_ALIAS_PY`) mandan su guion por
      STDIN: wrapper ajeno NO se pisa, ilegible NO se toca,
      `.michi-backup` antes, y el lanzador se re-sube ANTES de encender
      (clave a archivo inexistente = chat muerto). El alias es una FUNCIÓN
      de bash con marcas (no un shim: solo shells interactivas), fail-open
      en cascada y sin bucle (las funciones no viajan a subprocesos). `michi.exe` YA VIAJA
      en el instalador: `bundle.resources` + `beforeBuildCommand` que
      compila el crate — el workflow NO se toca (invariante #9); cae
      JUNTO al exe (medido). WSL (4d): mismos guiones con transporte
      `wsl.exe -d <distro>`, buzón por `\\wsl.localhost`
      (`relay_inject_fs` sirve al local y a WSL), distros con
      `~/.claude`, `origin`=`wsl-<distro>`, compás lento. DOS MORDIDAS:
      `wsl.exe` NO entrega `$1` a `sh -c` (ssh sí) → nombre y op DENTRO
      del comando contra lista cerrada; y un op desconocido caía en
      APAGAR y contestaba OK (✓ falso) → ahora BADOP.
      `tests/claude-falso.sh` prueba el relevo sin Claude Code.
      4e CHAT DE WINDOWS HECHO Y VALIDADO: `michi.exe wrap`, ajuste
      apuntando a michi.exe A SECAS (la llamada de la extensión se conoce
      por `--input-format`), `attend`/`handoff` por un `Speaker` —la
      terminal TECLEA, el chat MANDA protocolo, y R1-R5 y la red del
      /export NO se duplican—, y el interruptor escribe el settings.json
      por TEXTO (línea sola en su renglón, verificando que siga siendo
      JSON) porque es del usuario. `wrap_debug.txt` porque la extensión
      se come stderr: un enganche que no arranca se ve igual que uno que
      funciona. El aviso sale tras el `result` del PRIMER turno —con el
      mensaje en vuelo la extensión no lo pinta; en Linux va delante y ahí
      sí—. En DEV manda el michi.exe que compila el crate, no la copia que
      `tauri dev` rehace, y el interruptor reconoce sus rutas viejas o se
      encalla en OTHER.
- APUESTA #2 sin arrancar: tarjeta semanal compartible del gatito y
  gamificación ligera. NO hacer: rastrear otras herramientas, BD de
  historial, modo equipo.

## Consumo de recursos (medido en release)

Instalador 5.8 MB · exe 21.7 MB · RAM privada real **276 MB**
(`WorkingSetPrivate`; WorkingSet64 infla a 695). Release NO baja la RAM
(el peso son los ~9 procesos WebView2) y el gatito NO es el culpable:
cada ventana WebView2 tiene piso ~57 MB — de ahí que los pares de widget
se creen y destruyan.

## Retención de logs

Claude Code los borra a los 30 días y el analizador necesita ese
historial: `cleanupPeriodDays: 365` (VPS y Windows).

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

LECTURA DE ARCHIVOS GRANDES: antes de abrir entero uno de miles de
líneas (`index.html` ≈ 137k tokens), buscar con grep la parte y leer
SOLO ese rango — lo leído viaja en cada turno siguiente. Medido: 7
relecturas en una sesión de $96.86.

TRAS `git pull`, COMPROBAR LA HORA DEL BINARIO: si el pull
cae en el MISMO MINUTO que la última compilación, Cargo ve empate de
fechas y NO recompila (`Finished` en 0.1 s, sin `Compiling`) — se ejecuta
el exe viejo y el bug parece no arreglarse. Arreglo: `(Get-Item
src\main.rs).LastWriteTime = Get-Date` y recompilar; `cargo clean -p` NO
basta ("Removed 0 files").

**Simulador** (solo dev, `is_dev`): gatito / avisos / hallazgos.
`simRunning` es la bandera (NO simMascot). NUNCA toca localStorage ni
manda pushes; al parar, `processAcks()` restaura lo real. Pausa `simMin`
(mín. 5 s). Único control sin `t()`.

## Flujo de trabajo del repo

- Remoto: `github.com/oscarorozcos/michiclaude` — **PRIVADO**.
- Windows de Oscar (`C:\Users\oscar\Claude\MichiClaude`) para desarrollo
  y pruebas; VPS un clon espejo (`/opt/projects/michiclaude`). Al mover un
  clon en Windows: `target/` guarda rutas absolutas → `cargo clean`.
- Antes de trabajar en cualquier lado: `git pull`. Al terminar y verificar:
  commit (Conventional Commits en español) y push.
- La parte de negocio del analizador vive FUERA del repo
  (`~/.michiclaude/notas-negocio-analizador.md`): el git se publica.

## Contexto de producto

- Usuario objetivo: suscriptores Pro/Max de Claude Code que quieren saber
  cuánto les queda, cuándo se acaba y qué consume más.
- El coste en $ es NOCIONAL (equiv. API); la UI lo etiqueta así.
- Diferenciadores vs ccusage/claudeusagewin: cuota real + costo por
  proyecto + multi-máquina + gatito. GPL-3.0 con excepción de assets
  Bongo Cat. La confianza es prioridad: transparencia sobre el token y el
  endpoint no oficial.
