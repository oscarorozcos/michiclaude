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
- **Fila "claude.ai / otros"**: estimación por diferencia entre cuota global y
  actividad local (en % de cuota, nunca en $ inventados).
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
   llevan sufijo de origen (" · wsl", " · <servidor>"); el frontend etiqueta
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
- **Widget flotante** (`pill`, src/pill.html): pastilla opcional SIEMPRE
  visible (marca + % de sesión + barras S/W + punto de estado) que vive
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
  siguen con toasts). En modo gatito NINGÚN aviso va a toast: los cuatro
  salen por el globo `notif`, con un `kind` que viaja y vuelve con la ✕.
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
8. La fila "claude.ai / otros" se muestra solo si es estimable con datos
   fiables; en % de cuota, no en dólares.
9. No tocar `README.md`, `.github/workflows/release.yml` ni `app-icon.png`
   salvo petición explícita.
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
- [x] Fila "claude.ai / otros"
- [x] Fix de arrastre y flyout
- [x] `cargo check` limpio (verificado 2026-07-10; la sesión que se cortó a
      mitad de editar Cargo.toml no dejó nada roto)
- [x] Franja sobre la barra: DESCARTADA (2026-07-10, solapaba los iconos
      centrados de Windows 11 y bloqueaba sus clics). Sustituida por el icono
      de bandeja dinámico con %.
- [~] Icono de bandeja dinámico (`updateTray` + comando `update_tray`) —
      implementado; FALTA probar en vivo: legibilidad del número en temas
      claro/oscuro de Windows y actualización tras cada refresco.
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
- [~] Fuente remota VPS (Oscar usa Claude Code sobre todo en el VPS vía
      VS Code SSH): exportador + fusión implementados y el script probado
      contra los logs reales del VPS. FALTA: crear remotes.json en Windows y
      verificar en vivo la fusión (proyectos "· vps").
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
- [~] PRECIOS DINÁMICOS — IMPLEMENTADO 2026-07-26, falta probar en vivo.
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
      accionable al instante. FALTA: probarlo sin red.
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
- [~] Lectura incremental de .jsonl — IMPLEMENTADO 2026-07-26 en AMBOS lados
      (Rust y meter-export.py), falta probar el lado Rust en Windows. Dos
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
- [~] Fuente WSL implementada: `wsl.exe -l -q` (UTF-16LE) + escaneo de
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
- [x] Export CSV/JSON a carpeta elegida por el usuario (vacía = Descargas),
      comando `export_data`.
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
- [~] Alarmas de uso configurables + avisos de límite/restablecimiento con
      confirmación "Enterado" — implementado (2026-07-11); FALTA probar en
      vivo el ciclo completo (cruce de umbral, 100%, reset, banner).
- [x] Widget flotante (pill) sobre la barra — probado en vivo (2026-07-22):
      arrastre, persistencia, sin robo de foco, tema OK.
- [x] Widget gatito (2026-07-22, validado en vivo por Oscar): mascota con
      estados normal/llamas/zzz ligados a los avisos, cápsula de %, sticker
      que abre el panel, globo de información al hover (buckets dinámicos)
      y globo de notificación con ✕ en vez de toasts. Pose automática de
      globos, cola dinámica y soporte multi-monitor. Ver sección Ventanas.
- [ ] VERIFICAR alta automática de servidor (2026-07-27): el flujo nuevo
      (detectar Python -> subir el lector a ~/.michiclaude/ -> guardar el
      comando resuelto) NO se ha probado en vivo, porque el servidor de Oscar
      sigue guardado con la ruta vieja de cuando se configuró a mano. Para
      probarlo hay que borrarlo con el bote de la lista y volver a agregarlo
      dejando el comando VACÍO. Comprobar además el caso de error: un host sin
      Python debe fallar con ERR_NO_PYTHON y su mensaje traducido, no darse
      por bueno.
- [ ] MODO HUB (decidido 2026-07-11, pendiente de implementar tras la semana
      de pruebas): el VPS consolida los datos de todas las máquinas para que
      los totales cuadren en cualquier PC. Diseño acordado: (1) cada meter
      sube su resumen local por SSH a ~/.michiclaude/hosts/<hostname>.json
      en el VPS en cada ciclo; (2) meter-export.py devuelve sus logs + los
      resúmenes de los demás hosts, excluyendo el del host que pregunta
      (--exclude-host <hostname>) para no contar doble; (3) opcional: config
      compartida (servidores/presupuesto) guardada también en el hub para que
      una PC nueva herede todo al conectar el VPS.
- [ ] Auto-updater (tauri-plugin-updater)
- [ ] Precios de modelos automáticos (ver el pendiente detallado arriba:
      fuente pública + caché + tabla embebida corregida como respaldo)

## Diferenciadores estratégicos (post-pulido Windows, decididos 2026-07-24)

Tras investigar la competencia (Mac saturado con 8+ apps de menu bar; Windows
competido pero ganable; Linux sin app gráfica nativa = hueco). El combo actual
—cuota real + costo por proyecto + multi-máquina + gatito— ya es único; casi
nadie junta cuota Y costo, casi nadie hace multi-máquina, y NADIE tiene mascota.
Tres apuestas priorizadas, a trabajar DESPUÉS de pulir Windows:

- [ ] **APUESTA #1 — Terminar el Modo HUB** (ver arriba). Es el foso técnico
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
- [ ] **APUESTA #3 — De "medidor" a "asesor"** (más ambicioso, tras el Hub):
      insights accionables — proyección SEMANAL ("a este ritmo llegas al límite
      el jueves"), desglose caro por proyecto ("60% es lectura de caché"),
      sugerencia de ahorro por modelo ("usaste Opus donde Haiku bastaba →
      ahorro $X", con cuidado). Eleva de "app de gauges" a "app que me ayuda a
      no quedarme sin cuota / gastar menos".

NO hacer (dilución de foco): rastrear otras herramientas (Codex/Gemini/Copilot),
base de datos de historial largo (contradice "nada que se pueda perder"), modo
equipo/empresa (fuera del público Pro/Max individual).

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
corre en el Windows de Oscar.)

**Simulador de estados del gatito** (Preferencias → "🐱 Simular estados"):
recorre el ciclo COMPLETO —dibujo y globo— en cinco pasos: normal (sin
globo) → fire (globo de alarma, con TU umbral configurado) → break → zzz →
normal + globo de cuota restablecida. Usa los datos reales cuando existen
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

- Remoto: `https://github.com/oscarorozcos/michiclaude` (público desde 2026-07-24; URL vieja redirige).
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
  esta app: marcador de ritmo + proyección + fila claude.ai/otros + franja
  sobre la barra.
- Se publicará en GitHub (GPL-3.0, releases automáticas por tag). La confianza del
  usuario es prioridad: transparencia total sobre el manejo del token y el
  disclaimer del endpoint no oficial.
