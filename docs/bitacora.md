# Bitácora de desarrollo — MichiClaude (2026-07 → 2026-08)

Este archivo es el HISTORIAL completo del proyecto: jornadas, validaciones
en vivo, decisiones con su porqué, bugs con su autopsia. Era el CLAUDE.md
hasta el 2026-08-04, cuando pasó de 118k caracteres y Claude Code (que
solo carga 40k) empezó a cortarlo — las reglas VIGENTES se destilaron al
CLAUDE.md nuevo y el historial íntegro quedó aquí, consultable con grep.
Al cerrar una jornada, las validaciones nuevas se anotan AQUÍ; en
CLAUDE.md solo se actualizan reglas y pendientes.

Cierre del 2026-08-04, ya con este archivo mudado: hover y clics del
gatito con la cabeza recalibrada VALIDADOS en vivo por Oscar; el aviso
de hallazgos al cierre de sesión pasó a la lista de validación pasiva.

---

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
   "**/*" incluye `<sesión>/subagents/agent-*.jsonl` vía `project_jsonls()`
   (2026-08-04): Claude Code moderno pone ahí los transcripts de los
   subagentes — sin entrar a esa subcarpeta, sus tokens no se cobraban.
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
  IDA Y VUELTA del 2026-07-31 (dejarlo escrito evita repetirla): los
  huecos semanal/por-modelo SALIERON de la cápsula ("repetitivo con el
  detalle") y VOLVIERON a las horas — Oscar los extrañó como vistazo
  rápido. Quedaron: los huecos de vuelta, ancho de vuelta a 280 (las
  ventanas se definen en ensure_widget_windows, NO en el json), el grupo
  de contenido CENTRADO entre asa y flecha (márgenes auto en .mkbtn y
  .chev), y el gatito+Sesión sin truncarse. Lo que SÍ quedó de esa ronda:
  (1) SIN tooltips nativos en la cápsula (ni title del cap ni del bucket
  por modelo: se veían encima del detalle); (2) el hover para desplegar
  se probó y Oscar lo DEVOLVIÓ a clic el mismo día — no reintroducirlo;
  (3) la CABECERA GEMELA del detalle ya no es puro dibujo (parecía rota:
  su gatito no abría el panel y su asa no movía): el gatito abre el
  panel y el asa arrastra de verdad vía `drag_pill_from_card` — Rust
  pliega, muestra la pastilla (que quedó exactamente bajo la cabecera,
  el despliegue no la mueve) y le pasa el arrastre del sistema en el
  mismo gesto; la posición se persiste con salvados repetidos ~2.5 s.
  VALIDADO en vivo por Oscar el 2026-07-31 ("funciona bien"). El clic
  derecho oculta. Y el remate del mismo día: con el detalle ABIERTO la
  cabecera ya no repite los números (se veían duplicados con las filas)
  — CSS del pcard esconde .lab/.pct/.m del hdr; quedan asa, gatito y
  flecha. La regla de geometría IDÉNTICA entre cápsula y cabecera sigue
  (tamaños y posiciones); lo que cambia es solo el contenido visible.
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
   MATIZ OBLIGATORIO (2026-07-29): `security` lleva
   `"dangerousDisableAssetCspModification": ["style-src"]`, y NO se puede
   quitar sin romper la app COMPILADA. Tauri, al compilar, inyecta sus
   propios nonces y hashes en la CSP; y el estándar CSP dice que cuando una
   directiva tiene un nonce o un hash, se IGNORA `'unsafe-inline'`. Resultado:
   en release se bloqueaban todos los estilos escritos al vuelo —el ancho y el
   color de cada barra de gasto, las barras de la tendencia diaria, las del
   globo del gatito y las del detalle de la pastilla, y la línea de precios de
   Preferencias— mientras en desarrollo se veía perfecto, porque ahí no se
   inyecta nada. La app instalada se veía a medio pintar y NADIE lo habría
   achacado a la CSP. Lo encontró Oscar comparando dev contra release.
   Lo que este ajuste NO toca: `script-src` sigue con la protección de Tauri
   intacta, que es donde vive el riesgo de XSS de verdad. Solo se le pide a
   Tauri que no reescriba `style-src`, para que sobreviva el `'unsafe-inline'`
   que ya estaba declarado a mano. La alternativa —pasar los ~40 estilos en
   línea a asignaciones por JavaScript, que la CSP no bloquea— se descartó por
   ser un refactor grande de una interfaz recién validada.
   AL DIAGNOSTICAR: si algo se ve bien con `npm run dev` y mal con
   `npm run build`, sospechar de la CSP ANTES que del código.
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
    set_notif_visible, toggle_pill_card, set_pill_visible, update_tray): esos
    tienen que seguir en el hilo principal, y además son instantáneos.
    EXCEPCIÓN, y por un motivo OPUESTO: `set_pill_style` SÍ es `async fn`
    desde el 2026-07-29, no porque sea lento sino porque CREA una ventana.
    Crear una ventana desde un comando síncrono CONGELA la app entera en
    Windows: la ventana nueva necesita que el bucle de eventos avance para
    nacer, y ese bucle está detenido esperando a que el comando termine. Se
    esperan mutuamente. Pasó en vivo al cambiar a la pastilla —el gatito se
    quedaba en pantalla y el panel dejaba de responder a todo, hasta al
    filtro de días— y se arregló solo añadiendo `async`, que hace que Tauri
    lo ejecute fuera del hilo de la interfaz. En `setup()` la misma llamada
    puede ser síncrona porque ahí el bucle todavía no ha arrancado — por eso
    el gatito aparecía bien al arrancar y el fallo solo salía al cambiar de
    widget. REGLA para lo que venga: todo comando que cree ventanas tiene
    que ser async.
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
    EL CASO RARO: el menú del icono de bandeja (clic derecho) lo construye
    RUST al arrancar, cuando el idioma —que vive en el localStorage del
    panel— todavía no se conoce. Se quedaba en inglés para todo el mundo
    hasta el 2026-07-29, que lo vio Oscar con la app en español. Ahora el
    panel se lo manda ya traducido desde `applyI18n()` con el comando
    `set_tray_menu`, al cargar y en cada cambio de idioma; las etiquetas de
    `setup()` quedan como respaldo para los milisegundos previos.
    VALIDADO en vivo por Oscar el 2026-07-29 en español y japonés, cambiando
    el idioma con la app abierta: el menú cambia al momento, sin reiniciar. REGLA: si
    algún día se añade otra cosa que Rust dibuje con texto, tiene que llegarle
    igual — desde el panel, nunca escrita en el backend.
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
- [x] Fuente WSL — VERIFICADA EN VIVO el 2026-07-29, tras semanas anotada
      como "sin probar". Resulta que el Windows de Oscar SÍ tiene Ubuntu
      (llegó con una actualización y nunca lo usó); se descubrió mirando la
      lista de procesos por RAM, donde aparecía `vmmemWSL`.
      Como no había uso real, se probó con un registro sintético: un
      `.jsonl` de una sola entrada escrito en
      `\\wsl.localhost\Ubuntu\home\oscar\.claude\projects\-tmp-prueba-wsl\`
      y borrado al terminar. Salió la fila `prueba-wsl` con la etiqueta
      `wsl-Ubuntu` en las ventanas 1d/7d/30d. Con eso queda probada la cadena
      COMPLETA: `wsl.exe -l -q` con su UTF-16LE, el barrido de
      `home/*/.claude`, la lectura desde Windows por `\\wsl.localhost`, el
      parseo y el sufijo con el nombre REAL de la distro.
      REGALO: el coste salió $0.40 donde la tabla embebida daría $0.60 —
      es el precio INTRODUCTORIO de Sonnet 5 de la tabla descargada. Sin
      buscarlo, quedó probado que los precios dinámicos se aplican de verdad.
      TOKEN DE RESPALDO desde WSL — probado a medias el 2026-07-29, falta el
      último eslabón. NO hace falta iniciar sesión dentro de Ubuntu: basta un
      `.credentials.json` con token FALSO y `expiresAt` en el futuro, porque
      `get_quota` se queda con el PRIMER token válido (Windows -> WSL ->
      remotos) y si ese da 401 NO prueba con el siguiente. Así, con la
      credencial de Windows apartada y `remotes.json` apartado, cualquier
      respuesta HTTP demuestra que usó el de WSL, y un ERR_NO_TOKEN
      demostraría lo contrario.
      VERIFICADO de esa cadena: la app lista la distro (`Ubuntu`), recorre los
      hogares (`oscar`) y encuentra la carpeta `.claude` — los tres pasos que
      podían fallar. Falta ver la llamada final, que quedó tapada por un 429.
      LO QUE ESTORBÓ, y hay que preverlo al repetirlo: abrir y cerrar la app
      muchas veces seguidas (compilar, probar, medir) dispara una consulta en
      cada arranque y acaba en un 429 de 60 MINUTOS. Con el endpoint
      bloqueado no se puede leer ningún veredicto, porque la app respeta el
      Retry-After y deja de preguntar. Repetir la prueba en frío, sin haber
      reiniciado la app en un rato.
      CÓMO REPETIR LA PRUEBA (dos trampas que costaron intentos): escribir el
      archivo desde `wsl bash -lc 'echo "{...}"'` NO funciona — PowerShell se
      come las comillas y deja un archivo de 20 bytes; hay que escribirlo
      desde Windows por la ruta `\\wsl.localhost`. Y con `Set-Content` hay
      que usar `-Encoding ascii`: la opción `utf8` mete un BOM que rompe la
      primera línea del JSON y parecería un fallo de la app.
      Fuente WSL implementada (el sufijo es "wsl-<distro>" desde
      el 2026-07-29; antes todas caían bajo un "wsl" genérico y con dos
      instaladas no había forma de distinguirlas): `wsl.exe -l -q` (UTF-16LE) + escaneo de
      \\wsl.localhost\<distro>\{home/*,root}\.claude como fuente local extra
      (proyectos "nombre · wsl") y token de respaldo desde WSL antes que los
      remotos. FALTA probar en una máquina con WSL real.
- [x] El resumen del widget se RECALCULA entero al cambiar de widget, no se
      parchea (2026-07-29). `skinTheme` se decide SEGÚN el estilo —con la
      pastilla los globos siguen al panel; con el gatito, al selector "Globos
      del gatito"—, así que parchear solo `style` en el objeto anterior
      dejaba el tema del ciclo pasado: al irse a la pastilla y volver, el
      globo del gatito regresaba en oscuro con "Claro" elegido. Lo encontró
      Oscar probando el cambio de ventanas. REGLA: si se toca algo que
      `emitPill()` calcula, se llama a `emitPill(...lastPillArgs)` — nunca se
      escribe un campo suelto de `lastPill`.
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

- [x] ARTE REESCALADO a la mitad (2026-07-29, validado en vivo por Oscar en
      los dos temas: "se ven bien, no se ven mal a nivel visual"). Los gifs
      venían en 800² para dibujar un gato de ~180 px en pantalla — 19 veces
      los píxeles que se ven. A 400² sobra incluso para pantallas de alta
      densidad y el arte pasa de 11.1 MB a 3.0 MB. El `cat-break-black` de
      lienzo raro quedó en 706x430 (misma proporción, desvío 0.071%).
      NO se tocó código: el recorte del margen transparente está en
      PORCENTAJES desde el principio y por eso sobrevivió al cambio de
      escala. Verificado por programa antes de reemplazar: mismos fotogramas,
      transparencia intacta, proporción del lienzo idéntica.
      Hecho con `gifsicle --scale 0.5 --resize-method mix --colors 256 -O3`.
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
- [~] **APUESTA #3 — Analizador de fugas de tokens** — PRIMERA VERSIÓN
      FUNCIONANDO, validada en vivo por Oscar el 2026-07-29 (tarjetas, colores
      de severidad, Ignorar, selector de ventana e idiomas). Tres piezas:
      (1) MOTOR en `meter-export.py` (`scan_findings`, flag `--findings`) —
      pasada aparte sin caché que el ciclo del panel nunca paga;
      (2) RÉPLICA Rust (`scan_local_findings` + comando `get_findings`, async
      por partida doble: SSH y escaneo de disco) que analiza este PC + WSL y
      pide los hallazgos de cada servidor; mantener AMBOS lados en sincronía
      como la agregación;
      (3) PESTAÑA Hallazgos (cuarta, entre Fuentes de datos y Preferencias):
      severidad en el borde, costo en $ y tokens con "~" solo donde hay
      heurística, recomendación del catálogo en tono "hazlo así y sale
      gratis", Ignorar persistente (localStorage `fndIgnore`) y ventana
      propia (`fndDays`).
      Detectores v1: archivos releídos (MIDE los chars devueltos por cada
      lectura — no estima por tamaño de archivo), sesiones que acumulan
      contexto (costo de cache_read MEDIDO del log), peticiones mecánicas
      (solo git/pytest/cargo check/npm test-ci-install; lista corta a
      propósito) y MCP configurado sin invocar. Umbrales en constantes
      (REREAD_MIN 3 / 2000 tok, INFLATE 50k+10 turnos, MECH 5, tope 12).
      Detector 5 (2026-07-29; cargo check limpio y VALIDADO EN VIVO por
      Oscar el mismo día — la tarjeta salió etiquetada VPS-EU, o sea que la
      cadena exportador→SSH→fusión→i18n quedó probada entera): RUPTURAS DE
      CACHÉ. Turnos del hilo principal donde
      cache_read cae a menos de la mitad del contexto del turno anterior:
      el prefijo cacheado se perdió y la conversación entera se reescribió
      a 1.25x input en vez de leerse a 0.1x (causas típicas: pausa mayor al
      TTL o cambio de modelo). Costo MEDIDO como piso min(cache_write,
      contexto_previo). DOS EXCLUSIONES obligatorias: isSidechain (los
      subagentes llevan SU contexto; mezclarlos fabrica rupturas falsas) y
      compactaciones (isCompactSummary/compact_boundary ±120 s — ahí
      reescribir es el ahorro, no la fuga). Umbrales CACHEBREAK_MIN_PREV
      20k / CACHEBREAK_MIN_TOKENS 300k. Validado en el VPS con exploración
      independiente ANTES de escribir el detector, cuadre exacto: $80.85 en
      la sesión de los $403 (cada pausa sobre ~900k de contexto = ~$6 de
      reescritura), $22.27 y $4.37 en otras dos; los hallazgos previos
      salieron IDÉNTICOS con y sin el cambio (diff limpio). Es la fuga más
      cara del catálogo: inflate mide RELEER, este mide REESCRIBIR.
      LA VALIDACIÓN PAGÓ DOS VECES el primer día: el cálculo manual de
      relecturas exageraba ~100x (multiplicaba por el tamaño del archivo
      cuando casi todas las lecturas eran parciales — la trampa documentada,
      cazada por el propio detector midiendo), y el costo de contexto por
      sesión cuadró contra la agregación normal (camino independiente) al
      0.4%. Hallazgo estrella real: $403 de la semana eran UNA conversación
      de 1392 turnos releyendo su propio contexto.
      INDICADOR DE HALLAZGOS NUEVOS (2026-07-29, implementado; SIN validar
      en vivo aún — solo frontend, cero cambios en Rust): una señal, tres
      superficies — sticker cómic en la laptop del gatito (posición en
      variables --bx/--by/--bs de cat.html, como la zona de la cabeza),
      puntito en la cápsula de la pastilla y contador en la pestaña
      Hallazgos. Color = severidad de la tarjeta más cara (misma escala:
      rojo ≥$10, ámbar ≥$1 o MCP, acento el resto). CÓMO FUNCIONA: el panel
      —único que escanea— corre una pasada ligera de 1 día como mucho cada
      20 h (get_findings days:1, guard fndAutoLast) y guarda el resultado en
      localStorage `fndAuto` para que reiniciar no apague un aviso que nadie
      vio; los widgets solo reciben {n, sev, tip} dentro de quota:update.
      "VISTO" = la tarjeta llegó a pintarse en la pestaña (claves en
      `fndSeen`, tope 300): el aviso se apaga y esas tarjetas no lo vuelven
      a encender — se despacha una vez, no persigue, como el globo. Ignorar
      también lo apaga. Clic en sticker/puntito = evento `panel:findings` +
      show_panel: el panel abre YA en Hallazgos. Checkbox en Preferencias
      (encendido por defecto; localStorage `fndBadgeOff`) que apaga SOLO el
      widget — el contador de la pestaña se queda siempre porque no
      interrumpe. El sticker del gato va ANTES del early-return de ok:false
      en apply() (un fallo pasajero de cuota no apaga un aviso pendiente) y
      el contador de la pestaña vive en un <span> DENTRO del botón porque
      applyI18n pisa el textContent de todo [data-i18n].
      VALIDADO en vivo por Oscar el 2026-07-29 — y el aviso funcionó TAN
      bien que la primera vez pareció roto: Oscar abrió la pestaña buscando
      la notificación y ese vistazo marcó las tarjetas como vistas antes de
      ver el sticker (fndSeen tenía las 4 claves; se confirmó por consola).
      DISEÑO FINAL, iterado en capturas y VALIDADO por Oscar el 2026-07-29
      ("se ve y funciona bien"): en el GATITO es una PILITA DE POST-ITS
      pegada en la tapa de la laptop (dos notas asoman por detrás — ámbar y
      papel, la de papel sigue al tema de los globos — y encima el post-it
      ROJO ladeado con cinta adhesiva, número blanco y translúcido al 78%;
      toda la pilita es botón; posición en --bx/--by/--bs). El rojo es FIJO,
      sin tinte por severidad — decisión de Oscar: rojo = "hay algo nuevo",
      la severidad se ve en las tarjetas. En la PASTILLA es una CAMPANA roja
      SVG (más brillante en oscuro, #ff5a5a vs #ef4444) con meneo al clic, y
      la MARCA dejó de ser el sunburst: la cápsula lleva el sticker del
      gatito (variante -white/-black según tema, misma regla que los gifs) y
      el detalle desplegado lleva icon-mini-panel.png — la mascota ES la
      marca; los tres PNG los subió Oscar desde Windows. OJO: los tres son
      IDÉNTICOS byte a byte A PROPÓSITO (validado por Oscar el 2026-07-30 en
      ambos widgets y ambos temas: "está bien") — no es un error que haya
      que corregir; si algún día quiere variantes por tema o icono propio
      del detalle, basta reemplazar los archivos, el código ya lo soporta. En el PANEL el
      contador es cápsula roja flotante en la esquina de la pestaña, tope
      9+. PRIMERA APERTURA de Hallazgos: enseña AL INSTANTE el último
      resultado guardado (localStorage fndCacheSaved, por ventana) con
      "Analizando…" en el pie mientras el escaneo fresco corre detrás y
      reemplaza al llegar; si falla, lo viejo se queda y el pie avisa. Más
      la PRECARGA de fondo a los 15 s de arrancar. DOS TRAMPAS: pintar en la
      pestaña OCULTA no debe marcar visto (mataría el aviso — el marcado
      está condicionado a !$("tab-findings").hidden) y la primera prueba
      pareció rota porque el propio Oscar abrió la pestaña buscando la
      notificación y ese vistazo la despachó. Para re-armar el aviso al
      probar: borrar fndSeen y fndAutoLast del localStorage del panel.
      LOS TRES DETECTORES DE "LO INSTALADO": HECHOS los tres el 2026-07-30,
      con cargo check limpio ese mismo día en el Windows de Oscar.
      DETECTOR 9 — líneas de CLAUDE.md sin respaldo: HECHO 2026-07-30 (kind
      claudemd; umbrales CLAUDEMD_MIN_LINES 5 / CLAUDEMD_MAX_TOKENS 400;
      solo ventanas 7+). Las tres cubetas del doc: identificadores por línea
      (backticks + rutas/archivo.ext) buscados como subcadena en el texto
      crudo de los logs; sin identificadores = gris (sin opinión); roja solo
      si NINGUNA mención aparece. CLAUDE.md global + el de cada proyecto con
      actividad (cwd de las primeras líneas del .jsonl; dedup por ruta real
      por el symlink claude-code-meter). Costo PISO chars/4 × sesiones (~),
      NUNCA líneas × turnos (la trampa del doc). Validado con fixture al
      número exacto y en el VPS (367 identificadores, 3 sin respaldo y los
      3 correctos: rutas de Windows); regresión byte-idéntica sobre copia
      congelada. Cuesta ~7 s extra en --findings 7d+ (asumido y
      documentado); cargo check limpio 2026-07-30. Detalle completo en
      docs/analizador-fugas.md §11.
      EL GLOBO DEL DÍA — IMPLEMENTADO 2026-07-30 y VALIDADO EN VIVO por
      Oscar el mismo día (capturas: globo con hallazgo real del VPS "164
      turnos · ~$34.17", clic abrió Hallazgos con la tarjeta VPS-EU, y al
      verla se despacharon globo y post-it — la cadena completa probada de
      una vez; solo frontend: index.html + notif.html, cero Rust). Un hallazgo
      NUEVO con costo ≥ umbral configurable (fndNudgeUsd, $1 default,
      campo en Preferencias) sale como globo notif kind "findings" al
      terminar la pasada diaria: título de tarjeta + ~$X, clic = panel
      directo en Hallazgos. Máx 1/día, la cuota gana (ackPending o
      infoBalloon puesto → reintenta mañana), sin toast, cada hallazgo
      avisa UNA vez (fndNudged), comparte el checkbox fndBadgeOff.
      PROBAR: borrar fndAutoLast/fndNudged/fndSeen y recargar; a los 90 s
      escanea y si hay hallazgo de hoy ≥$1 sale el globo. Detalle en
      docs/analizador-fugas.md §11.
      ... Y ELIMINADO 2026-08-04 (decisión de Oscar, tras un mes de uso):
      el globo era más intrusivo que útil y su trabajo lo hace el
      indicador pasivo. EN SU LUGAR: los hallazgos avisan como Consejos —
      post-it rojo / campana / contador de pestaña que se encienden cada
      vez que hay hallazgos NO VISTOS, sin tope diario. Para que eso pase
      el mismo día, la pasada ligera de 1d ahora también se dispara AL
      NACER UN RECIBO (sesión local terminada = el momento en que nacen
      hallazgos), con freno propio de 15 min (fndEventLast, marcado ANTES
      de la pasada para no reentrar); la diaria de 20 h queda como
      respaldo para hallazgos que nacen sin cerrar sesión local (VPS).
      fndPass(tag) es la función compartida y cada disparo refresca
      fndAutoLast (una pasada por cierre pospone la diaria: mismo dato).
      FUERA: fndNudge/fndNudgeUsd/fndNudged/fndNudgeSev, el campo del
      umbral en Preferencias, las claves i18n fnd_nudge_* (×8), el kind
      "findings" de notif.html y el paso "globo del día" del simulador
      (quedó en 2 pasos). Los localStorage viejos (fndNudgeUsd/fndNudged)
      quedan huérfanos e inofensivos. Solo frontend: index.html +
      notif.html, cero Rust. SIN validar en vivo: probar cerrando una
      sesión local con una fuga fresca — post-it/campana deben encender
      a los ~10-13 min (recibo + pasada) SIN globo.
      SECCIÓN CONSEJOS — PRIMERA ENTREGA HECHA Y VALIDADA EN VIVO por
      Oscar el 2026-07-30 (capturas: las 5 pestañas caben, el acordeón
      abre/cierra, el filtro "clear" deja las 3 fichas correctas, y el
      cambio de idioma ES↔EN repinta las fichas al momento): quinta
      pestaña "tips", molde de tarjeta compartido con variante `.fnd.tip`
      (acordeón, sin costo ni severidad), 6 fichas ×8 idiomas
      (tip_<id>_t/_b en I18N, ids en TIPS), filtro cliente y repintado al
      cambiar idioma. Único ajuste de la validación: el cuerpo de las
      fichas a 12.5px/1.5 (el 11px del fix de Hallazgos es para una línea,
      no para párrafos — lo notó Oscar). Diseño completo en
      docs/consejos-coach.md — LEERLO antes de tocar esta sección.
      MOTOR DE REGLAS DE SESIÓN ACTIVA — HECHO 2026-07-30 con cargo check
      limpio ese día; SIN validar en vivo (necesita sesión de Claude Code
      activa EN WINDOWS — la del VPS no cuenta: el coach mira la máquina
      donde corre la app): comando `get_coach` (Rust, lectura incremental
      por offset de los logs tocados en 30 min, SOLO locales) devuelve
      hechos medidos y el panel les pone el anti-spam (tipSeen por
      sesión+regla, tope diario 5) y los pinta: ficha primera, abierta,
      línea "Ahora: <dato>" en acento y contador acento en la pestaña.
      Tres reglas v1: ctx≥120k → compact; pausa≥6min con ctx≥30k → cache;
      mismo archivo leído ≥3 veces → attach. Detalle en
      docs/consejos-coach.md §10.3.
      VALIDADO PARCIALMENTE en vivo el 2026-07-31 (la regla de contexto
      alto disparó con ~372k reales y el contador acento salió); del
      feedback de Oscar salieron tres ajustes YA implementados:
      la ficha dice a qué sesión aplicar ("· proyecto · local", campo
      project en CoachHit — cargo check limpio 2026-07-31, compilación
      desde cero en 1m41s tras la mudanza del repo), el coach avisa en el
      widget (post-it ACENTO junto a la pilita del gatito, punto acento
      junto a la campana de la pastilla, clic = panel en Consejos vía
      panel:tips, campo coach en quota:update, mismo interruptor
      fndBadgeOff) y el "sin datos" del widget dice el MOTIVO (reason =
      errText en quota:update, #why en card.html, tooltip del tray) —
      decisión: sin animación especial de sin-datos, el dibujo del gatito
      refleja solo el estado real de la cuota.
      COACH VALIDADO EN VIVO COMPLETO el 2026-07-31 (captura de Oscar):
      regla del caché con dato real ("26 min de pausa"), ficha con
      "test · local" y post-it turquesa en la laptop del gatito. El
      post-it ganó su cinta adhesiva tras la validación (sin ella parecía
      un cuadrito — Oscar). Su pregunta "¿por qué Consejos no manda globo
      como Hallazgos?" quedó respondida y documentada en
      docs/consejos-coach.md §10.3: a propósito, el globo es solo para lo
      que duele en dólares.
      RESUMEN DE SESIÓN — IMPLEMENTADO 2026-07-31 y VALIDADO EN VIVO el
      2026-08-01 (captura de Oscar: «Crear calculadora web completa con
      pruebas» — 4 min · 0 comandos · 7 archivos): sesión quieta 10+ min
      con 5+ turnos → tarjeta «ai-title» + minutos/comandos/archivos
      editados, arriba de Consejos, una vez por sesión. Detalle en
      docs/consejos-coach.md §8.
      MINI-AUDITORÍA AL CIERRE — IMPLEMENTADA 2026-08-02 (SIN cargo check
      ni validación en vivo; idea de Oscar: "si ya vigilamos esa sesión
      larga que gasta, lo que interesa es el AHORRO, no solo que acabó").
      CoachSess acumula `cost` (usage × tarifa por turno, cost_of) y
      `gaps` (pausas ≥6 min con contexto ≥30k, contadas turno a turno);
      al disparar done/sum, `coach_leaks()` arma la lista con lo que ya
      está EN MEMORIA (cero re-escaneo): reread ≥3 del archivo más releído,
      ctx final ≥120k, gaps>0 — kinds que casan con las fichas del catálogo
      (attach/compact/cache). La tarjeta del resumen gana `· ~$X` (oculto
      bajo medio centavo) y líneas ⚠ vía `tipLeak()`; el push de ntfy gana
      SOLO el conteo ("· 1 aviso de ahorro", clave ntfy_done_save ×8 con
      singular en los 5 idiomas que declinan) — ni dólares ni archivos ni
      reglas por el canal público (regla de privacidad intacta). Claves
      nuevas tip_leak_reread/ctx/cache/gap ×8. AJUSTE del 2026-08-02 tras
      la segunda prueba real de Oscar (push sin sufijo y un minuto después
      el consejo del caché en el panel): el push sale a los 5 min de
      silencio pero la regla viva del caché pide 6 — historias distintas
      por 60 segundos. Ahora cerrar con contexto ≥30k (sin llegar a los
      120k del ctx) ES fuga al cierre (kind "cache"): el usuario está
      lejos y el TTL se vence antes de que lea el push. ctx y cache son
      excluyentes (else if) para no contar el mismo contexto dos veces.
      PROBAR: tanda en test-agente que relea un archivo 3+ veces o cierre
      con 30k+ de contexto, quieta 5 min → push con "· N avisos de
      ahorro"; la tarjeta con ~$ y ⚠ a los 10 min.
      "CLAUDE ESTÁ ESPERANDO TU APROBACIÓN" — IMPLEMENTADO 2026-08-02 (SIN
      cargo check ni prueba en vivo) tras el falso positivo que cazó la
      prueba real de Oscar: dejó la tarea, Claude se detuvo en un permiso,
      y a los 5 min el push dijo "terminó · 6 min, 42 turnos" con el
      permiso en pantalla; el final real quedó mudo (notified ya gastado)
      y el resumen se lo comió el tope diario. TRES arreglos: (1) señal
      `pending_tool` (tool_use sin tool_result = sesión detenida) → regla
      `ask` a los 3 min (COACH_ASK_QUIET), push prioridad 4 con minutos
      detenido, rearmable al crecer el log (`asked`) y dedup frontend por
      sesión+turno (ntfyAsked); (2) con pending_tool puesto NI "terminó"
      NI el resumen disparan — así el "terminó" queda armado para el final
      real; (3) el resumen (`sum`) exento del tope diario de 5 del coach.
      Claves ntfy_ask_body(_p) ×8; misma casilla `done` de Preferencias.
      VALIDADO EN VIVO 2026-08-02 (capturas del teléfono de Oscar): DOS
      atascos avisados con prioridad alta ("Claude espera tu aprobación en
      test-agente · 4 min detenido" a las 4:27 y 4:33), CERO "terminó"
      falsos en medio, y el final real a las 5:00 con "· 28 min, 117
      turnos · 3 avisos de ahorro". El circuito ask→done→auditoría entero.
      Y de esa misma prueba salió OTRO bug, arreglado el mismo día: la
      TARJETA del recibo con la auditoría no aparecía en Consejos. Causa:
      Rust emite el `sum` UNA vez (banderín done) y coachHits se REEMPLAZA
      en cada sondeo de 3 min — si la pestaña no estaba abierta justo en
      esa ventana, la tarjeta moría sin ser vista. Arreglo: almacén
      `coachSums` en localStorage (tope 5) — el sondeo guarda los recibos
      nuevos, renderTips los lee de ahí (en simulador sigue leyendo
      coachHits: el sim no toca localStorage), y solo se borran cuando
      llegaron a PINTARSE con la pestaña visible y sin filtro activo.
      `coachCount()` (fichas + recibos pendientes, sin doble conteo)
      alimenta el contador de la pestaña y el aviso del widget — así el
      post-it sobrevive a reinicios igual que el de hallazgos.
      Y REDISEÑO DEL CICLO DE VIDA 2026-08-03 (pedido de Oscar: "como
      Hallazgos"): almacén `coachCards` (tope 12, sustituye a coachSums)
      con TODAS las tarjetas vivas — recibos y "Ahora" —, cada una con ✕,
      contraer/expandir recordado (`min`), ver la pestaña apaga el aviso
      (`v`) SIN despachar nada, y caducidad automática a las 24 h de
      nacer (TIP_TTL — no a medianoche: un recibo de las 11 pm debe
      llegar a la mañana). Tope diario 5→10 y UNA tarjeta viva por regla
      (la medición nueva reemplaza a la vieja). tipSeen se marca al
      ENTRAR al almacén, no al verse. coachHits queda SOLO para el
      simulador. Ciclo completo probado con arnés en el VPS (nacer →
      reemplazo → visto → contraído → ✕ → caducar). Detalle en
      docs/consejos-coach.md §8bis. Las FICHAS con ✕ ya se vieron en vivo
      (capturas de Oscar 2026-08-03); y el RECIBO del ciclo nuevo quedó
      VALIDADO el 2026-08-04 (ver la entrada del resumen de sesión, abajo).
      Y DE ESA MISMA PRUEBA, UN BUG GORDO (2026-08-03, a0d02bc): con tres
      fichas calientes encendidas (162k ctx, 10 relecturas) NO salieron ni
      el push de "terminó" ni el recibo — un PENDIENTE FANTASMA:
      pending_tool quedaba atorado en true (un tool_use cuyo tool_result
      no se vio con la forma esperada) y bloqueaba done y sum para
      siempre. Blindaje doble: (1) un turno nuevo del hilo principal
      LIMPIA pending_tool (una sesión que avanza no está esperando
      permiso; si ese mismo mensaje trae tool_use, el bucle de bloques lo
      repone) y (2) los tool_use de SUBAGENTES ya no tocan el pendiente.
      Además OBSERVABILIDAD: coach_debug.json en la carpeta de datos, se
      escribe en cada sondeo con las compuertas de cada sesión viva (sid,
      turnos, quiet_min, ctx, pending, asked, notified, sum_done, gaps,
      cost) y los hits emitidos — al depurar "no llegó el push", LEER ESE
      ARCHIVO PRIMERO. VALIDADO EN VIVO 2026-08-02 tras el blindaje
      (línea de tiempo de Oscar: sesión termina 7:20 → post-it turquesa
      SOLO, sin abrir el panel, 7:27 → push "1 min, 19 turnos · 1 aviso
      de ahorro" 7:29; y en sus capturas la ficha con ✕ y el contraer
      recordado). Y el RECIBO también VALIDADO el mismo día (captura:
      «Mantenimiento ligero de la calculadora» · 1 min · 0 comandos · 3
      archivos · ~$1.24 · ⚠ cerró con 53k — el caché venció). De su
      pregunta "¿por qué no volvió el post-it con el recibo?" salió el
      ÚLTIMO bug de la cadena: el panel al cerrarse solo se OCULTA y la
      pestaña Consejos seguía activa por dentro — el sondeo pintaba el
      recibo recién nacido en la ventana invisible y lo marcaba VISTO SIN
      VERSE. Arreglo: el marcado exige document.hasFocus() además de la
      pestaña visible (variante nueva de la trampa ya documentada en
      Hallazgos). El recibo NO manda push propio A PROPÓSITO: su push fue
      el "terminó · N avisos de ahorro"; el recibo es el detalle.
      FLUJO COMPLETO CRONOMETRADO por Oscar el 2026-08-02 en proyecto
      virgen (test-local), TODO dentro de especificación: atasco 7:55 →
      push aprobación 8:00 (compuerta 3 min + sondeo); terminó 8:04 →
      post-it 8:12 (ficha caché, 6 min + sondeo) → push 8:14 ("2 avisos
      de ahorro" = las DOS ⚠ del recibo, no dos fichas — confusión
      esperable) → recibo 8:18 re-encendiendo el post-it (el fix
      hasFocus probado en vivo). El aviso de APROBACIÓN es solo-celular
      a propósito (frente a la PC ya ves la pregunta en la terminal).
      DE ESA PRUEBA, DOS CAMBIOS (2026-08-02): (1) BUG REAL — Hallazgos
      solo escaneaba la PRIMERA vez por arranque y las sesiones
      posteriores no aparecían hasta reiniciar; ahora el reporte en
      memoria se refresca al abrir la pestaña si tiene >5 min (lo viejo
      queda a la vista con "Analizando…" mientras corre el fresco).
      (2) BITÁCORA DEL FLUJO (pedido de Oscar: "necesitamos logs para no
      adivinar con capturas"): flog() apunta con hora cada activación —
      pushes ok/error, tarjetas del coach al nacer, vistas/✕, globos,
      escaneos de hallazgos y pasada diaria — en localStorage flowLog
      (tope 300); botón 📜 junto a los simuladores (solo dev) la copia
      al portapapeles, Mayús+clic la vacía. Complementa a
      coach_debug.json (compuertas Rust): entre los dos se reconstruye
      cualquier "no me llegó X" sin especular.
      Y ORDEN DE HALLAZGOS POR RECIENTES (2026-08-02, pedido de Oscar: su
      hallazgo fresco quedaba hasta abajo por barato): Finding gana `ts`
      (última actividad de la sesión, epoch, serde(default) — exportador
      viejo manda 0) en los TRES detectores de sesión (reread/inflate/
      cachebreak), en Rust Y meter-export.py (invariante #1; OJO parse_ts
      en Python devuelve datetime — va int(ts.timestamp())). El panel
      ordena por ts desc y luego costo. AMPLIADO 2026-08-04 (Oscar validó
      el detector de hooks y su tarjeta fresca salió hasta abajo): también
      llevan ts los agregados con actividad — hooks_noise (máx last_ts de
      las sesiones donde disparó), subagents y mech (máx ts de sus turnos).
      SOLO los de estado puro (mcp, skills, claudemd) quedan sin hora,
      abajo por costo — no describen actividad sino configuración. El
      TOPE de 12 sigue cortando por costo en el backend: solo cambia el
      orden de lectura. Verificado con datos reales del VPS (regresión
      byte-idéntica salvo los ts nuevos, 10/10 hallazgos iguales).
      Y LO MISMO EN CONSEJOS (2026-08-04, pedido de Oscar al ver su ficha
      fresca del caché debajo de dos recibos viejos): renderTips junta las
      tarjetas VIVAS (recibos y fichas calientes) en UNA corriente ordenada
      por `born` desc — la más reciente arriba —; las fichas frías del
      catálogo quedan abajo en su orden de siempre. Solo frontend
      (index.html); el blindaje del recibo malformado pasó a ser por
      tarjeta (una rota ya no tumba a las demás). PROBAR: sesión de Claude Code en Windows con trabajo real
      (5+ turnos), cerrarla o dejarla quieta 10-30 min con MichiClaude
      abierto; la tarjeta aparece en Consejos en el siguiente sondeo.
      SIMULADOR DE HALLAZGOS Y CONSEJOS (idea de Oscar, 2026-07-31; solo
      dev, botón "🧪 Simular hallazgos" junto al del gatito): tres pasos
      con la pausa de simMin — tarjetas falsas + contador + post-it rojo,
      el globo del día en rojo, y fichas calientes + resumen + post-it
      turquesa. MISMAS REGLAS que el simulador del gatito: nada toca
      localStorage (guards !simFnd en los render para no pisar
      fndSeen/tipSeen; fndAutoScan y coachPoll se inhiben), simRunning se
      enciende para heredar la inhibición de acks, y al parar se restaura
      el estado real. Los dos botones se paran entre sí. Sirve para
      testear lo VISUAL en segundos; la detección real se valida con uso.
      LISTA DE PRUEBAS PENDIENTES (Oscar pidió mantenerla; se van
      cerrando conforme mande capturas — actualizar aquí):
      [x] cargo check del resumen de sesión — limpio 2026-07-31 (4.54s)
      [x] resumen de sesión en vivo VALIDADO 2026-08-04 (captura +
          bitácora de Oscar, sesión real en test-local): ficha del caché
          en vivo 17:25 (7 min de pausa, 39k ctx) → push "terminó · 2 min,
          18 turnos · 1 aviso" 17:28 → recibo «Crear calculadora Python
          con pruebas» (~$1.10, ⚠ cerró con 39k) 17:31 → vistas con foco
          17:33 apagando el aviso — el ciclo §8bis entero con datos
          reales, incluido el fix del "visto sin verse". DE PASO reapareció
          la confusión documentada del 2026-08-02, ahora al revés: push
          "1 aviso" (las ⚠ del recibo) contra contador "2" del panel
          (tarjetas vivas: ficha + recibo). Dos veces la misma duda ya es
          señal — si vuelve, considerar renombrar el sufijo del push
          (p. ej. "· 1 fuga en esta sesión").
      [x] simulador de hallazgos: VALIDADO 2026-07-31 con capturas de
          Oscar — tarjetas, contadores 4/3, globo del día, post-it rojo
          del gatito y los indicadores de la pastilla, todo a la primera.
          De esa validación salió el rediseño del indicador de consejos
          de la PASTILLA: FOCO ÁMBAR encendido (maqueta de Oscar, SVG
          inline tipo Tabler, #fbbf24 oscuro / #e0930b claro, pulso al
          clic) en vez del punto acento; en el GATITO sigue el post-it
          turquesa. La campana roja queda para hallazgos.
      [x] foco ámbar en la pastilla VALIDADO 2026-08-04 (captura de Oscar
          con el simulador de hallazgos): campana roja y foco ámbar
          conviven en la cápsula, cada uno con su color. Este cierre cubre
          también el viejo pendiente del "punto turquesa en la pastilla" —
          ese punto YA NO EXISTE ahí (lo sustituyó el foco ámbar en el
          rediseño); el turquesa quedó solo en el post-it del gatito.
      [x] VALIDADO 2026-08-01 con capturas de Oscar: pie del gasto SOLO en
          Principal, pestañas en dos líneas sin barra horizontal, Sí/No del
          "Canal nuevo", y la cabecera del detalle limpia (asa, gatito y
          flecha; los números solo en las filas). Falta ver el motivo del
          sin-datos EN LA PASTILLA: con el token vigente no hay error que
          enseñar, así que sale la próxima vez que caduque.
      [x] motivo del "sin datos" VALIDADO EN VIVO 2026-08-01 (capturas de
          Oscar tras hibernar el PC): el globo del gatito lo explica bien.
          De ahí salieron DOS arreglos del mismo día: (1) Windows CORTA el
          tooltip de la bandeja a 128 caracteres y el motivo largo se
          quedaba en "…Usa Claude" —parecía un error de la app—, así que
          si no cabe entero se deja solo la primera frase (firstSentence,
          corta en . y en 。; FR y DE caben enteros y la conservan);
          (2) la PASTILLA no decía nada, solo un "—" mudo: el detalle
          (pcard) ahora pinta el motivo con la misma regla que el gatito
          (antes del early-return de ok:false). La cápsula se queda sin
          texto a propósito: mide 54 px.
      [~] tarjeta de subagentes con datos reales — LA PRUEBA DESTAPÓ UN BUG
          GORDO (2026-08-04): Claude Code moderno (visto en v2.1.221) YA NO
          escribe los turnos del subagente en el .jsonl de la sesión con
          isSidechain:true — los pone en <sesión>/subagents/agent-*.jsonl,
          una SUBCARPETA a la que ningún escáner entraba (todos recorrían
          la carpeta del proyecto PLANA). Consecuencia doble: el detector
          de subagentes era ciego ante subagentes reales (se validó con
          fixture sintético del formato viejo) y sus tokens TAMPOCO
          entraban al costo por proyecto. Lo cazó Oscar: lanzó un Explore
          real con Opus ("Backgrounded agent · finished 3m59s") y el conteo
          de isSidechain:true en los .jsonl planos dio CERO. Verificado en
          el VPS generando un subagente-sonda y mirando su transcript:
          sessionId = el de la sesión MADRE, isSidechain:true, usage
          normal. ARREGLO (mismo día): project_jsonls() en lib.rs y
          meter-export.py (planos + */subagents/*.jsonl) usado por la
          agregación y por scan_findings; y como el sessionId es el de la
          madre, los turnos sidechain YA NO tocan el estado de la sesión
          (turns/first_cr/last_cr/cr_cost/cb) — su cache_read chico
          rompería el detector de infladas y fabricaría rupturas; solo
          suman a su tarjeta. Sus tool_use SÍ cuentan (un MCP invocado por
          el subagente ES un MCP usado). El coach queda plano a propósito
          (excluye sidechains). Regresión en el VPS: mismos hallazgos,
          +1 archivo, +8,558 tokens y +$0.06 — exactamente la sonda.
          VALIDADO EN VIVO el mismo día (captura de Oscar tras recompilar):
          "Los subagentes trabajaron 44 turnos aparte · local · $2.33 ·
          399k tok" — cargo check limpio y la cadena completa
          subcarpeta→escaneo→tarjeta funcionando con su exploración real.
          Y de esa ronda, ajustes de UI (2026-08-04): las tarjetas de
          Hallazgos se CONTRAEN con clic como las fichas de Consejos (se
          pliega solo la recomendación; título/origen/costo se quedan;
          pose recordada en localStorage fndMin, guard !simFnd; Ignorar
          lleva stopPropagation para no plegar de paso — VALIDADO en vivo
          por Oscar el mismo día, captura con 3 tarjetas plegadas), y quedó explicado
          que el contador de la pestaña no encendió por la trampa del
          vigilante — el escaneo corrió con la pestaña abierta.
          MÁS TRES del mismo día (SIN validar en vivo): (1) la ZONA DE LA
          CABEZA del gatito estaba mal calibrada DE ORIGEN (nunca se movió
          — se verificó en el historial de git): se pasaba 6% por la
          derecha y por arriba sobre teclado/vacío (clic ahí abría el
          panel) y dejaba fuera cachete izquierdo y barbilla (ahí
          arrastraba). Se RECALIBRÓ midiendo los píxeles blancos del gif
          con un decodificador GIF propio en el VPS (scratchpad, stdlib
          puro) y viendo las zonas dibujadas sobre el fotograma: cabeza
          real x[50%,86.5%] y[53%,87.5%] → --hx:50% --hy:52% --hw:37%
          --hh:36%. (2) El post-it TURQUESA del coach ahora es PILITA como
          el rojo (dos orillas asoman detrás: ámbar y papel — una nota
          sola se leía como cuadrito, mismo feedback que motivó su cinta).
          (3) La etiqueta del interruptor del widget decía solo
          "hallazgos" pero SIEMPRE cubrió también al coach (fnd y coach
          pasan ambos por fndBadgeOn() en el resumen): ahora dice
          "Avisarme en el widget (hallazgos y consejos)" ×8 idiomas.
          Aclarado además: apagarlo deja los contadores de pestaña
          SIEMPRE encendidos en el panel — no interrumpen.
          Y PULIDO VISUAL de la misma noche (pedidos de Oscar; (a) y (b)
          VALIDADOS en vivo el 2026-08-04 con su captura — pilita
          turquesa con orillas, separada de la roja, y contadores de
          pestaña legibles en ambos colores; del (c) falta confirmar el
          hover y los clics de la cabeza recalibrada):
          (a) post-it turquesa más grande (.95bs, fuente 10.5) y
          más separado de la pilita roja (offset 1.8bs); (b) CONTRASTE de
          los avisos acento — el contador de Consejos ponía blanco sobre
          #56c7d6 (~2:1, invisible) y ahora usa --accent-ink (tinta
          #0c2f36 en oscuro >8:1, blanco en claro donde el acento es
          profundo), y el papel del post-it turquesa pasó de #2ea3b4 a
          #128097 para que su número blanco dé ~4.7:1 (regla UX: el color
          del texto se elige según el fondo, nunca blanco fijo sobre
          acento claro); (c) el HOVER del globo resumen vive ahora SOLO
          en la cabeza del gato — pasar el mouse por la laptop ya no lo
          despliega; salir de la ventana entera lo pliega (así no
          parpadea al cruzar de la cabeza a la laptop) y rozar la cabeza
          <300ms cancela el temporizador para que no salga tarde.
      [x] detector de hooks con un hook real VALIDADO 2026-08-04 (captura
          de Oscar): hook PostToolUse de prueba en test-hook (imprime
          ~3.4k chars por disparo) + tanda de 20 Write con Haiku vía
          `claude --model haiku --allowedTools Write -p` — OJO: sin
          --allowedTools el modo -p no puede pedir permisos y sale sin
          escribir nada (le pasó a Oscar al primer intento). Tarjeta
          "PostToolUse:Write · local · ~$0.02 · ~18k tok" correcta. De esa
          prueba: TERCERA vez de la trampa del vigilante (pestaña abierta
          al nacer la tarjeta = vista al instante, sin campana ni globo —
          comportamiento correcto) y la aclaración de que los hallazgos
          NUNCA van al celular (regla de privacidad ntfy) — su único aviso
          con texto es el globo del día, que además pide costo ≥$1.
          La carpeta test-hook se borra tras la prueba (el hook es ruido).
      [ ] alta de servidor SIN Python → ERR_NO_PYTHON traducido
      [ ] alarmas reales: cruzar umbral, 100%, y ventana nueva
          reconocida (trackResets/windowChanged con datos de verdad)
      [x] ntfy básico VALIDADO EN VIVO 2026-08-01 (capturas de Oscar):
          bloque en Preferencias, QR escaneado con la cámara, suscripción
          en la app y "Enviar prueba" llegando al teléfono. Dejó activada
          la casilla de alarmas de %.
      [ ] ntfy camino completo: push de alarma de % real, 100% real y el
          programado llegando con la PC APAGADA — va junto con las
          alarmas reales de abajo
      [x] "tu agente terminó" VALIDADO EN VIVO 2026-08-01 a la primera
          (captura del teléfono de Oscar: "Terminó tu sesión en agente ·
          8 min, 20 turnos"), con la casilla del nombre activada. De ahí
          salió UN BUG: decía "agente" y la carpeta era "test-agente".
          Causa: el coach usaba el nombre de la CARPETA DE LOGS, que
          codifica la ruta entera cambiando cada separador por "-"
          ("C--Users-oscar-Claude-test-agente"), y el recorte se quedaba
          con el último trozo. Arreglado leyendo el `cwd` real de la
          sesión (lo mismo que ya hacía la agregación) y mandando el
          nombre YA RESUELTO desde Rust (`pname`), así que el panel no
          adivina: se quitó fndProj de los usos del coach.
          HALLAZGOS ARREGLADO IGUAL el mismo día, en Rust Y en
          meter-export.py (invariante #1). TRAMPA que casi muerde: en los
          hallazgos ese nombre NO es solo display — `proj` casa las
          sesiones con el detector de CLAUDE.md (sess_pi vs pj), así que
          cambiarlo habría dejado ese hallazgo en costo 0 y lo habría
          tirado EN SILENCIO. Por eso son dos campos: `proj` (carpeta de
          logs, para casar, intacto) y `disp` (cwd real, para enseñar).
          `fndProj` del panel ya no recorta un nombre que venga limpio —
          solo los codificados de exportadores viejos o logs sin cwd.
          Regresión verificada en el VPS contra la versión anterior:
          mismos 10 hallazgos, mismo orden y mismos costos, solo cambian
          los nombres ("-opt-projects-michiclaude" -> "michiclaude").
      AVISOS AL CELULAR (ntfy) — IMPLEMENTADO 2026-08-01; cargo check
      limpio y lo BÁSICO VALIDADO EN VIVO por Oscar ese mismo día (QR +
      suscripción + prueba en su teléfono). Falta el camino de eventos
      reales (ver lista). SUSTITUYE a la propuesta de Telegram
      (descartada por decisión de Oscar 2026-08-01: fricción de BotFather,
      chat_id personal y, lo decisivo, no puede avisar con la PC apagada).
      Diseño completo en docs/avisos-ntfy.md — LEERLO antes de tocar esto.
      Lo esencial: opt-in APAGADO por defecto; ntfy_config.json (enabled/
      topic/server/alarms; topic = contraseña del canal, CSPRNG getrandom,
      "michi-"+12 [a-z0-9]; server editable solo a mano, self-host gratis);
      comandos get_ntfy/save_ntfy/ntfy_push (async, publicación JSON a la
      raíz — los headers HTTP no aguantan UTF-8 y hay 8 idiomas)/ntfy_qr
      (matriz al canvas, sin dependencia de imagen; enlace ntfy://host/topic
      porque la app ntfy NO trae escáner — se usa la cámara del sistema).
      La estrella: al 100% va el aviso inmediato + el "ya volvió" PROGRAMADO
      (header delay, +120 s de colchón por el jitter) que ntfy entrega con
      la PC APAGADA; si el reset semanal no cabe en los 3 días del servidor
      público, no se programa NI se promete ("puedes apagar" solo cuando es
      verdad). REGLA NUEVA de privacidad: por ntfy viajan SOLO porcentajes,
      horas de reset y frases del diccionario — nunca proyectos, rutas ni
      dólares (los topics son públicos por diseño). Rust no redacta avisos
      (invariante #10); textos reutilizan breakBody/weekBody/notif_back_*;
      un push por ventana gracias a los banderines notifS/notifW; el
      simulador nunca manda pushes (guard simRunning en ntfyPush);
      fallos a ntfy_debug.json sin bloquear nada local. Dependencias
      nuevas mínimas: getrandom, qrcode (sin features). Botón CANAL NUEVO
      (2026-08-01, pregunta de Oscar que destapó el hueco): regenera el
      topic en dos pasos —patrón del bote de borrar servidor— para cuando
      el canal se filtre (un QR en una captura regala la contraseña);
      comando ntfy_regen, el canal viejo muere y hay que re-escanear.
      "TU AGENTE TERMINÓ" (2026-08-01, lo pidió Oscar tras ver que el valor
      real está en dejar a Claude trabajando e irse): regla `done` en
      coach_scan — sesión quieta 5 min (COACH_DONE_QUIET) con 5+ turnos
      (COACH_DONE_TURNS), banderín `notified` propio. NO es una ficha:
      coachPoll la aparta antes del filtro de Consejos (ni gasta el tope
      diario ni sale en el panel) y la manda al celular. Dedup por partida
      doble —el estado del coach vive en memoria y al reiniciar la app una
      sesión recién callada volvería a reportarse—: localStorage `ntfyDone`,
      tope 100, y máximo 3 pushes por sondeo. El NOMBRE del proyecto es una
      casilla aparte (`names`, apagada): la regla general prohíbe nombres
      por ntfy, y la casilla advierte que el canal es público. Hereda las
      limitaciones del coach: solo ESTA máquina y solo si la app estuvo
      abierta durante la sesión. Detalle en docs/avisos-ntfy.md.
      Y DECISIÓN del mismo día: ntfy NO viaja en los ajustes compartidos
      del hub — esa pantalla promete "no guarda llaves ni contraseñas" y
      el topic ES la contraseña del canal; además cada máquina con su
      canal se silencia por separado en la app ntfy (ver
      docs/avisos-ntfy.md). README ACTUALIZADO el 2026-08-01 a petición
      expresa de Oscar (única excepción al invariante #9 en esta ronda):
      sección "Avisos en el celular" con el alta paso a paso, la tabla de
      qué llega, el caso de DOS O TRES PCs (cada una su canal, los ajustes
      compartidos NO lo copian, y la doble notificación con dos equipos
      encendidos como comportamiento esperado), el QR tratado como
      contraseña y los límites del servidor gratuito; más el punto 5 de
      Privacidad y "Los avisos al celular, en claro". De paso se borró de
      la tabla de Preferencias la fila "Diseño de la pastilla" (el diseño
      coral se eliminó el 2026-07-25 y el README seguía anunciándolo).
      Confirmación del "Canal nuevo" cambiada a SÍ/NO explícito (Oscar:
      un botón que cambia de texto no se lee como pregunta) con claves
      btn_yes/btn_no ×8 — reutilizables. PENDIENTE: la prueba en vivo de
      la lista.
      FAQMISSES + ISSUE PRE-LLENADO — HECHO 2026-07-31 (SIN validar en
      vivo ni cargo check; tocó Rust: comando open_faq_issue con base
      constante ISSUES_URL y lanzado por rundll32, no cmd/start — el &
      de la query rompería cmd). Búsqueda 4+ letras sin ficha → se
      apunta local (faqMisses, dedup, tope 50, 1.5 s de pausa); pie de
      Consejos con "N búsquedas sin ficha este mes" + botón que abre el
      issue pre-llenado con la lista. CAVEAT: repo privado = issues 404
      para no-colaboradores; útil para todos al hacerlo público. CON
      ESTO EL DOC DE CONSEJOS QUEDA COMPLETO (§10: 5/5). PROBAR: en
      Consejos buscar algo inexistente ("docker" p. ej.), esperar 2 s,
      borrar el filtro → pie con el contador; clic en "Proponerlas en
      GitHub" → navegador con el issue redactado.
      (Después de cerrar esto: las pruebas pendientes de la lista. La
      propuesta de TELEGRAM quedó DESCARTADA 2026-08-01 — la sustituyó
      ntfy, ver arriba. El modelo local quedó DESCARTADO dentro de la
      app también en su variante "modelo-lector" — fase 2 opcional aparte,
      compuertas en ~/.michiclaude/notas-coach-local.md. Las ideas del doc
      de estrategia de Oscar 2026-08-01 —dashboard móvil QR con servidor
      HTTP local, edición VPS headless— quedaron ANALIZADAS y en fila,
      sin fecha: la primera toca invariantes (dependencia axum + promesa
      de red) y la segunda espera señal de demanda real.)
      (1) DETECTOR skills instaladas sin uso — HECHO 2026-07-30 (Python
          validado en el VPS: caza exactamente eliminar-proyecto y respeta
          las ventanas; cargo check limpio 2026-07-30). UNA tarjeta agregada
          (kind skills_unused, count + nombres en `file`), solo con ventana
          de 7+ días ("no usaste tu skill HOY" no dice nada). Fuentes de
          uso: <command-name> en los logs + tool_use Skill + el `skillUsage`
          de ~/.claude.json (Claude Code YA registra cada uso con fecha —
          descubierto en esta sesión). DECISIÓN CLAVE: solo cuenta
          ~/.claude/skills/ como "instalado"; la carpeta de plugins NO — es
          el catálogo ENTERO del marketplace cacheado (docenas de skills que
          nadie instaló) y contarla fabricaría hallazgos falsos;
      (2) DETECTOR subagentes caros — HECHO 2026-07-30 (kind subagents,
          umbral SUB_MIN_TOKENS 50k, costo MEDIDO del usage propio de cada
          turno isSidechain; validado con fixture sintético al centavo —
          $0.2775 — porque los logs del VPS tienen CERO sidechains; falta
          verlo con datos reales en Windows y el cargo check);
      (3) DETECTOR hooks ruidosos — HECHO 2026-07-30 (kind hooks_noise,
          umbrales HOOKNOISE_MIN_FIRES 15 / HOOKNOISE_MIN_TOKENS 10k). El
          formato se averiguó GENERANDO un log real (hook de prueba +
          `claude -p` con Haiku en carpeta temporal, luego borrada): cada
          disparo queda como attachment `hook_success` con `hookName` y
          `content` = lo que entró al contexto; dedup por uuid. Tokens ~
          chars/4 (tarjeta con "~") y costo piso a input del modelo
          dominante de la sesión. Validado al centavo con el fixture
          amplificado (20×2960 chars = 14 800 tok = $0.0148 Haiku) y
          regresión limpia en 7d/30d; cargo check limpio 2026-07-30, falta un
          hook real (Oscar no usa hooks — OJO: el VPS tampoco, y todas las
          menciones de "hook" en sus logs son conversaciones SOBRE hooks,
          no salida de hooks: el detector mira attachments, no texto);
      REGLA de los tres, ya acordada: señalan lo instalado que NO se usa y
      lo que cuesta CARGARLO — nunca califican si una skill que sí se usa
      "gastó de más" (categoría prohibida del doc).
      Después: el detector de líneas de CLAUDE.md sin respaldo, el aviso EN
      EL MOMENTO con texto (el globito 1×/día — el indicador ya es la
      versión pasiva), el fix personalizado por `entrypoint` (VS Code vs.
      terminal, con respaldo genérico) y la verificación antes/después
      (necesita semanas de historial, ya asegurado con cleanupPeriodDays=365
      en ambos lados). El de rupturas de caché ya está (detector 5).
      Idea original: Diseño completo en `docs/analizador-fugas.md`
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

## Consumo de recursos (medido 2026-07-29 en el Windows de Oscar)

Cifras REALES, no estimadas. Se midieron con la app compilada en RELEASE,
sumando `michiclaude.exe` y todos sus procesos hijos de WebView2 (hay que
seguir la cadena de PID padre: `Get-Process msedgewebview2` a secas incluye
los de OTRAS apps del sistema — Copilot, Outlook nuevo, Teams — y da un
número que no es nuestro).

| | Antes | Después |
|---|---|---|
| Instalador NSIS | 12.3 MB | **5.8 MB** |
| `michiclaude.exe` | 21.7 MB (release) · 46 MB (dev) | igual |
| Datos en disco | < 1 MB | igual |
| RAM (suma de working sets) | ~830 MB | ~695 MB |
| **RAM privada — la cifra HONESTA** | | **276 MB** |

CUIDADO CON LA MEDICIÓN (esto invalidó medio día de conclusiones): sumar
`WorkingSet64` de cada proceso CUENTA VARIAS VECES la memoria que comparten
entre sí, y con 10 procesos de WebView2 infla el resultado más del doble. La
cifra real es la suma de `WorkingSetPrivate`
(`Win32_PerfRawData_PerfProc_Process`): 276 MB, no 695. Para dar contexto, en
la misma máquina y el mismo momento: VS Code 799 MB, Brave 730 MB,
explorer 360 MB. O sea, un tercio del editor — bastante mejor de lo que
parecía. Está publicado en el README con el comando para reproducirlo.

LO QUE HAY QUE SABER ANTES DE VOLVER A DIAGNOSTICAR ESTO:

1. **Compilar en release NO baja la RAM.** dev 817 MB vs release 830 MB. El
   `.exe` sí baja a la mitad, pero el peso está en los ~11 procesos de
   WebView2 y a esos les da igual el perfil de compilación. Se comprobó.
2. **El gatito NO es el culpable** (es la SEGUNDA vez que lo parece y no lo
   es; la primera fue con la lentitud, que era el hilo de la UI bloqueado).
   Medido con el simulador: cargar los cuatro dibujos sube ~120 MB de pico y
   los DEVUELVE (939 -> 828), así que tampoco hay fuga de memoria.
3. **El costo real son las SEIS ventanas.** Cada WebView2 tiene un piso de
   ~57 MB aunque esté vacía y oculta — se ve en la propia medición: la
   pastilla (una cápsula con dos líneas) y el globo de aviso pesan eso. Seis
   ventanas son ~345 MB de piso antes de dibujar nada, más la infraestructura
   (GPU, red, almacenamiento, crashpad).
4. Para comparar: el instalador está muy bien (una app equivalente en
   Electron ronda 90-150 MB), pero la RAM está al nivel de Slack o Discord,
   que es justo lo que un widget de bandeja no debería costar.

ARREGLADO 2026-07-29 (~115 MB): la pastilla y el gatito son EXCLUYENTES, así
que dos de las seis ventanas no se mostraban NUNCA — con el gatito puesto,
`pill` y `pcard` se cargaban para no pintar nada jamás. Ahora `pill`, `pcard`,
`cat` y `card` YA NO ESTÁN en tauri.conf.json: las crea
`ensure_widget_windows()` en Rust, solo el par del estilo elegido, y al
cambiar de widget se crea el par nuevo y se DESTRUYE el viejo (si solo se
ocultara, quien probara los dos acabaría con las cuatro cargadas hasta
reiniciar). En el json solo quedan `main` y `notif`, que salen con los dos
estilos.
CONSECUENCIA para el mantenimiento: el TAMAÑO de esas cuatro ventanas ya no
se toca en el json, se toca en `ensure_widget_windows()`. Y las capabilities
tienen que seguir listando las seis etiquetas: los permisos van por etiqueta,
así que una ventana creada en caliente hereda los mismos.
No choca con la regla de no redimensionar ventanas transparentes: esa prohíbe
cambiar el TAMAÑO de una ventana viva, y aquí cada una nace con el suyo fijo.
Lo que lo hizo barato: TODOS los usos de esas ventanas ya toleraban su
ausencia (`if let Some` / `else { return }`), así que no hubo que blindar
nada — solo crearlas antes de medirlas en `set_pill_style`.

## Retención de los logs (requisito del analizador)

Claude Code borra `~/.claude/projects/**/*.jsonl` a los **30 días** por
defecto. El analizador de fugas compara ventanas ANTES y DESPUÉS de aplicar
un fix, así que sin historial no tiene contra qué comparar — y lo borrado no
se recupera. Se sube con `cleanupPeriodDays` en `~/.claude/settings.json`:

- VPS: puesto en **365** el 2026-07-29 (respaldo en `settings.json.bak`).
- Windows de Oscar: **365** confirmado el 2026-07-29 (ya estaba puesto en su
  `settings.json`; verificado por Oscar mirando el archivo). AMBOS lados
  conservan ya un año de logs — el antes/después solo espera historial.

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
- El desarrollo y las pruebas ocurren en el PC Windows de Oscar
  (`C:\Users\oscar\Claude\MichiClaude` — mudado ahí el 2026-07-31 desde
  Downloads; C:\Users\oscar\Claude es su carpeta madre para todos los
  proyectos. OJO al mover un clon en Windows: target/ guarda rutas
  ABSOLUTAS y el build script de Tauri falla con "failed to read plugin
  permissions" apuntando a la ruta vieja — se arregla con `cargo clean`,
  que además liberó 20 GB); en el VPS vive
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

---

2026-08-04 (noche) — DETECTOR 10 `claudemdsize`: CLAUDE.md más grande de lo
que Claude Code carga (40k chars). Nació de que nos pasó en carne propia:
nuestro CLAUDE.md llegó a 118.8k y durante semanas dos tercios de las
reglas no se leyeron en ninguna sesión — el único aviso era una línea
amarilla en la terminal que nadie vio. Implementado en Rust y
meter-export.py reutilizando la enumeración del detector de líneas (costo
de implementación casi cero); tarjeta de estado (costo 0, tokens ~ del
tramo sin leer, count = tamaño en k), claves fnd_claudemdsize_* ×8. Solo
ventanas 7d+. Validado en el VPS con umbral bajado a 20k (tarjeta con 27k
y ~1858 tok correctos contra el CLAUDE.md recién adelgazado) y regresión
limpia con el umbral real. Pendiente: cargo check en Windows. Y de la
limpieza salió otra confirmación: el CLAUDE.md destilado ya no dispara ni
el detector de líneas sin respaldo.

2026-08-04 (noche) — ERR_NO_PYTHON y ERR_BAD_PYTHON VALIDADOS EN VIVO por
Oscar, con capturas. Receta del host sin Python (reutilizable): contenedor
Alpine desechable en el VPS (`docker run … alpine:3.20` con openssh, llaves
de authorized_keys, puerto 127.0.0.1:2223) + alias SSH en Windows con
ProxyJump por el VPS; OJO: hay que conectar UNA vez a mano desde PowerShell
para aceptar la llave del host (la app usa BatchMode y no puede contestar).
Resultado: alta con host sin Python → error traducido y el campo "¿Dónde
está Python?" se REVELA con el error delante (nace oculto — diseño
confirmado); ruta falsa en ese campo → ERR_BAD_PYTHON traducido; nada quedó
guardado en la lista. BONUS cazado por Oscar probando en alemán: los
mensajes transitorios del alta se escriben con t() del momento y el cambio
de idioma no los repinta — la UI cambió a alemán y el error siguió en
español. Arreglo: applyI18n limpia rMsg al cambiar idioma (un transitorio
en el idioma viejo confunde más de lo que informa). Contenedor y llaves de
prueba eliminados al cerrar.


---

## Ronda de rediseño UX/UI (2026-08-05) — detalle completo

Se movió aquí desde CLAUDE.md el mismo día, al pasar ese archivo de los
40k caracteres que Claude Code carga (la regla que vigila el detector 10;
nos volvió a pasar en carne propia). En CLAUDE.md queda el contrato de la
ronda y los invariantes; el porqué de cada sección vive aquí.

- [ ] EN CURSO (desde 2026-08-05): ronda de REDISEÑO UX/UI sobre la
      maqueta de Oscar (docs/rediseno-v5.html). RESPALDO COMPLETO del
      estado anterior en el tag `pre-rediseno-20260805` (recuperar:
      `git checkout pre-rediseno-20260805`; comparar: `git diff`).
      CONTRATO DEL REDISEÑO (pedido de Oscar): (1) SOLO reacomodo y
      estética — cero pérdida de funcionalidad, textos, mensajes de
      error, confirmaciones, campos o iconos; (2) sección por sección
      del menú, nunca todo a la vez; (3) toda NOVEDAD funcional de la
      maqueta se consulta con Oscar ANTES de implementarla; (4) si algo
      del diseño choca con un invariante, avisar antes. CHOQUES YA
      DETECTADOS en la maqueta (resolver al portar): fuentes de Google
      (viola CSP/privacidad — van embebidas o tipografía del sistema),
      textos hardcodeados en español (todo pasa por t(), invariante
      #10), buckets y modelos hardcodeados (render dinámico, invariante
      #6), selector de idiomas con Italiano y sin 中文 (son 8 fijos), el
      GATITO desaparecido del selector de estilo y sin sus selectores de
      arte/globos, capa sin la opción "Detrás", Consejos sin las
      tarjetas VIVAS del coach, Hallazgos sin "volver a mostrar", pie
      sin la cifra Semana, ntfy sin interruptor maestro opt-in, y
      "Simular estados" ausente. Después del rediseño: el resto de
      pendientes en orden.
      AVANCE: S1 encabezado+pestañas+paleta base VALIDADA por Oscar
      (capturas 2026-08-05; decisiones: tipografía del SISTEMA se queda —
      sin fuentes web —, y el contraste del degradado aprobado en
      pantalla). S2 hero de Principal implementada (anillo con el mismo
      truco pathLength=100 — JS del medidor intacto salvo la clase warn
      de remQ —, ritmo unificado dentro del hero, eyebrow "A este ritmo"
      retirado a propósito por la maqueta). AJUSTES pedidos por Oscar
      sobre S2 (2026-08-05, con capturas): el anillo COMPLETO se encimaba
      con el reset → vuelve al MEDIO anillo de siempre (misma geometría y
      pathLength); y el texto de las barras se veía apagado sobre el
      violeta → el hero REDEFINE --txt-mut/--txt-dim localmente (todo lo
      de dentro hereda sin repintar reglas) y el % lleva clase `pctv` con
      más peso. REGLA NUEVA del rediseño: toda tarjeta con fondo propio
      redefine esos dos tonos en vez de tocar sus hijos uno por uno.
      Y el acomodo definitivo de las barras (Oscar 2026-08-05): nombre +
      % en UNA línea, y debajo `.bmeta` con un dato por línea TODOS a la
      izquierda — el reset de las filas mini se iba a la derecha y
      rompía la alineación.
- TIPOGRAFÍA (2026-08-05, Oscar la pidió embebida): Inter (texto), Sora
  (`--disp`: títulos y cifras grandes) y JetBrains Mono (`--mono`) viven
  en `src/fonts/` — woff2 variable, subconjuntos latin y latin-ext, 238 KB,
  licencia OFL con su copia en `fonts/LICENSES.md` (obligatoria al
  redistribuir). NUNCA se piden a un CDN: rompería la CSP y la promesa de
  privacidad, y un fallo del servidor dejaría la app sin tipografía. Sin
  glifos CJK: en ja/ko/zh la pila cae sola al sistema — por eso los
  respaldos de --font/--mono se conservan enteros. Se aplican SOLO donde
  Oscar lo indique, sección por sección (por ahora: todo el panel hereda
  Inter/JetBrains, y --disp está en el título y el % del medidor).
- S3 GASTO POR PROYECTO (2026-08-05): filas con AVATAR de iniciales
  (`projInitials`: primera letra de la primera palabra + primera de la
  última — "claude-code-meter"→CM, "MichiClaude"→MC; con una sola palabra,
  sus dos primeras letras), nombre + chip de origen, barrita bajo el
  nombre e importe a la derecha. El avatar hereda el color de PALETTE de
  su barra (el color sigue identificando al proyecto) y su fondo es una
  CAPA a opacidad, NO `color-mix()` — esa función es demasiado reciente
  para darla por segura en WebView2. "Más proyectos (N)" pasa a enlace:
  ya existía y ya desplegaba (projOpen), solo cambió de aspecto.
- S4 TENDENCIA + MODELOS + PIE (2026-08-05): barras de tendencia con
  degradado, esquinas redondeadas y el DÍA MÁS CARO destacado en violeta
  con halo (clase `top`, nueva; `today` y `zero` intactas — el día sin
  actividad sigue siendo hueco, no barra de valor cero). Modelos: barra
  segmentada en cápsula con separación entre tramos y leyenda en píldoras
  (el nombre del modelo va en `<em>` para destacarlo del %; sigue saliendo
  de prettyModel, invariante #6). Pie: tarjeta con degradado y las cifras
  en --disp a 22px.
- BUG DEL REDISEÑO (2026-08-05, lo vio Oscar en captura): la lista de
  proyectos salió descuadrada —avatar e importe a la izquierda, nombre a
  la derecha— porque en CSS Grid los hijos que fijan su fila se colocan
  ANTES que los automáticos, y el nombre acababa en la 3ª columna. Se
  rehízo con FLEX + envoltorio `.ptx`, como la maqueta. REGLA: en filas
  con "algo que ocupa dos líneas" a los lados, flex antes que grid.
- CONTENEDOR BASE: `.sect` deja de ser un bloque separado por línea y pasa
  a ser TARJETA con fondo (--card, radio r-lg). Es transversal a
  propósito, como la paleta: da el lenguaje visual a todas las pestañas de
  una vez y el contenido de cada una se sigue rediseñando por turnos. El
  selector de periodo (.dsel) pasa a cajita con borde — plano sobre la
  tarjeta se perdía.
- S5 FILTROS DE LA TARJETA DE GASTO (2026-08-05, dos maquetas de Oscar).
  El `<select>` de periodos DESAPARECIÓ: ahora hay dos disparadores
  gemelos en el encabezado (embudo = proyectos, calendario = fechas) que
  abren POPOVERS FLOTANTES con velo — `position:fixed` a propósito: el
  panel tiene scroll propio y dentro de él se irían con el scroll.
  Cancelar / ✕ / velo / Esc REVIERTEN (foto del estado al abrir); solo
  Aplicar confirma.
  · FECHAS: presets (Hoy/7/15/30) DENTRO del calendario — "Hoy" ES el
    periodo de 1 día, no un botón aparte — más rango libre a dos clics
    (si se elige al revés se ordena solo). Rejilla de 42 celdas con los
    días vecinos en gris, punto turquesa en hoy, tope 90 días con aviso;
    ni futuro ni más allá de esos 90. Calendario DIBUJADO A MANO
    (invariante #4) y meses/días desde `Intl` con el idioma activo.
    Estado: `curDays` (preset) o `spendRange` (rango libre) en
    localStorage — nunca los dos a la vez.
  · PROYECTOS: filtro solo de FRONTEND (la lista ya viene agregada; con
    filtro el total pasa a ser la suma de los elegidos y por eso lleva
    etiqueta "2 de 8 proyectos" — una cifra sin decir de qué es sería
    justo lo que prohíbe el invariante #8). Conjunto VACÍO = todos, nunca
    "ninguno" por accidente. Buscador, "Todos", contador en el botón y
    CHIPS en la tarjeta para quitar uno a uno o todos. Persiste en
    `projFilter`. Con filtro se enseñan todos los elegidos aunque
    vinieran de la cola plegada.
  · El PIE "Hoy" del final del panel se ELIMINÓ (Oscar 2026-08-05): decía
    lo mismo que el total de arriba en cuanto el periodo era hoy. Su
    contenido —la cifra grande y la nota de privacidad— vive ahora en la
    caja del total, dentro de la tarjeta de gasto. CONSECUENCIA ASUMIDA:
    con un periodo que no sea hoy ya no se ve el gasto del día suelto; la
    cifra que manda es la del periodo elegido, que es lo que se está
    mirando (`cost_today` sigue llegando del backend por si vuelve a
    hacer falta).
  · "Borrar" del calendario vuelve al valor por DEFECTO (hoy) y CIERRA:
    dejarlo vacío obligaba a elegir algo para poder salir, y cerrar sin
    elegir mantenía el periodo anterior — justo el que se quería borrar.
  · Los controles viven en su PROPIA fila alineada a la izquierda: colgados
    del título se descolocaban al pasar el título a dos líneas.
  · Orden de Principal: cuota → gasto → MODELO MÁS USADO → tendencia
    (intercambiadas las dos últimas a petición de Oscar).
  TRADUCCIÓN SIN DICCIONARIO: los nombres de mes y día salen de
  `Intl.DateTimeFormat(lang)` — los 8 idiomas funcionan sin ampliar I18N
  (solo Hoy/Borrar/Aplicar/avisos están en el diccionario). El primer día
  de la semana es lunes salvo en en/ja/ko/zh.
  LÍMITE HONESTO: con rango, las máquinas del HUB quedan FUERA (sus fotos
  son de ventanas que terminan hoy y nadie puede recortarlas); Rust lo
  marca con `hub_skipped` y el panel lo dice en pantalla — callarlo sería
  enseñar un total incompleto (invariante #8). Tampoco se SUBE foto al
  hub mientras hay rango: envenenaría lo que leen las demás máquinas.
  "Hoy" y la serie diaria de 30 días siguen ancladas a AHORA a propósito.
  BUG cazado en la prueba: en Python faltaba el corte superior y el rango
  devolvía todo hasta hoy. Verificación que lo destapó y que conviene
  repetir si se toca esto: dos rangos contiguos de 7 días deben sumar
  EXACTAMENTE la ventana de 14 (dio 0.0000 de diferencia).
- S6 FUENTES DE DATOS (2026-08-05): las cuatro fuentes pasan de lista de
  viñetas a REJILLA DE TARJETAS con icono (se entienden de un vistazo).
  Sus textos salieron del `cfg_note` viejo partiéndolo automáticamente en
  `src1_t/src1_d`…`src4_t/src4_d` ×8 idiomas — sin reescribir traducciones
  a mano. Formulario con campos más altos y foco en acento; botón primario
  en degradado y secundarios (hub/export) en tono apagado. Servidor
  guardado = tarjeta con icono, no fila con línea.
  Y AJUSTES COMPARTIDOS SE MUDA a la pestaña Ajustes (petición de Oscar):
  es un ajuste, no una fuente; solo DEPENDE de un servidor. Como allí no
  se ve la lista, aparece un aviso ámbar (`hub_cfg_needsrv`) cuando no hay
  ninguno — antes el contexto lo daba estar debajo de la lista.
- S7 HALLAZGOS (2026-08-05): el encabezado del analizador queda en su
  tarjeta y las tarjetas de hallazgo van SUELTAS debajo (una tarjeta
  dentro de otra se leía como un cajón). Cada hallazgo estrena ICONO por
  tipo (`FND_ICON`: rayo=cachebreak, gráfico=inflate, hoja=reread,
  terminal=mech, nodos=subagents, enchufe=mcp…) en cuadrito con el color
  de la SEVERIDAD, importe destacado y unidades apagadas, e "Ignorar"
  como píldora en la esquina. El borde izquierdo de color se retiró: con
  fondo de tarjeta y el icono ya coloreado, sobraba.
  OJO al tocar esto: las fichas de CONSEJOS comparten el molde `.fnd`
  (variante `.tip`) — cualquier cambio en .fnd/.fnd-t/.fnd-f les llega
  también, y por eso tienen sus propios overrides.
  · El SELECTOR de Hallazgos pasa al MISMO calendario del gasto: el
    popover es UNO SOLO y `calTarget` ("spend"/"fnd") decide a quién
    aplica lo elegido; Hallazgos guarda su par en `fndDays`/`fndRange`.
    Para que el rango sea de verdad, `get_findings` acepta `end` y el
    analizador (Rust Y Python, invariante #1) gana CORTE SUPERIOR en sus
    tres filtros de ventana — sin él el rango devolvía todo hasta hoy,
    la misma mordida que ya pasó en el gasto. "Borrar" vuelve a HOY en el
    destino que esté abierto. Regresión verificada: sin rango, hallazgos
    y costes idénticos a la versión anterior.
  · PIE en dos piezas: el enlace de recuperar lo ignorado ARRIBA y
    destacado en acento (antes se perdía dentro de una línea gris), con
    el número dentro de la frase ("Volver a mostrar 2 hallazgos que
    ocultaste" — `fnd_restore` pasa a función; `fnd_hidden` se retiró) y
    la nota del "~" debajo en gris.
- ANCHO DEL PANEL 400 → 446 (2026-08-05): lo cazó Oscar comparando con la
  maqueta — a 400 px los textos se apretaban ("Semanal · todos los …"
  cortado con puntos suspensivos, el ritmo partido en dos líneas). 446 =
  los 430 de la maqueta + los 8 px de padding del body por lado.
  `position_panel` usa `outer_size()`, así que el flyout se recoloca solo;
  no hay ningún ancho hardcodeado en Rust. Cambia tauri.conf.json → hay
  que RECOMPILAR para verlo.

### Consejos y remates de Hallazgos (2026-08-05)

- El "¿No encontraste lo que buscabas?" DEJA de ser un pie permanente y
  pasa a ocupar el HUECO de la búsqueda sin resultados: aparece solo
  mientras el filtro no encuentra nada y se va al borrarlo. Razón (mi
  recomendación, aceptada por Oscar): un banner fijo se vuelve invisible a
  los dos días, y el ofrecimiento significa algo justo en el momento en que
  al usuario le falta un consejo. El registro LOCAL de búsquedas fallidas
  del mes (faqMisses, cero telemetría) se mantiene: es lo que viaja en el
  issue, para que la propuesta lleve todo lo buscado y no solo lo último; y
  la búsqueda EN CURSO se añade siempre, porque el registro espera 1.5 s de
  pausa y pulsar rápido no hacía nada. "Descartar" se retiró: ya no hay
  nada permanente que cerrar (tips_dismiss fuera de los 8 idiomas).
- Buscador con ✕ para vaciarlo (lo pidió Oscar señalándolo en captura).
- COMANDOS resaltados en Hallazgos y Consejos con `withCmds()`: envuelve en
  <code> los "/clear", "/compact"… y lo que va entre «comillas angulares»,
  que en el diccionario son siempre órdenes de terminal. Escapa ANTES de
  tocar (nada llega a innerHTML sin escapeHtml).
- El botón de periodo no se parte en dos líneas (white-space:nowrap) y en
  el encabezado el TÍTULO se encoge antes que el control: "03 ago – 05 ago"
  cabía justo y saltaba de línea.
- Copy del pie de Hallazgos reescrito en los 8 idiomas: de "~ = estimado;
  el resto está medido de tus logs" a "Los importes con ~ son aproximados.
  El resto está medido directamente de tus registros" — la abreviatura con
  signo igual se leía como jerga.
- Los hallazgos enseñan CUÁNDO pasaron (`fmtWhen`, a la derecha de la
  fuente): "ahora mismo" / "hace 20 min" / "hace 3 h" hasta las 6 h, luego
  la hora, "ayer HH:MM" y por fin "31 jul 20:38". Con varios hallazgos en
  pantalla, saber cuál es de cuándo era imposible (petición de Oscar
  2026-08-05, con la línea marcada en su captura). Los detectores de
  ESTADO PURO (mcp, skills, claudemd) NO llevan ts y no enseñan nada: no
  describen un momento sino una configuración. Fechas y horas con `Intl`
  en el idioma activo; solo "ahora/min/h/ayer" van al diccionario.
- El campo "Filtrar…" de Consejos llevaba el estilo del sistema y
  desentonaba con todo: ahora usa el mismo campo del rediseño.

### S8 — Ajustes y rastro de los avisos (2026-08-05)

- AJUSTES en tarjetas por tema (General · Avisos · Precios · Exportar ·
  Ajustes compartidos · Acerca de) en vez de una lista corrida. Las filas
  son "etiqueta a la izquierda, control a la derecha" separadas por una
  línea tenue, y las CASILLAS se pintan como interruptores (el checkbox
  nativo desentonaba con todo). Al invertir el orden hubo que mover el
  <input> detrás del <span> en las 6 filas con casilla. "Avisos" agrupa
  alarmas de %, presupuesto y celular: son el mismo tema ("cuándo quiero
  que me avise") y estaban sueltos. Encabezados nuevos `prefs_general` y
  `prefs_alerts` ×8 idiomas.
- RASTRO DE LOS AVISOS en la bitácora del flujo (`flowLog`), a raíz de la
  duda de Oscar ("no sé si los post-its funcionan o si nunca se dan las
  condiciones"): `fndBadgeCalc` y `renderTipsDot` anotan cuando el aviso
  se ENCIENDE o se apaga, y cuando no se enciende dicen por qué ("las N
  tarjetas ya estaban vistas"). Eran los únicos avisos sin huella.
  DIAGNÓSTICO de su caso, leído en su propia bitácora: el circuito está
  intacto (el panel sigue mandando `fnd` y `coach` en quota:update y la
  pastilla los pinta) — lo que pasa es que a las 14:54 hizo un escaneo
  manual con la pestaña abierta (10 tarjetas → vistas) y la pasada diaria
  de las 14:55 encontró esas mismas 10, ya vistas: badge nulo, sin
  campana. Y no hubo ninguna pasada por cierre de sesión porque en
  Windows no nació ningún recibo `sum` en todo el día. Trampa del
  vigilante, cuarta aparición.
- ACERCA DE (2026-08-05, pedido de Oscar: "aunque sea estático"). NO quedó
  estático: reúne cosas que ya existían sueltas — la comprobación de
  versión (que hasta ahora solo corría sola a los 8 s del arranque) con su
  botón, el atajo a Releases (`open_releases`, URL constante en Rust) y
  "Reportar un problema", que abre el formulario de issues con la VERSIÓN
  y el sistema ya escritos: quien reporta casi nunca los incluye y sin
  ellos el reporte no sirve. La versión sale del comando nuevo
  `app_version` (env!("CARGO_PKG_VERSION")): escribirla en el frontend
  sería una segunda verdad que se queda vieja sola.
- Botones de ntfy: "Canal nuevo" y "Enviar prueba" se encimaban. El campo
  del canal manda ahora en su fila (.ntfy-url) y los botones saltan de
  línea si no caben; la confirmación de "Canal nuevo" pasó a fila propia,
  lo que de paso quitó un `style.display` que peleaba con [hidden]
  (invariante 10bis). Los botones secundarios (Copiar, Canal nuevo, CSV,
  JSON, Actualizar ahora, hub) van en tono apagado: el degradado es para
  la acción principal de cada tarjeta.
- BUG del mudanza de Ajustes compartidos (2026-08-05, lo vio Oscar: botones
  muertos): `loadRemotes()` —quien habilita los botones vía
  syncHubButtons— solo corría al abrir Fuentes de datos. Cuando el bloque
  vivía ahí, bastaba; al mudarlo a Ajustes, entrar directo a esa pestaña
  dejaba `remotes=[]` y los botones desactivados para siempre. Arreglo:
  loadRemotes corre al ARRANCAR (get_remotes solo lee remotes.json, local
  y barato) y al abrir cualquiera de las dos pestañas que dependen de los
  servidores. LECCIÓN para el resto del rediseño: al mover un bloque de
  pestaña, buscar qué inicialización dependía de ABRIR la pestaña vieja.
  Y el bloque quedó penúltimo (antes de Acerca de), como en la maqueta.

### Coach multi-fuente (2026-08-05) — local + WSL + servidores SSH

Pedido de Oscar tras entender la limitación con el ejemplo del doctor: los
Hallazgos (análisis de laboratorio) ya veían el VPS, pero el coach (la
enfermera del momento) era ciego fuera de lo local — y él trabaja por SSH
DENTRO del VPS, donde más gasta. Implementación:
- Rust: coach_scan recorre también las distros WSL; CoachHit gana `origin`
  (lo pone get_coach al fusionar, como el origen del export) y
  fetch_remote_coach trae los hits de cada servidor.
- meter-export.py: réplica completa del motor bajo `--coach` (atajo: sin
  agregación de gasto), con TODO el detalle — pendiente fantasma blindado,
  gaps de caché, dedup de tool_use por id, ai-title, leaks del cierre — y
  estado incremental propio en ~/.cache/michiclaude/coach_state.json (el
  exportador es un proceso nuevo por sondeo; sin estado releería sesiones
  enteras cada 3 min). El estado solo guarda sesiones vivas: se poda solo.
- Frontend: fichas, recibos y los pushes de "terminó"/"espera tu
  aprobación" enseñan el origen cuando no es local.
VALIDACIÓN EN VIVO (la mejor posible): el --coach corrido en el VPS
detectó LA PROPIA SESIÓN de trabajo de esta jornada — compact con 802k de
contexto, attach con 26 relecturas, 1021 turnos, $401.86, pending:true
mientras Claude ejecutaba herramientas. Sondeo incremental: 73-91 ms.
VERIFICACIÓN DEL CIRCUITO DE INDICADORES (pedida por Oscar): el resumen
emite fnd+coach antes del early-return de ok:false; la pastilla pinta
fdot/tdotc y el gatito hasfnd/hastip, ambos antes de su early-return; y
desde hoy fndBadgeCalc y renderTipsDot dejan rastro en flowLog al
encenderse/apagarse. PRUEBA EN VIVO PARA WINDOWS: recompilar y esperar el
primer sondeo (60 s) — esta sesión del VPS siempre está enorme, así que
la ficha compact con origen VPS-EU y el post-it turquesa DEBEN aparecer.
VALIDADO EN VIVO EN WINDOWS (2026-08-05, capturas + bitácora de Oscar,
a la primera): el sondeo trajo los consejos del VPS — fichas "compact"
(816k) y "attach" (26 lecturas) con su "michiclaude · VPS-EU", los
comandos /clear y /compact resaltados en cajita, el POST-IT TURQUESA del
gatito encendido con su contador (2, luego 1), el rastro nuevo en la
bitácora ("tips: AVISO ENCENDIDO (2 sin ver)" → "vistas con foco — aviso
apagado") y hasta el push de "Terminó tu sesión en michiclaude · VPS-EU"
con el origen dentro. La campana ROJA no encendió y el rastro dijo por
qué: "fnd: sin aviso — las 10 tarjetas ya estaban vistas" (trampa del
vigilante, ya no muda: ahora se explica sola). Acerca de con su estilo y
el bug rojo: validado.
MATIZ CONOCIDO que salió en la prueba: el push de "terminó" saltó para la
sesión del VPS aunque sigue viva (1727 min, 1041 turnos) — 5 minutos de
silencio entre turnos disparan "done" una vez por sesión, igual que en
local con una pausa larga. Semántica asumida del diseño, no bug: el
banderín notified impide que se repita.
- CAMPANA/POST-IT ROJO VALIDADOS EN VIVO (2026-08-05, tras re-armar
  fndSeen/fndAutoLast por consola): post-it rojo "9+" en el gatito,
  contador 5 en la pestaña, "fnd: AVISO ENCENDIDO" en la bitácora. Con
  esto TODO el sistema de avisos (rojo y turquesa, gatito y pastilla,
  panel y pushes) queda comprobado de punta a punta con datos reales.
- MARCO FANTASMA del panel (lo vio Oscar): la ventana es fija (no puede
  redimensionarse en vivo — regla de las transparentes) y el panel medía
  su contenido: pestañas cortas dejaban un hueco TRANSPARENTE debajo que
  enseñaba lo de atrás. Arreglo: .panel pasa de max-height a HEIGHT — 
  llena siempre la ventana con fondo sólido; lo corto deja espacio vacío
  interior (intencional) y lo largo sigue con scroll interno.

---

## Cierre de jornada — 2026-08-05

RONDA DE REDISEÑO UX/UI: TERMINADA Y VALIDADA, las cinco pestañas con
capturas de Oscar en el mismo día. De la maqueta v5 al panel real:
paleta azul-noche/violeta, tipografía embebida (Inter/Sora/JetBrains,
OFL), tarjetas con fondo, hero con chip de estado, avatares de proyecto,
popovers de filtros (calendario de rango + proyectos con buscador y
chips), hallazgos con icono por tipo y "cuándo pasó", consejos con el
ofrecimiento de proponer en el hueco de la búsqueda, ajustes en tarjetas
con interruptores, y Acerca de con versión real y reporte pre-llenado.

LO GRANDE que cayó además del rediseño: COACH MULTI-FUENTE (local + WSL
+ SSH) — el exportador replica el motor completo bajo --coach con estado
incremental en el servidor (~80 ms/sondeo), validado en vivo con la
propia sesión de trabajo (816k ctx) y con las fichas, el post-it
turquesa, el push con origen y el rastro en flowLog funcionando a la
primera en Windows. Y el AVISO ROJO validado también (post-it 9+ tras
re-armar fndSeen).

BUGS CAZADOS EN LA JORNADA: campo origin sin comodín en done/sum (no
compilaba), ajustes compartidos muertos al mudarse de pestaña (su init
dependía de abrir la pestaña vieja), lista de proyectos descuadrada
(grid vs flex), falta de corte superior en los rangos del analizador
Python, y el marco fantasma del panel (max-height → height).

PRÓXIMA SESIÓN: lo que Oscar traiga. En la lista viven: validación
pasiva natural (alarmas/ntfy/aviso al cierre), decisión del updater
(repo público + tag), capturas del README, y las ideas apuntadas para su
momento (hub con rangos por día; armonizar el widget con la estética v5
si algún día apetece).

---

## Cierre de jornada — 2026-08-06/07

REPORTE EJECUTIVO: DE IDEA A PESTAÑA FUNCIONANDO en dos días. Nació del
documento de estrategia que trajo Oscar (context rot / medir desperdicio
en vez de consumo): primero el análisis con tabla comparativa de 22
puntos (docs/presion-y-rendimiento.md — veredicto: el Nivel 1 ya lo
cubríamos casi entero; lo nuevo viable era rendimiento + antes/después),
luego el diseño del reporte con mockups de IA externa (Oscar eligió el
A, documento ejecutivo en llano), y de ahí las tres fases.

FASE 1 — MOTOR DE DATOS (verificado en el VPS con logs reales):
- Turnos útiles `uturns` (mensajes HUMANOS: fuera meta, sidechain,
  tool_result, comandos, inyecciones <ide_…) en totales, proyectos y
  serie daily (que ganó también tokens/día). is_user_turn réplica exacta
  Rust↔Python; caché de escaneo v2 ambos lados. Regresión con logs
  CONGELADOS y --end fijo: campos viejos idénticos byte a byte;
  coherencia 7d+7d=14d exacta; muestreo del filtro sin falsos (el <ide_
  se cazó ahí: el IDE inyecta avisos con rol user sin marcar meta).
- Histórico de cuota quota_history.json (90 días, una foto por ciclo,
  freno 150 s; log_quota desde refresh() solo con lectura buena).
  Validado en Windows: la primera foto nació con s/w/sr/wr correctos.
- Marcas de arreglo fndHist/fndMarks (solo hallazgos de estado, escaneos
  ≥7d sin rango; visto ≥3d + desaparecido ≥2d = arreglado).

FASE 2 — PESTAÑA REPORTE (validada con capturas de Oscar): 6.ª pestaña,
chips Semana/Mes/Personalizado (calendario compartido, target "rep"),
héroe de rendimiento, "¿te duró más o menos?" del histórico de cuota
(con estado honesto "juntando datos"), gráfica 4 semanas, proyectos con
delta vs periodo anterior y "qué lo encareció" de hallazgos reales,
marcas con antes/después (mínimo 5 días o "midiendo"), y "para los días
que vienen" con recomendación por fuga.

RONDAS DE CAPTURAS (dos): (1) velocidad — caché por periodo + render
progresivo; pasada ligera de hallazgos 20h→3h (el porqué del "nunca vi
el post-it rojo": hallazgos del VPS no disparan cierre local y a 20 h
llegaban tarde — 4.ª mordida de la trampa del vigilante); 6 pestañas en
una fila; re-render al cambiar idioma (gráfica en español dentro del
panel en japonés). (2) maqueta michiclaude-hero-grafica.html de Oscar —
héroe EFICIENCIA/VOLUMEN con ≈$ real pegado a cada dato de tokens, nota
"no es contradicción" solo cuando divergen, regla "1M tok ≈ $X" con la
tarifa MEDIDA del periodo (mejora sobre la maqueta, que traía tarifa
fija), gráfica grande con conmutador tokens/$ estimado, barras de
volumen y detalle al tocar. Margen transparente 8→5→3→1 px; scrollbar
overlay fina.

PRIMERA MEDICIÓN REAL del rendimiento: ~51k tok/turno en el VPS (7d) —
nuestras propias sesiones son intensas; el reporte de Oscar marcó
"empeoró 13%"… con nosotros mismos como causa. El medidor midiéndonos.

PENDIENTE AL CIERRE: fase 3 (export HTML del mockup A), validación
natural del post-it rojo (ahora con la pasada de 3 h tiene cómo), y que
el histórico de cuota junte días para llenar "¿te duró más o menos?".

## 2026-08-07 (segunda sesión) — "Leído" estilo Gmail y diseño de remediación

AVISOS POR TARJETA (pedido de Oscar): abrir la pestaña de Hallazgos o
Consejos —aunque sea por error— ya NO marca nada como visto. Cada
tarjeta se marca LEÍDA con su propio clic (el mismo que pliega/
despliega); el contador de pestaña y el post-it del widget descuentan
una por una, como Gmail descuenta correos abiertos. Ignorar apaga la
suya; restaurar ignorados revive las no leídas; el ✕ del coach despacha.
Cayeron los marcados masivos del render (y con ellos el requisito de
document.hasFocus, que solo existía para que la precarga invisible no
matara el aviso). La TRAMPA DEL VIGILANTE (4 mordidas) queda ENTERRADA:
ya no existe "nace vista por estar mirando la pestaña". Sin claves i18n
nuevas. Detalle fino: en el coach, guardar coachCards ANTES de repintar
el contador (lee de localStorage — al revés quedaba desfasado un clic).

REMEDIACIÓN — DISEÑO DESTILADO en `docs/remediacion.md`: análisis de una
propuesta externa (handoff de otra IA + mockups). Se conservó lo bueno
(intención-no-comando, regla de oro "en la duda pregunta", confianza
progresiva con candados, clasificador de tarea viva por TodoWrite) y se
corrigió lo que chocaba: archivar JSONL a 30d se dejaba ciega a la
propia app (→ ≥365d), el "modelo local" para casos dudosos ya estaba
descartado en presion-y-rendimiento.md, el countdown no puede ser globo
(regla única), los checks "Aplicar /compact//clear" mienten sin canal de
escritura (→ relevo ConPTY `michi claude`, el "tmux nativo" con 5 reglas
anti-choque), y el "handoff Pro" necesita una IA que no hay. 4 etapas,
cada una útil sola; NO arrancar hasta cerrar el reporte.

## 2026-08-07 (tercera sesión) — cierre del reporte y prompts guardados

"LEÍDO" ESTILO GMAIL — VALIDADO por Oscar desde el inspector: los
contadores y post-its descuentan tarjeta por tarjeta al clicarlas y
abrir la pestaña o el post-it ya no borra nada. Tema cerrado.

REPORTE EJECUTIVO — CERRADO HASTA DONDE ESTÁ (decisión de Oscar): las
fases 1 y 2 quedan como están, funcionando; se retoma solo si al usarlo
falta algo o pide ajustes. La fase 3 (export HTML del mockup A) NO se
arranca; queda anotada en el pendiente como lo primero si se retoma.
Siguen vivos de esa área el cargo check de la fase 1 en Windows y la
validación en vivo, que caerán con el uso normal.

PROMPTS DE DISEÑO DE REMEDIACIÓN — guardados como referencia en
`docs/prompts-diseno-remediacion.md` (rescatados del transcript de la
sesión anterior): bloque de estilo común con la paleta y tipografía
REALES del panel + 7 prompts (tarjeta de intención, modo automático con
candados, countdown, registro de acciones, manómetro en widgets,
tarjeta educativa, relevo en terminal) + las 3 notas de uso (handoff
Pro fuera a propósito, correcciones de honestidad ya incluidas, y que
lo que devuelva la otra IA es referencia visual — la integración se
traduce al sistema real del panel). Referenciado desde remediacion.md
y desde el pendiente de REMEDIACIÓN en CLAUDE.md, cuyo candado se
re-redactó: ya no es "hasta cerrar el reporte" (cerrado hoy) sino
"decisión explícita de Oscar" (matar procesos es clase nueva de
capacidad).

## 2026-08-07 (cuarta sesión) — remediación etapa 1a: manómetro de presión

Arranca la etapa 1 de remediación (consejero con intención — la única
que no necesita la decisión pendiente de Oscar: no toca nada, solo
mide). Primera pieza: el MANÓMETRO DE PRESIÓN DE CONTEXTO, puntos 9-10
de presion-y-rendimiento.md ("muy viable y barato: el dato ya existe").

CÓMO: regla nueva `press` en el motor del coach — un hit por sesión con
contexto y quieta <10 min (PRESS_QUIET_MAX), value = tokens de contexto
crudos y campo aditivo `quiet` (minutos quieta). Implementada en las DOS
piezas del motor (Rust `coach_scan` + `--coach` del exportador,
invariante #1); viaja por el canal de siempre (`get_coach`, fusión con
origin intacta — un exportador viejo simplemente no la manda y el
manómetro remoto no existe, degradación honesta). NO es ficha ni aviso:
coachPoll la aparta como done/ask (no gasta tope diario ni tipSeen),
elige la más fresca (menos quieta; empate → más contexto) y emitPill la
monta como campo `press` de quota:update con el % ya redondeado sobre
200k (PRESS_FULL, constante del frontend con su comentario). Sin hit en
un sondeo el manómetro se APAGA solo (la sesión se durmió).

UI: arco de manómetro SVG inline (pathLength=100 + stroke-dasharray, sin
fuentes externas) en la cápsula de la pastilla y del gatito — diminuto,
sin texto ni tooltip (regla de la cápsula); el NÚMERO vive en el detalle
pcard (fila con barra y proyecto·origen) y en el globo del hover del
gatito (bloque con barra). Umbrales PROPIOS 60/85 (presión de contexto,
no ritmo de cuota): calma = acento/tinta, ámbar ≥60, rojo ≥85. Se pinta
ANTES del early-return de ok:false como las campanas: la presión sale de
los logs locales y un fallo del endpoint de cuota no la toca. En
card.html hizo falta `.blk[hidden]{display:none}` — la misma trampa del
10bis (display:flex anula al atributo hidden). Clave i18n `press_lab`
×8. Previews de navegador actualizados en las 4 ventanas.

Decisiones: press NUNCA va a ntfy ni al hub (es lectura local); no pasa
por el interruptor de avisos (es lectura, como el % de sesión); 200k
como techo es constante comentada del frontend — el backend manda tokens
crudos a propósito para que un cambio de techo sea un solo número.

VERIFICADO: node --check en los scripts de las 5 ventanas, py_compile
del exportador, press_lab en los 8 idiomas, cero firmas Rust tocadas
(campo aditivo + regla nueva). PENDIENTE: cargo check en el Windows de
Oscar y verlo en vivo con una sesión real. Siguen 1b (parser TodoWrite +
clasificador) y 1c (tarjeta de intención + clipboard).

## 2026-08-07 (quinta sesión) — remediación etapa 1 COMPLETA: clasificador y tarjeta de intención

La 1a (manómetro) quedó VALIDADA en vivo por Oscar en cuanto compiló:
sus capturas mostraron el arco en la pastilla, el 86% rojo en el detalle
con "michiclaude · VPS-EU" y la fila en el globo del gatito. De paso
preguntó qué significa y qué hacer — la respuesta es justamente 1b+1c,
así que se implementaron en esta misma jornada.

1B — SEÑALES EN EL MOTOR (Rust + Python, invariante #1): el estado del
coach gana todos_open/todos_total (del ÚLTIMO TodoWrite de la sesión —
la señal reina), trail (últimos 20 archivos tocados con
Read/Edit/Write) y commit_clean (hubo `git commit` y nada se editó
después; cualquier edición lo apaga). El hit `press` los lleva como
campos aditivos topen/ttotal/cont/gclean, donde cont = Jaccard % de los
últimos 10 archivos contra los 10 previos (¿sigue en lo mismo?). El
estado viejo del exportador migra solo (setdefault contra el default).

DECISIÓN DE ARQUITECTURA: el motor manda HECHOS crudos; el veredicto
Alive/Boundary/Uncertain vive UNA sola vez, en JS (`intentVerdict`):
topen>0 → alive; lista cerrada al 100% o commit limpio → boundary;
cont≥40 → alive; si no, unsure. Así el invariante #1 solo carga con los
hechos y la lógica no se duplica en tres lados. La señal de "lenguaje de
cierre" sigue FUERA (solo-español vs app de 8 idiomas, ya documentado).

1C — TARJETA DE INTENCIÓN: con presión ≥80% (INTENT_PCT) coachPoll
sintetiza el hit LOCAL `intent` y lo mete al pipeline normal de
tarjetas del coach — hereda gratis el anti-spam por sesión (tipSeen),
el leído estilo Gmail, el ✕, el TTL de 24 h y el aviso
post-it/foco/contador. Exenta del tope diario (perder por tope justo el
aviso que más ahorra sería un contrasentido) y se REFRESCA en cada
sondeo sin renacer (conserva born/min/v; despachada NO resucita). La
tarjeta pregunta la intención en llano — "¿Sigues trabajando en lo
mismo?" / "¿Ya terminaste?" — con el comando pequeño al lado (el
usuario aprende el mapeo), evidencia medida siempre visible ("Michi
detectó: lista 5/6 · sigues en los mismos archivos · último msg hace X
min"), insignia "Recomendado" SOLO cuando el veredicto no es unsure
(regla de oro), advertencia ámbar en /clear si hay pendientes, botón
"Copiar comando" y "Ahora no". El clic de copiar NO pliega ni marca la
tarjeta (stopPropagation): copiar no es terminar de leer.

CLIPBOARD: dep nueva tauri-plugin-clipboard-manager (la justificada en
el diseño), invocada DIRECTO con plugin:clipboard-manager|write_text —
sin wrapper npm (invariante #4). Capability
clipboard-manager:allow-write-text añadida. Escribe al portapapeles
SOLO al clic del usuario.

VALIDACIÓN: node --check en el panel, py_compile, paridad de las 16
claves int_* ×8 por conteo, y prueba de fuego REAL — el exportador
nuevo corrió sobre los logs de este VPS (estado aislado con
XDG_CACHE_HOME para no pisar el del exportador productivo) y detectó
esta misma sesión de trabajo: press con topen=5, ttotal=6 (la lista de
tareas real del momento), cont=50 y quiet=0 → veredicto alive →
recomendaría /compact. El simulador "🧪 Simular hallazgos" gana una
tarjeta intent falsa para probar lo visual sin esperar presión real.
PENDIENTE: cargo check en Windows (la dep nueva se descarga en la
primera compilación) y ver la tarjeta nacer en vivo.

### Sexta sesión (2026-08-07) — la prueba real que se hizo sola

El pendiente "ver la tarjeta nacer en vivo" se resolvió de la forma más
poética posible: la sesión de Claude Code del VPS en la que CONSTRUIMOS
la tarjeta llegó al 100% de presión (se compactó trabajando), y Michi la
cazó en el Windows de Oscar sin simulador ni re-armado — "digamos que
fue prueba real jaja" (Oscar, con capturas).

Lo que confirmaron las capturas, punto por punto:
- La tarjeta nació sola en Consejos: "Tu sesión ya pesa mucho · 100%",
  proyecto "michiclaude · VPS-EU" (el origin remoto pintado bien).
- El VEREDICTO acertó: evidencia "lista de tareas: 0 de 5 sin terminar
  · commit reciente sin cambios después · último mensaje hace 3 min" →
  frontera → insignia RECOMENDADO en /clear. Exactamente lo que el
  clasificador debía concluir con esos hechos (la lista de todos ya
  estaba completada y el último commit no tenía ediciones después).
- Globo del gatito con la fila "Presión de contexto 100%" en rojo entre
  Sesión y Semanal; arco del manómetro en la cápsula conviviendo con el
  % de sesión (94%); contador "1" en la pestaña Consejos.
- De rebote: cargo check y la compilación con la dep nueva
  tauri-plugin-clipboard-manager pasaron en Windows (nada de esto
  existiría en pantalla sin ella).

Con esto la ETAPA 1 de remediación queda validada en vivo de punta a
punta salvo un clic: el botón "Copiar comando" (pegar y ver /compact o
/clear). Las etapas 2-4 siguen sin arrancar a la espera de la decisión
explícita de Oscar (matar procesos = clase nueva de capacidad).

Remate: Oscar probó el botón "Copiar comando" y funcionó. ETAPA 1
COMPLETA Y VALIDADA al 100%, sin pendientes.

## 2026-08-07 (séptima sesión) — remediación etapa 2: automático out-of-band

Oscar dio el GO explícito a las etapas 2-4 (la decisión que faltaba:
matar procesos es clase nueva de capacidad). Se implementó la ETAPA 2
completa; las 3-4 (el relevo ConPTY) esperan a que esta pase cargo check
en Windows y se valide en vivo — misma disciplina por etapas que
funcionó con la 1, y además el relevo construye sobre el registro y el
desbloqueo progresivo que nacen aquí.

Qué se construyó (decisiones detalladas en docs/remediacion.md
§"Decisiones de la etapa 2"):

- **Rust, 5 comandos nuevos** (todos async + spawn_blocking, 10ter):
  `scan_zombies` (foto de procesos por PowerShell/CIM sin deps nuevas;
  zombie = proceso que casa con la firma de un MCP stdio de
  ~/.claude.json Y padre muerto o PID de padre reciclado),
  `kill_zombie` (re-verifica PID+ejecutable+arranque ±2 s justo antes
  del Stop-Process; "gone" si ya no está, ERR_ZOMBIE_CHANGED si el PID
  ya es de otro), `scan_archivable` + `archive_old` (mueve .jsonl ≥365d
  a %APPDATA%\<app>\archive conservando estructura; WSL fuera hasta la
  etapa 4), `get_action_log` (registro actions_log.json, tope 200,
  datos crudos que el panel traduce — invariante #10).
- **Frontend:** sección "Remediación automática" en Ajustes (toggles
  zombie ON / archive OFF por defecto, candado "Michi no automatiza lo
  que no has visto", revisar/cerrar/archivar a mano, registro de
  acciones) + tarjeta de zombies en Consejos por el pipeline normal
  (nace solo cuando el automático no puede actuar; su "Cerrar todos" ES
  la primera manual que desbloquea; clave zombie|arranque-más-nuevo
  para que un lote nuevo re-avise sin resucitar lotes despachados) +
  sondeo `remPoll` horario y archivado auto una vez al día.
- **i18n:** 28 claves × 8 idiomas (paridad verificada por script en la
  sesión).
- Sin tocar meter-export.py: nada de esto viaja por SSH (SOLO LOCAL),
  así que el invariante #1 no se activa.

Trampa evitada sobre la marcha: `#[cfg]` sobre bloques-expresión en
posición de cola dentro de un closure NO compila tras el strip (el
bloque queda en posición de statement); se cambió a la pareja de
funciones cfg'd, el mismo patrón de `wsl_claude_dirs`.

PENDIENTE para validar la etapa 2 (en el Windows de Oscar): cargo
check, ver la sección en Ajustes, "Revisar ahora" con y sin zombies
(fabricar uno: abrir una sesión con un MCP stdio y matar la terminal),
el clic manual que desbloquea, el kill automático a la hora siguiente,
el registro con auto/manual, y el archivado con un .jsonl viejo de
laboratorio (tocar mtime con `(Get-Item f).LastWriteTime=...`).

### Validación en vivo de la etapa 2 (2026-08-07, Windows de Oscar)

Zombies VALIDADO de punta a punta: detección, cierre manual, desbloqueo
del candado, cierre automático a los 90 s del arranque y registro con
sus dos líneas (`03:40 manual` / `03:45 auto`, ambas «fantasma»). El
`cargo check` queda implícito: la app compiló y arrancó con el código
corregido. Archivado, pendiente de la prueba de laboratorio.

Dos bugs que SOLO salían en Windows real — ninguno era visible leyendo
el código, y por eso la regla de validar en la máquina de verdad antes
de dar una etapa por buena:

1. **Barras.** La firma sale de `~/.claude.json` con barra normal
   (`@modelcontextprotocol/server-memory`) y la línea de comando del
   proceso ya resuelto lleva barra invertida
   (`…\node_modules\@modelcontextprotocol\server-memory\dist\index.js`):
   NINGÚN MCP lanzado con npx casaba jamás. Se normalizan ambos lados a
   `/` antes de comparar (commit 68d84e0).
2. **El script del kill moría en el parser de PowerShell.** Iba en UNA
   línea y PowerShell no acepta el `}` de un bloque seguido de otra
   sentencia sin separador: el script no llegaba a ejecutarse, stdout
   salía vacío y TODO cierre acababa en ERR_ZOMBIE_KILL ("No se pudo
   cerrar") mientras `Stop-Process` a mano funcionaba perfecto. Ahora
   lleva saltos de línea reales; REGLA: script de PowerShell escrito
   desde Rust, saltos reales SIEMPRE. El escaneo nunca lo sufrió porque
   es una tubería de una sola sentencia (commit 144986a). De paso, el
   veredicto ya no se decide con `$?` —que con `-ErrorAction
   SilentlyContinue` no distingue "no pude" de "ya no estaba"— sino
   re-consultando el PID, y un veredicto irreconocible deja foto cruda
   en `rem_debug.json` (sin eso el fallo era indistinguible desde la UI:
   nos costó tres rondas de terminal descubrirlo).

Cómo se fabricó el zombie de laboratorio (lo primero que falló): un MCP
bien educado NO sirve. Con `@modelcontextprotocol/server-memory`, al
matar el `cmd` de arriba se cerró toda la cadena sola — cuando su
cliente muere, él se va. Los zombies reales los dejan los MCP que
ignoran el cierre de stdin. Receta que sí funciona: `mcp-fantasma.js`
con `setInterval(function(){},1000000)`, `claude mcp add fantasma --
node <ruta>` y lanzarlo con `powershell -Command "Start-Process node
-ArgumentList '<ruta>' -WindowStyle Hidden"` — ese powershell
intermedio muere en el acto y deja al node huérfano de nacimiento.

Nota de comunicación (Oscar es nuevo en terminal): NUNCA dar comandos
con huecos tipo `<PID>` o `EL_NUMERO_NUEVO` — los pega literales y
PowerShell escupe un error que no dice nada útil. O el número ya
puesto, o un comando que busque por nombre y no necesite sustituir
nada.

Archivado validado el mismo día con un .jsonl de laboratorio (copia de
uno real con `LastWriteTime` a -400 días): lo detectó, apareció el botón
"Archivar ahora" —que solo nace cuando hay algo que archivar— y el
archivo acabó en `%APPDATA%\<app>\archive\C--Users-oscar\` conservando la
estructura, con su línea en el registro. ETAPA 2 CERRADA.

De la validación salieron además dos arreglos de i18n: "1 archivos" y
"1 logs" — todos los textos con contador necesitan su ternario de
singular (los 5 idiomas que lo distinguen; ja/ko/zh no).

### Etapa 3a — el relevo `michi claude`, validado en vivo (2026-08-08)

Crate aparte `relevo/` (paquete `michi`, fuera de `src-tauri` para que la
app no gane dependencias ni el vigilante de `npm run dev` lo recompile).
Compiló a la PRIMERA en el Windows de Oscar y el paso transparente
funcionó de entrada: Claude Code entero dentro de la ConPTY —colores,
flechas, resize, `/login` con navegador— sin enterarse de que hay alguien
en medio del cable. Seis pruebas, todas pasadas: transparencia, `michi
status` desde otra terminal, inyección real de `/compact` (se escribió y
se ejecutó sola), y el candado negándose con texto vivo en el prompt.

Por el camino cayeron TRES fallos, y los tres enseñan algo distinto.

**1. Los avisos del terminal no son teclas.** Por el mismo cable de
entrada llegan cambios de foco (`ESC [ I` / `ESC [ O`), respuestas de
posición del cursor (`R`), identificación (`c`), estado (`n`) y medidas
(`t`). Contaban como actividad humana y reiniciaban la ventana de calma,
así que bastaba con SALIR de la terminal —justo lo que hace el usuario
para ir al panel de MichiClaude— para que nunca se pudiera inyectar.
`KeyWatch::feed` devuelve ahora `human` y solo eso mueve el reloj.

**2. El prompt no se puede modelar solo con lo que entra.** Con `hola`
sin enviar, `status` decía `texto: no` y la inyección se aplicó: salió
`hola/compact` como un solo mensaje. R5 aguantó —no se borró nada, que
era el peor caso previsto— pero el guardián falló. Dos causas de diseño:
`typed` era un booleano APARTE del buffer de la línea (dos fuentes de
verdad; al desincronizarse mandó el booleano, ahora se DERIVA del
buffer), y el Enter limpiaba el modelo a ciegas (ahora aparta la línea a
`pending` y espera a ver si Claude REACCIONA: bytes por la PTY después
del Enter = enviado; 3 s de silencio = no se envió y la línea vuelve).

**3. La causa REAL, que no era ninguna de las dos.** El diagnóstico
nuevo (`michi status --debug`, que enseña CUENTAS de teclas y `line_len`,
nunca contenido) lo destapó en una ronda: con `hola` escrito, `k_print:
0` y `k_esc: 38`. El relevo no había contado una sola tecla en su vida.
En Windows Terminal, **ConPTY pide `win32-input-mode` (`ESC [ ? 9001 h`)
al arrancar y el terminal se lo concede a TODA la ventana** — incluida la
nuestra, que es quien reenvía esa petición sin saberlo. Con ese modo cada
tecla viaja como `ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _` y no llega ni un
carácter suelto. Las letras alcanzaban a Claude porque el relevo reenvía
los bytes intactos; el contador las veía como ruido. Y el terminador `_`
cae dentro de `0x40..0x7e`, así que las secuencias cerraban limpias y
nada chirriaba. `KeyWatch::win32_key` las decodifica (`Uc` es el carácter
en decimal, solo con `Kd` = pulsación). Validado: `hola oscar` = 10, y
`k_print: 10`, `line_len: 10`, `typed: true`, `ERR_RELAY_TYPED`.

Reglas que salen de aquí, para no repetirlas:

- **Envolver una terminal no es reenviar bytes.** Hay un protocolo que el
  terminal y la ConPTY negocian a espaldas de quien está en medio, y el
  de en medio HEREDA esa negociación sin enterarse.
- **Un guardián que cuenta cosas tiene que exponer sus cuentas.** Un
  `k_print: 0` valió más que tres rondas de teoría. Y se puede hacer sin
  romper la privacidad: cuentas y longitudes, jamás contenido.
- **Una sola fuente de verdad.** Un booleano "resumen" al lado del dato
  real acaba mandando él, y mintiendo.
- **Fail-closed de verdad:** mientras no se sabe si un Enter envió, se
  cuenta como que hay texto.

### El manómetro llevaba meses clavado: el techo no era 200k (2026-08-08)

Validando la etapa 3b, la cabecera de Claude Code cantó el bug sin querer:

```
Claude Code v2.1.225
Opus 5 (1M context) · Claude Max
```

**1M.** El manómetro de presión dividía entre `PRESS_FULL = 200000`, una
constante puesta cuando 200k era el techo de TODOS los modelos. Opus y
Sonnet saltaron a 1M en la 4.6, y Fable/Mythos nacieron ahí.

No hizo falta creerse la cabecera: los propios logs lo tenían medido.
Contexto máximo alcanzado por modelo en las sesiones de Oscar:

| modelo | máximo real |
|---|---|
| claude-opus-5 | **998.248** |
| claude-fable-5 | 836.644 |
| claude-opus-4-8 | 641.326 |

Casi un millón. Con el techo viejo esas sesiones marcaban **100%**
permanente: gauge en rojo, gatito alarmado y tarjeta de intención
disparada. Y al revés, la tarjeta saltaba en cuanto se cruzaban 160k
tokens (el 80% de 200k), que en un modelo de 1M es el **16%** del
depósito. La sesión de trabajo de ese mismo día, medida en el VPS, iba
por 480.757 tokens: 48% real, 100% según el panel.

**Por qué se arregló ANTES de la etapa 3c y no después.** La 3c es el
countdown que aplica `/compact` SOLO. Su disparador es este porcentaje.
Construir el automático encima de una cifra 5× equivocada habría hecho
que Michi comprimiera el historial —perdiendo contexto real— con el 84%
del depósito libre. Un número mal calibrado es inofensivo mientras solo
se mira; deja de serlo en cuanto algo actúa sobre él.

**El arreglo no necesitó ni una descarga nueva.** Las tres fuentes de la
cascada de precios publican el techo en el MISMO archivo que ya bajamos
cada 24 h: LiteLLM en `max_input_tokens`, models.dev en `limit.context`,
OpenRouter en `context_length`. `PriceEntry` gana un campo `ctx` y el
caché en disco lo hereda; `ctx_for()` lo lee y, si la fuente no lo dijo
(o el caché es de una versión anterior), cae a `ctx_table()`, respaldo
embebido que decide por VERSIÓN y no por lista de modelos —invariante
#6—, hermano de `price_table()`.

Tres detalles que costaron pensarlos:

- **La duda se resuelve hacia abajo.** Sin dato, 200k. Quedarse corto
  hace que el manómetro avise antes de tiempo (molesto); pasarse haría
  que no avisara nunca (el usuario choca con el muro sin previo aviso).
  El fallo seguro de un avisador es avisar de más.
- **`price_key()` recorta el sufijo `[1m]`** para casar el id del log con
  las tablas públicas. Si el techo se resolviera después de esa
  normalización, una variante de contexto largo se leería como su base de
  200k. Por eso `ctx_for()` mira el id CRUDO antes de buscar en la tabla.
- **Se guarda el modelo, no el techo ya resuelto.** El estado de la
  sesión (Rust y Python) recuerda el id del último turno y el techo se
  calcula en cada sondeo: así una tabla recién descargada corrige la
  cuenta sola, en vez de arrastrar un número viejo hasta que la sesión
  muera.

En el panel el denominador vive en UN solo sitio (`pressFull()` /
`pressPct()`) — la lección de la trampa del booleano resumen, aplicada
antes de que muerda: tres divisiones repartidas por el archivo eran
exactamente la forma en que este bug sobrevivió tanto tiempo.

Regresión: export normal y `--findings` idénticos byte a byte; `--coach`
solo añade la clave nueva.

Lección general: **una constante con el nombre de un límite externo es
una fecha de caducidad esperando.** No estaba mal escrita — estaba mal
envejecida, y nada en el código podía avisarlo. Cuando el límite lo
publica alguien de fuera, el número se busca, no se escribe.

### Auditoría de las tres fuentes de precios (2026-08-08)

Recién metido el techo de contexto en la tabla de precios, Oscar hizo la
pregunta correcta: *"¿y si mañana una fuente dice 2 millones y otra menos?
¿coinciden o manejan parámetros distintos?"*. La cascada es de RESPALDO,
no de verificación cruzada — la primera que responde manda —, así que el
número puede depender de qué servidor estuviera vivo ese día. Se
descargaron las tres y se cruzaron modelo por modelo.

**Precios: coinciden al céntimo.** Cero discrepancias en todos los modelos
que las tres comparten. Fable 5 a 10/50, Opus 5 a 5/25, Sonnet 5 a 2/10.

**Techo: una sola discrepancia.** `claude-sonnet-4-5`: LiteLLM dice 200k,
models.dev dice 1M. Las dos tienen razón a medias — ese modelo es de 200k
con un beta de 1M, así que el número correcto depende de si el beta está
activo. No hay tabla que pueda saberlo; lo sabe la máquina del usuario.

**Y la pregunta destapó un fallo que llevaba ahí desde el principio.**
OpenRouter escribe la versión con PUNTO —`claude-opus-4.8`— donde LiteLLM,
models.dev y los propios logs usan GUIÓN. La tercera fuente casaba **6 de
sus 14** modelos:

```
casan hoy:                   6
casarían con punto→guión:   14
las 8 que faltaban: haiku-4-5, opus-4-1, opus-4-5, opus-4-6,
                    opus-4-7, opus-4-8, sonnet-4-5, sonnet-4-6
```

Nunca explotó porque LiteLLM siempre responde primero. Era un tercer
paracaídas con un agujero que nadie había mirado, y no solo para el techo:
también para los **precios**. `claude-opus-4-8`, uno de los modelos que
Oscar usa a diario, era uno de los ocho.

Arreglo: `price_key()` unifica punto→guión **entre dígitos** (para no tocar
`anthropic.claude-opus-5`). Va dentro de `price_key()` a propósito, que es
el único punto por el que pasan las dos partes —guardar y buscar—, así que
ambas quedan con la misma forma y siguen casando. Normalizarlo solo al
guardar habría roto la búsqueda de `claude-2.1`.

**La evidencia por encima de la tabla.** Para el caso sonnet-4-5 y para
cualquier fuente que se quede corta, `ctx_full()` compara el techo de la
tabla con el contexto MÁXIMO que esa máquina ha alcanzado de verdad. Si lo
medido supera a la tabla, la tabla está demostrablemente mal y mandan los
tokens. Detalle que costó pensarlo: no se puede devolver lo visto a secas
—una sesión de 480k daría 480k de techo y el manómetro volvería a marcar
100%—, así que se sube al primer escalón de `CTX_LADDER`, una lista de
MAGNITUDES (200k/1M/2M/5M), no de modelos.

Lo que este cambio NO resuelve, dicho para que no se olvide: una fuente que
INFLE el techo (2M donde son 1M) silenciaría el aviso, y contra eso la
evidencia no sirve. La respuesta buena es el detector de auto-compacts que
ya está apuntado como pendiente: Claude Code comprime cerca del límite
real, y esa es la medida más honesta que existe.

**Lo que cambió de opinión por el camino.** Al calibrar el techo escribí que
quedarse corto era "el fallo seguro" de un avisador. Eso vale mientras
Michi solo MIRA. En cuanto la etapa 3c aplique `/compact` sola, equivocarse
por abajo deja de ser molesto y pasa a ser destructivo: comprimiría un
historial sano. Con automatización los dos errores duelen, y lo que hace
falta es puntería, no una dirección segura. De ahí sale una regla para la
3c: **el automático se gana con certeza; si el techo no es de fiar, la 3c
aconseja pero no actúa.**

Lección general: **una cascada de respaldo no es una cascada verificada.**
Mientras la primera fuente responda, las otras dos son código que nadie
ejecuta — y el código que nadie ejecuta se pudre sin avisar. Si hay
paracaídas de repuesto, hay que abrirlos de vez en cuando.

### El relevo deja de depender de que te acuerdes (2026-08-08)

Tres piezas del mismo problema, planteado por Oscar: *"los usuarios se les
olvide y empiecen a trabajar y se den cuenta de que MichiClaude no ejecutó
nada"*. Un automático que depende de un hábito no es un automático.

**El atajo del PATH.** La pregunta que lo desatascó fue suya: *"¿se puede
hacer genérico o hay que especificar por herramienta?"*. La respuesta cambió
el diseño: **las terminales y los editores no interpretan `claude`** —
ejecutan un shell, y el shell resuelve el comando. Perseguir "el top 10 de
herramientas" era perseguir el objeto equivocado; el eje real eran cuatro
shells. Y por encima de los cuatro hay algo mejor: un `claude.cmd` propio
primero en el PATH, porque ahí resuelve **Windows**, no el shell. Un
mecanismo, y vale para Windows Terminal, VS Code, Cursor, Warp, Alacritty y
los que salgan mañana. Validado: `claude` a secas abrió con relevo y el panel
lo detectó solo.

Lo que costó de la primera prueba: **una pestaña nueva no es una terminal
nueva.** Windows Terminal heredó su entorno al arrancar y se lo pasa a cada
pestaña, así que el PATH nuevo no llegaba. El aviso decía "abre una terminal
NUEVA" — engañoso, porque una pestaña lo parece. Ahora dice que hay que
cerrar la VENTANA. Y el `.cmd` pasó a ASCII puro: la raya del comentario
salía como `â€”` porque un `.cmd` no declara codificación y cmd.exe lo lee
con la página de códigos que toque. En un comentario es cosmético; en un
archivo de órdenes, una bomba de relojería.

**Y el indicador estaba en el sitio equivocado.** Lo levantó Oscar: *"¿no
estaría bien ver de forma visual qué sesión está activa con relevo, y no
darme cuenta al final de que no?"*. Tenía razón y era un fallo de diseño mío
— el indicador vivía en el panel, que es donde NO tienes los ojos. Trabajas
en la terminal.

Plan A (poner el título al arrancar) **no sobrevivió**: Claude Code pone
«Claude Code» en cuanto arranca. Estaba declarado como best-effort antes de
probarlo, así que el plan B ya estaba pensado: como el relevo ve pasar todos
los bytes, `TitleMark` intercepta la secuencia OSC del título y le antepone
la marca. La pestaña queda «michi · MichiClaude · Claude Code» y la marca
sobrevive a cada reescritura de Claude porque se pega a todas.

Es la ÚNICA excepción al paso transparente del relevo, así que va acotada:
solo `ESC ] 0|1|2 ;`, **leyendo el número entero y no el primer dígito**
—`ESC]10;` es el color de primer plano y tratarlo como título le habría
metido la marca dentro—, sin apilar marcas, y con tope de 1024 bytes que
suelta lo retenido tal cual. Fail-open: lo peor posible es quedarse sin
marca, jamás comerse la salida. Diez casos probados con un puerto de la
máquina de estados antes de tocar un compilador.

Lecciones:

- **Cuando algo "hay que acordarse de hacerlo", el diseño está incompleto.**
  No es un problema de documentación ni de disciplina del usuario.
- **Antes de integrar N herramientas, buscar qué tienen debajo.** Diez
  terminales eran cuatro shells, y cuatro shells eran un PATH.
- **Un indicador va donde están los ojos**, no donde es cómodo ponerlo.
- **Declarar "best-effort" antes de probar** convirtió un fallo en un paso
  previsto: el plan B ya estaba pensado cuando el plan A cayó.

## 2026-08-08/09 (cierre) — el automático se prueba solo en vivo y foto completa de pendientes

VALIDADO EN VIVO, y con la mejor evidencia posible: el ciclo completo del
automático sobre la sesión de chat del VPS (Windows → SSH → relevo →
extensión). Primera corrida: countdown y silencio — dos fallos de UX
(rechazo `ERR_RELAY_BUSY` quemaba la sesión para siempre; la cuenta
acababa sin veredicto). Arreglados (reintento a 10 min + cierre ✓/✕) y la
SEGUNDA corrida la vivió Oscar sin tocar nada: cuenta atrás, ✓ verde,
`auto · aplicó /compact en «VPS-EU»` en el registro, 872 960 tokens
liberados — y ese /compact cayó sobre la conversación de trabajo real.

También de estas sesiones:
- **Manómetro tras compactar**: todo `compact_boundary` pone `last_ctx=0`
  (mentía hasta 10 min y causaba el /compact redundante "No messages to
  compact"). Rust + exportador, regresión byte a byte.
- **Auto-compactación de Claude Code** investigada sobre el binario
  v2.1.226 y decisión tomada: no se apaga ni se sugiere (red de
  seguridad + precompute); entramos al 80% vs su ~94%. Un /compact
  inyectado se registra `manual` → `acomp` nunca se avisa a sí mismo.
- **La compactación no deja `usage`** en el log: no es facturable desde
  los .jsonl; solo se ve en cuota. Si un día se enseña: estimado o nada.
- **Interruptor del chat** (`set_chat_relay` + `CHAT_WRAP_PY`): el
  wrapper de VS Code en servidores SSH se enciende desde Ajustes; 8
  casos en banco (ajeno no se pisa, ilegible no se toca, backup,
  NOWRAP). Validado por Oscar: "VPS-EU ✓".
- **Lista blanca analizada y cerrada en 2** con regla de entrada
  (libera + no destruye + verificable); /usage//context//cost nativos
  son lo que el widget vuelve innecesario; /doctor se recomienda, jamás
  se inyecta.
- **presion-y-rendimiento.md** llevaba desde el 05 diciendo "sin
  arrancar" con las fases 1-2 vivas: corregido y añadida la sección
  "Qué queda vivo de este doc".

FOTO DE PENDIENTES al cierre (consolidada por Oscar, 2026-08-09):

- BLOQUEADOS POR DECISIÓN DE OSCAR: updater (repo público + tag v* +
  probar), michi.exe en el instalador (release.yml, solo desde la web),
  capturas del README, lanzamiento (repo privado hasta que diga).
- CÓDIGO DEL RELEVO: alias `~/.bashrc` del VPS (el más corto; cierra
  chat ✓ + terminal SSH ✗), WSL entero (relevo en la distro + alias),
  chat del Windows local (modo wrap en michi.exe).
- VALIDACIÓN PASIVA (usar la app): alarmas reales (umbral/100%/ventana
  nueva), camino ntfy completo (con PC apagada), hallazgo naciendo
  natural con panel cerrado, y el automático arreglado (✓/✕ + reintento)
  — la primera ya pasó en vivo este mismo día.
- APARCADOS CON DISEÑO: HUB+rangos (espera 2.ª máquina), Reporte fase 3
  (export HTML mockup A), detector de pegado masivo, apuesta #2 (tarjeta
  del gatito + gamificación), /export como red pre-/clear, ficha
  recomendando /doctor.
- DE presion-y-rendimiento §"Qué queda vivo" (orden de valor): fórmula
  del % de desperdicio (DISEÑO previo), botón "copiar resumen de
  traspaso", frecuencia de auto-compacts como hallazgo (la señal ya
  está; con el relevo debería BAJAR — prueba medible de que Michi
  trabaja), push ntfy "reporte listo", hábito sin /clear, marcas de
  arreglo manuales, auditoría semántica de CLAUDE.md (pide modelo).
- DE consejos-coach.md: hooks opt-in ("futuro, quizá nunca"), fix
  personalizado por entrypoint (cimiento puesto), botón de issues útil
  al hacerse público el repo.
- DECISIONES CERRADAS (no rediscutir sin leer su doc): lista blanca de
  2, auto-compact no se apaga, compactación no facturable, marketing
  honesto (cuota real + momento elegido + fugas medibles, jamás "ahorra
  compactando"), y los descartes de raíz (score único, modelo local,
  telemetría, rastrear otras herramientas, BD historial, modo empresa,
  podar CLAUDE.md automático).

Lecciones:

- **Una cuenta atrás que acaba sin decir qué pasó es peor que no haber
  avisado**: deja al usuario adivinando si actuaste.
- **Un rechazo transitorio no puede costar un castigo permanente**:
  `done` solo tras aplicar de verdad.
- **Las cabeceras de estado de los docs también se regresionan**: un
  "sin arrancar" viejo esconde pendientes reales y hace rediscutir lo
  hecho. Al cerrar etapa, actualizar el doc que la diseñó.
- **"$0" sin decir QUÉ cuesta cero confunde**: la inyección es gratis,
  la compactación no — y la diferencia importa para el pitch.

## 2026-08-09 — auto-/clear con red: /export verificado antes de borrar

Oscar pidió, con la captura de una conversación de 729 turnos delante,
que Michi decida el /clear como ya decide el /compact. El "jamás" del
auto-/clear tenía escrita su propia salida (remediacion.md §lista blanca:
"candidato razonable: inyectar /export antes como red") y eso exacto se
construyó. Diseño completo en remediacion.md §El auto-/clear con red;
aquí el resumen y lo que se aprendió.

- La regla (a)(b)(c) de la lista blanca no se relajó: a /clear se le
  CONSTRUYÓ la (b). El relevo teclea `/export <ruta>` (ruta que genera
  ÉL — jamás viaja por el canal; la lista del canal sigue en 2),
  VERIFICA que la copia existe con contenido, y solo entonces borra.
  Sin copia: `ERR_RELAY_EXPORT` y cero /clear.
- Verificado en el binario 2.1.226 ANTES de escribir código: `/export`
  a secas abre un menú interactivo (inyectarlo así atraparía el REPL);
  con argumento escribe directo. Por eso la ruta es obligatoria.
- La secuencia corre en un hilo propio en las TRES piezas: esperar la
  copia tarda segundos y bloquear el bucle principal habría dejado el
  estado sin refrescar >15 s — el panel habría dado la sesión por
  muerta. De ahí también `ERR_RELAY_BUSY` mientras dura.
- `STATE_V` 1→2 como compuerta de compatibilidad: un relevo viejo
  ignoraría la marca `export` y borraría SIN copia — el panel no le
  pide la red a un v1 (ni manual ni automático).
- El automático del /clear exige, además de todo lo del /compact:
  interruptor propio `relayClear` (nace OFF), 3 manuales ganadas,
  veredicto Boundary del clasificador (en la duda gana /compact, que
  no borra) y relevo v≥2. El manual de la tarjeta de intención lleva
  la red siempre que el relevo sepa (v2).
- VALIDADO en banco de PTY real en el VPS: terminal 13/13 (regresión
  /compact intacta, /export ANTES de /clear, copia en disco, claude
  sordo → ERR_RELAY_EXPORT y nada borrado) y chat stream-json 6/6
  (sid casado, orden, eco de ambas inyecciones visible). PENDIENTE:
  cargo check y validación en vivo en el Windows de Oscar.
- Copias en `<datos>/handoff/` (Windows) y `~/.michiclaude/handoff/`
  (Linux), nombre `handoff-<pid>-<epoch>.md` sin ni un dato del
  usuario; caducan a los 90 días al arrancar el relevo.

Lecciones:

- **A una prohibición sana no se le quita el candado: se le construye
  la condición que le faltaba.** /clear no cumplía "no destruye";
  con copia verificada en disco, la cumple. La prohibición de fondo
  (jamás borrar sin red) sigue intacta y ahora es código.
- **La verificación buena es un hecho del disco, no un texto en
  pantalla**: el archivo existe con contenido. Los textos cambian de
  idioma y de versión; los archivos no.
- **Todo lo que espera dentro del relevo va en su hilo**: la misma
  lección que el 10ter de la app (síncrono congela), versión PTY.

## 2026-08-09 (segunda) — el Enter pegado al texto: por qué falló la red en la primera prueba real

Oscar compiló, abrió sesión con relevo (`v:2`, `ready:true`) y corrió
`michi inject /clear --export`. Respuesta: `ERR_RELAY_EXPORT`. La red
hizo exactamente lo que promete —no se borró nada— pero la copia no
aparecía y en pantalla no salía ningún error.

Reproducido en el VPS contra Claude Code REAL (dos sondas de PTY que
solo se diferencian en el ritmo del tecleo):

- `"/export <ruta>\r"` escrito de una vez → la línea se queda ESCRITA
  en el prompt y no se ejecuta. Cero salida, cero error.
- El texto, 0,6 s de pausa, y el `\r` aparte → `Conversation exported
  to:` y archivo de 762 bytes en disco.

Causa: la TUI de Claude Code trata el texto y el Enter que llegan en la
MISMA ráfaga de lectura como un PEGADO, y un pegado no se envía solo.
Con `/compact` (9 bytes) colaba; con una ruta de ~110 bytes, jamás.

Arreglo: `type_line()` en las dos piezas de PTY escribe el texto, duerme
250 ms (`ENTER_GAP_MS`) y manda el Enter aparte. Se aplica a TODOS los
comandos: el fallo dependía del largo de la línea y de la velocidad de
la máquina, así que el `/compact` validado el 2026-08-08 estaba vivo de
suerte y podía morder en cualquier momento. El modo chat no lo necesita
(ahí un mensaje es una línea JSON, no teclas).

Validado end-to-end contra Claude Code real: `aplicado: /clear (copia:
…)` con una copia de 912 bytes que CONTIENE la conversación. Banco de
falso claude: 13/13 sin regresión.

Nota de método: el banco de falso claude NO podía cazar esto — su
"claude" lee líneas de stdin, así que el ritmo le da igual. Un banco
prueba tu código contra tu idea del mundo; solo el programa real prueba
tu idea del mundo.

Lecciones:

- **Escribir en una PTY no es mandar bytes: es imitar a un humano.** Y
  un humano no teclea 110 caracteres y el Enter en el mismo instante.
- **Si la TUI no reacciona a algo que SE VE escrito en pantalla,
  sospechar del RITMO antes que del contenido.** La ruta era correcta,
  los permisos también, el comando también.
- **Un fallo que depende del largo de la entrada es un fallo dormido**:
  el /compact "funcionaba" y solo escondía el mismo defecto.

## 2026-08-09 (tercera) — el binario que no se recompiló: empate de mtime

Tras subir el arreglo del Enter, Oscar hizo `git pull` (confirmado:
`git log` en `0e9a283`), recompiló y el fallo SEGUÍA. Media hora de
diagnóstico para algo que no estaba en el código:

- `Select-String ENTER_GAP_MS src\main.rs` → el fuente SÍ traía el
  arreglo (tres coincidencias).
- `cargo build --release` → `Finished` en **0.11 s**, sin ninguna línea
  `Compiling`.
- `cargo clean -p michi` → **"Removed 0 files"**.
- `dir michi.exe` → `08:26`. `dir src\main.rs` → **`08:26` también**.

El `git pull` cayó en el MISMO MINUTO que la compilación anterior.
Cargo decide por fecha de modificación y ante un empate no recompila:
el `.exe` que se ejecutaba seguía siendo el de antes del arreglo. El
"sigue sin funcionar" era literalmente cierto — nunca llegó a probarse
el código nuevo.

Arreglo: `(Get-Item src\main.rs).LastWriteTime = Get-Date` + `cargo
build --release` → `Compiling michi v0.1.0` en 7,55 s y binario de las
08:40. Anotado como regla en CLAUDE.md §Comandos.

Lecciones:

- **Antes de dudar del arreglo, comprobar que el arreglo se ejecutó.**
  Tres señales lo decían y ninguna era el mensaje de error: build
  instantáneo, `clean` que no borra nada, y la hora del binario.
- **`cargo clean -p <paquete>` no es garantía**: dijo "Removed 0 files"
  y nadie se alarmó. La prueba buena es la HORA del ejecutable, no lo
  que diga la herramienta.
- **Al guiar a alguien por comandos, pedir la hora del binario después
  de compilar.** Es una línea y corta en seco toda esta clase de
  diagnóstico fantasma.

## 2026-08-09 (cierre) — el /clear automático nace, y dos bugs que lo tapaban

Jornada larga que empezó con una pregunta de Oscar ("¿puede Michi decidir
el /clear como decide el /compact?") y acabó con la función construida,
validada a mano en Windows y esperando su primer disparo automático.

**Lo entregado** (6 commits, todos con su porqué):

- `8b4bd40` auto-/clear con red: `/export` verificado antes de borrar.
  Detalle y decisiones en remediacion.md §El auto-/clear con red.
- `0e9a283` el Enter va SEPARADO del texto. Bug real que también
  amenazaba al /compact ya validado — vivía de suerte por ser corto.
- `c53cf7f` la lección del binario que no se recompiló (empate de mtime).
- `65f391f` validación en vivo en Windows.
- `5d7c6be` botón «abrir la copia» en el registro de acciones.
- `137d881` regla de lectura de archivos grandes en CLAUDE.md.

**Estado del auto-/clear al cerrar** (medido, no supuesto): interruptores
ON, desbloqueo ganado (/compact 2/2, /clear 5/3), relevo del chat del VPS
vivo en v2 y casado por `sid` EXACTO, veredicto **Boundary** (topen 0,
ttotal 5, gclean true). Lo único que falta es presión: **676k de 1M =
68%**, y el umbral son 80. Oscar decidió NO bajar `INTENT_PCT` para
forzarlo — se gana por diseño, no por carrera.

**Idea de producto de Oscar, anotada para cuando haya datos:** que
MichiClaude no solo señale la fuga sino que enseñe la práctica, con un
bloque copiable para el CLAUDE.md. Tres fichas candidatas, cada una
colgada de un detector que YA existe: claudemdsize (partir el archivo en
índice + historial), reread (leer por rangos) y la fuga al cierre. Regla
de diseño acordada: **una ficha entra solo si hay una señal medible que
la dispare** — un consejo sin dato medido es un post de blog, y eso no es
lo que hace fuerte a este producto. La regla anti-relectura que se puso
hoy en CLAUDE.md es el banco de pruebas: si el detector `reread` deja de
dispararse, la ficha se escribe con el antes y el después.

**Qué queda vivo, por orden:** ver el auto-/clear dispararse solo; medir
el efecto de la regla de lectura; el indicador de relevo en el widget
(lo pidió Oscar al ver que el chat de VS Code no dice si está relevado);
el alias de `~/.bashrc` en el VPS; `michi.exe` en el instalador (toca el
workflow, invariante #9); WSL y el chat del Windows local. Bloqueados por
decisión: updater (repo público + tag) y capturas del README.

Lecciones:

- **Un bug que depende del largo de la entrada es un bug dormido**: el
  /compact "funcionaba" y escondía exactamente el mismo defecto.
- **Antes de dudar del arreglo, comprobar que el arreglo se ejecutó.**
- **Un banco prueba tu código contra tu idea del mundo; solo el programa
  real prueba tu idea del mundo.**
- **A una prohibición sana no se le quita el candado: se le construye la
  condición que le faltaba.**

## 2026-08-10 — etapa 4: el alias de ~/.bashrc para las terminales SSH

El fleco de las terminales de los servidores, cerrado desde el propio VPS
(la máquina donde va a vivir). Detalle de diseño en remediacion.md §"El
alias de ~/.bashrc"; aquí la jornada y sus lecciones.

**Qué se hizo:** guion `TERM_ALIAS_PY` embebido en lib.rs (viaja por
SSH-STDIN, jamás interpolado; veredictos de una palabra), comandos
`term_relay_status`/`set_term_relay` (misma coreografía que el wrapper del
chat: re-subir el relevo ANTES de encender), ejecutor SSH generalizado
(`remote_verdict_py`, ahora compartido con `chat_wrap_remote` sin cambiar
su firma), interruptor en Ajustes bajo el del chat (oculto sin servidores,
invariante #8) y claves `rly_term_*` en los 8 idiomas.

**El enganche es una FUNCIÓN de bash, no un alias ni un shim:** necesita
decidir (¿TTY?, ¿está el relevo?, ¿ya relevado?) y `~/.bashrc` solo lo
leen las shells interactivas — los scripts ni se enteran, que es el
reparto correcto. Fail-open en cascada al `command claude`. Sin bucle por
construcción: las funciones de bash no viajan a subprocesos, así que el
relevo resuelve `claude` por PATH y da con el binario real.

**Validación: banco de 29 comprobaciones contra un HOME falso, 29/29.**
Ciclo on/off que devuelve el archivo byte a byte, backup exacto una sola
vez, idempotencia, marcas rotas = MANUAL sin tocar nada, bloque viejo
reemplazado entero, permisos 600 conservados, `bash -n`, y la función
corriendo de verdad: TTY simulada con `script(1)` → banner del relevo y
el claude real debajo; sin TTY o con `MICHI_RELEVO` → directo al real.
La invocación por STDIN (`python3 - status`) probada tal cual la hará
Rust, y el guion re-extraído del lib.rs para confirmar que lo embebido es
idéntico a lo probado.

Lecciones:

- **El único fallo de la primera pasada era el guard funcionando:** el
  banco corría DENTRO de una sesión ya relevada (este Claude Code del VPS
  va bajo michi-relevo.py) y el `MICHI_RELEVO` heredado disparaba el
  fail-open anti-anidamiento — el relevo hijo cedió al claude real, que
  es exactamente lo prometido. Validación en vivo gratis; el banco ahora
  limpia la variable con `env -u`.
- **`"#` dentro de un raw string de Rust lo CIERRA:** las marcas del
  bloque (`A = "# >>> …"`) contienen comilla+almohadilla y matan un
  `r#"…"#` — el guion va en `r##"…"##`. Sin toolchain en el VPS lo cazó
  la revisión a mano; cargo check en Windows lo habría dicho, pero mejor
  no viajar roto.
- **CLAUDE.md rozó su tope de 40k al anotar el avance** (40.314): el
  diseño aplazado de HUB+rangos se movió ÍNTEGRO a hub-modo-equipo.md y
  quedó el puntero. La regla del archivo aplicada al archivo.

**Pendiente que abre:** cargo check en el Windows de Oscar (aquí no hay
toolchain) y la validación de punta a punta desde el panel (encender el
interruptor, abrir una SSH nueva, ver el banner). De la etapa 4 quedan:
WSL, chat del Windows local y michi.exe en el instalador.

**Cierre del pendiente (mismo día, más tarde):** cargo check limpio en el
Windows de Oscar (11.63 s, con `Compiling` de verdad — no hubo empate de
fechas) y la validación de punta a punta COMPLETA: interruptor nuevo en
Ajustes → «VPS-EU ✓ — abre una sesión SSH nueva para que lo tome», y en
una SSH nueva `claude` mostró el banner «michi · relevo activo (sesión
N)». Verificado además del lado del servidor: bloque con marcas en
`~/.bashrc` (líneas 120–130), backup `.michi-backup` creado en el
instante del encendido, `michi-relevo.py` re-subido fresco y `bash -n`
limpio. El alias de ~/.bashrc queda CERRADO.

## 2026-08-10 (tarde) — el banner del relevo dentro del chat de VS Code

Oscar pidió una señal visible de que el chat va relevado (maqueta previa
con otra IA: banner como primer mensaje y pestaña con su nombre). Se
implementó en el modo `wrap` y costó TRES intentos, cada uno con su
lección:

1. **Pegado al init:** el banner se emitía justo detrás del
   `system/init`. No se pintó nunca — en el arranque la interfaz del chat
   todavía no está lista y la línea se pierde sin dejar rastro.
2. **Delante del primer mensaje:** movido a la primera actividad de
   usuario (con re-armado al cambiar el `session_id`, para que cada
   conversación estrene el suyo). Tampoco se pintó.
3. **La causa real — la FORMA de la línea:** medido contra el binario de
   la extensión (2.1.226), el replay del CLI no es un `user` a secas:
   lleva `session_id`, `uuid`, `parent_tool_use_id`, `timestamp` e
   `isReplay`. La extensión DESCARTA EN SILENCIO lo que no case con la
   sesión. `replay_line()` imita esa forma campo a campo y el banner
   apareció a la primera. Al hijo se le sigue mandando la forma corta.

Lecciones:

- **El eco al chat y el mensaje al hijo son DOS formas distintas** y no
  se pueden confundir: `user_line()` hacia Claude, `replay_line()` hacia
  la extensión. El eco de las INYECCIONES iba con la forma corta desde
  siempre — o sea que el `/compact` inyectado podía no verse en el chat
  pese a que el diseño lo exige («nada a tus espaldas»). El banner
  destapó un fallo silencioso que llevaba tiempo ahí.
- **Un descarte silencioso se diagnostica midiendo, no leyendo:** la
  forma buena salió de correr el binario real con
  `--replay-user-messages` y mirar la línea que emite él.
- Banco propio (10/10) con un claude falso que habla stream-json: init
  intacto, banner único por conversación, re-armado al cambiar de sesión,
  el hijo recibiendo solo los mensajes reales y el paso directo sin
  protocolo. VALIDADO EN VIVO en el chat del VPS.

**Pendiente que abre:** el guion viaja EMBEBIDO en la app
(`include_str!`), así que hasta que Oscar recompile en Windows su
MichiClaude re-subirá la versión vieja al arrancar. `git pull` + build
para que quede permanente.

## 2026-08-10 (cierre) — michi.exe viaja en el instalador

Tercer fleco de la etapa 4, y el de más valor de producto: sin él, todo lo
construido en la etapa 3 solo existía en la copia de desarrollo de Oscar.
Detalle de diseño en remediacion.md §"michi.exe dentro del instalador".

Lo que enseñó la jornada:

- **El pendiente decía "workflow, invariante #9" y no hacía falta ningún
  workflow.** `beforeBuildCommand` mueve la construcción del crate al
  propio Tauri, así que el CI lo hace solo. Un pendiente puede estar
  bloqueado solo por cómo se enunció.
- **Una verificación con `git pull` que dice "Already up to date" no es
  una verificación:** Oscar compiló 6m24s con la configuración vieja
  porque yo le di los comandos ANTES de empujar los cambios. Empujar
  primero, pedir después.
- Su salida de PowerShell delató que `npm run dev` funciona desde
  `src-tauri` (npm sube a buscar el package.json) — de ahí el doble
  intento de ruta en el comando previo. Leer la salida entera del usuario
  paga: ahí venía un dato que yo no había pedido.

## 2026-08-10 (noche) — WSL, y dos fallos que solo salen probando

Etapa 4d cerrada y VALIDADA EN VIVO (detalle y diseño en remediacion.md
§"WSL, la tercera máquina"). Oscar puso el correctivo que la abrió: yo
proponía dejar WSL dormido porque ÉL trabaja por Remote-SSH, y su
respuesta fue que la app es para más gente que él. Tenía razón: el modo
que uno no usa sigue siendo el modo de alguien.

Lo que enseñó la jornada:

- **Lo que compila y parece razonable puede no ejecutarse nunca.**
  `wsl.exe -- sh -c 'guion' michi <arg>` no entrega `$1` (ssh sí). El
  código era simétrico al de SSH, pasó `cargo check`, y estaba roto.
  Solo se vio ejecutando el comando A MANO en la máquina de verdad.
- **El fallo grave no era ese, era el silencio.** Con la operación vacía
  los guiones caían en la rama de "apagar", no encontraban nada que
  quitar y contestaban OK: el interruptor enseñaba ✓ de algo que jamás
  tocó la distro. Escribimos la regla "nada a tus espaldas" para el
  relevo y la incumplió el propio panel. Ahora una operación que no se
  reconoce contesta BADOP. Callar es peor que fallar.
- **Diagnosticar por hipótesis tiene un límite.** Perseguí "¿corre como
  root?" con dos comandos antes de rendirme a lo obvio: ejecutar a mano
  el comando exacto que hace la app. Ese fue el que habló, y a la
  primera. Cuando dos observaciones se contradicen, deja de teorizar y
  reproduce.
- **Un `git pull` que dice "Already up to date" no es una verificación:**
  antes, Oscar compiló 6m24s con la configuración vieja porque le di los
  comandos antes de empujar. Empujar primero, pedir después.
- **Probar sin el programa real vale:** `tests/claude-falso.sh` (cinco
  líneas que imprimen lo que llega) permitió validar la cadena entera sin
  instalar Claude Code en la distro ni gastar cuota. Al relevo le basta
  una PTY viva que reaccione.

De paso quedó probado que la marca del título (`TitleMark`) también
funciona en WSL, que no lo habíamos mirado.

## 2026-08-10 (cierre) — el chat de Windows, y la ETAPA 4 COMPLETA

Último fleco del relevo, pedido por Oscar con el argumento que lo cambió
todo hoy: "es para más usuarios que yo". Diseño y detalle en
remediacion.md §"El chat de VS Code en Windows".

El relevo llega ya a las TRES máquinas (Windows local, SSH y WSL) por las
DOS vías (terminal y chat), y todo está validado en vivo.

Lecciones de la jornada:

- **Un enganche invisible necesita rastro desde el primer día.** La
  extensión se come stderr: un wrapper que no arranca se ve EXACTAMENTE
  igual que uno que funciona (la conversación sigue, el relevo no
  aparece). Estuvimos dos rondas suponiendo; `wrap_debug.txt` contestó a
  la primera y además demostró que el paso directo funciona (la llamada
  `auth status --json`, sin protocolo, se dejó pasar tal cual).
- **32 minutos de diferencia entre dos binarios nos tuvieron ciegos:** el
  ajuste apuntaba a la copia que `tauri dev` deja junto al ejecutable, y
  esa copia era de ANTES de compilar el rastro. Al ver un `michi.exe`
  VIVO en la lista de procesos se entendió todo: el enganche funcionaba
  desde el principio; lo que faltaba era el binario nuevo.
- **El banco encontró un fallo que habría borrado ajustes ajenos:** con un
  settings.json escrito en una sola línea, nuestra clave compartía renglón
  con los ajustes del usuario y al apagar el interruptor se los llevaba.
  Se cazó antes de tocar un archivo de verdad. De ahí que la línea se
  inserte siempre sola en su renglón.
- **No duplicar la coreografía:** en vez de escribir un segundo attend
  para el chat, se extrajo un `Speaker` — la terminal teclea, el chat
  manda protocolo, y R1-R5 y la red del /export son una sola
  implementación. Es la misma decisión que en `relay_inject_fs` y
  `relay_from_json` esta misma tarde: si hay dos copias de una regla, un
  día divergen.

**Residual:** el banner del chat no se pinta en Windows (en Linux sí). No
es el mecanismo —el eco del /compact inyectado usa la MISMA línea de
replay y sale perfecto—, es cuándo se emite. Pendiente de rastro,
cosmético.

**Cierre real del 4e (misma noche):** el aviso del chat de Windows YA SE
PINTA. Tres rondas creyendo que el mecanismo fallaba, y no era eso:

1. Dos rondas con un binario viejo — el ajuste apuntaba a la copia que
   `tauri dev` rehace en cada arranque. Corregido: en debug manda el
   michi.exe que uno compila. Un fantasma solo se caza mirando QUÉ
   ejecutable está corriendo (`Get-Process michi | Select Path`), no
   releyendo el código.
2. Y la última: el aviso llegaba mientras el mensaje del usuario iba en
   vuelo, y ahí la extensión no lo pinta; el eco de un /compact inyectado
   —la MISMA línea— sí salía porque llega con el chat en reposo. Se emite
   ahora tras el `result` del primer turno. La pista no vino de una idea
   nueva sino de comparar dos usos del mismo mecanismo, uno que funcionaba
   y otro que no, y preguntar en qué se diferenciaban.

De paso: el interruptor tiene que reconocer sus PROPIAS rutas anteriores.
Si no, tras mover cuál michi.exe se usa, ve su ruta vieja como "wrapper
ajeno", se niega a pisarla (regla correcta con uno de verdad ajeno) y se
queda encallado en OTHER sin forma de salir desde la interfaz.

## 2026-08-11 — la presión de contexto deja de ser un arco y pasa a ser una idea

Petición de Oscar con dos bocetos HTML propios: la presión de contexto del
gatito, contada como una BOMBILLA que se degrada, con el gato "pensándola".
La columna que pidió, de abajo arriba: gato → bombilla en medio → cápsula del
% de sesión, y "que no quede amontonado".

El manómetro anterior era un arco SVG de 13 px metido dentro de la cápsula.
Funcionaba y era honesto, pero competía por el espacio con el %, el rótulo y
la cuenta atrás del automático: se VEÍA y no se LEÍA. Un dibujo que cambia de
forma —filamento limpio, onda, maraña, dos trozos y una grieta— se entiende
sin mirarlo fijo, que es justo lo que hace un widget de bandeja.

**Lo que se conservó tal cual** (era la mitad del trabajo): niveles con los
umbrales de siempre (60 y 85, más un paso nuevo en 40 que solo cambia el
dibujo), el punto del RELEVO —ahora en el casquillo—, la bombilla fuera del
early-return de la cuota (la presión sale de los logs, no del endpoint), y el
número exacto con su proyecto en el globo del hover. La pastilla NO se tocó:
sigue con su arco.

**El truco que hizo barato el cambio: `.stage`.** La ventana tenía que crecer
hacia arriba, y sobre esos 210x157 estaba calibrado casi todo el widget en
PORCENTAJES: el recorte de los gifs, la zona de clic de la cabeza, los dos
post-its. Recalcularlos habría sido una tarde y un rosario de bugs finos. En
vez de eso, el gato y todo lo suyo se metieron en un `.stage` que mide
EXACTAMENTE lo que medía la ventana (210x157) y va pegado al fondo: los
porcentajes resuelven contra él y ni uno cambió de significado. Verificado
pintando la zona de la cabeza de rojo y forzando los post-its: caen donde
caían.

**Tres trampas que no se ven leyendo el código:**

1. **El gato se hundía.** La posición guardada es la esquina SUPERIOR
   izquierda; al crecer la ventana 48 px hacia arriba, quien tuviera el gatito
   posado sobre la barra de tareas (la posición por defecto) se lo habría
   encontrado medio tapado. `migrate_cat_geometry` conserva el borde INFERIOR
   una sola vez (campo `geom`), que es lo que ya hacía `set_pill_style` al
   alternar pastilla ↔ gatito. Y va en píxeles FÍSICOS: con pantalla al 150%
   son 72, no 48 — de ahí el factor de escala.
2. **Los globos caían sobre la bombilla.** Su solape está medido contra el
   BORDE de la ventana, no contra el gato. Sumarles `CAT_TOP_H` los devuelve
   exactamente donde estaban respecto a la cabeza. Por eso el alto de la
   franja es una constante y no un número suelto en tres sitios.
3. **El vidrio no contrastaba.** El primer render salió correcto de geometría
   y mudo de lectura: el cristal casi blanco sobre el papel del globo (casi
   blanco también) desaparecía, y a escala 0.85 el filamento no se distinguía.
   Se arregló con lo que sí se lee a 26 px — la TEMPERATURA del vidrio, cálida
   encendida y fría muerta — y subiendo la bombilla a escala 1:1, con los
   rayos acortados para que no rocen el borde de la nube.

**Cómo se verificó sin Windows.** El VPS no tiene ni toolchain de Rust ni
Pillow, así que: (a) un decodificador GIF en stdlib para MIRAR el arte y medir
dónde caen las llamas del estado `fire` (arriba-izquierda) y las Z del `zzz`
(arriba-derecha) — la bombilla se colocó en el pasillo libre que queda entre
las dos; (b) una composición de la ventana nueva con las cajas de la columna
encima, para ver choques y aire; (c) chromium headless renderizando un banco
que EXTRAE el `<style>` y el marcado reales de cat.html —nada retecleado— en
los cuatro niveles y las dos pieles. `cargo check` sigue pendiente del Windows
de Oscar: aquí no hay cargo.

De paso, el simulador recorre los cuatro niveles (`p` en SIM_CAT → `simPress`,
resuelto DENTRO de emitPill y no parcheando `lastPill`, que la regla prohíbe).
Sin eso, ver el estado "muerta" costaba llenar un contexto de verdad hasta el
85%.

**Y una cuarta trampa, de regalo: `.hidden` no existe en SVG.** Al copiar el
patrón del punto del relevo (`$("x").hidden = !relay`) saltó la duda: `hidden`
es una propiedad de `HTMLElement`, y ese punto es un `<circle>`. Comprobado en
Chromium: no está en `SVGElement.prototype`, así que la asignación crea una
propiedad suelta en JS y NO toca el atributo — el punto nace oculto y se queda
oculto PARA SIEMPRE, sin un solo error en consola. El CSS sí funcionaba y por
eso engañaba: `[hidden]` es un selector de ATRIBUTO y le da igual el
namespace. Lo mismo pasaba en `pill.html` desde la etapa 3b: **el punto del
relevo de la pastilla no se ha enseñado nunca**. Los dos van ya con
`toggleAttribute`. Lección: una propiedad que no existe no avisa, solo no hace
nada; cuando el mismo patrón se copia a otro tipo de elemento, hay que
comprobar que el patrón siga siendo válido ahí.

**Lo que queda peor y hay que saberlo:** la ventana es 48 px más alta, y esos
48 px son transparentes pero SÍ atrapan el clic (una ventana es un rectángulo;
no hay hit-testing por píxel). Sobre el escritorio no molesta; encima de otra
ventana, es un poco más de superficie muerta.

## 2026-08-11 (segunda) — la bombilla, en su sitio: pequeña, suelta y con su propia ficha

Oscar probó la primera versión y volvió con capturas y ajustes. Todos apuntaban
al mismo sitio: la bombilla se había comido el widget en vez de sumarse a él.

Lo que pidió y quedó: bombilla PEQUEÑA (34x44 en vez de 76x96), SIN globo de
pensamiento, animada en cada estado, en el eje de la cápsula y con poco aire
entre las tres piezas; la cápsula vuelve a su alineación de siempre —posada
sobre la cabeza y ladeada 15.5°— y solo SUBE cuando hay bombilla que alojar
(`body.hasidea`), bajando sola cuando no hay sesión; la información de contexto
sale del globo de resumen y pasa a una ficha propia al pasar el mouse por la
bombilla.

**Al quitar el globo, la ventana volvió a caber en sí misma.** Los 48 px de
franja que había ganado esta mañana se devolvieron: `CAT_TOP_H` desaparece, los
dos globos recuperan sus solapes de siempre y se acaba la zona muerta
transparente que se tragaba clics. Queda `CAT_GEOM_V1_TOP` con un único
cometido: DESHACER el desplazamiento en las configuraciones que alcanzaron a
guardar la versión 1 (migración `geom` 1 → 2). Una corrección de posición no se
puede "revertir con el código": el archivo del usuario ya cambió.

`.stage` se queda aunque hoy mida lo mismo que la ventana. Sale gratis, ancla al
gato abajo y, si algún día vuelve a crecer por arriba, ninguna de las
calibraciones en porcentajes cambia de significado. Ese fue el trabajo de la
mañana; conservarlo cuesta una línea.

**Separar cuota de contexto era lo correcto y no solo una preferencia.** El
globo del resumen habla del PLAN (sesión, semanal, buckets por modelo) y la
presión de la SESIÓN que tienes abierta; mezcladas, había que desplegar el
resumen entero para mirar un número. La ficha nueva vive DENTRO de la ventana
del gatito, no en una ventana Tauri: cada WebView2 arranca en ~57 MB y esto es
una etiqueta de dos renglones.

**El bug que solo se ve renderizando.** La clase de estado del `<body>` se
llamaba igual que la clase del elemento: `ptip`. Como la regla del elemento es
`.ptip{display:none}`, el selector casaba TAMBIÉN con el `<body>` — y al pasar
el mouse por la bombilla el widget ENTERO desaparecía. No hay error en consola,
no hay nada raro en el diff: solo una ventana que se apaga. Salió a la primera
captura del estado de hover, y de ahí la regla: **el nombre de una clase de
ESTADO en el body nunca puede coincidir con el de una clase de ELEMENTO**
(ahora `body.showtip` frente a `.ptip`).

De paso, dos trampas del banco de pruebas que conviene no repetir: en
`chromium --headless`, `--window-size` va en píxeles de DISPOSITIVO (con
`--force-device-scale-factor=2` el viewport CSS es la mitad, y las columnas de
más se quedan fuera del recorte pareciendo vacías), y con factores altos este
chromium de snap devuelve la captura en blanco — se amplía con `zoom` en CSS.
Y el banco necesita un ancestro `position:relative`: en vivo ese papel lo hace
la ventana, y sin él todo lo absoluto se va al fondo del viewport.

El simulador de la bombilla ya no viaja dentro del guion del gatito: tiene botón
propio (💡 Simular contexto, solo dev) porque prueban cosas distintas —aquel
recorre estados de ánimo y avisos, este los cuatro dibujos y el salto de la
cápsula—, y empuja con `emitPill`, así que cualquier refresco durante la prueba
sigue enseñando el nivel simulado.

**Calibración final de la columna (misma tarde, con capturas de Oscar).** Tres
números y el porqué, para que nadie los "arregle" luego: la bombilla va en el
eje de la CABEZA (`left:68%`), no en el de la cápsula (72.4%), y casi posada en
ella (`top:47px`, ~3 px de aire); la cápsula con bombilla baja al 20%. Repartido
por el hueco, el trío se leía como tres cosas sueltas; junto y pegado al gato se
lee como algo SUYO. Comprobado contra los tres estados que podían chocar: las
llamas del `fire` quedan al otro lado, las Z del `zzz` libres —moverla a la
izquierda ayudó— y la ficha del hover no la toca. Único roce: en `zzz` la
bombilla se posa sobre la punta del gorro de dormir; con su trazo y su sombra se
lee como encima, y se deja así antes que meter una excepción por estado
(además apenas puede darse: el gato duerme por el semanal agotado y la bombilla
exige sesión tocada hace <10 min, así que solo coinciden en esa cola).
Rematado con dos detalles de Oscar: la bombilla lleva la MISMA inclinación que
la cápsula (15.5°) —así se leen como piezas del mismo juego y no como un icono
pegado— y los tipos de la ficha son los del globo de modelos (12.5/11.5/10):
el mismo dato no puede leerse más chico en una superficie que en otra.

## 2026-08-11 (noche) — nace el análisis local: la insignia inteligente de /clear vs /compact

Primera pieza de IA local en MichiClaude, tras la investigación de Oscar en
`docs/modelos-locales-cpu.md` (Qwen3.5-2B + llama.cpp medidos en su i7 sin
GPU) y una conversación larga sobre dónde SÍ aporta un modelo chico y dónde
no. Diseño completo en `docs/analisis-local.md`; aquí el porqué de lo
construido y las trampas.

**El caso elegido** (de Oscar, con su captura de la tarjeta genérica): cuando
la tarjeta de intención sale sin insignia —veredicto `unsure`, ni TodoWrite ni
commit limpio que decidan—, hoy la pregunta clave ("¿lo que sigue necesita lo
ya hablado?") se le devuelve al usuario. El modelo local la contesta leyendo
el ai-title y los últimos 3 mensajes humanos, y pinta una insignia PUNTEADA
("Análisis local · tema nuevo") distinta a propósito del "Recomendado" sólido:
una inferencia no puede vestirse de hecho.

**Las decisiones que ordenaron todo:**

1. **El modelo consume hechos, jamás vive en el motor.** El exportador es
   stdlib puro y así se queda (invariante #1): la evidencia viaja en el hit
   `press` (campos aditivos `title`+`msgs`) por el mismo SSH de siempre, y el
   análisis corre SOLO en la máquina del panel. Las sesiones del VPS se
   analizan igual sin que el VPS sepa que existe un modelo.
2. **`user_turn_text` es el único filtro.** La evidencia necesita el TEXTO de
   los turnos humanos y `is_user_turn` solo contaba: se refactorizó para que
   el texto sea la fuente y el bool la envuelva — en Rust y en Python. Dos
   implementaciones del mismo filtro habrían divergido tarde o temprano.
3. **HTTP a mano sobre TcpStream.** reqwest está sin la feature `blocking` y
   el patrón de la casa es async → spawn_blocking; antes que añadir features
   o deps (invariante #4), un POST HTTP/1.1 contra 127.0.0.1 son 40 líneas de
   std — con des-chunkeo a nivel de BYTES (los tamaños de chunk son bytes;
   por chars se descuadraría con UTF-8).
4. **El truncado de mensajes va por CHARS, no bytes** (300): un corte por
   bytes parte un carácter UTF-8 por la mitad y revienta el JSON del hit.
   `chars().take(300)` en Rust ≙ `[:300]` en Python — la réplica coincide.
5. **llama-server nace y muere en cada análisis** (guard con Drop que cubre
   todos los `?`): la app pesa 276 MB y un residente de 2 GB mata el pitch.
   Flags directos de la investigación: -ngl 0, --no-mmap, sin razonamiento
   (12x), temp 0 y gramática GBNF — el enum se FUERZA, no se pide.
6. **Una invocación por sesión aunque falle** (`aiTried` en la tarjeta):
   reintentar en cada sondeo sería arrancar un servidor de 1.3 GB en bucle
   cada 3 minutos contra un fallo persistente.
7. **La evidencia no se persiste.** `msgs` vive en el hit en memoria; al
   almacén de tarjetas solo entra el veredicto `{rec, reason}`. Y el sesgo
   asimétrico va cosido en DOS capas: el prompt ("when in doubt NEVER answer
   clear") y el render (la insignia de /clear solo con razón `tema_nuevo`).
8. **Los hechos mandan hasta el final**: la insignia se pinta solo si el
   veredicto determinista SIGUE en unsure al momento de pintar — si entre
   tanto apareció un TodoWrite, la inferencia se calla sola.

**Lo que NO hace, por diseño y para siempre:** tocar el automático. El
auto-/clear sigue exigiendo Boundary determinista + relayClear + 3 manuales +
red de /export; un "tema_nuevo" del modelo no abre ni una compuerta.

**v1 sin embeddings a propósito:** la escalera completa (hechos → embeddings
→ 2B) está en el diseño, pero lo que decide si esto sirve es la CALIDAD del
veredicto del 2B, y Oscar ya tiene llama-server y el GGUF instalados — cero
descargas para empezar a probar hoy. Los embeddings son un atajo de velocidad
y llegan en la etapa 2 si el veredicto demuestra valer.

**Cómo se prueba sin esperar una sesión al 80%:** Ajustes → Análisis local
(IA) → encender, ruta del .gguf → **Probar**: es la MISMA tubería real
(`ai_intent`) con evidencia de ejemplo que cambia claramente de tema — lo
esperado es `clear · tema nuevo` en segundos (arranque frío ~10-20 s).

Pendiente de `cargo check` en el Windows de Oscar (aquí no hay toolchain);
el JS y el Python pasaron sus verificadores. 18 claves i18n nuevas ×8
idiomas.

## 2026-08-11 (cierre) — descarga guiada: el análisis local sin escribir rutas

Oscar probó la pantalla nueva y vio lo que vería un usuario nuevo: dos cajas
de ruta vacías y un error. Pregunta suya: "¿hay manera de que lo haga en
automático cuando active?". La hay, y era la mitad de la etapa 2 que valía la
pena adelantar (la otra mitad, los embeddings, siguen esperando su turno).

Al encender el interruptor, si falta algo aparece **Descargar todo
(~1.4 GB)** — o "(~17 MB)" si solo falta llama.cpp — con progreso en vivo y
una nota que dice de dónde viene cada cosa. `ai_setup` baja el zip del
release de GitHub y el GGUF de Hugging Face, verifica las huellas SHA-256,
descomprime, rellena la config y enciende. Las cajas de ruta quedan como
ajuste avanzado: quien ya tiene los archivos (Oscar) no ve el botón.

Decisiones y por qué:

- **URLs y huellas en CUATRO CONSTANTES del binario** (b10362 de llama.cpp y
  el GGUF exacto de la investigación, con sus SHA-256 consultadas de las
  fuentes al implementar). Es la regla del updater: nada de esto puede salir
  jamás de un archivo descargado. Al actualizar el pin, las cuatro juntas.
- **Verificación con `Get-FileHash` y descompresión con `Expand-Archive`**:
  PowerShell del sistema antes que un crate de sha256 o de zip (invariante
  #4, la misma decisión que la etapa 2 de remediación). Si la huella no
  casa, el archivo SE BORRA — medio archivo corrupto no puede quedarse
  esperando a que alguien confíe en él.
- **`llama-server.exe` se BUSCA dentro de lo descomprimido** (`find_ls`): el
  zip de llama.cpp ha cambiado de forma entre builds y suponer la ruta es
  apostar a que no vuelva a cambiar.
- **Sin resume**: media descarga se rehace entera. La verificación es por
  huella del archivo completo; reanudar añadiría estados a medias por
  ahorrar minutos de una operación que se hace UNA vez.
- **Idempotente**: el botón baja solo lo que falte, así que "reintentar"
  tras un fallo es el mismo clic.
- **Es la única conexión de la app que no va a api.anthropic.com** — GitHub
  y Hugging Face, una vez, opt-in y anunciada en la propia interfaz. Quedó
  escrito en CLAUDE.md porque toca el matiz del invariante #3.

Pendiente igual que la v1: `cargo check` y la prueba en vivo en el Windows
de Oscar (aquí ni toolchain ni Windows). El camino feliz del usuario nuevo
quedó en: encender → Descargar → esperar la barra → Probar.

**Postdata del mismo día — "no me llega el consejo de /clear".** Oscar abrió
una sesión de prueba aparte, cambió de tema varias veces y no salió nada;
preguntó si se había acabado el límite diario. No: la tarjeta de intención
está EXENTA del tope de 10. Lo que pasaba es que el detonante no es el cambio
de tema sino la PRESIÓN ≥80% del techo de esa sesión — una sesión recién
abierta anda por el 1-2%, y con techo de 1M harían falta ~800k tokens. El
tema solo decide CUÁL de las dos sugerencias sale, una vez que la tarjeta ya
nació. Segunda intuición suya, también descartada: dejarla quieta unos
minutos tampoco la saca — el hit `press` exige sesión tocada hace <10 min
(`PRESS_QUIET_MAX`), así que la quietud la APAGA en vez de encenderla (lo que
sí nace con una sesión quieta es la ficha `cache`, y solo con ≥30k de
contexto). Queda escrito en el diseño porque es la pregunta que cualquiera
se hará la primera vez.

De ahí salió el botón **🎯 Simular intención** (solo dev): crea la tarjeta con
veredicto `unsure` y corre el `ai_intent` DE VERDAD sobre la evidencia de tu
sesión activa más fresca — tus mensajes reales, no un ejemplo, cuando los
hay. Sin él, validar la insignia significaba esperar días a que una sesión
larga cayera además en la zona gris. Detalle de implementación: en modo
simulación las tarjetas se reconstruyen desde `coachHits` en cada render, así
que el veredicto se cuelga del HIT (`_ai`) y no del envoltorio persistido —
si no, se perdía en el primer repintado.

**Primera prueba del 🎯 y el fallo del propio simulador (2026-08-12, madrugada).**
La tarjeta salió perfecta —86%, proyecto y origen reales, las dos opciones—
pero con la insignia RECOMENDADO determinista, no con la del modelo. No era
un fallo del análisis: `siFakeIntent` copiaba el `cont` REAL de la sesión
viva, y en una sesión de trabajo ese Jaccard va alto ("sigues en los mismos
archivos") → `intentVerdict` = **alive** → el render suprime la inferencia,
tal y como manda la regla #2 del diseño (los hechos ganan). O sea: el
mecanismo funcionó exactamente como debía y lo que estaba mal era el banco
de pruebas. Las señales deterministas del hit simulado van ahora NEUTRAS
(topen/ttotal/cont/gclean en cero); la evidencia —título y mensajes— sigue
siendo la real. De paso, el veredicto se escribe también en `flowLog`: el
`simMsg` vive en Ajustes y la tarjeta en Consejos, así que un "unsure"
—que por diseño no pinta insignia— se veía igual que un fallo.

Lección: un simulador que hereda demasiado del estado real puede reproducir
el camino EQUIVOCADO con total fidelidad. Al forzar un escenario hay que
neutralizar justo las variables que lo definen.

## 2026-08-12 — el primer veredicto del modelo local, y el mecanismo equivocado

Segunda prueba del 🎯 (ya con las señales deterministas neutras) y el rastro
del flujo dio la respuesta en una línea: `sim intención: ERR_AI_BADOUT`. El
modelo arrancaba, contestaba, y su salida no se podía leer.

**La causa, verificada en la documentación de llama.cpp:** el parámetro
`grammar` (GBNF) SOLO existe en el endpoint NATIVO `/completion`. En
`/v1/chat/completions` —el que usamos, porque es el que aplica la plantilla
de chat del modelo— se ignora **en silencio**: no da error, simplemente no
restringe nada. Así que el 2B contestaba en prosa libre y
`serde_json::from_str` moría. La vía correcta en ese endpoint es
`response_format` con esquema (`{"type":"json_object","schema":{…}}`), que
llama-server convierte él mismo a gramática: mismo blindaje, endpoint
correcto — los `enum` se cumplen al MUESTREAR, no al validar.

Lección para el archivo: **un parámetro ignorado en silencio es peor que uno
rechazado.** Si la petición hubiera fallado con 400, el diagnóstico habría
sido inmediato; al aceptarla y no aplicarla, el fallo aparece tres capas más
abajo, disfrazado de "el modelo no sabe responder". Antes de dar por bueno un
mecanismo de restricción, hay que comprobar que el ENDPOINT concreto lo
implementa.

De ahí salió también **`ai_debug.txt`** (carpeta de datos, se sobrescribe):
petición y respuesta CRUDA del último intento. Es la misma familia que
`quota_debug.json`, `wrap_debug.txt` y `rem_debug.json`, y la misma lección
que dejó el chat de VS Code el 2026-08-10: *un enganche invisible necesita
rastro desde el primer día*. Aquí se saltó ese paso al construir y costó una
ronda entera de adivinar.

De paso, el parseo se volvió tolerante: mira `reasoning_content` si `content`
viene vacío, recorta al primer `{…}` por si el modelo pone algo delante, y
detecta el campo `error` del servidor en vez de tratarlo como salida ilegible.

**Lo que la prueba SÍ validó** (todo lo demás de la cadena funciona): la
tarjeta nace con datos reales (86%, proyecto y origen correctos), el
clasificador determinista manda —en la primera prueba dio `alive` por el
`cont` real y suprimió la inferencia, exactamente como está diseñado—, los
botones de copiar comando funcionan, el simulador no ensucia el almacén real
("Ahora no" filtra `coachHits`, no localStorage) y el modelo carga y responde
en el tiempo esperado.

**Segunda autopsia, mismo día — el modelo sí contestaba, pero pensando.**
Con el `ai_debug.txt` ya escribiendo, el segundo `ERR_AI_BADOUT` se resolvió
en una lectura:

```
"finish_reason":"length", "content":"",
"reasoning_content":"Thinking Process:\n\n1. **Analyze the Request:**..."
```

Qwen3.5 **razona por defecto**. El `--reasoning-budget 0` que le pasamos a
llama-server es solo un DEFAULT del servidor y la plantilla de chat lo pisa,
así que el modelo gastó sus 40 tokens redactando un "Thinking Process:" y
dejó `content` vacío. Y un detalle que conviene recordar: la gramática del
`response_format` restringe SOLO el canal `content` — lo que el modelo
escriba razonando no pasa por ella, así que no hay blindaje que valga si el
razonamiento está encendido.

Lo humillante y lo útil: **la solución llevaba escrita desde el principio en
`modelos-locales-cpu.md` §3**, en la sección de configuración del cliente —
`{"chat_template_kwargs": {"enable_thinking": false}}` y, como alternativa "a
prueba de balas", `/no_think` al final del mensaje. Yo leí ese documento
entero para diseñar esto y aun así implementé solo la mitad de la receta: la
del servidor. Ahora van las dos, cinturón y tirantes.

Regla que queda: **cuando un documento de investigación dice "el servidor
solo pone un default y el cliente lo pisa", eso es una instrucción para el
CLIENTE, no una curiosidad.**

Datos buenos del mismo volcado: el prefill fue de 208 tokens a 60.8 tok/s
(3.4 s) y la generación a 13.9 tok/s — o sea que el análisis completo saldrá
en ~6-8 s con el servidor ya caliente, dentro de lo prometido. Y confirmó que
corre el GGUF descargado por la app y el build `b10362` que pineamos.

**FUNCIONA (2026-08-12, 00:39).** `sim intención: clear · tema_nuevo`, con la
insignia punteada "Análisis local · tema nuevo" sobre la opción `/clear` y
claramente distinta del "RECOMENDADO" sólido del clasificador determinista.
La evidencia era la de ejemplo —"commit y push de la bombilla" seguido de
"planeemos las capturas del README"— y el veredicto es el correcto: tema
nuevo, no necesita lo anterior.

Cadena validada de punta a punta: motor (Rust + exportador) → evidencia en el
hit `press` → llama-server bajo demanda → esquema que fuerza el enum →
insignia que dice de dónde viene. Tres autopsias hicieron falta y ninguna fue
del diseño: binario viejo (empate de mtime), `grammar` ignorado en el
endpoint de chat, y el razonamiento encendido por defecto.

**Lo que queda para cerrar la v1** (validación pasiva, con el uso):
1. Ver la insignia en una tarjeta REAL —sesión al 80% con veredicto
   `unsure`—, no simulada.
2. Anotar unos días si ACIERTA. Ese es el dato que decide la etapa 2
   (embeddings como peldaño previo) o si hay que afinar el prompt.
3. Cuando el repo sea público: el espejo de modelos en GitHub Releases.

## 2026-08-12 (tarde) — el automático por INFERENCIA: el modelo puede disparar el /clear

Oscar lo pidió con todas las letras: que el `/clear` se aplique solo cuando lo
recomiende el modelo, con las reglas y la red que ya existen, para probarlo
unos días. Eso cruza la que yo había escrito como **regla #1** del análisis
local ("el modelo jamás sustituye una compuerta"), así que lo primero fue
decírselo y lo segundo diseñarlo de forma que la red aguante. Su decisión,
implementada — y la regla #1 REESCRITA en el diseño en vez de dejarla
mintiendo.

**La forma: camino PARALELO, no sustitución.** El auto-/clear tiene ahora DOS
razones válidas, cada una con su interruptor:

| | (a) HECHO | (b) INFERENCIA |
|---|---|---|
| Dispara | `Boundary` (lista al 100% o commit limpio) | `unsure` + modelo dice `clear`/`tema_nuevo` |
| Interruptor | `relayClear` | `relayClearAi` (cuelga del anterior, nace OFF) |
| Cuenta atrás | 15 s | **30 s** |

Todo lo demás se exige IGUAL: interruptor maestro, 3 manuales de `/clear`
ganadas a mano, relevo v≥2, widget A LA VISTA, una vez por sesión sellada
antes de empezar, cualquier toque la para, R1-R4 al escribir, y la **copia
`/export` verificada en disco o no hay `/clear`**. Esa red es lo que hace la
apuesta defendible: un `/clear` por inferencia equivocada cuesta una copia que
sigue en `<datos>/handoff/`, no la conversación.

**Dos exigencias extra que solo tiene el camino (b):**

1. **`topen === 0`.** El veredicto `unsure` ya lo implica (con tareas abiertas
   sería `alive`), pero se comprueba OTRA VEZ a propósito. Defensa en
   profundidad: el día que alguien toque `intentVerdict`, esta puerta sigue
   cerrada. Un hecho no se sobreescribe con una opinión.
2. **`reason === "tema_nuevo"`.** El sesgo asimétrico llevado al automático:
   `tema_cruzado`, `tarea_viva` y `cierre` NO borran, caen al `/compact`.

**El detalle que decidía si esto servía de algo: el automático tiene que
ESPERAR el veredicto.** Al llegar al 80% con `unsure`, el automático de
siempre aplicaría `/compact` en el PRIMER sondeo — antes de que el modelo
alcance a hablar — y el camino nuevo nunca se usaría. Ahora, con (b) armado y
el análisis en marcha, el sondeo se abstiene y espera al siguiente
(`aiPending`). Acotado: 10 min desde que nació la tarjeta, y un fallo del
análisis marca `aiErr` para dejar de esperar de inmediato. La presión solo
sube, así que esperar nunca empeora nada. Sin esta pieza el resto era decorado.

**La cuenta atrás es el doble (30 s)** y con eso queda completa una escalera
que el proyecto ya venía usando sin nombrarla: **5 s cuando lo pides tú, 15
cuando lo decide un hecho medido, 30 cuando lo decide una inferencia.** Cuanto
más blanda la razón, más tiempo para pararla.

**Cómo se audita la prueba:** el rastro del flujo distingue quién decidió —
`relevo auto: aplicado /clear por IA (tema_nuevo)` frente a `… por hecho`. Ese
es EL dato de estos días. Y si aparece un `por IA` donde no debía, la copia
está a un clic desde el registro de acciones.

**Orden de retirada si sale mal** (escrito ANTES de probar, que es cuando se
piensa con la cabeza fría): apagar `relayClearAi` y el resto del automático
sigue como estaba → si el problema es el veredicto, afinar el prompt → si es
sistemático, volver a la v1 (solo insignia). El camino (a) nunca depende del
modelo.

**Nota de mantenimiento:** CLAUDE.md quedó otra vez pegado al tope de 40k
(39.982). Es la tercera vez en el día que meter una regla nueva obliga a
recortar prosa de otras. Cuando vuelva a apretar, lo sano es mover el bloque
de REMEDIACIÓN —7.5 k, y su propio doc ya dice tenerlo todo— y dejar aquí solo
el puntero.

**La cuenta atrás no decía QUÉ iba a aplicar (encontrado 2026-08-12 al
escribir los ejemplos).** Oscar pidió los casos del `/clear` explicados con
ejemplos del día y "qué voy a ver en cada uno". Al ir a describir lo que se ve
—en vez de asumirlo— salió el hueco: el widget pintaba SOLO el segundero, así
que la cuenta de un `/compact` y la de un `/clear` eran idénticas en pantalla.
Una resume y la otra BORRA, y con dos razones posibles (hecho o inferencia) la
ambigüedad crecía justo el día que se enciende el camino nuevo.

Lo más llamativo: el texto completo —"Michi va a aplicar /clear en 30 s, toca
para parar"— ya viajaba en el evento `relay:auto` desde la etapa 3c-2. Estaba
construido y **nadie lo pintaba**. Se emitía, se traducía a 8 idiomas y se
tiraba.

Arreglado: el chip lleva el comando y el color habla — ÁMBAR `/compact 15`,
ROJO `/clear 30`. En la pastilla cabe entero; en el gatito no caben las dos
cosas, así que mientras la cuenta corre el "Sesión X%" se aparta (`body.autorun`)
y la cápsula queda dedicada a lo único que importa esos segundos. Verificado
renderizando los tres estados con el marcado idéntico al de producción —la
primera captura mentía porque al banco le faltaba el `id` del `%` y no aplicaba
la regla que lo esconde—.

Regla que queda, hermana de la del veredicto ✓/✕: **una cuenta atrás que no
dice qué va a hacer deja al usuario adivinando igual que una que acaba en
silencio.** Y la lección de proceso: escribir la documentación de cara al
usuario ("qué vas a ver") encuentra huecos que revisar el código no encuentra,
porque obliga a mirar la pantalla y no la lógica.

**La mudanza anunciada (2026-08-12, tarde).** CLAUDE.md tocó su tope por
tercera vez en el día al apuntar el pendiente de la ficha proporcional, y se
ejecutó lo que la nota de ayer dejaba dicho: el bloque de REMEDIACIÓN (7,3k)
se mudó ÍNTEGRO a `remediacion.md` §"REGLAS VIGENTES — resumen operativo", y
en CLAUDE.md queda un puntero de ~15 líneas con solo lo transversal (crate
aparte, lista blanca, la red del /export, las dos razones del auto-/clear, la
cuenta atrás y el invariante del workflow). De 40.248 a 33.942: seis mil
bytes de margen para dejar de pellizcar palabras en cada regla nueva.
Verificado byte a byte que el bloque llegó entero antes de borrarlo del
origen.

## 2026-08-12 (tarde) — auditoría pre-público: el repo está listo, quedan dos decisiones

Oscar puso en palabras el freno real del lanzamiento: el miedo a que un
usuario descargue algo roto y no pueda actualizarlo. El antídoto es probar el
updater, y para eso el repo tiene que ser público — así que se auditó TODO lo
que se publicaría, historial incluido (lo que está en el historial se publica
con el repo y ya no se puede retirar después sin reescribirlo).

**Limpio, verificado:**
- gitleaks sobre los 397 commits: cero fugas. Barrido manual extra de
  patrones (ghp_, sk-ant, AKIA, llaves SSH, correos personales, IPs
  públicas) sobre TODO el historial: nada.
- Las notas de negocio del analizador: solo se referencia su RUTA externa
  (`~/.michiclaude/`), el contenido jamás entró al repo — el diseño funcionó.
- Archivos borrados en el historial: arte viejo y un .pyc; inofensivos.
- Workflow de release: los secretos van por `secrets.*` de GitHub, nada
  incrustado. Updater: pubkey (pública por diseño), endpoint y RELEASES_URL
  correctos y constantes.
- README: usuarios ficticios e IPs de documentación (TEST-NET). Un solo tag
  (`pre-rediseno-20260805`), sin ramas sueltas ni stashes.

**Las dos decisiones que solo puede tomar Oscar, ANTES de abrir:**
1. **Los correos de autor de los commits** (396 con una dirección personal,
   9 con otra) se publican con el repo. Opciones: aceptarlo (normal en open
   source) o reescribir el historial AHORA al correo noreply de GitHub —
   gratis mientras el repo es privado y sin colaboradores; imposible de
   deshacer limpiamente después.
2. **`docs/modelos-locales-cpu.md`** trae contexto de OTRO negocio (despliegue
   en equipos de clientes, pipeline de destilación). Ya está en el historial:
   quitarlo de verdad = la misma reescritura. Opciones: publicarlo (es
   investigación honesta y da credibilidad) o sacarlo en la misma pasada que
   el punto 1.

La bitácora misma se publica y se queda: es la transparencia que el producto
vende. Si Oscar decide reescribir (1 y/o 2), es una sola operación con
`git-filter-repo` + force push; después, repo público → tag pre-release →
probar el updater de punta a punta.

## 2026-08-12 (tarde) — limpieza pre-público: el historial se reescribe UNA vez

Decisión de Oscar sobre las dos preguntas de la auditoría: las dos cosas
fuera, y en general nada personal en el repo. Como lo que está en el
historial se publica con el repo, la única forma real es reescribirlo — y el
momento es AHORA, con el repo privado y sin colaboradores: gratis hoy,
imposible de hacer limpio mañana.

Lo que cambia (con respaldo `.bundle` completo en `~/.michiclaude` antes de
tocar nada):

1. **Correos de autor → noreply de GitHub** (los dos personales, 405 commits)
   y nombre normalizado a "Oscar". Los commits futuros nacen ya con el
   noreply (config del repo en las dos máquinas).
2. **`docs/modelos-locales-cpu.md` fuera de TODO el historial**: trae
   contexto de otro proyecto (despliegue en equipos de clientes,
   destilación). Se muda ÍNTEGRO a `~/.michiclaude/`, junto a las notas de
   negocio — mismo patrón: el conocimiento se usa, el contexto no se
   publica. Las referencias del código y los docs apuntan ahora "a la
   investigación de modelos (fuera del repo)"; las menciones narrativas de
   la bitácora se quedan (cuentan QUÉ se aprendió, no exponen el contexto).
3. **El username viejo de GitHub** (contenía el prefijo del correo personal)
   sustituido por el actual en los contenidos históricos de CLAUDE.md y en
   un mensaje de commit.

Nota para el futuro: el clon de Windows queda desincronizado por la
reescritura — `git fetch origin && git reset --hard origin/main` (y
`git fetch --tags --force`), NUNCA `git pull`, que intentaría fusionar las
dos historias.

**Ejecutado y verificado (misma tarde).** 408 → 407 commits (el que solo
añadía el doc se podó solo). Verificación completa post-reescritura: UNA sola
identidad de autor (el noreply con ID), el doc fuera de TODO el historial,
cero rastros de los correos/username viejo en contenidos y mensajes, gitleaks
limpio sobre la historia nueva, y el tag `pre-rediseno-20260805` reescrito y
re-empujado. El force push llegó al remoto (verificado con ls-remote: main =
hash nuevo). El token de GitHub salió del config del repo (filter-repo había
tirado el remoto original): ahora un credential.helper lo lee de
`~/.secrets/github-token` al momento del push, y los commits futuros nacen
con el noreply en este clon — falta la MISMA config en el clon de Windows.
Matiz honesto: GitHub conserva un tiempo los objetos viejos inalcanzables en
su servidor; como el repo jamás fue público y nadie más tiene los hashes, el
riesgo práctico es cero, y al hacerse público solo se clona lo alcanzable.

## 2026-08-12 (noche) — REPO PÚBLICO, y el release #1 muere por los iconos

Oscar lo hizo público. Primer tag `v0.1.0` → primera ejecución REAL del
workflow de release (escrito hace semanas, jamás corrido) → rojo a los 7m51s:
`icons/icon.ico not found`. La causa: `.gitignore` ignoraba `src-tauri/icons/`
entero — en el Windows de Oscar los iconos existen porque los generó una vez
con `npm run icons`, pero el runner parte de un clon limpio. El clásico
"funciona en mi máquina" en su forma más pura, y nunca se pudo ver antes
porque el workflow nunca había corrido.

Arreglo: los iconos generados van COMMITEADOS, como en la plantilla oficial
de Tauri (son artefactos deterministas de app-icon.png y el build los
necesita); fuera del repo quedan solo las variantes móviles (android/ios).
Regenerar: `npm run icons` si algún día cambia app-icon.png.

Lección: un workflow que nunca ha corrido es una promesa, no una pieza.
La primera ejecución ES parte de la validación — por eso el updater se
prueba completo ANTES de que exista un solo usuario.

**Release #2 verde… a medias (misma noche).** El instalador se publicó, pero
sin `.sig` ni `latest.json`: el endpoint del updater daba 404 — o sea, app
instalable pero incapaz de enterarse de versiones nuevas, que era EL punto de
toda la prueba. Causa: faltaba `"createUpdaterArtifacts": true` en el
`bundle` de tauri.conf.json — sin ella Tauri v2 no firma los artefactos y el
workflow no tiene con qué armar el latest.json (su includeUpdaterJson por
defecto no encuentra nada que incluir). Segunda pieza del updater que solo se
podía descubrir EJECUTANDO: el workflow nunca había corrido y la config nunca
había empaquetado un updater de verdad.

**Release #3: VERDE Y COMPLETO (2026-08-12, 18:45).** Tercera ejecución, la
buena: instalador + firma + latest.json publicados, y el endpoint del updater
responde el JSON firmado (verificado desde fuera con curl, el mismo camino
que recorrerá cada instalación). El primer release público de MichiClaude
existe: v0.1.0. Dos fallos quemados por el camino —iconos ignorados y
createUpdaterArtifacts ausente— que solo la ejecución real podía enseñar.
Queda el cierre del círculo: instalar el exe de Releases, publicar v0.1.1 y
ver a la app actualizarse sola.

## 2026-08-12 (noche) — EL UPDATER FUNCIONA: el círculo completo en una tarde

La tarde empezó con Oscar confesando el freno real del lanzamiento: "no
quiero que un usuario descargue algo roto y no pueda actualizarlo". Terminó
con la v0.1.0 instalada desde Releases detectando, descargando, verificando
la firma e instalándose la v0.1.1 sola, con la configuración intacta. El
miedo ya no tiene objeto: el canal de reparación existe y está probado.

Cuatro mordidas en el camino, ninguna evitable sin ejecutar:

1. **Release #1, rojo:** `icons/icon.ico not found` — los iconos estaban
   gitignorados; en la máquina de desarrollo existen, el runner parte de un
   clon limpio. Van commiteados, como en la plantilla oficial de Tauri.
2. **Release #2, verde a medias:** instalador sin `.sig` ni `latest.json` —
   faltaba `createUpdaterArtifacts: true` en el bundle. App instalable pero
   sorda a versiones nuevas, que era EL punto.
3. **Release #3 (v0.1.1), verde con versión vieja:** el tag se creó sin
   `git pull` previo y apuntó al commit anterior al bump. La red funcionó
   sola: latest.json anunciaba 0.1.0 y ninguna app se habría "actualizado"
   a lo mismo. Se borró release+tag y se re-etiquetó desde el VPS.
4. **La franja nunca llegó sola:** el check automático corría UNA vez, 8 s
   tras arrancar — y la app llevaba abierta desde antes del release. Para
   una app de bandeja que vive semanas sin reiniciar, eso es no enterarse
   nunca. Ahora: al arrancar Y cada 12 h, con guarda `v===updVer` para no
   re-anunciar lo ya anunciado (la REGLA ÚNICA de los globos: cerrado no
   vuelve).

Lección de la jornada, la misma cuatro veces: **cada pieza de la tubería
que nunca había corrido escondía exactamente un fallo, y ninguno era
visible leyendo el código.** El workflow, el empaquetado del updater, el
proceso humano de etiquetar y la cadencia del check — los cuatro se
estrenaron hoy y los cuatro mordieron una vez. Por eso se prueba con cero
usuarios.

Estado: MichiClaude es público, con dos releases reales y un canal de
actualización validado. Lo que queda para el LANZAMIENTO (cuando Oscar
quiera ser encontrado): capturas del README, el espejo de modelos en un
release, y la apuesta #2 (tarjeta compartible + gamificación) como pieza
de crecimiento.

## 2026-08-12 (tarde-2) — El espejo de modelos: el análisis local ya no depende de servidores ajenos

Idea de Oscar del 2026-08-11 ("¿y si Hugging Face quita la URL o el
modelo deja de existir? ¿no es mejor dejarlo en mi GitHub cuando sea
público?"), ejecutada el mismo día que el repo se abrió — era el único
bloqueador.

**Qué se hizo:** release `modelos-v1` en el propio repo con copias byte a
byte del GGUF (Qwen3.5-2B, 1.8 GB) y el zip de llama.cpp (b10362). En el
código, dos constantes nuevas (`AI_LS_URL_MIRROR` / `AI_MODEL_URL_MIRROR`)
y `ai_fetch()`: intenta la fuente original, y si falla la RED **o la
HUELLA**, cae al espejo. El fallo de huella importa tanto como el de red:
que Hugging Face responda 200 con OTRO archivo (lo reemplazaron) es
exactamente el escenario que el espejo cubre. La misma SHA-256 valida
ambas fuentes — la autoridad es la huella, no el servidor.

**Dos detalles que evitaron romper lo de la mañana:**

1. El release va como **PRERELEASE**: `releases/latest` (el endpoint del
   updater validado horas antes) ignora prereleases. Sin esa marca,
   `modelos-v1` habría tapado a la v0.1.1 y el updater se habría quedado
   ciego. Verificado tras subir: `latest.json` sigue anunciando 0.1.1.
2. El tag NO empieza con `v` → el workflow de release (`tags: v*`) no se
   dispara: no compila nada, no publica instaladores fantasma.

**Verificación en vivo, círculo completo:** bajados los originales al VPS
→ huellas idénticas a las constantes → subidos con `gh release upload` →
descargado el zip DE VUELTA del espejo sin ninguna autenticación → huella
idéntica otra vez. El camino que recorrería la app de un usuario nuevo con
Hugging Face caído está probado de punta a punta, salvo el salto mismo
(imposible de probar sin tumbar la fuente original; el código del salto
son 10 líneas de bucle sobre las mismas dos funciones ya validadas).

**Regla para el futuro:** cambio de build o de modelo = actualizar las
SEIS constantes juntas Y subir las copias a un release `modelos-v2` —
no se reutiliza el viejo, misma regla que el updater: un binario ya
publicado no se reemplaza.

**Cerrado el 2026-08-13:** `cargo check` limpio en el Windows de Oscar
(`Compiling michiclaude` presente — no fue el empate de mtime —, sin
warnings). Antes se auditó desde el VPS lo que no necesita compilador: las
seis constantes emparejadas, `ai_fetch` con un único llamador de
`ai_download` y tipos que cuadran, y ningún uso huérfano de la firma
vieja. Comprobadas además las cuatro URLs en vivo (espejo y originales,
200 las cuatro) y que `releases/latest` sigue anunciando la v0.1.1 con su
`.sig` — el prerelease `modelos-v1` no tapó al updater, que era el riesgo
real de haber subido un release el mismo día.

## 2026-08-13 — la ficha `compact` deja de avisar al 12%: el umbral se hace proporcional

El último bug conocido vivo, y era un déjà vu: la ficha `compact` del
coach (y, se descubrió al hacerlo, también el ⚠ "ctx" del recibo de
cierre en `coach_leaks()`) disparaba a los 120k FIJOS de `COACH_CTX_HIGH`.
Con un modelo de techo 1M eso es el 12% del contexto — la app gritaba
"¡compacta!" con la sesión recién empezada, y Oscar salta entre modelos a
diario. Exactamente el mismo bug que tuvo el manómetro clavado en 200k
durante meses (§2026-08-08).

**El arreglo:** `COACH_CTX_HIGH` (120k) muere y nace `COACH_CTX_PCT`
(60): el umbral es ahora el 60% de `ctx_full(model, ctx_seen)` — la misma
función, ya validada, que le da el techo al manómetro, con la evidencia
medida de la máquina incluida. Con modelo desconocido `ctx_for()` cae a
200k y el umbral queda en los 120k de siempre: el comportamiento viejo es
el caso degenerado del nuevo. Cuatro sitios, dos por lado (invariante #1):
la ficha y `coach_leaks` en `lib.rs`, y sus réplicas en `meter-export.py`.
Sin `coach_leaks` el recibo habría contado otra historia que la ficha
(fuga a 120k en una sesión que la ficha consideraba holgada con techo 1M).

**Por qué se pudo hacer sin esperar el cierre de la prueba en vivo:** el
pendiente decía "al cerrar la prueba" por prudencia, pero se verificó en
el código que el `/compact` AUTOMÁTICO va por otro camino —
`relayAutoCheck` dispara con `pressPct ≥ INTENT_PCT` (80% del techo, del
hit `press`) — y no lee la ficha ni `COACH_CTX_HIGH`. Cambiar la ficha no
mueve nada de lo que Oscar está midiendo.

**Verificado:** `py_compile` limpio; grep sin referencias huérfanas a la
constante vieja; `st`/`CoachSess` llevan `model` y `ctx_seen` en los
cuatro sitios (el hit `press` vecino ya los usaba). Docs en sincronía:
consejos-coach.md (dos menciones), CLAUDE.md (regla + pendiente cerrado).
Pendiente de Windows: `cargo check` (el VPS sigue sin toolchain).
Cerrado el mismo día: `cargo check` limpio en el Windows de Oscar
(`Checking michiclaude` en 8.68 s tras el pull de 1a3fb8a y el toque de
mtime — trabajo real, no el empate). El arreglo queda VERIFICADO en los
dos lados; el VPS recibirá el exportador nuevo cuando Oscar recompile y
arranque la app (viaja embebido).

## 2026-08-13 — la rampa invisible: compás adaptativo del coach y candado antes de la cuenta

**La prueba en vivo que lo destapó.** Primer intento del auto-/clear por
HECHO: sesión Haiku en el VPS llenada con lecturas masivas (lib.rs +
meter-export.py ≈ 197k tok). El manómetro nunca pasó de ~30% en pantalla
y Claude Code auto-compactó al ~94% ("Compacted chat · auto · 197k tokens
freed"). El automático de Michi jamás vio el 80%.

**Autopsia (dos causas, las dos de diseño):**
1. `coachPoll` corría FIJO cada 3 min y el manómetro reporta `last_ctx`
   (el contexto del ÚLTIMO turno, no el pico). La rampa 60k→197k cupo
   entera entre dos sondeos: el panel midió 30%, y al siguiente sondeo la
   compactación ya había puesto `last_ctx=0`. El pico existió pero pasó
   entre dos fotos. No es solo un caso de laboratorio: skills, subagentes
   o un prompt de "lee todo" hacen exactamente esa rampa en uso real.
2. Aunque se hubiera detectado: durante la rampa Claude está GENERANDO,
   el candado (R2) habría rechazado la inyección al final de la cuenta y
   `relayAutoCheck` sellaba el reintento a 10 min (`AUTO_RETRY_MIN`) —
   carrera perdida contra el auto-compact del ~94%.

**Arreglo (solo frontend, cero Rust, invariante #1 intacto):**
- Compás adaptativo `coachSched()`: el sondeo se auto-agenda según lo
  visto — 3 min sin sesión activa (el compás de siempre), 60 s con hit
  `press` vivo, 20 s con presión ≥55 (`COACH_WARM_PCT`), 10 s con ≥70
  (`COACH_HOT_PCT`) O salto de contexto ≥15k tok entre sondeos
  (`COACH_RAMP_TOK`, con prev>0: estrenar sesión no es rampa). El costo
  es acotado: `get_coach` es incremental por offset y el SSH de las
  remotas solo se paga mientras dura la banda alta, que se autolimita
  (o dispara o la sesión se calma). Las transiciones van al `flowLog`
  ("coach: compás 10 s (presión 72%, rampa)"). La cadencia de CUOTA no
  se tocó (3 min, regla del 429 — el coach no habla con la API).
- `relayAutoCheck` ahora exige `rly.ready` ANTES de `autoStamp`: ocupado
  es transitorio, no se sella nada y el siguiente sondeo (rápido bajo
  presión) reintenta gratis. El flujo en rampa queda: sondeo caliente ve
  ≥80% a mitad del turno gigante → espera en silencio → el turno termina
  → relevo `listo` → cuenta atrás → inyección con el candado en verde.
- Sellado del intento y resto de compuertas: SIN CAMBIOS (una vez por
  sesión, widget a la vista, desbloqueo manual, veredictos).

**Además quedó validado de la prueba fallida:** el fallback a `/compact`
con lista abierta funcionó de punta a punta EN REAL (tarjeta de intención
con "3 de 5 sin terminar", ⚠ en la opción /clear, auto-/compact aplicado
y registrado). Y la mordida conocida de siempre: la sesión de la prueba
quedó sellada como "done" — el próximo intento necesita sesión nueva.

**Verificado:** sintaxis del bloque `<script>` completo con `node --check`
limpio (el VPS no corre la app; la prueba funcional queda para el Windows
de Oscar tras el pull). Docs en sincronía: CLAUDE.md (regla del compás y
del candado en §Coach) y el comentario de `rlyPoll` sobre el compás.

## 2026-08-13 (2) — primera corrida REAL del camino por inferencia; la cuenta arranca con el veredicto

**Lo que pasó (prueba en vivo, sesión Haiku en el VPS):** el compás
adaptativo funcionó a la primera — "coach: compás 10 s (presión 80%)" en
el flowLog, tarjeta de intención y post-it casi inmediatos. Claude Code
SE SALTÓ el TodoWrite del guion (leyó y dijo "listo" a secas), así que no
hubo Boundary… y eso destapó el estreno involuntario del camino DIFÍCIL:
veredicto `unsure` → análisis local REAL (primera vez fuera del
simulador: "ai: veredicto clear · tema_nuevo", 13 s) → cuenta de 30 s
para /clear con red /export y la insignia punteada en la tarjeta. Oscar
tocó el gatito durante la cuenta (abrió el panel) y "cancelado (el
usuario)" — la ventana de cancelación TAMBIÉN quedó validada en real.
Pendiente de ver: la inyección completándose (✓ + copia en
`~/.michiclaude/handoff/` del VPS). El análisis local v1 ya tiene su
primer punto de la muestra: acierto razonable (sesión de lecturas sueltas
juzgada tema nuevo).

**Cronometría del hueco que molestó a Oscar:** aviso 18:21:08, veredicto
18:21:21 (13 s de modelo local), cuenta 18:21:33 (hasta 10 s extra
esperando el siguiente sondeo). Ese último tramo era evitable: ahora
`maybeAiIntent`, al guardar el veredicto, llama `relayAutoCheck(pr)` EN
EL ACTO — el pr del último sondeo (≤10 s bajo presión) sigue fresco y la
función re-verifica todas sus compuertas igual (autoRun, sellado, ready,
widget). El hueco restante es el costo del modelo (~13 s en la máquina de
Oscar) y no se puede comprimir sin cambiar de peldaño (embeddings, etapa
2). Por el camino del HECHO (Boundary, sin IA) aviso y cuenta ya nacían
en el mismo sondeo.

**Verificado:** `node --check` limpio del script completo. Nota de
mordida conocida: el intento cancelado sella `relayAuto[sid]` con
timestamp — la MISMA sesión reintenta sola pasados 10 min si vuelve a
estar activa (un mensajito la despierta); no hace falta sesión nueva.

## 2026-08-13 (3) — la carrera del primer sondeo caliente: /compact ganándole al veredicto

**Vista en vivo (18:32, segunda prueba del día):** en el MISMO segundo
nacieron la cuenta de /compact, la tarjeta de intención y el "ai:
analizando"; el veredicto "clear · tema_nuevo" llegó 10 s tarde a una
cuenta ya corriendo y la sesión quedó sellada con /compact — el /clear
por inferencia no se pudo ver. De paso quedaron validados el compás con
rampa ("compás 10 s (presión 85%, rampa)") y el segundo acierto seguido
del análisis local (2 de 2 tema_nuevo en sesiones de lecturas sueltas).

**Autopsia:** `relayAutoCheck(pr)` se llamaba ARRIBA en coachPoll, antes
de que el bucle de tarjetas guardara la de intención y antes de
`maybeAiIntent`. En el PRIMER sondeo que ve presión ≥80, `aiPending`
buscaba la tarjeta en el almacén, no la encontraba (aún no existía),
contestaba "no hay análisis en camino" y la cuenta de /compact arrancaba
sin esperar. En los sondeos siguientes la tarjeta ya existía y la espera
funcionaba — por eso la prueba de las 18:21 no lo destapó (ese primer
sondeo lo frenó la compuerta `ready`; el orden correcto se dio solo).
Con el compás de 3 min la carrera era casi imposible de ver; el compás
de 10 s la volvió reproducible al primer intento.

**Arreglo (tres piezas, solo frontend):**
1. `relayAutoCheck(pr)` se movió DESPUÉS de `saveCoachCards` y
   `maybeAiIntent` en coachPoll: cuando decide, la tarjeta existe y el
   análisis (si toca) ya está lanzado y sellado con `aiTried`.
2. `aiPending` ahora exige `aiTried`: sin lanzamiento real (IA apagada,
   exportador viejo sin `msgs`, veredicto no-unsure) no hay espera — se
   decide con lo determinista al momento, como siempre. Evita el plantón
   de 10 min (`AI_WAIT_MIN`) esperando un veredicto que jamás saldrá.
3. Simétrico en el `catch` del análisis: si el modelo falla, se llama
   `relayAutoCheck(pr)` en el acto (antes solo pasaba en el éxito).

**Verificado:** `node --check` limpio. La sesión 4043897 quedó sellada
("done" por el /compact aplicado): la próxima prueba del /clear necesita
sesión nueva.

## 2026-08-13 (4) — tercera prueba: la tubería entera bien, y un límite nuevo: el chat no tiene /export

**Lo validado (registro 18:45):** la carrera del sondeo caliente quedó
arreglada (tarjeta + "ai: analizando" SIN cuenta atrás), la cuenta
arrancó EN EL MISMO SEGUNDO que el veredicto (18:45:25, el ajuste de
maybeAiIntent), tercer acierto seguido del análisis local (3 de 3
tema_nuevo) y el fail-closed de la red actuó tal como promete.

**El límite descubierto:** el relevo tecleó
`/export <ruta handoff>` y el chat de VS Code contestó "/export isn't
available in this environment". El comando /export existe en la TUI de
la terminal pero NO en el entorno del chat de la extensión — la copia no
puede nacer, la verificación no la encuentra y el /clear se niega
(ERR_RELAY_EXPORT, no se borró nada: el fail-closed es la estrella de la
prueba). La validación anterior del /clear con red fue por terminal; en
CHAT el auto-/clear hoy NO puede completarse.

**Camino de fondo (diseño pendiente, zona de reglas duras del relevo):**
en modo chat el relevo conoce el `session_id` exacto — puede hacer la
copia ÉL MISMO desde el jsonl de la sesión (~/.claude/projects/…) en vez
de teclear /export: mismo destino handoff/, misma verificación en disco,
sin depender de un comando que el entorno no tiene. Tocaría
michi-relevo.py y michi.exe EN SINCRONÍA (mismas constantes/esquema) —
leer docs/remediacion.md antes; no se hace en caliente.

**Mientras tanto:** la prueba del final feliz del auto-/clear va por
TERMINAL (ahí /export sí existe). Ojo con el casado: una sesión de
terminal casa por cwd y es fail-closed ante ambigüedad — si en el mismo
cwd vive otro relevo (p. ej. el chat de trabajo en el proyecto), no casa
nunca. Truco: lanzar `claude` desde una carpeta neutra (~) y leer con
rutas ABSOLUTAS.

## 2026-08-13 (5) — FINAL FELIZ: el auto-/clear por inferencia, completo en vivo

**Registro (18:56-18:57, sesión de terminal en el VPS, cwd ~ para el
casado sin ambigüedad):** tarjeta + "ai: analizando" + "compás 10 s
(presión 88%, rampa)" en el mismo tick; veredicto "clear · tema_nuevo" a
los 22 s y cuenta atrás EN EL ACTO; a los 34 s "relevo auto: aplicado
/clear por IA (tema_nuevo)". El relevo tecleó `/export <handoff>` en la
TUI (ahí el comando SÍ existe), verificó la copia y aplicó el /clear.
Primera vez que el camino entero — rampa → compás caliente → análisis
local → inferencia → red /export → /clear — corre de punta a punta sin
intervención. El ✓ cerró la cuenta en la cápsula y el registro de
acciones guarda el "auto · aplicó /clear".

**Detalles que la corrida validó de propina:** la cuenta SOBREVIVIÓ al
cambio de sesión reina a mitad de camino (18:56:22, un sondeo vio la
sesión del chat al 20% — autoRun es independiente del sondeo, como debe);
el análisis local queda 4 de 4 (tema_nuevo, correcto) en sesiones de
prueba; y el contraste chat/terminal quedó medido el mismo día con la
misma tubería: chat = ERR_RELAY_EXPORT (fail-closed, /export no existe en
la extensión), terminal = aplicado. El comportamiento divergente es del
ENTORNO, no del relevo.

**Cerrado en CLAUDE.md:** auto-/compact y auto-/clear por inferencia
pasan de "en prueba" a COMPLETOS en vivo; el pendiente nuevo es el diseño
de la copia propia del relevo en modo chat (jsonl + sid, zona de reglas
duras) y la muestra del análisis local en uso natural.

## 2026-08-13 (6) — el auto-/clear llega al chat: la copia sin /export

**Qué se construyó:** la red del /clear en modo CHAT ya no depende de
`/export` (que la extensión no tiene, medido en vivo el mismo día): el
relevo hace la copia ÉL MISMO. `session_jsonl(sid)` localiza el JSONL de
la sesión por NOMBRE (`projects/*/<sid>.jsonl` — el sid es UUID único,
sin reproducir la transformación de carpetas; respeta CLAUDE_CONFIG_DIR),
copia con tmp+rename a `handoff/…jsonl` y verifica el hecho del disco.
R4 tras la copia (busy/hijo muerto → el /clear pierde, la copia queda).
Lista blanca, generación de ruta y fail-closed: SIN CAMBIOS. Terminal:
SIN CAMBIOS (/export ahí funciona y está validado). STATE_V sigue en 2.
Detalle en remediacion.md §"La copia SIN /export en el chat"; réplica
exacta wrap_handoff (.py) ↔ rama chat de handoff() (main.rs, por sp.sid).

**Banco en el VPS (claude falso SIN /export, como el real):** 3/3 —
(1) /clear con red: acuse ✓ con ruta, copia IDÉNTICA byte a byte (diff),
banner + eco del /clear pintados en el chat, cero /export tecleado;
(2) regresión: /compact a secas sin copia nueva; (3) fail-closed: sid
sin jsonl → ERR_RELAY_EXPORT y CERO /clear. py_compile limpio.

**Pendiente:** `cargo check` del crate en el Windows de Oscar (el VPS no
compila Rust; `npm run dev` compila el relevo solo con su
beforeDevCommand) y la prueba en vivo con el chat real — el .py viaja
embebido: recompilar en Windows y arrancar la app lo re-sube al VPS, y
la sesión de chat tiene que ser NUEVA (el wrap viejo en marcha no se
entera).

**Mordida del día (ajena al código):** al apagar el banco se mató por
patrón `michi-relevo.py wrap` — que también casaba con los relevos
REALES de las pestañas de chat del VPS: una pestaña de Oscar murió con
SIGTERM (sin pérdida: el jsonl queda y la pestaña se reabre). Regla para
el futuro: en máquinas con relevos vivos, matar por PID exacto, jamás
por patrón.

## 2026-08-13 (7) — el auto-/clear del CHAT validado en vivo, y la sesión que arde gana el trono

**FINAL FELIZ EN EL CHAT (19:43-19:44):** primera corrida real de la
copia sin /export — tarjeta a las 19:43:11 (presión 89%), veredicto
tema_nuevo a los 26 s, cuenta de 30 s, y "aplicado /clear por IA" a las
19:44:10 con la pestaña del chat renacida y la copia .jsonl en
handoff/. La tabla queda 4 de 4: /compact y /clear automáticos, en
terminal y en chat. El análisis local: 5 de 5 aciertos.

**El culpable de los "minutos muertos" que preguntó Oscar:** la sesión
REINA se elegía SOLO por frescura. Registro en vivo: 19:39:53 la pesada
al 77% (compás 10 s) → 19:41:06 la reina era OTRA sesión al 29% (un
mensaje en el chat de trabajo la volvió más fresca) y el compás se abrió
a 60 s → 19:43:11 la pesada (ya al 89%) recuperó el trono. Dos minutos
de sombra que en uso real (varios chats vivos) serían constantes.
Arreglo: `arde()` — una sesión ≥ INTENT_PCT gana el trono SIEMPRE; la
frescura solo desempata (entre dos que arden o dos que no). Con eso el
compás caliente se queda con la sesión peligrosa y el automático no
pierde de vista al incendio por un mensaje en otra ventana.

**Lo que queda de latencia y su porqué (medido en esta corrida):**
detección ≤10-20 s (compás caliente), veredicto 13-26 s (llama en CPU —
LA cifra que los embeddings de la etapa 2 bajarían a ms), cuenta 30 s
(DELIBERADA: es la ventana de cancelación, no se recorta). Piso real
actual ≈ 1 min desde que la sesión cruza el 80% estando quieta.

## 2026-08-13 (8) — visor de copias handoff y etapa 2 del análisis local (embeddings)

**Visor de copias (pedido de Oscar: "¿hay manera de verlo si lo
requiero?"):** el "abrir la copia" del registro de acciones solo servía
para copias LOCALES (Explorador) y las de Oscar viven en el VPS. Ahora el
botón dice "ver la copia" y abre un overlay del panel (patrón .pop
reusado) con el CONTENIDO: .jsonl como transcript legible, .md tal cual.
Piezas: RemAction.origin (aditivo), relay_inject_remote devolviendo la
ruta del acuse (se tiraba), read_handoff(name, origin) con nombre
validado a [A-Za-z0-9._-] antes de componer nada (en remoto viaja en un
comando ssh), tope 4 MB, i18n en los 8 idiomas. "Abrir en la carpeta"
queda solo para locales.

**Etapa 2 del análisis local — embeddings (pedido del mismo día):**
la escalera queda completa (determinista → embeddings → 2B → nada).
`ai_emb_verdict()` corre DENTRO de ai_intent_impl antes de arrancar el
2B: mismo llama-server con `--embeddings --pooling mean -c 512` (puerto
+1, guard kill-on-drop), prefijo `query: ` en ambos lados (e5 lo exige),
coseno TEMA (título+viejos) ↔ RECIENTE (último msg); <0.45 →
clear·tema_nuevo, >0.65 → compact·tema_cruzado, banda media → el 2B.
Fail-quiet en cadena: sin GGUF o con cualquier fallo, v1 exacta. via/sim
aditivos en AiVerdict → flowLog ("(embeddings 0.38)" / "(modelo)") y
ai_debug.txt con tema/reciente/sim (vectores no). Modelo:
multilingual-e5-small-q8_0 (~126 MB, cstr/multilingual-e5-small-GGUF),
SUBIDO al release-estante modelos-v1 como asset NUEVO (aditivo — la
regla del modelos-v2 es para reemplazos) y verificado con descarga
anónima + huella idéntica (0a34067a…53e8). Constantes de la descarga:
NUEVE. ai_setup baja solo lo que falte → con la v1 instalada el botón
ofrece "Descargar el modelo rápido (~126 MB)".

**Verificado:** node --check limpio (visor + i18n + escalera JS);
espejo round-trip con huella idéntica. PENDIENTE Windows: cargo check
(visor Rust + embeddings Rust comparten commit con el relevo del chat),
descargar el e5 con el botón nuevo y ver el primer `via:emb` en vivo.
Los umbrales EMB_NEW/EMB_CROSS no se afinan hasta tener muestra natural.

## 2026-08-13 (9) — la etapa 2 estrenada en vivo (decidió el 2B) y la similitud que viajaba a ciegas

**Cuarta corrida del auto-/clear del día, primera con la escalera
completa (20:25-20:27):** todo el camino otra vez perfecto — 98% de
presión, veredicto a los 31 s, /clear con red aplicado, y el VISOR
estrenado con la copia remota (transcript legible del jsonl traído por
SSH, botón "ver la copia" en el registro). PERO el veredicto vino
"(modelo)": el peldaño de embeddings NO decidió, y con el diseño de esa
mañana era imposible saber por qué — el ai_debug.txt del 2B PISA el
rastro del emb (se sobrescribe, por diseño de esa familia), así que
"banda media legítima" y "el peldaño falló en silencio" se veían
idénticos.

**Arreglo (mismo día):** ai_emb_sim (la medida) se separó de
ai_emb_verdict (la decisión), y la similitud viaja AHORA con el veredicto
del 2B: `sim` va en AiVerdict también cuando decide el llm. El flowLog
distingue: "(modelo · sim 0.52)" = midió banda media y el 2B decidió;
"(modelo)" a secas = el emb no pudo medir (GGUF ausente o fallo
silencioso). Esa diferencia ES el diagnóstico de la etapa 2 en campo.

**Pendiente de la próxima corrida:** ver si sale "sim" en el flowLog. Si
sale a secas "(modelo)", el peldaño está fallando en el Windows de Oscar
(sospechosos: flag --embeddings del build de llama.cpp, o el GGUF de e5
con su versión) — ai_debug.txt tras un "Probar" lo dirá, porque el
Probar con la evidencia de ejemplo debería dar sim baja y decidir por
embeddings en segundos.

## 2026-08-13 (10) — el e5 estaba roto: autopsia con banco propio y cambio a EmbeddingGemma

**El diagnóstico llegó por los micrófonos nuevos:** el emb_server.log de
Oscar dijo la causa exacta — "bert model needs to define token type
count": la conversión GGUF de cstr es vieja y no trae un metadato que el
llama.cpp moderno exige. El peldaño moría al arrancar, siempre.

**Banco de embeddings EN EL VPS (primera vez):** se bajó el build Linux
de llama.cpp b10362 (el MISMO pineado para Windows) + libgomp extraída
de un .deb sin sudo — y con eso las pruebas que antes exigían
ida-y-vuelta con el Windows de Oscar se hicieron aquí en minutos:
- cstr e5: reproduce el fallo exacto de Oscar. mili e5: core dump.
  keisuke e5: carga… pero el tokenizer está dañado — "receta de
  carbonara"↔"CSS del widget" da 0.93, MÁS que una subtarea del mismo
  proyecto (0.90). Matriz completa pooling{mean,cls,last}×prefijo{query,
  sin}: TODAS solapadas. Sin separación no hay umbral: e5-small en GGUF
  comunitario está muerto como opción.
- EmbeddingGemma-300M (GGUF OFICIAL ggml-org, 500k descargas): separa
  limpio SIN prefijos — tema nuevo 0.15-0.36, continuación 0.53, mismo
  tema entre idiomas 0.84 — y CALZA con los umbrales 0.45/0.65 del
  diseño (el prefijo STS de su ficha comprime hacia la banda media: se
  descartó con medida, no con opinión). Validación final con los flags
  EXACTOS del Rust: probar=0.358→clear·emb, carbonara=0.155,
  idiomas=0.844→compact·emb; 3 pares en 0.3 s ya cargado.

**Cambios:** constantes AI_EMB_* → gemma (HF oficial + espejo modelos-v1
verificado con descarga anónima y huella b5ce9d77…0d63); el e5 roto
RETIRADO del estante (jamás lo referenció un release de la app — no es
"reemplazar un binario publicado"); ai_emb_path() ignora la ruta del e5
aunque siga en la config (sin esto, quien lo descargó hoy quedaba
bloqueado para siempre) y ai_setup borra el archivo huérfano y pisa la
ruta muerta; flags: -c 1024, sin --pooling (el GGUF oficial trae el
suyo), sin prefijos; tamaños de la UI 126→319 MB y total 1.4→1.7 GB ×8
idiomas.

**Verificado:** carga + salud + separación con los flags exactos en el
banco del VPS; espejo round-trip; node --check limpio. Pendiente Windows:
cargo check (npm run dev) + Probar — esperado "✓ clear · tema nuevo ·
embeddings 0.36" en segundos.

## Cierre 2026-08-13 — la jornada de los automáticos

Día récord: 14 commits, y el proyecto cruzó su meta fundacional — Michi
aplicando /compact y /clear SOLO, de punta a punta, en terminal Y en
chat (tabla 4/4, validada en vivo por Oscar en cuatro corridas).

**Lo construido hoy, en orden:** compás adaptativo del coach con cazador
de rampas (3 min → 10 s bajo presión) · cuenta pegada al veredicto ·
compuerta `ready` antes de la cuenta · arreglo de la carrera del primer
sondeo caliente · la sesión que arde gana el trono de la reina · la
copia SIN /export para el chat (el límite del /export de la extensión,
descubierto, diseñado, implementado y validado el mismo día) · visor de
copias handoff (local/SSH/WSL, transcript legible) · etapa 2 del
análisis local con EmbeddingGemma (banco de llama.cpp en el VPS, autopsia
de los e5 rotos, espejo verificado, Probar en Windows clavando el número
del banco: 0.36).

**Estado al cierre:** todo pusheado (`ba06442`), cargo check implícito
pasado (la app compiló y corrió todo en el Windows de Oscar). El sistema
queda EN VALIDACIÓN PASIVA: Oscar lo usa normal y reporta cualquier
rareza de clear/compact — el rastro para revisarlas es flowLog +
emb_debug.txt + registro de acciones con su "ver la copia". Los
pendientes vivos quedan en CLAUDE.md §Estado: primer via:emb en sesión
real, muestra natural para los umbrales, validación pasiva de alarmas/
ntfy/hallazgos, y el ruteo inteligente sigue BLOQUEADO hasta confirmar
estas pruebas del día a día.

---

## 2026-08-14 — Etapa 0 del ruteo: el hook SÍ impone el modelo del subagente (A/B en el VPS)

Primera pieza del ruteo inteligente, y la única que el plan permitía
tocar con las pruebas del día a día abiertas: es un experimento aparte,
no comparte código con el coach ni con el gatito.

**La pregunta:** ¿un `PreToolUse` puede reescribir el modelo con el que
NACE un subagente, devolviendo `hookSpecificOutput.updatedInput`? De
ella cuelga el Hook B entero (el ahorrador silencioso).

**Cómo se probó** (`scripts/ruteo-etapa0/`, commit del experimento):
hook de juguete que solo actúa con la marca `RUTEO-TEST` y falla
callado; sesión headless de Claude Code 2.1.231 con el hook en settings
de proyecto, padre en Sonnet, subagente `general-purpose`; el veredicto
NO se le pregunta al subagente (los modelos se equivocan sobre sí
mismos) sino al `agent-*.jsonl` que escribe Claude Code.

**Resultado — A/B con 27 s de diferencia, todo lo demás igual:**

| Corrida | Modelo real en el transcript |
|---|---|
| Con marca (hook actúa) | `claude-haiku-4-5-20251001` |
| Control sin marca (hook calla) | `claude-sonnet-5` (hereda del padre) |

ÉXITO. La apuesta técnica del Hook B se sostiene y no hizo falta el
plan B (frontmatter `model:` / `CLAUDE_CODE_SUBAGENT_MODEL`).

**Lo que el log enseñó y el diseño no sabía:**

1. **El nombre de la herramienta no es estable**: en este build llega
   como `Agent`, no `Task`. El matcher `Task|Agent` la agarró por los
   pelos — el matcher doble es OBLIGATORIO, no adorno. Si un día no
   dispara, sospechar del nombre ANTES que del script.
2. **El input NO trae `model`**: llegó `antes=(no venía)` y el hook lo
   AÑADIÓ. `updatedInput` no solo reescribe campos, también agrega los
   que no existen. Y el input traía `run_in_background`, que es
   justamente por qué hay que devolver el objeto COMPLETO (§10.1).
3. **La variante A basta**: `updatedInput` a secas, sin
   `permissionDecision: allow`. La variante B queda documentada por si
   una versión futura la exige.
4. **Contexto gratis para el Hook B real**: el payload del hook trae
   `cwd`, `session_id`, `transcript_path`, `permission_mode` y
   `effort:{level}`. El `cwd` da el proyecto sin adivinar — el
   `modo_proyecto` de `router_state.json` se puede resolver ahí mismo.

**Lo que queda de esta etapa:** la corrida en Windows nativo (el
`hook-model-test.ps1` es traducción literal del Python y NO se pudo
ejecutar en el VPS — no hay PowerShell). Mecánicamente WSL y VPS son el
mismo caso (Linux, mismo script), así que la matriz de Oscar se cierra
con esa única corrida pendiente.

**Lo que NO cambia:** la compuerta sigue puesta. Etapa 1 en adelante
espera a cerrar las pruebas en vivo del auto-/clear y del análisis
local — comparten zona de código y contaminarían la medición.

### Apéndice del 2026-08-14 — tres trampas de Windows en la etapa 0

Anotadas mientras Oscar corría el experimento en su Windows (Claude
Code v2.1.232); ninguna es del mecanismo, las tres son del entorno:

1. **PowerShell 5.1 lee los `.ps1` sin BOM como ANSI.** Mis cuatro
   scripts iban con tildes y rayas largas en los comentarios: el `—` se
   convierte en `â€"` y ese `"` CIERRA la cadena a media línea →
   `MissingEndCurlyBrace` y el script no compila. Arreglado pasando los
   cuatro a ASCII puro. REGLA para el Hook B de verdad (irá embebido con
   `include_str!`, donde nadie avisa en compilación): grep de no-ASCII.
   El fallo, eso sí, se comportó como debía — `hook error ... non-blocking`
   y el subagente corrió igual.
2. **El menú `/hooks` es de SOLO LECTURA** en esta versión ("To add or
   modify hooks, edit settings.json directly"). El README lo daba como
   camino recomendado para INSTALAR; corregido: sirve para verificar.
   Instalar es `instalar-hook.ps1`, que fusiona con los hooks que ya
   haya en vez de pisarlos.
3. **El modelo puede estar CLAVADO** en `.claude\settings.json`
   ("pins Haiku 4.5 — that applies on restart"). `/model` cambia la
   sesión de ya, pero reiniciar la devuelve al clavado. Orden correcto:
   hook → reiniciar → `/model` → prueba. Y si la sesión ya está en
   Haiku, el experimento no demuestra nada: el subagente nacería en
   Haiku con hook o sin él.

Cuarto dato, este a favor: en Windows la herramienta TAMBIÉN llega como
`Agent` (lo dijo el error: `PreToolUse:Agent hook error`). Dos builds
distintos, mismo nombre no-documentado — el matcher `Task|Agent` se
queda.

### Cierre de la etapa 0 (2026-08-14, tarde) — validada también en Windows nativo

El experimento corrió en el Windows de Oscar (Claude Code v2.1.232,
sesión en Sonnet 5) y el A/B salió solo, gracias al fallo de
codificación de la primera corrida:

| Hora | Estado del hook | Modelo real del subagente |
|---|---|---|
| 12:34:23 | roto (`hook error`, no bloqueante) | `claude-sonnet-5` (hereda del padre) |
| 12:39:54 | ya en ASCII, funcionando | `claude-haiku-4-5-20251001` |

Misma máquina, misma sesión, mismo `general-purpose`, 5 min de
diferencia: la única variable fue que el `.ps1` compilara. El error de
codificación regaló el grupo de control.

El log de Windows confirma los tres hechos del VPS, ahora en el otro
mundo: la herramienta llega como `tool_name: "Agent"`; el input NO trae
`model` (`antes=(no venia)`) y `updatedInput` lo AÑADE; y basta la
forma mínima, sin `permissionDecision`.

**ETAPA 0 CERRADA.** Los dos mundos donde corre Claude Code están
cubiertos: Linux (VPS por SSH; WSL es el mismo caso mecánico — mismo
`hook-model-test.py`, mismo `~/.claude`) y Windows nativo (PowerShell).
La apuesta técnica del Hook B se sostiene y el plan B queda de respaldo.

La compuerta NO se mueve: etapa 1 sigue esperando a que cierren las
pruebas en vivo del auto-/clear y del análisis local. Comparten zona de
código (coach/gatito) y arrancar ahora contaminaría esa medición.

## 2026-08-14 (2) — % de desperdicio estructural: fórmula, obra y dos arreglos del panel

Jornada en el VPS (chat de VS Code). Tres frentes, los tres cerrados aquí
y pendientes solo de `cargo check` + vistazo visual en Windows.

**1. La fórmula (fila 18 de presion-y-rendimiento.md).** Era el diseño
previo obligatorio y quedó escrito en su § propia. Lo esencial: sumar
todos los hallazgos y dividir está MAL por tres razones verificadas en el
código — los detectores se pisan (inflate contiene a reread vía
cache_read; mech cobra el turno entero sin excluir subagentes), los más
estructurales valen $0 (mcp/skills, resta de conjuntos), y el tope de 12
por costo decapita justo a los baratos. La salida: UNA LÍNEA DE FACTURA
POR DETECTOR (input: claudemd+hooks_noise; cache_write: cachebreak;
cache_read/turno entero: excluidos) → numerador disjunto por
construcción, sin restas a mano. Como deja fuera más de lo que arriesga,
el número es un PISO y el copy dice "al menos" — invariante #8 con
dirección segura. Fusión multi-origen: suma de numeradores ÷ suma de
denominadores, JAMÁS promedio de porcentajes.

**2. La obra (tres piezas, invariante #1).** `scan_findings` (Python) y
`scan_local_findings` (Rust) calculan `waste` ANTES del tope de 12:
{struct_cost, struct_tokens, total_cost, sessions, days, end, estimated,
items[]} con `items` = tarjetas estructurales sin recortar (tope 100)
para que el panel descuente las ignoradas con `fndKey`. `get_findings`
ahora devuelve `FindingsPack{findings, waste}` — los 3 usos del frontend
desempaquetan `.findings`. Tarjeta en Reporte bajo el héroe con los 3
estados degradados diseñados (ventana corta / juntando datos / nada que
señalar), comparación "antes: Y%" (segunda pasada con --end corrido) y
nota "no contamos" con MCP/skills. i18n `wst_*` ×8.

VALIDACIÓN en el VPS: regresión `--end` congelado 7d y 30d → findings y
campos viejos byte-idénticos; y `waste.total_cost` == `cost_week` de la
agregación normal AL CÉNTIMO en ambas ventanas — dos caminos
independientes que cuadran exactos. Dato real del VPS: 11.2% de
desperdicio en 30d ($230 de $2,057), TODO cachebreak — la fuga más cara
del catálogo también manda aquí. La maqueta de la otra IA (prompt del
doc) sirvió de referencia con 4 correcciones anotadas en el chat: su
tarjeta de "subagentes sin rastro" era falsa, el "trabajo real 86%"
afirmaba lo no demostrado, el cachebreak no lleva "~" (es MEDIDO) y
mezclaba ventanas de 7 y 30 días.

**3. Panel, dos peticiones de Oscar del día:** (a) adiós a la rendija
transparente — era el padding de 1px del body que el anillo del borde
necesitaba; ahora el borde es `outline` con offset -1px (hacia adentro,
por encima del contenido: un inset lo tapaba el sticky) y padding 0;
(b) el panel ya NO se cierra al perder el foco — era flyout y estorbaba
al consultarlo trabajando; solo cierran el ✕ y el menú del tray. CLAUDE.md
actualizado en ambos.

## 2026-08-14 (3) — Detector 11: frecuencia de auto-compacts (kind acompact)

La mitad que faltaba de la fila 11: la regla `acomp` del coach avisa del
EVENTO; ahora Hallazgos mide el HÁBITO. Tarjeta por PROYECTO con ≥3
auto-compacts en la ventana (por sesión sería confeti), costo PISO
obligatorio: la compactación NO trae usage, lo único medible es
`preTokens` (mismo campo que la regla acomp) cobrado UNA vez al input del
modelo dominante, con "~". Solo `trigger != manual` — las del relevo
entran como manual y quedan fuera solas. Dedup por uuid (reanudaciones).
NO entra al numerador del % de desperdicio. Tres piezas en sincronía;
marcas de arreglo lo incluyen.

Validación con moraleja: la regresión congelada (7d/30d) dio el resto de
tarjetas y el waste byte-idénticos, pero NO salió la tarjeta acompact en
30d pese a haber 3 autos reales en los logs — la instrumentación línea a
línea enseñó que los 3 se CUENTAN bien y caen 2+1 en dos proyectos
distintos (las sesiones de julio llevan el disp viejo claude-code-meter):
bajo umbral, silencio honesto — el detector funcionando exactamente como
se diseñó. El cuadre exacto quedó en un fixture sintético: 3 autos =
175k pre = $0.175 a precio haiku, con el uuid duplicado deduplicado, la
manual y la fuera-de-ventana excluidas, y callando con solo 2. De paso se
explicó el `cost_today` "inestable" de la regresión: es relativo a AHORA
(no al --end) y le pasa igual al exportador viejo — ruido del reloj, no
de los cambios.

## 2026-08-14 (4) — Detector 12: pegado masivo — y el bug de uturns que cazó el diseño

El pendiente decía "diseñarlo y validarlo antes de prometerlo", y el
diseño pagó: la exploración de los 1,025 mensajes humanos reales del VPS
(mediana tecleada 290 chars, p90 1.7k) enseñó que los 10 "mensajes" más
grandes NO eran pegotes — eran los resúmenes de continuación de la
compactación ("This session is being continued…"), que viajan con rol
user y PASABAN el filtro user_turn_text. Doble consecuencia: el detector
habría acusado pegotes del sistema, y —el bug de regalo— uturns llevaba
contándolos como turnos útiles desde la fase 1, diluyendo el rendimiento.

Arreglo en la raíz: isCompactSummary fuera de user_turn_text (AMBOS
lados) + caché de escaneo v2→v3 (el patrón documentado: un caché viejo
devolvería los uturns de antes en silencio). Delta verificado EXACTO:
842→824 uturns en 30d = los 18 resúmenes únicos de la ventana, ni uno
más.

El detector: kind `paste`, umbral POR MENSAJE 5k chars (~17× la mediana
real), tarjeta por PROYECTO con ≥3 pegotes y ≥10k tokens, costo PISO
chars/4 × input dominante ("~"), dedup uuid, fuera del waste
(conductual), fix que no regaña (un error de consola no tiene ruta que
mencionar). Réplica Rust con chars().count() — bytes divergiría con
tildes. Fixture con cuadre exacto (50k chars = 12.5k tok = $0.0125
haiku; resumen/meta/corto/fuera-de-ventana excluidos; el volumen calla 3
pegotes chicos). En los datos reales del VPS la tarjeta VIVE (7d: 6
pegotes, 11.4k tok en michiclaude) pero cae al puesto 16 — el tope de 12
la deja fuera porque aquí dominan los inflates; en la máquina de un
usuario típico saldría. cargo check pendiente en Windows.

## 2026-08-15 — Las 4 piezas de integridad: que un borrado no se disfrace de mejora

Oscar trajo un ADR externo (multi-harness + persistencia con SQLite).
Veredicto y análisis completo en `docs/adr-multiharness-y-persistencia.md`:
la Parte 1 se rechaza (choca con el NO vigente y con el foso
Claude-específico; la capa "medidor" está saturada y gratis), la Parte 2
diagnostica un riesgo REAL pero con una solución sobredimensionada. Oscar
aprobó la versión ligera: 4 piezas, cero SQLite.

**El riesgo, en una frase:** los `.jsonl` no son nuestros. Si un limpiador
tipo conversation-reclaim los recorta, MichiClaude leería menos y cantaría
"mejoraste" — la mentira exacta que prohíbe el invariante #8. El fixture lo
enseña sin piedad: 504,000 → 30,200 tokens, una "mejora" del 94% que era un
borrado.

**Lo construido.** (1) Detector pasivo montado sobre el caché de escaneo,
que YA guardaba tamaño+mtime: archivo que encogió o desapareció →
`integrity.json` (local, no viaja), con réplica en el exportador y el
origen puesto por Rust. (2) Comparaciones NO CONCLUYENTES en el Reporte
(héroe, volumen, contradicción, desperdicio) cuando el tramo está tocado.
(3) `daily_history.json`, la serie diaria fusionada de 400 días — RESPALDO,
no jefe. (4) Las marcas de arreglo congelan su "antes" al nacer, sacado del
cuadernito.

**Las dos decisiones de diseño que más costaron pensar.** La primera: el
cuadernito NO manda. Ayer mismo el fix de `uturns` corrigió 30 días hacia
atrás porque los logs crudos seguían ahí; con rollups congelados al mando
—como pedía el ADR— ese bug habría quedado fosilizado en la historia. Un
store protege contra borrados Y congela errores: por eso lo vivo manda
siempre y el cuadernito solo rellena lo que ya no se puede ver. La segunda:
un recorte se DETECTA en una fecha, pero sus bytes pueden ser de cualquier
día. No se puede atribuir a un periodo, así que cualquier hecho desde el
arranque del periodo más viejo ensucia la comparación entera — fingir
precisión ahí habría sido peor que el hueco.

**Falsos positivos, cazados antes de nacer.** Dos guardas, ambas probadas:
solo se juzgan las raíces que se pudieron LEER (con WSL apagado sus
archivos "faltan" sin haberse borrado — habría sido una alarma falsa
diaria en el Windows de Oscar) y solo cuenta si el archivo de verdad no
existe (envejecer fuera de la ventana ≠ borrarse). Y el archivador propio
no puede dispararla: mueve archivos ≥365d y el caché solo guarda los de
~32 días. Cero solape.

**Validación.** Fixture de extremo a extremo con cuadre AL BYTE
(56,379−5,659=50,720); silencio en la primera corrida sin caché, sin
cambios y tras avisar (no repite); regresión con logs reales 7d/30d
byte-idéntica en findings, waste, totales y serie diaria; 9 casos de la
lógica de cobertura en node, incluidos los negativos; forma del JSON
verificada contra el struct de Rust; i18n ×8. Pendiente: `cargo check` en
Windows y WSL, que queda verificado POR CONSTRUCCIÓN (misma función, otra
raíz) pero no ejecutado.

**Aviso de mantenimiento:** CLAUDE.md quedó en ~39.7k de los 40k. La
próxima entrada que se le añada debería venir con una poda: lo que ya está
en los docs de diseño no necesita repetirse ahí.

## 2026-08-15 (2) — Purga del archivo: el ciclo de vida completo, y WSL entra al archivador

Nació de la pregunta de Oscar sobre el caso viral de los 60 GB de logs.
Es creíble (tool results enteros en el log, enjambres de agentes, y las
reanudaciones que COPIAN el archivo entero — la razón de nuestra dedup
por uuid), y destapó una verdad incómoda: con `cleanupPeriodDays: 365`
hacemos crecer el disco 12× más que la fábrica, y el archivador de la
etapa 2 solo MOVÍA. El disco nunca bajaba. Faltaba el último escalón.

Diseño completo en remediacion.md §"Purga del archivo": ciclo VIVO →
ARCHIVADO → PURGADO; siete reglas de seguridad en Rust que el panel no
puede saltarse (suelo 180 d, doble reloj con sidecar `.arch`, allowlist
canónica, simulacro, palabra de confirmación proporcional, tope por
pasada, solo .jsonl); nace en "nunca" y el automático es opt-in con el
candado de primera manual. Decisiones de Oscar: purga apagada de
nacimiento (rotundo), el usuario elige el plazo con advertencia, y el
VPS SOLO INFORMA (`--du` + un `find -mtime +365` acotado) — desde la app
nunca se borra por SSH.

Hallazgo de paso: WSL NUNCA se archivaba (`archivable_files` solo miraba
`~/.claude` local). Arreglado: `archive_roots()` cubre las distros, cada
una a su subcarpeta.

Validación sin toolchain: réplica línea a línea del algoritmo en Python,
18/18 — incluida la trampa de un symlink dentro del archivo apuntando a
un log VIVO (no entra; el vivo queda intacto). `--du` contra logs reales
y fixture. `cargo check` pendiente en Windows.

Nota de mantenimiento: CLAUDE.md pasó por su primera PODA (el bloque de
validación pasiva narraba historia que ya vive en remediacion.md y la
bitácora); quedó en 39.7k. La regla desde hoy: cada entrada nueva ahí
viene con una poda equivalente.
