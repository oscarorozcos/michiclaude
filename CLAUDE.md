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
4. Coste equivalente-API con la tabla `price_for()` por substring del modelo
   (opus/fable/mythos, haiku, sonnet/default).
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
  los avisos: normal / cat-fire (alarma de % pendiente = `ackPending:alarm`,
  se calma al abrir el panel o cerrar el globo) / cat-zzz (semana al 100%,
  hasta el reset). En modo gatito las alarmas de % NO van a toast de
  Windows: salen como globo `notif` (los demás avisos y la pastilla normal
  siguen con toasts). Los gifs (800², transparentes, en variantes -black/-white
  elegidas según el tema) se recortan por CSS
  (unión visible x[39,748] y[0,530] medida con decodificador propio) — NO
  editar los archivos. `place_balloon()` coloca los globos con pose
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
- [ ] Micro-pendientes de pulido: quitar los backticks de \`claude\` en el
      banner "Token vencido"; nota sobre subagentes en README/tooltip (el
      costo local puede subestimar con agentes — limitación compartida con
      ccusage, cuota no afectada); capturas para el README; idea cancelada
      2026-07-25: placa translúcida tras el sticker en tema oscuro.
- [ ] USABILIDAD fuente remota (detectado 2026-07-24): el campo "comando" del
      formulario de servidores viene por defecto con la ruta PERSONAL de Oscar
      (`python3 /opt/projects/michiclaude/scripts/meter-export.py`), que no
      sirve para otros usuarios y confunde. Además hoy el usuario tendría que
      copiar meter-export.py al servidor a mano y saber su ruta — no está
      explicado ni automatizado. Arreglo mínimo: default genérico
      (`python3 ~/meter-export.py`) + nota corta. Ideal: que MichiClaude SUBA
      el script solo por SSH la primera vez (nombre + host y listo). Encaja
      naturalmente con el Modo HUB — resolver ahí.
- [ ] Lectura incremental de .jsonl por offset (hoy: escaneo completo por ciclo)
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
- [x] Tema claro/oscuro con toggle ◐ persistido.
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
      botón ⚙ se eliminó — las pestañas siempre están visibles.
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
- [ ] Precios de modelos configurables (JSON externo)

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
