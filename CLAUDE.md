# CLAUDE.md — MichiClaude (antes Claude Code Meter)

Contexto del proyecto para Claude Code. **Léelo completo antes de modificar nada.**

## Qué es esta app

Widget de bandeja para Windows 11 hecho con **Tauri 2** que mide en tiempo real
el uso de Claude (suscripción). Muestra:

- **Cuota real del plan** (sesión de 5 h + límites semanales con buckets por
  modelo) — la misma que aparece en claude.ai → Configuración → Uso. Los
  límites son compartidos entre claude.ai, Claude Code e IDEs.
- **Marcador de ritmo**: línea que indica cuánto del periodo transcurrió; si el
  consumo la adelanta, el color pasa a ámbar/rojo.
- **Proyección de burn rate**: "a este ritmo llegas al 100% en X min, antes/después del reset".
- **Gasto por proyecto** (equivalente API, 7 días) y **modelo más usado**,
  desde los logs locales de Claude Code.
- **Nota bajo el gasto**: los dólares son SOLO de Claude Code. Lo usado en
  claude.ai también gasta el límite semanal pero NO se puede medir en $, así
  que se dice con palabras en vez de con una cifra inventada.
- **Icono de bandeja dinámico**: el icono del tray se redibuja con el % de
  sesión (color por estado + barrita semanal), como los medidores de
  batería/CPU. Sustituye a la antigua "franja sobre la barra" (descartada
  2026-07-10: Windows 11 centra los iconos y cualquier overlay encima de la
  barra los tapa y les bloquea los clics).

## Arquitectura

```
src/index.html          # Frontend completo: HTML+CSS+JS vanilla, un solo archivo,
                        # sin frameworks, sin bundler, sin dependencias npm de runtime.
src-tauri/src/main.rs   # Entry point (windows_subsystem = "windows")
src-tauri/src/lib.rs    # Backend: comandos, tray, ventanas, Win32
src-tauri/tauri.conf.json
src-tauri/capabilities/default.json
scripts/meter-export.py  # Exportador remoto (corre en el VPS vía SSH; solo stdlib)
app-icon.png            # Fuente de iconos (npm run icons los genera)
.github/workflows/release.yml  # Compila y publica instalador en tags v*
```

### Fuentes de datos (dos, independientes)

**A) Cuota real — comando `get_quota` (Rust):**
1. Lee el token OAuth de `~/.claude/.credentials.json`
   (campo `claudeAiOauth.accessToken`). Respeta `CLAUDE_CONFIG_DIR` si existe.
   Si falta o su `expiresAt` ya pasó, intenta las máquinas de `remotes.json`
   (`ssh <host> cat ~/.claude/.credentials.json`) y usa el primer token
   vigente — así el meter no depende de usar Claude Code en Windows si se usa
   a diario en el VPS. El token remoto viaja por SSH y solo vive en memoria.
   NUNCA se llama a la API con token vencido (provoca 429 temporales).
2. `GET https://api.anthropic.com/api/oauth/usage` con `Bearer <token>` y
   header `anthropic-beta: oauth-2025-04-20`.
3. Devuelve el JSON crudo. Es un **endpoint no oficial** — puede cambiar de
   forma; por eso el frontend extrae buckets de forma **recursiva y dinámica**
   (`extractBuckets()`: busca objetos con `utilization`/`resets_at`) y pinta
   los que existan (Sonnet, Opus, Fable, Cowork, futuros).
4. Errores esperados y su mensaje: sin `.credentials.json` → "inicia sesión en
   Claude Code"; 401 → "token expirado, ejecuta claude update".
5. Escribe `quota_debug.json` con la respuesta para diagnóstico.

**B) Detalle local — comando `get_local_stats` (Rust):**
1. Parsea `~/.claude/projects/**/*.jsonl` (solo actividad de Claude Code en
   esta máquina; los chats de claude.ai web NO dejan logs locales).
2. **Deduplicación** por `message.id + requestId` (los .jsonl duplican
   entradas por reanudaciones/streaming).
3. Tokens "de trabajo" = input + output + cache_write. **`cache_read` se
   excluye** (infla ~100×) salvo para el cálculo de coste, donde entra a 10%
   del precio de input.
4. Coste equivalente-API con `price_for()`: primero la tabla DESCARGADA
   (cascada LiteLLM → models.dev → OpenRouter, caché de 24 h en
   `prices_cache.json`, apagable en Preferencias) y, si el modelo no está,
   la tabla embebida `price_table()` — que decide por VERSIÓN, no por
   familia (Opus 4.5+ $5/$25 vs Opus 3/4.0/4.1 $15/$75; Fable/Mythos
   $10/$50; caché = 1.25x y 0.1x del input). Un modelo que no esté en
   ninguna de las dos se marca `estimated` y la UI lo señala con "~".
5. Agrega: por proyecto (ventana 1/7/30 días, con desglose `by_model` por
   proyecto), por modelo, coste hoy y de la ventana (`cost_week`), y la serie
   `daily` de los últimos 30 días para la gráfica de tendencia. Los proyectos
   llevan sufijo de origen (" · wsl-<distro>", " · <servidor>"); el frontend etiqueta
   "local" a los que no llevan sufijo.

**C) Fuentes remotas opcionales (dentro de `get_local_stats`):**
1. Si existe `%APPDATA%\com.oscarorozco.michiclaude\remotes.json`
   (`{"remotes":[{"name":"vps","host":"<alias ssh>","command":"python3 /opt/projects/michiclaude/scripts/meter-export.py"}]}`),
   por cada fuente se ejecuta `ssh -o BatchMode=yes <host> <command>`.
2. `scripts/meter-export.py` replica en Python la MISMA agregación que
   `get_local_stats` (dedup, `<synthetic>` fuera, cache_read excluido, precios)
   y emite un JSON con la forma de `LocalStats`. Mantener ambos en sincronía.
3. El meter fusiona: totales sumados, proyectos etiquetados `nombre · vps`,
   modelos agregados. Si el SSH falla, se ignora en silencio (los datos
   locales nunca se bloquean por la red).

### Ventanas

- **Panel** (`main`): flyout sin decoraciones, transparente, alwaysOnTop,
  skipTaskbar. Se abre con clic en el tray; se oculta al perder foco (excepto
  durante arrastre); ✕ oculta a bandeja; arrastrable desde el encabezado.
- **Widget flotante** (`pill`, src/pill.html): cápsula opcional SIEMPRE
  visible que vive
  ENCIMA de la barra de tareas — nunca dentro ni tapando iconos (lección
  aprendida de la franja descartada). No roba foco (`WS_EX_NOACTIVATE`),
  arrastrable solo desde el asa ⠿ (al soltar persiste posición vía
  `pill_moved`), clic abre el panel, clic derecho la oculta. Visibilidad y
  posición en `pill_config.json`; se activa desde ⚙ Preferencias o el menú
  del tray ("Floating widget"). NUNCA llama al endpoint: el panel le emite
  `quota:update` (con tema y tooltip localizados) y la pill pide el último
  dato con `pill:ready` al cargar. (El "diseño coral" alternativo se
  eliminó el 2026-07-25 a petición de Oscar: solo existe el diseño
  original de pastilla y el gatito.)
  REDISEÑO 2026-07-27 (maqueta de Oscar): cápsula de cristal (280x54) con
  borde de acento, asa ⠿, marca, "Sesión X%" y hasta dos porcentajes con
  icono — semanal (calendario) y el bucket semanal POR MODELO (destellos),
  que es un hueco VARIABLE: el icono se queda, el modelo cambia (Fable hoy,
  lo que salga mañana) y si el endpoint no reporta ninguno no se pinta.
  Iconos SVG en el propio archivo: la fuente Tabler de la maqueta se
  descarga de fuera y la CSP no lo permite. Color de los porcentajes: el
  VERDE de "todo bien" se cambió por el acento de la app (`--acc`: #56c7d6
  en oscuro = tag "Suscripción" del panel, #0b7c8c en claro = pestaña
  activa), pero ÁMBAR y ROJO se conservan — son los que avisan de que vas
  por encima del ritmo, y perderlos sí sería un retroceso.
  La caja lleva 6 px de margen dentro de su ventana: pegada al borde, el
  halo del box-shadow se cortaba en recto y se veían dos esquinas
  rectangulares sobre el escritorio. Ese mismo margen en `pill` y `pcard`
  es lo que mantiene alineada la cabecera al desplegar — si se cambia en
  una, hay que cambiarlo en la otra.
  Clic en la cápsula = desplegar el detalle; clic en la MARCA = abrir el
  panel; ⠿ arrastra (y pliega antes); clic derecho oculta.
- **Detalle de la pastilla** (`pcard`, src/pcard.html, 280x300): la maqueta
  crece al hacer clic, pero una ventana transparente NO se puede
  redimensionar en vivo (WebView2 deja de pintar). Son dos ventanas: al
  desplegar se OCULTA la pastilla y se muestra `pcard` con la cabecera
  idéntica en su mismo sitio, y parece que creció. `toggle_pill_card()`
  elige la pose: hacia abajo si cabe, y si no —el caso normal, con el
  widget pegado a la barra— ancla por el borde inferior y las filas salen
  ENCIMA de la cabecera (`body.up` invierte el orden). Ventana fija más
  alta de lo necesario: el hueco transparente sobrante no estorba porque un
  clic en cualquier parte pliega. El globo de aviso NO es un globo de
  cómic cuando el widget es la pastilla: es un POPOVER (`body.cap` en
  notif.html, maqueta de Oscar 2026-07-28) — cristal, esquina de 16, icono
  en pastilla de color, título + línea secundaria, ✕ redonda y una flecha
  pequeña (cuadrado girado 45°) en vez del pico grande. La SEVERIDAD tiñe
  borde, resplandor e icono (`--sev`: acento = informativo, ámbar = ojo,
  rojo = crítico) para que se entienda sin leerlo; la calcula
  `balloonMeta()` en el panel y el cómic del gatito la ignora. La línea
  secundaria solo se pone donde aporta (la alarma de %, con el tiempo que
  falta para el reset). Con la pastilla sigue al tema del panel, NO a los
  selectores "Arte/Globos del gatito", que ahí no se ven. La punta de la
  cola se mete 40 px en el gatito pero solo 8 en la cápsula (`notif_overlap`):
  el gato tiene márgenes transparentes de sobra y la cápsula mide 44 px, así
  que con 40 la cola le caía encima del texto. Globo y detalle NUNCA a la
  vez: el globo tiene prioridad y pliega el detalle; al plegarlo a mano,
  `pcard` emite `notif:ready` y el aviso pendiente vuelve.
- **Widget gatito** (estilo `cat` en `PillConfig.style`, elegible en
  Preferencias; validado en vivo 2026-07-22): el gato Bongo SUSTITUYE a la
  pastilla (nunca conviven). Cuatro ventanas fijas: `cat` (gif animado +
  cápsula "Sesión X%" sobre la cabeza + zona invisible `.head` sobre la
  cabeza que abre el panel — el sticker de la pantalla se retiró el
  2026-07-26; la laptop y los márgenes solo arrastran. Las coordenadas de
  la zona son variables CSS `--hx/--hy/--hw/--hh` para recalibrarla),
  `card` (globo cómic de información al hover, con
  buckets semanales extra DINÁMICOS — Fable, futuros), `notif` (globo de
  alarma persistente con ✕) y la pastilla oculta. Estados del gif según
  los avisos, de más grave a más leve (`mascotState()`): cat-zzz (semana al
  100% = `hit:week`, hasta el reset semanal) / cat-break (SESIÓN al 100% =
  `hit:session`, de descanso hasta el reset de la sesión) / cat-fire (alarma
  de % pendiente = `ackPending:alarm`, se calma al abrir el panel o cerrar
  el globo) / normal. Los dos banderines `hit:*` los limpia `trackResets()`
  al detectar ventana nueva, así que el estado dura exactamente lo que dura
  la espera real. La cápsula del % nace OCULTA (`body.nodata`) y solo
  aparece con una lectura de sesión de verdad: al arrancar o sin token no
  debe verse texto de relleno. Si falta el arte de un estado, `cat.html`
  cae al gif normal en vez de dejar la ventana transparente vacía.
  En modo gatito las alarmas de % NO van a toast de
  Windows: salen como globo `notif` (los demás avisos y la pastilla normal
  siguen con toasts). NINGÚN aviso va ya a toast de Windows mientras haya
  widget en pantalla —gatito o PASTILLA (2026-07-27)—: todos salen por el
  globo `notif`, con un `kind` que viaja y vuelve con la ✕. `place_balloon()`
  ancla al widget que esté puesto (cola al 62% del ancho en el gato, al 50%
  en la pastilla). El toast queda SOLO como respaldo cuando el usuario apagó
  el widget: ahí no hay dónde colgar el globo, y entonces sí se repite cada
  5 min. Motivo del cambio: un toast se va en segundos y quien se levanta de
  su lugar no se entera nunca.
  REGLA ÚNICA (2026-07-27, decisión de Oscar): el globo se queda hasta que
  el usuario le dé ✕ o abra el panel, y no vuelve. NADA de auto-cierre por
  temporizador — un reloj no sabe si el usuario estaba frente a la
  pantalla. Un hover lo oculta (para dejar salir el `card`) pero NO cuenta
  como leído: `notif:ready` lo restaura al terminar. Un globo a la vez;
  si hay varios pendientes gana el primero de `ACK_KINDS`.
  SEGUNDA REGLA, la que evita mentir: cerrar el globo NO cambia el dibujo
  del gatito, que refleja siempre el estado REAL. Cerrar el de descanso no
  devuelve la cuota, así que el gato sigue en `break` hasta el reset de
  verdad; solo la alarma lo calma, porque ahí el estado ERA "hay un aviso
  sin ver". Avisos: `break` ("Sin cuota de sesión. Vuelvo en 8 min.",
  banderín `notifS`), `zzz` ("Semana agotada. Vuelvo el lunes.", banderín
  `notifW`, sustituye al toast `notif_week_limit`) y los restablecimientos
  (`notif_back_*`, versión corta de los `notif_reset_*` porque el toast
  debe explicar cómo callarlo y el globo lleva su ✕ a la vista). break y
  zzz no tienen `ackPending` —no esperan confirmación de nada— así que su
  pendiente vive en `infoBalloon`. El día del `zzz` lo da
  `toLocaleDateString(lang,{weekday:"long"})`, sin claves nuevas, y a menos
  de 24 h del reset se dicen las horas; las frases omiten el artículo donde
  varía con el día (portugués "na segunda" pero "no sábado"). Los gifs (800², transparentes, en variantes -black/-white
  elegidas según el tema) se recortan por CSS
  (unión visible x[39,748] y[0,530] medida con decodificador propio) — NO
  editar los archivos. EXCEPCIÓN: `cat-break-black.gif` llegó en lienzo
  1411x860 (visible x[17,1383] y[74,803]); es el mismo dibujo a otra escala
  (proporción idéntica al 0.11% a la del -white) y se recoloca con la regla
  `.cat.odd-canvas`, que hay que BORRAR si algún día se reexporta a 800². `place_balloon()` coloca los globos con pose
  automática (arriba/abajo según espacio en el monitor ACTUAL del gato,
  origen+tamaño = multi-monitor OK) y cola dinámica (`balloon:pose` →
  `--tailx`) que siempre apunta al gato; nunca dos globos a la vez.
- **Capa en pantalla** (`PillConfig.layer`, elegible en Preferencias):
  "top" (siempre al frente, default) / "normal" / "bottom" (pegado al
  escritorio). `apply_layer()` la aplica y `reassert_layers()` la re-afirma
  en CADA ciclo (`update_tray`) porque Windows degrada el always-on-top.
  REGLA: widget y globos (`card`, `notif`) van SIEMPRE en la misma capa —
  son una sola pieza para el usuario (2026-07-26; antes la alarma se
  forzaba al frente y se veía incoherente). El panel (`main`) no participa.
- **CRÍTICO — ventanas transparentes**: NUNCA redimensionar en vivo una
  ventana transparente (`set_size` con la ventana visible u oculta): WebView2
  deja de pintar el contenido aunque la geometría quede bien (bug sufrido
  2026-07-22, tres intentos de fix fallidos). El patrón correcto es SIEMPRE
  ventanas de tamaño fijo que se muestran/ocultan.
- **Tray icon dinámico**: única presencia permanente en la barra. El panel
  dibuja el % de sesión en un canvas 32×32 (número coloreado por estado +
  barrita semanal al pie) y lo manda por el comando `update_tray`
  (RGBA → `tray.set_icon`), junto con el tooltip ("Sesión X% · reset en …").
  Clic izquierdo muestra el panel, clic derecho menú (abrir panel / salir).
  Con cuota en error, el icono muestra "–" gris (nunca datos inventados).
  El módulo Win32 `win_taskbar` solo se usa ya para apoyar el panel encima
  de la barra (`Shell_TrayWnd` vía crate `windows`).

## INVARIANTES — no romper nunca

1. `get_quota` y `get_local_stats`: no cambiar firmas (get_local_stats acepta
   `days: Option<u32>` — ventana 1/7/30, clamp 1..90 — desde 2026-07-11); no
   eliminar la deduplicación ni la exclusión de `cache_read`. Cualquier campo
   nuevo en LocalStats debe replicarse en `scripts/meter-export.py` y llevar
   `#[serde(default)]` para tolerar exportadores viejos.
2. La función `demo()` del frontend (datos ficticios: vps-infra, edge-gallery,
   rocket-code-prep, MAX 5×, $47.20…) existe SOLO para abrir `index.html`
   suelto en un navegador (cuando `window.__TAURI__` no existe). **PROHIBIDO**
   que datos de demo se rendericen en la app real o se copien a otros archivos.
3. Seguridad: el token nunca se loggea, nunca se muestra en UI, nunca viaja a
   otro dominio que `api.anthropic.com`. Mantener CSP restrictiva en
   `tauri.conf.json`. Sin telemetría.
4. Frontend: `index.html` y `bar.html` vanilla. No agregar frameworks,
   bundlers ni dependencias npm de runtime. Dependencias Rust nuevas: solo
   imprescindibles y con features mínimas.
5. Los porcentajes SIEMPRE redondeados a entero en UI (`Math.round`).
6. Buckets de cuota: render dinámico, nunca hardcodear nombres de modelos.
   Lo mismo aplica a `prettyModel()`: separa el id en palabras (familia) y
   números (versión) en cualquier orden, sin listas — así los modelos que
   salgan mañana se identifican solos. NO volver a un regex con familias
   fijas ni exigir versión de dos dígitos (eso dejaba "claude-opus-5" como
   "Opus" sin versión, detectado 2026-07-26).
7. El tag del plan del header: usar el que reporte el endpoint; si no viene,
   "Suscripción". No inventar "MAX 5×".
8. NUNCA poner una cifra donde no se puede calcular. La fila
   "claude.ai / otros" se ELIMINÓ el 2026-07-28 (decisión de Oscar): su
   etiqueta prometía el desglose de claude.ai y el número era el consumo
   TOTAL de la semana — el mismo que ya sale en el medidor de arriba. Ese
   desglose NO es calculable: el gasto local está en dólares y la cuota en
   porcentaje, y el endpoint nunca expone cuánto vale el tope en dinero. En
   su lugar va una nota de texto pegada a los dólares (`spend_only_cc`), que
   es donde nace el malentendido; en un tooltip o en el README no la lee
   quien la necesita. Por lo mismo, con la ventana de 1 día el pie oculta la
   segunda cifra: "hoy" son las últimas 24 h y la ventana de 1 día también,
   y enseñar el mismo número dos veces con dos nombres parece un error.
   Ambos cambios validados en vivo por Oscar el 2026-07-28.
9. No tocar `README.md`, `.github/workflows/release.yml` ni `app-icon.png`
   salvo petición explícita.
10ter. Todo comando de Rust que HAGA ESPERA (SSH, red, escaneo de disco
    largo) tiene que ser `async fn` y delegar en
    `tauri::async_runtime::spawn_blocking`: Tauri ejecuta los comandos
    SÍNCRONOS en el mismo hilo que dibuja la ventana, así que uno bloqueante
    congela el panel entero mientras dura. Se descubrió el 2026-07-28 porque
    cambiar de pestaña tardaba segundos: abrir "Fuentes de datos" dispara
    `test_remote`, que es un SSH de 1-2 s. Envueltos así: test_remote,
    install_remote, get_local_stats, export_data, save_hub_config y
    load_hub_config. NO envolver los que tocan ventanas (hover_card,
    set_notif_visible, toggle_pill_card, set_pill_*, update_tray): esos
    tienen que seguir en el hilo principal, y además son instantáneos.
    VALIDADO en vivo por Oscar el 2026-07-28: el panel pasó a sentirse
    fluido al cambiar de pestaña. OJO al diagnosticar "la app va lenta":
    los gifs del gatito parecían el sospechoso obvio y no tenían nada que
    ver; la pista buena fue que get_quota, la operación MÁS pesada, nunca
    se sintió lenta porque ya era asíncrona.
10bis. `[hidden]{display:none !important}` en index.html: NO quitarlo. El
    navegador aplica `hidden` desde su hoja por defecto, así que cualquier
    regla propia con display (`.cfg-row`, `.btnrow`, `.fld` son flex) lo
    anulaba en silencio. Se descubrió el 2026-07-28 porque las filas del
    gatito seguían viéndose con la pastilla puesta, y arrastraba algo peor:
    el simulador —que solo debe existir en desarrollo— se veía en RELEASE.
10. UI multiidioma: diccionario `I18N` en index.html (inglés por defecto,
    español incluido; autodetección por `navigator.language`, persistido en
    localStorage). Todo texto visible pasa por `t()` — nunca hardcodear
    cadenas en un solo idioma. El backend Rust devuelve errores como códigos
    `ERR_*` (ERR_NO_TOKEN, ERR_TOKEN_EXPIRED, ERR_RATE_LIMITED:<min>,
    ERR_API:<status>, ERR_NET, ERR_BAD_RESPONSE) que el frontend traduce.
    Tono claro y accionable en ambos idiomas.
11. Tema claro/oscuro: variables CSS con override en `body.light`, toggle ◐
    en el header, persistido en localStorage.

## Comportamiento ya validado — no regresionar

- Panel con datos reales (gauge sesión, buckets dinámicos incl. Fable,
  proyección con burn rate, gasto por proyecto con dedup, split de modelos con
  nombres bonitos, totales hoy/semana).
- Arrastre del panel desde el encabezado (con guarda anti-blur durante drag).
- ✕ oculta a bandeja; clic en tray reabre; flyout se oculta al perder foco.
- Estados de error legibles con punto rojo en la línea de estado.
- Notificaciones de umbral CONFIGURABLES: el usuario define sus alarmas de
  sesión en % (chips en ⚙ Preferencias, localStorage `alarms`, default
  [80,95]). Al cruzar un umbral, el toast se REPITE cada 5 min hasta que el
  usuario abra el panel; confirmado, silencio hasta el siguiente umbral o la
  siguiente ventana. Al cruzar varios de golpe suena solo el más alto.
  Límite semanal al 100%: un aviso por ventana.
- CRÍTICO: el `resets_at` del endpoint trae jitter entre consultas — la
  detección de "ventana nueva" SIEMPRE con tolerancia (`windowChanged`,
  10 min sesión / 360 min semana), nunca comparación exacta de strings
  (causaba re-disparos de alarmas cada ciclo).
- Avisos de RESTABLECIMIENTO (sesión y semanal): solo si la ventana anterior
  llegó al 100% (`hit:*`); la notificación de WINDOWS se repite cada 5 min
  HASTA que el usuario abra/enfoque el panel (eso confirma y limpia
  `ackPending:*`). Sin banners dentro de la app (decisión de Oscar
  2026-07-11). Nunca quitar el mecanismo de confirmación.
- Controles nativos (select, spinners): `color-scheme` en body sigue el tema
  para que los desplegables se vean bien en oscuro.
- El panel es el ÚNICO que llama al endpoint; el icono del tray se actualiza
  desde su mismo ciclo de refresco (`updateTray` en cada render).

## Estado actual / pendientes conocidos

- [x] Panel completo con datos reales
- [x] Buckets semanales dinámicos (incluido Fable)
- [x] Fila "claude.ai / otros" — ELIMINADA 2026-07-28, ver invariante 8
- [x] Fix de arrastre y flyout
- [x] `cargo check` limpio (verificado 2026-07-10; la sesión que se cortó a
      mitad de editar Cargo.toml no dejó nada roto)
- [x] Franja sobre la barra: DESCARTADA (2026-07-10, solapaba los iconos
      centrados de Windows 11 y bloqueaba sus clics). Sustituida por el icono
      de bandeja dinámico con %.
- [x] Icono de bandeja dinámico (`updateTray` + comando `update_tray`) —
      validado en vivo: legibilidad en barra clara y oscura (ver la entrada
      del contorno, más abajo) y actualización en cada ciclo de refresco.
- [x] 429 del endpoint: espera de 5 min (el backoff rápido 5→40 s queda solo
      para errores de red; nunca reintentar rápido un rate-limit).
- [x] Instancia única (tauri-plugin-single-instance, registrado el primero):
      instancias duplicadas de dev eran sospechosas del 429.
- [x] 429: se respeta el Retry-After del servidor, el cuerpo del error se
      vuelca a quota_debug.json, y si el token local ya venció (expiresAt) no
      se llama a la API (evita bloqueos por reintentos con token muerto).
- [x] Cadencia de cuota bajada a 3 min (60 s disparaba 429 recurrentes) y el
      gauge/icono conservan el último dato bueno hasta 15 min ante fallos
      transitorios (nunca se borra la lectura por un error pasajero).
- [x] Ajustes en el panel (botón ⚙): alta/baja de servidores SSH con prueba
      de conexión (comandos get_remotes/save_remotes/test_remote); escribe
      remotes.json. Este PC y claude.ai no requieren configuración.
- [x] Fuente remota VPS: exportador + fusión, verificados en vivo. Los
      proyectos del servidor salen etiquetados con el nombre corto que el
      usuario le puso, y desde el 2026-07-29 ese nombre se puede editar con
      un clic (antes había que borrar el servidor y darlo de alta otra vez).
- [x] PULIDO Windows ronda 1 (2026-07-24/25, validado por Oscar): barrido de
      mayúsculas/gramática en TODAS las cadenas visibles de los 8 idiomas
      (JA/KO/ZH sin caja); nota de Fuentes de datos como lista con viñetas;
      formulario de servidores con etiquetas+pistas y lista con pastilla de
      estado, chip contador, icono de bote y CONFIRMACIÓN en dos pasos al
      eliminar; pantalla de bienvenida adaptativa (setup sin Claude Code /
      banner Token vencido con datos); "Diseño de la pastilla" (coral)
      ELIMINADO a petición de Oscar; arte del gatito y sticker en variantes
      -black/-white según tema (geometría verificada idéntica al recorte
      CSS); sticker translúcido al 40% con hover a 100%; el tag del plan
      confirmado como dinámico-con-respaldo (el endpoint NO envía el plan —
      verificado con quota_debug.json real de Oscar; los campos con nombres
      en clave tipo iguana_necktie confirman API interna inestable).
      Experimento revertido: apagar el texto turquesa de la pantalla en los
      gifs oscuros vía parche de paleta (funcionó, pero a Oscar no le
      convenció el resultado — el parche queda documentado en el historial
      af16d6d por si se retoma con otra mezcla).
- [~] Micro-pendientes de pulido (2026-07-27): HECHO los backticks de
      \`claude\` (ahora comillas tipográficas por idioma) y la nota de
      subagentes (README + tooltip de "costo estimado" en los 8 idiomas).
      FALTA: capturas para el README (las tiene que hacer Oscar). Hecho
      también el 2026-07-27: guía del caso SSH reescrita con el enfoque de
      Oscar (problema → 3 requisitos → escenario con un usuario ficticio →
      preguntas rápidas), el sufijo de los proyectos remotos documentado como
      lo que es (el nombre corto que elige el usuario, no "· vps") y Mythos
      fuera de la tabla de precios por ser de acceso por invitación. Idea
      cancelada 2026-07-25: placa translúcida tras el sticker en tema oscuro.
- [x] Icono de bandeja legible en cualquier tema (2026-07-27, validado por
      Oscar en barra clara y oscura; número a 24 px y contorno de 4 px tras
      pedirlo él más grande): el número se
      dibujaba en verde/ámbar/rojo sólido, que sobre una barra CLARA se lava.
      Se le añade contorno oscuro (strokeText) y marco a la barrita semanal,
      así se lee sobre fondo claro, oscuro o de color sin detectar el tema de
      Windows (que además no cubriría fondos personalizados).
- [x] BUG capa "Siempre al frente" — RESUELTO (2026-07-27: Oscar lo dejó
      corriendo con su uso normal y el gatito ya no se hundió; antes fallaba
      tras un rato largo. Si reapareciera, la siguiente vía está anotada
      abajo). Causa y arreglo: Hipótesis fuerte: la llamada
      `set_always_on_top(true)` de Tauri NO llega al sistema cuando su estado
      interno ya dice que la ventana es topmost; Windows la degrada por su
      cuenta (otra app se activa, cambia de escritorio, se conecta un monitor)
      y todas nuestras re-afirmaciones eran no-ops — por eso el gatito se
      quedaba detrás para siempre y ni el hilo de 2 s lo rescataba. Arreglo:
      `win_taskbar::force_topmost()` llama a `SetWindowPos(HWND_TOPMOST,
      SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE)` por Win32 directo, que reinserta
      la ventana en la banda topmost sin depender de ningún estado cacheado;
      `apply_layer()` lo hace además de la llamada de Tauri. SWP_NOACTIVATE es
      obligatorio (el widget no debe robar foco). Si aún así reapareciera,
      lo siguiente sería reaccionar al cambio de ventana en primer plano con
      `SetWinEventHook` (EVENT_SYSTEM_FOREGROUND) en vez de sondear cada 2 s.
      Descripción original del bug:
      pese al ajuste, (a) el globo de información/resumen a veces queda DETRÁS
      de otras ventanas al abrirse, y (b) tras un rato el gatito se va atrás
      solo. Ya intentado sin éxito definitivo: `apply_layer()` en las tres
      ventanas (cat/card/notif), `reassert_layers()` en cada `update_tray`
      (3 min) y luego un hilo cada 2 s con guarda de panel abierto. Sigue
      fallando de forma INTERMITENTE, así que el remedio actual (re-aplicar
      `set_always_on_top`) no basta. Hipótesis a investigar: otras ventanas
      TAMBIÉN son topmost y compiten dentro de esa banda (SetWindowPos con
      HWND_TOPMOST solo reordena dentro del grupo); reaccionar al cambio de
      ventana en primer plano con `SetWinEventHook`
      (EVENT_SYSTEM_FOREGROUND) en vez de sondear cada 2 s; o forzar la
      re-elevación con Win32 directo (`SetWindowPos` + `SWP_NOACTIVATE`)
      en vez del wrapper de Tauri. Al depurar, confirmar primero el valor
      real de `layer` en pill_config.json (en "normal"/"bottom" el
      comportamiento observado sería el correcto).
- [x] PRECIOS DINÁMICOS — implementados y probados en vivo, con red y sin ella.
      `price_for()` consulta primero la tabla descargada y cae a la embebida
      (ya corregida). Cascada en `fetch_prices()`: LiteLLM → models.dev →
      OpenRouter; primera que responde gana (NO es verificación cruzada).
      Caché en `prices_cache.json`, refresco al arrancar y cada 6 h saltando
      si el caché tiene menos de 24 h; `prices_config.json` guarda el
      interruptor y URLs opcionales. Comandos get_prices_status /
      set_prices_auto / refresh_prices_now; Preferencias muestra fuente,
      antigüedad ("hace 2 h" vía Intl.RelativeTimeFormat, sin claves i18n
      nuevas) y botón de actualizar. Modelos sin tarifa conocida se marcan
      "~" en la leyenda (`ModelAgg.estimated`). Los precios frescos viajan al
      exportador del VPS por STDIN (`--prices-stdin`): una sola fuente de
      verdad; un exportador viejo ignora el flag y sigue con su tabla. Los
      tres parsers se validaron contra las fuentes reales (18/11/17 modelos,
      valores idénticos) y Oscar lo confirmó en vivo (fuente litellm, 18
      claves tras normalizar). El interruptor se sacó de la UI el 2026-07-27
      (solo empeoraba y era fácil tocarlo sin querer): vive en
      prices_config.json y en su lugar hay un botón ⓘ que enseña las fuentes.
      Si la descarga falla o lleva más de una semana sin lograrlo, aparece un
      aviso ⚠ junto a "costo estimado" — no un toast: no es urgente ni
      accionable al instante. PROBADO SIN RED el 2026-07-28 (Oscar apagó el
      wifi): la app degrada bien —dice "Token vencido" en vez de callar, el
      medidor muestra "—" en vez de inventar cifras, los costes locales se
      calculan igual, el servidor SSH queda marcado "sin conexión" y los
      precios caen al caché con su fuente y antigüedad a la vista. FALTA solo
      el camino del ⚠: sin caché o con más de una semana sin actualizar. Para
      forzarlo hay que cerrar la app DEL TODO (la instancia única hace que un
      segundo `npm run dev` no reinicie nada) y renombrar prices_cache.json
      antes de arrancar. VERIFICADO el 2026-07-28: con el caché fuera y sin
      red sale el ⚠ y Preferencias dice "Sin conexión — se usa la tabla
      incluida". OJO al reproducirlo: `Rename-Item` falla si el destino ya
      existe de un intento anterior, y la app recrea prices_cache.json en
      cuanto vuelve la red.
      Descripción original de la investigación:
      Hallazgo URGENTE: `price_for()` cobra $15/$75 a opus/fable/mythos, que
      es la tarifa del difunto Opus 4.1; las reales son Opus 5/4.8 $5/$25 y
      Fable 5 $10/$50 (sonnet y haiku sí están bien). Los costes de Opus
      salen ~3x inflados. Investigación: NO existe API oficial de precios de
      Anthropic (`/v1/models` da id, display_name, límites y capabilities,
      pero ningún precio). Tres fuentes públicas verificadas en vivo, todas
      con los modelos actuales correctos: LiteLLM
      (`model_prices_and_context_window.json`, el estándar de facto — es lo
      que usa ccusage, y refleja hasta el precio introductorio de Sonnet 5),
      models.dev/api.json (esquema más limpio) y openrouter.ai/api/v1/models
      (sin auth; los más frescos porque facturan con ellos). Diseño acordado
      (opción B): descarga diaria a un `prices_cache.json` en la carpeta de
      datos, con CASCADA de fuentes de mayor a menor confiabilidad —
      (1) LiteLLM: el estándar de facto, comunidad enorme, actualiza el
      mismo día del lanzamiento y trae hasta las tarifas introductorias y
      de contexto largo; (2) models.dev: open source, esquema más limpio
      (`cost:{input,output,cache_read,cache_write}`), comunidad menor;
      (3) OpenRouter: los datos más frescos porque facturan con ellos,
      pero es una empresa comercial (dependencia de un tercero con
      intereses propios) — se usa como último recurso de red. Si las tres
      fallan → caché guardado → si no hay caché, tabla embebida
      (CORREGIDA) → si el modelo no aparece en ninguna, marcarlo como
      estimación en la UI en vez de cobrarlo en silencio. Solo cascada de
      RESPALDO: nunca consultar varias para comparar (descartado por
      sobreingeniería). Con URLs configurables, toggle en Preferencias
      (por defecto activo) y nota
      honesta en el README (es un GET anónimo, no viaja nada del usuario;
      matiza la promesa "solo api.anthropic.com"). El exportador del VPS NO
      debe duplicar la tabla: MichiClaude le pasa los precios frescos por
      argumento y `meter-export.py` usa la suya solo como respaldo.
      Alternativa descartable si se prefiere no tocar la promesa de red:
      espejo propio en el repo actualizado por GitHub Actions.
- [x] USABILIDAD fuente remota — RESUELTO 2026-07-27: el exportador viaja
      dentro del binario (include_str!) y se sube por SSH desde stdin a
      ~/.michiclaude/meter-export.py al dar de alta el servidor; el campo
      "comando" queda vacío y opcional (solo para script propio, y entonces
      no se escribe nada en el servidor ajeno). Al arrancar se refresca en
      los remotos que usan nuestra ruta, para que no se desincronice del
      backend tras actualizar la app. Descripción original:
      formulario de servidores viene por defecto con la ruta PERSONAL de Oscar
      (`python3 /opt/projects/michiclaude/scripts/meter-export.py`), que no
      sirve para otros usuarios y confunde. Además hoy el usuario tendría que
      copiar meter-export.py al servidor a mano y saber su ruta — no está
      explicado ni automatizado. Arreglo mínimo: default genérico
      (`python3 ~/meter-export.py`) + nota corta. Ideal: que MichiClaude SUBA
      el script solo por SSH la primera vez (nombre + host y listo). Encaja
      naturalmente con el Modo HUB — resolver ahí.
- [x] Lectura incremental de .jsonl — IMPLEMENTADO 2026-07-26 en AMBOS lados
      (Rust y meter-export.py) y el lado Rust ya corriendo en el Windows de
      Oscar desde el 2026-07-27 con las cifras correctas. Dos
      optimizaciones que NO cambian ningún número: (1) todos los agregados
      están acotados en el tiempo, así que un archivo cuya última escritura
      sea anterior a la ventana más amplia (la elegida o los 30 días de la
      tendencia, +2 de margen) se salta sin abrirlo — el coste pasa de crecer
      con todo el historial a quedarse en "el último mes"; (2) de los
      recientes se cachea el PARSEO indexado por tamaño+mtime
      (`scan_cache.json`), nunca el coste, para que un cambio de precios se
      aplique a todo al instante. La retención del caché se adapta a la
      ventana pedida (--days 90 tras un ciclo de 7 descarta el caché en vez
      de devolver de menos). Es reconstruible: si se borra o no se entiende,
      se recalcula. Medido en el exportador con 50 MB: 1.06 s -> 0.06 s,
      caché de 172 KB, salida IDÉNTICA byte a byte contra una copia congelada
      de los logs en ventanas 1/7/30/90, frío y caliente, más casos límite
      (caché corrupto, archivo modificado, archivo viejo). El lado Rust lleva
      corriendo en el Windows de Oscar desde el 2026-07-27 con las cifras
      correctas.
      OJO: los duplicados TAMBIÉN cruzan archivos (365 en los logs reales),
      así que la dedup global al fusionar es imprescindible — una medición
      inicial que comparaba nombres en vez de rutas los daba en cero.
- [x] Token de respaldo desde remotes.json cuando el local venció (2026-07-10):
      el meter ya no depende de usar Claude Code en Windows.
- [x] Autostart (tauri-plugin-autostart): solo builds release, se activa una
      única vez (marker `autostart_configured`); si el usuario lo desactiva
      en el Administrador de tareas, se respeta.
- [x] LICENSE GPL-3.0 con excepción de assets Bongo Cat (2026-07-24; antes
      MIT — Oscar eligió copyleft para que los derivados sigan abiertos y
      con crédito). Los gifs/sticker de la mascota quedan fuera de la GPL.
- [x] RENOMBRADO a "MichiClaude" (2026-07-24, decisión de Oscar): repo,
      productName, identifier (com.oscarorozco.michiclaude — se cambió
      porque aún no había usuarios; la config vieja de %APPDATA% no migra),
      exe (michiclaude.exe), crates, títulos de ventanas y marca en UI.
      La carpeta del VPS se renombró a /opt/projects/michiclaude (con
      symlink de compatibilidad desde la ruta vieja claude-code-meter).
- [x] Multiidioma (8): EN default, ES, PT, FR, DE, JA, KO, ZH — diccionario
      I18N, autodetección, selector en ajustes, errores del backend como
      códigos ERR_* traducidos en el frontend.
- [~] Fuente WSL — SIGUE SIN PROBARSE: no hay ninguna máquina con WSL a
      mano, ni la de Oscar ni el VPS. El camino en el código se puede seguir
      línea por línea pero NO se ha ejecutado nunca; si WSL no se detecta,
      simplemente no salen esas filas y no rompe nada.
      Fuente WSL implementada (el sufijo es "wsl-<distro>" desde
      el 2026-07-29; antes todas caían bajo un "wsl" genérico y con dos
      instaladas no había forma de distinguirlas): `wsl.exe -l -q` (UTF-16LE) + escaneo de
      \\wsl.localhost\<distro>\{home/*,root}\.claude como fuente local extra
      (proyectos "nombre · wsl") y token de respaldo desde WSL antes que los
      remotos. FALTA probar en una máquina con WSL real.
- [x] Tema claro/oscuro con toggle ◐ persistido. Alcanza también a las
      ventanas del gatito (2026-07-27): la cápsula del % y los dos globos
      (`card`, `notif`) invierten el cómic en oscuro (relleno #20242c, trazo
      y texto #e9ebef, coral a #d97757 porque el #C15F3C se apaga sobre
      fondo oscuro). Antes eran lo único del widget que se quedaba en papel
      blanco. El tema viaja en el resumen `quota:update`, así que `notif`
      lo escucha solo para eso; en `card` se aplica ANTES del early-return
      de `ok:false`, para que siga al panel aunque conserve el dato viejo.
      TODAS las ventanas del widget emiten `pill:ready` al cargar para pedir
      el último resumen; `notif` era la única que no lo hacía y salía con el
      tema de fábrica hasta el siguiente ciclo, ignorando lo elegido
      (2026-07-28). Si se añade otra ventana que dependa del resumen, tiene
      que emitirlo también.
      Además el ARTE del gatito y la PIEL de sus globos se eligen por
      separado (2026-07-27): dos selectores en Preferencias, cada uno
      "Según el tema" (default, sigue al ◐) / Claro / Oscuro, en
      localStorage `catArt` y `catSkin`. El resumen lleva `artTheme` y
      `skinTheme` además de `theme`; los widgets caen a `theme` si faltan.
      La CÁPSULA del % va con los globos, no con el gato: es información,
      igual que ellos (decisión de Oscar, a prueba — si no convence, se
      quita). Las dos filas se esconden con la pastilla (clase `catOnly`).
- [x] Periodo dinámico del gasto por proyecto (1d/7d/30d, persistido) y
      desglose por modelo de cada proyecto (tooltip al pasar el mouse).
- [x] Gráfica de tendencia diaria (30 días, calculada de los logs — no hay
      base de datos propia que se pueda perder).
- [x] Presupuesto semanal con notificación toast de Windows (funciona con el
      panel cerrado; anti-spam 1 aviso/semana). Se compara contra la suma de
      los últimos 7 días de la serie diaria, no contra la ventana elegida.
- [x] Export CSV/JSON — REHECHO 2026-07-29 (maqueta de Oscar). Antes era un
      volcado con tres tablas en una (proyectos, modelos y días) y una columna
      `name_or_date` que a veces era un nombre y a veces una fecha. Ahora es
      UNA FILA POR HECHO: fecha × proyecto × modelo × origen, con costo y
      tokens — todas las columnas aplican a todas las filas, así que Excel
      puede hacer tablas dinámicas. 25 filas para 30 días de datos reales.
      DETALLES QUE IMPORTAN:
      · BOM al inicio del CSV: sin él Excel lo abre como texto de Windows y un
        "·" se ve como "Â·". Tres bytes que salvan todos los acentos.
      · Campos entre comillas con las internas duplicadas. Antes se sustituían
        las comas por espacios, que mutila el dato en vez de citarlo.
      · Las filas solo se calculan al exportar (`want_rows`): hacerlo en cada
        ciclo del panel sería trabajo tirado y engordaría la foto del hub.
      · El exportador remoto las devuelve con `--rows`, y el ORIGEN lo pone
        quien lee con el nombre que el usuario dio al servidor — el script
        remoto no sabe cómo se llama a sí mismo.
      · Títulos traducidos a los 8 idiomas (`exp_cols`), y el origen local usa
        `src_local`. Sin fila de totales A PROPÓSITO: rompería las tablas
        dinámicas, y Excel la calcula con un clic.
      · Periodo PROPIO (1/7/15/30, persistido): mirar 1 día en pantalla y
        querer exportar 30 es un caso normal.
      VERIFICADO EN VIVO el 2026-07-29 con los dos formatos: el CSV abre en
      Excel con los acentos correctos (se acabó el "Â·") y el JSON trae las 28
      filas de los DOS orígenes, ordenadas por fecha descendente y por costo
      dentro de cada día. Al cuadrar los totales contra el servidor salía una
      diferencia de $0.57: no era un fallo, era que la sesión de Claude Code
      seguía gastando MIENTRAS medíamos — la fila del día creció $1.10 en dos
      minutos. Un export es una foto, no un cierre contable.
      El JSON exporta las MISMAS filas, más lo único que un CSV no puede
      llevar: `generated_at`, `window_days` y la nota del costo. Quien lo
      procese con un script no tiene que adivinar de cuándo es ni de qué
      ventana. Y conserva la precisión completa, sin los 4 decimales del CSV.
      OJO (mordió el 2026-07-29): `ExportRow.origin` DEBE llevar
      `#[serde(default)]`. El exportador remoto manda sus filas SIN ese campo
      —a propósito, no sabe cómo lo llamó el usuario— y sin el default serde
      daba por inválida la respuesta entera: las filas del servidor
      desaparecían del reporte sin ningún aviso.
- [x] Panel en 3 PESTAÑAS (2026-07-21, decisión de Oscar): Principal
      (cuota/proyección/gasto/tendencia/modelos) · Fuentes de datos
      (nota + servidores con estado y alta) · Preferencias (idioma, widget,
      alarmas, presupuesto, export). Pie con Hoy/Semana fijo en todas. El
      botón ⚙ se eliminó — las pestañas siempre están visibles. Encabezado y
      pestañas van en `.p-top`, STICKY (2026-07-27): el relleno superior del
      panel vive en `.p-top`, no en `.panel`, y los márgenes negativos lo
      llevan de borde a borde. Si se devuelve el `padding-top` a `.panel` o
      se le pone `margin-bottom` a `.tabs`, se abre una rendija transparente
      por la que se ve pasar el contenido al hacer scroll.
- [x] Leyenda de modelos completa (todos los usados, "<1%" para los mínimos).
- [x] Umbrales de prueba devueltos a valores normales (Oscar, 2026-07-29):
      los 25 que había puestos eran para ver el simulador.
- [~] Alarmas de uso configurables + avisos de límite/restablecimiento con
      confirmación "Enterado" — implementado (2026-07-11). La parte VISUAL
      quedó validada el 2026-07-28 con el simulador (gatito y pastilla).
      Falta solo la detección real: cruzar un umbral de verdad, llegar al
      100% y comprobar que se reconoce la ventana nueva.
- [x] Widget flotante (pill) sobre la barra — probado en vivo (2026-07-22):
      arrastre, persistencia, sin robo de foco, tema OK.
- [x] Widget gatito (2026-07-22, validado en vivo por Oscar): mascota con
      estados normal/llamas/zzz ligados a los avisos, cápsula de %, sticker
      que abre el panel, globo de información al hover (buckets dinámicos)
      y globo de notificación con ✕ en vez de toasts. Pose automática de
      globos, cola dinámica y soporte multi-monitor. Ver sección Ventanas.

  --- Jornada 2026-07-27 (26 commits). Lo validado en vivo por Oscar y lo
      que quedó pendiente de mirar: ---

- [x] Cuarto estado del gatito, `break` (sesión agotada, espera al reset) y
      su arte en los dos temas. OJO: `cat-break-black.gif` vino en lienzo
      1411x860 en vez de 800² y se recoloca por CSS (`.cat.odd-canvas`).
- [x] La cápsula del % nace oculta: al arrancar enseñaba "session —" en
      inglés, que era el texto de relleno del HTML y parecía un error.
- [x] Encabezado y pestañas del panel FIJOS al hacer scroll.
- [x] Simulador de estados (solo builds de dev) con pausa ajustable, que
      recorre dibujo Y globo sin ensuciar el estado real.
- [x] Regla única de globos: se quedan hasta que el usuario los cierre, y
      cerrar el globo NO cambia el dibujo del gatito (el dibujo va con el
      estado real). Cuatro avisos: alarma, break, zzz y restablecimiento.
- [x] Los globos siguen al tema oscuro, y arte del gatito / piel de los
      globos se eligen por separado en Preferencias.
- [x] La CÁPSULA del % va con los globos y no con el gato — a prueba desde
      el 2026-07-27 y CONFIRMADA por Oscar el 2026-07-29 tras usarla dos
      días. (Si algún día se quisiera al revés: cambiar `curSkin` por
      `curArt` en `cat.html`.)
- [x] REDISEÑO DE LA PASTILLA en cápsula + detalle desplegable (`pcard`),
      con el % en color de acento — VALIDADO en vivo por Oscar el 2026-07-28
      en los dos temas: la cabecera se queda en su sitio al desplegar, el
      arrastre funciona, y las esquinas dejaron de recortarse al meter la
      caja 6 px (el halo se cortaba contra el borde de la ventana).
- [x] AVISOS de la pastilla como POPOVER con severidad (icono en pastilla de
      color, título + línea secundaria, ✕ arriba a la derecha, flecha
      pequeña) — validado en vivo el 2026-07-28 con el simulador: se ven los
      tres colores y el fondo opaco se lee sobre cualquier escritorio.
      LECCIÓN: el primer intento salió transparente y apilado porque una
      edición mía borró las reglas base del CSS de notif.html. Si algo de ese
      archivo se ve "sin estilo", mirar primero que sigan ahí `*{box-sizing}`,
      `.box`, `.msg` y `.x` — el bloque `body.cap` solo las ESPECIALIZA.
      El fondo va OPACO a propósito: con alfa sobre una ventana transparente
      se lee el escritorio a través del aviso.
- [x] El globo/popover acompaña al widget al arrastrarlo y se aparta del
      detalle desplegado (nunca los dos a la vez) — validado 2026-07-28.
- [~] Simulador con la PASTILLA ("🔔 Simular avisos") — validado en vivo el
      2026-07-28. Con eso, del ciclo de alarmas ya está visto TODO lo visual;
      lo que sigue sin probarse de verdad es la DETECCIÓN: cruzar un umbral
      real, tocar el 100% y que `trackResets()`/`windowChanged()` reconozcan
      la ventana nueva. Eso solo se comprueba usando la cuota.
- [x] `cargo check` limpio con todo lo del 2026-07-27 (comando
      `toggle_pill_card`, ventana `pcard`, `notif_dx`, `is_dev`): verificado
      por Oscar en Windows el 2026-07-28, sin errores ni advertencias.
      RECORDATORIO: en el VPS no hay toolchain de Rust, así que esa
      verificación SIEMPRE corre en el Windows de Oscar.
- [x] Alta automática de servidor — VERIFICADA en vivo el 2026-07-29: Oscar
      borró su VPS y lo volvió a agregar con el comando VACÍO. La app detectó
      Python, subió el lector a `~/.michiclaude/meter-export.py` y guardó el
      comando resuelto sola. Comprobado en el servidor que el archivo existe y
      corre. FALTA aún el caso de error (un host SIN Python debe fallar con
      ERR_NO_PYTHON traducido, no darse por bueno).
      DOS COSAS QUE SALIERON DE ESA PRUEBA:
      (1) El archivo subido llevaba saltos CRLF —git los mete al clonar en
      Windows e `include_str!` los embebe—, así que el shebang quedaba como
      `python3\r`. Funciona porque se ejecuta con `python3 archivo`, pero se
      sube con permiso de ejecución y quien probara `./meter-export.py` en el
      servidor se topaba con un error incomprensible. `upload_exporter`
      normaliza los saltos.
      (2) El campo "Comando a ejecutar" DESAPARECIÓ (2026-07-29). Nació
      oculto y revelándose al fallar la detección, pero eso no servía: si no
      hay Python, `install_remote` se detiene ANTES de subir el lector, así
      que el comando que escribiera el usuario apuntaría a un archivo que no
      existe. En su lugar el campo pregunta "¿dónde está Python?"
      (`lbl_py`/`hint_py`), que es un dato que el usuario SÍ puede averiguar
      con `which python3`; `install_remote(host, python)` verifica ese binario
      con `verify_python` —si no, se guardaría un comando roto y el servidor
      saldría "conectado" sin devolver datos— y arma el comando la app.
      El caso "script propio" ya no está en la interfaz: quien lo necesite
      puede editar remotes.json a mano.
      (3) El NOMBRE de un servidor guardado se edita haciendo clic en él
      (2026-07-29): antes había que borrarlo y volver a darlo de alta por
      una errata. Como el nombre es el sufijo que llevan sus proyectos
      ("proyecto · nombre"), al cambiarlo se recarga el panel.
      OJO al probar cambios del exportador: tras el alta automática, el que
      corre en el servidor es la copia EMBEBIDA en el binario, que la app
      re-sube al arrancar. Editar `scripts/meter-export.py` en el VPS ya NO
      tiene efecto — hay que recompilar.
- [x] MODO HUB — LAS TRES FASES IMPLEMENTADAS Y VERIFICADAS contra el VPS
      real (2026-07-28/29). Fase 1 (subir): Cada ciclo deja la foto de ESTA máquina en cada servidor, en
      `~/.michiclaude/hosts/<máquina>.json`, vía `upload_summary()`.
      Identidad en `hub_identity.json` (id irrepetible + nombre, por defecto
      COMPUTERNAME); el id viaja DENTRO del archivo y el guard lo comprueba
      EN EL SERVIDOR dentro del mismo comando SSH: si existe con otro id no
      sobreescribe y sale con código 3 (dos PCs con el mismo nombre se
      pisarían en cada ciclo sin que nadie se entere). Guard probado en el
      VPS con los tres casos (nuevo / mismo id / otro id).
      CRÍTICO: se sube lo LOCAL A SECAS, antes de fusionar. Subir lo ya
      fusionado haría que las máquinas se hagan eco y los totales se
      multipliquen solos.
      `RemoteSource` gana `share` ("all"/"picked") y `shared`, ambos con
      serde(default) — en fase 1 siempre "all", pero el campo existe desde ya
      porque afecta a cómo se construye la subida.
      Los fallos no bloquean nada y quedan en `hub_debug.json`; todavía NO
      hay nada en la interfaz (eso es la fase 2).
      VERIFICADA EN VIVO el 2026-07-28: compila limpio en Windows y el
      archivo aparece en el VPS como `hosts/oscar-huawei.json` con la
      máquina OSCAR-HUAWEI, SOLO los dos proyectos locales de Windows
      (claude-code-meter-tauri $10.37 y system32 $0.30) —ninguno del VPS,
      que era la comprobación que importaba—, más su serie diaria y sus
      modelos. Nota: el nombre del ARCHIVO va en minúsculas (`safe_name`)
      y el del campo `machine` conserva el original.
      Diseño completo y casos en `docs/hub-modo-equipo.md`.
      FASE 2 (fusionar) IMPLEMENTADA 2026-07-28: `meter-export.py` devuelve
      una clave `hosts` con los resúmenes que encuentre en
      `~/.michiclaude/hosts/`, SIN fusionar —la etiqueta la pone quien lee, y
      lleva el nombre de la MÁQUINA, no el del servidor: el VPS es el punto
      de encuentro, no el origen—. `fetch_remote` le pasa
      `--exclude-host <id>` para que no le devuelva lo suyo, y el lado Rust
      vuelve a filtrar por id por si un exportador viejo ignorase el flag:
      recibir lo propio de vuelta significaría contarlo dos veces en cada
      ciclo. Cada resumen viaja con `seen_at` (mtime del archivo) y NO se
      descarta nada por antigüedad: la app no puede distinguir "se fue" de
      "está de vacaciones". Probado en el VPS con una máquina falsa
      (`pc-trabajo.json`): exclusión en los dos sentidos, sin el flag salen
      las dos, un resumen corrupto se ignora sin tumbar a los demás, sin
      carpeta `hosts/` todo sigue igual, y la fusión simulada no duplica el
      proyecto local.
      VENTANA (arreglado 2026-07-28 al verlo en las capturas de Oscar): cada
      máquina sube UNA FOTO POR VENTANA (`HUB_WINDOWS` = 1/7/15/30) y el
      SERVIDOR elige la pedida en `read_hosts(exclude_id, days)`. Quien lee
      NO puede recortar un resumen ajeno —su desglose por proyecto ya viene
      sumado—, así que sin esto el selector 1d/7d/30d movía los proyectos
      del servidor pero dejaba clavados los de las otras máquinas: un número
      de otra ventana sin ninguna señal. Si falta la ventana pedida se cae a
      `stats` y se marca `window_exact:false` en vez de darla por buena.
      `HUB_WINDOWS` tiene que coincidir SIEMPRE con las opciones del selector
      del panel.
      FASE 3 (configuración compartida) IMPLEMENTADA 2026-07-28: dos botones
      al final de FUENTES DE DATOS —no en Preferencias: dependen de tener un
      servidor, así que se leen después de la lista— (`save_hub_config` / `load_hub_config`) que guardan y
      traen los ajustes en `~/.michiclaude/config.json` del servidor. Viajan
      idioma, tema, alarmas, presupuesto, ventana, estilo del widget, capa,
      los dos temas del gatito y la LISTA de servidores (nombre/host/comando:
      es una libreta de direcciones, no un secreto, y con varios servidores
      es lo más pesado de rehacer). NO viaja la posición del widget (cada
      pantalla es distinta), ni la identidad de la máquina, ni las LLAVES
      SSH — MichiClaude no las tiene ni las lee, de eso se encarga el ssh del
      sistema, y esa promesa no se toca. Los servidores se FUSIONAN por host
      al traerlos, nunca se reemplazan: quitarle a una máquina un servidor
      que la otra no conocía la dejaría sin acceso, y recuperarlo cuesta
      mucho más que borrar una fila de sobra. Al traerlos se
      recarga el panel: hay ajustes que solo se leen al arrancar y
      repintarlos uno a uno sería fácil de olvidar al añadir el siguiente.
      DECISIÓN: es MANUAL a propósito. Una sincronización automática de ida y
      vuelta acabaría pisando en una máquina lo que acabas de cambiar en la
      otra, y unos ajustes que cambian solos son peores que unos que no se
      comparten. Al guardar se escribe en TODOS los servidores; al traer gana
      el primero que responda. Es UNA foto que se reemplaza en cada guardado,
      sin historial. Traer va en DOS PASOS —como el bote de borrar servidor—
      y el segundo dice de cuándo es lo guardado: reemplaza la configuración
      entera sin deshacer, así que confirmar sin esa fecha sería decir que sí
      a ciegas. El botón armado se desarma solo a los 8 s.
      VERIFICADO el 2026-07-28 mirando el archivo real en el VPS: se guarda
      `config.json` con idioma, tema, ventana, widget, capa y los dos temas
      del gatito. Las tres fases del hub dejan datos correctos —
      `hosts/<máquina>.json` con sus cuatro ventanas y `config.json` con los
      ajustes. La lista de servidores se pide FRESCA al backend al guardar:
      leer la variable del panel la dejaba vacía si el usuario no había
      abierto Fuentes de datos en esa sesión. ANÁLISIS COMPLETO en `docs/hub-modo-equipo.md`
      (2026-07-28): cómo funciona por dentro, el interruptor de qué compartir
      —que va POR SERVIDOR, no global—, los casos de alta/baja/formateo/
      vacaciones con ejemplos numéricos, por qué un permiso de administrador
      dentro de la app sería decorativo, y por qué el modo EQUIPO sigue
      descartado. Leerlo antes de tocar código del hub. Diseño base: el VPS consolida los datos de todas las máquinas para que
      los totales cuadren en cualquier PC. Diseño acordado: (1) cada meter
      sube su resumen local por SSH a ~/.michiclaude/hosts/<hostname>.json
      en el VPS en cada ciclo; (2) meter-export.py devuelve sus logs + los
      resúmenes de los demás hosts, excluyendo el del host que pregunta
      (--exclude-host <hostname>) para no contar doble; (3) opcional: config
      compartida (servidores/presupuesto) guardada también en el hub para que
      una PC nueva herede todo al conectar el VPS.
- [~] Auto-updater (tauri-plugin-updater) — IMPLEMENTADO 2026-07-29, sin
      probar en vivo (hace falta publicar un tag). Comprueba GitHub al
      arrancar (8 s después, para no competir con la primera carga) y avisa
      en una franja de la cabecera FIJA —visible desde cualquier pestaña— más
      un globo persistente del widget, por si el usuario no abre el panel en
      días. Comandos propios en Rust (`check_update`, `install_update`,
      `open_releases`) en vez de la API JS del plugin: invariante #4, sin
      dependencias npm de runtime.
      DOS DECISIONES DE SEGURIDAD:
      (1) Un fallo al instalar —el caso "se perdió la llave y se firmó con
      otra"— NO deja al usuario congelado sin enterarse: enseña "descárgala
      una vez a mano" con botón a las descargas.
      (2) Esa URL es una CONSTANTE en Rust (`RELEASES_URL`) y jamás sale de
      un archivo descargado. Se descartó la idea de un "archivo de avisos"
      remoto: al no ir firmado, quien lo manipulara podría mandar a todos los
      usuarios a donde quisiera. El texto puede venir de fuera; el destino
      del botón, nunca.
      La llave pública vive en tauri.conf.json (es pública a propósito); la
      privada solo en los secretos del repo y en las copias de Oscar. Si se
      pierde: llave nueva + versión nueva + los usuarios instalan a mano UNA
      vez; no se pierde ningún dato porque todo es local.
      El workflow ya firma (lo aplicó Oscar el 2026-07-29 desde la web: el
      token de este entorno NO puede tocar `.github/workflows/`, GitHub lo
      rechaza sin el permiso `workflow` — cualquier cambio ahí hay que
      pedírselo a él). NO hace falta `uploadUpdaterJson`: tauri-action ya lo
      publica por defecto (y OJO, `includeUpdaterJson` no existe).
      Los dos secretos ya están cargados en el repo (Oscar, 2026-07-29).
      FALTA: (a) DECIDIR si el repo se hace público —con él privado las
      releases no se pueden descargar sin autenticación y el updater
      devolvería 404 a todo el mundo, incluido Oscar— y (b) publicar un tag
      para probarlo de punta a punta. Hasta entonces el código está puesto y
      no estorba: sin releases, la comprobación no encuentra nada y calla.

## Diferenciadores estratégicos (post-pulido Windows, decididos 2026-07-24)

Tras investigar la competencia (Mac saturado con 8+ apps de menu bar; Windows
competido pero ganable; Linux sin app gráfica nativa = hueco). El combo actual
—cuota real + costo por proyecto + multi-máquina + gatito— ya es único; casi
nadie junta cuota Y costo, casi nadie hace multi-máquina, y NADIE tiene mascota.
Tres apuestas priorizadas, a trabajar DESPUÉS de pulir Windows:

- [x] **APUESTA #1 — Modo HUB TERMINADO** (2026-07-29, ver arriba). Es el foso técnico
      real y lo más difícil de copiar; los demás leen una sola máquina. Además
      es lo más vendible en el CV de Oscar ("sistema distribuido que consolida
      uso entre máquinas"). Prioridad #1: no es función nueva, es rematar lo
      que ya lo hace único.
- [ ] **APUESTA #2 — El gatito como motor de marketing** (lo único que 0
      competidores tienen). (a) **Tarjeta semanal para compartir**: botón que
      genera una imagen bonita del resumen (cuota, proyecto top, gatito en su
      estado) lista para redes → marketing viral incorporado (Oscar ya postea
      el gato a mano). (b) **Gamificación ligera**: rachas ("N días sin pasarte
      del presupuesto → gato feliz"), estados de ánimo con el tiempo. Barato,
      imposible de copiar (es la marca), ataca el problema real: que nadie
      conoce un proyecto nuevo. Empezar por la tarjeta (mejor esfuerzo/impacto).
- [ ] **APUESTA #3 — Analizador de fugas de tokens** — SIGUIENTE
      IMPLEMENTACIÓN GRANDE. Diseño completo en `docs/analizador-fugas.md`
      (2026-07-29): los cinco elementos de un hallazgo, el catálogo de
      detectores, por qué es DETERMINISTA y nunca un modelo local, y la
      redacción de los mensajes. Leerlo antes de escribir código.
      Orden acordado: (1) MCP servers inactivos —resta de conjuntos, el más
      barato—, (2) archivos releídos, (3) sesiones que se inflan, y luego el
      antes/después, que no depende del código sino de tener semanas de
      historial a los dos lados.
      DOS TRAMPAS ANOTADAS: el CLAUDE.md inflado se MIDE con los conteos de
      cache read que ya están en los JSONL, nunca se estima multiplicando
      líneas por turnos (exagera ~5x porque tras el primer turno está
      cacheado); y el 70% del tiempo va en validar que los números son
      correctos, no en escribirlos.
      La parte de NEGOCIO (precio, auditorías, posicionamiento) vive FUERA
      del repo en `~/.michiclaude/notas-negocio-analizador.md`: al hacer
      público el repo para el updater se publica también todo el historial de
      git, y un archivo borrado después sigue siendo legible.
      Idea original, más amplia:
      insights accionables — proyección SEMANAL ("a este ritmo llegas al límite
      el jueves"), desglose caro por proyecto ("60% es lectura de caché"),
      sugerencia de ahorro por modelo ("usaste Opus donde Haiku bastaba →
      ahorro $X", con cuidado). Eleva de "app de gauges" a "app que me ayuda a
      no quedarme sin cuota / gastar menos".

NO hacer (dilución de foco): rastrear otras herramientas (Codex/Gemini/Copilot),
base de datos de historial largo (contradice "nada que se pueda perder"), modo
equipo/empresa (fuera del público Pro/Max individual).

## Retención de los logs (requisito del analizador)

Claude Code borra `~/.claude/projects/**/*.jsonl` a los **30 días** por
defecto. El analizador de fugas compara ventanas ANTES y DESPUÉS de aplicar
un fix, así que sin historial no tiene contra qué comparar — y lo borrado no
se recupera. Se sube con `cleanupPeriodDays` en `~/.claude/settings.json`:

- VPS: puesto en **365** el 2026-07-29 (respaldo en `settings.json.bak`).
- Windows de Oscar: PENDIENTE.

## Comandos

```powershell
npm install        # CLI de Tauri (solo devDependency)
npm run icons      # regenera src-tauri/icons desde app-icon.png
npm run dev        # desarrollo (compila Rust la 1.ª vez, luego rápido)
npm run build      # release: EXE + instalador NSIS en src-tauri/target/release/bundle/nsis/
cd src-tauri; cargo check   # verificación rápida del backend
```

Verificación obligatoria al terminar cualquier cambio en Rust: `cargo check`
limpio dentro de `src-tauri`, y listar archivos tocados con el motivo.
(En el VPS NO hay toolchain de Rust: es solo espejo de código. `cargo check`
corre en el Windows de Oscar. CONSECUENCIA: al cambiar la FIRMA de una
función hay que buscar TODOS sus usos con grep antes de subir — el
compilador no está para avisar. Pasó el 2026-07-29 con `wsl_claude_dirs`:
se actualizó el uso de la agregación y se olvidó el del token de
respaldo, y el error lo descubrió Oscar al compilar.)

**Simulador** (Preferencias). Se adapta al widget elegido:
- Con el GATITO ("🐱 Simular estados") recorre el ciclo COMPLETO —dibujo y
  globo— en cinco pasos: normal (sin globo) → fire (globo de alarma, con TU
  umbral configurado) → break → zzz → normal + globo de cuota restablecida.
- Con la PASTILLA ("🔔 Simular avisos") no hay mascota que cambiar, así que
  recorre los AVISOS, uno por severidad e icono del popover: alarma ámbar →
  alarma roja → break → zzz → presupuesto (SOLO si el usuario configuró uno:
  sin cifra real no se inventa un importe) → restablecida. La alarma sale dos
  veces a propósito: verlas seguidas es la única forma de comparar ámbar y
  rojo. El paso puede sobreescribir lo que calcula `balloonMeta()`.
`simRunning` es la bandera de "simulación en curso" — NO `simMascot`, que
con la pastilla es null siempre. Usa los datos reales cuando existen
(tu `resets_at`, tu alarma) y solo inventa la fecha si aún no hay lectura.
Los globos simulados NO tocan localStorage: al terminar no debe quedar
ningún aviso pendiente falso; `processAcks()` y `notif:ack` se inhiben
mientras corre, y `simStop()` llama a `processAcks()` para devolver el
globo REAL que hubiera pendiente. Pausa AJUSTABLE entre cada paso
(campo de minutos al lado, admite decimales: 0.5 = 30 s, mínimo 5 s,
persistido en localStorage `simMin`), para ver el arte sin esperar a agotar
la sesión o la semana. Mientras corre,
`mascotState()` devuelve el estado forzado y los refrescos normales no lo
pisan; vive en memoria (cerrar la app cancela). SOLO aparece en compilaciones
de desarrollo (comando `is_dev` = `cfg!(debug_assertions)`), por eso es el
único control cuyos textos no pasan por `t()` — nunca llega a un usuario.

## Flujo de trabajo del repo

- Remoto: `https://github.com/oscarorozcos/michiclaude` — PRIVADO a fecha
  2026-07-29 (comprobado: da 404 sin sesión). La nota de "público desde
  2026-07-24" era falsa. CONSECUENCIA para el auto-updater: los archivos de
  una release privada NO se pueden descargar sin autenticación, así que el
  endpoint configurado devolvería 404 a todo el mundo. El updater no puede
  funcionar hasta que el repo (o al menos sus releases) sea público.
- El desarrollo y las pruebas ocurren en el PC Windows de Oscar; en el VPS vive
  un clon espejo (`/opt/projects/michiclaude`) para revisión de código.
- Antes de empezar a trabajar en cualquiera de los dos lados: `git pull`.
  Al terminar y verificar: commit (Conventional Commits en español) y push.

## Contexto de producto (para decisiones de diseño)

- Usuario objetivo: suscriptores Pro/Max que usan Claude Code y quieren saber
  (1) cuánto les queda, (2) cuándo se les acaba al ritmo actual, (3) qué
  proyecto/modelo consume más.
- El coste en $ es **nocional** (equivalente API) para suscriptores; solo es
  gasto real para usuarios de API key. La UI lo etiqueta como "equiv. API".
- Competencia de referencia: ccusage (CLI, $/proyecto), claudeusagewin y
  usage-monitor-for-claude (tray Windows con cuota real). Diferenciadores de
  esta app: marcador de ritmo + proyección + franja
  sobre la barra.
- Se publicará en GitHub (GPL-3.0, releases automáticas por tag). La confianza del
  usuario es prioridad: transparencia total sobre el manejo del token y el
  disclaimer del endpoint no oficial.
