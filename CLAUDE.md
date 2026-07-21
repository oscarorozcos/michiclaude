# CLAUDE.md — Claude Code Meter

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
1. Si existe `%APPDATA%\com.oscarorozco.claude-code-meter\remotes.json`
   (`{"remotes":[{"name":"vps","host":"<alias ssh>","command":"python3 /opt/projects/claude-code-meter/scripts/meter-export.py"}]}`),
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
  dato con `pill:ready` al cargar.
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
  [80,95]); máx. un aviso por umbral por ventana; al cruzar varios de golpe
  suena solo el más alto. Límite semanal al 100%: un aviso por ventana.
- Avisos de RESTABLECIMIENTO (sesión y semanal): solo si la ventana anterior
  llegó al 100% (`hit:*`); el toast se repite cada 5 min + banner verde en el
  panel HASTA que el usuario pulse "Enterado" (`ackPending:*` en
  localStorage). Nunca quitar el mecanismo de confirmación.
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
- [ ] Lectura incremental de .jsonl por offset (hoy: escaneo completo por ciclo)
- [x] Token de respaldo desde remotes.json cuando el local venció (2026-07-10):
      el meter ya no depende de usar Claude Code en Windows.
- [x] Autostart (tauri-plugin-autostart): solo builds release, se activa una
      única vez (marker `autostart_configured`); si el usuario lo desactiva
      en el Administrador de tareas, se respeta.
- [x] LICENSE MIT (preparación para repo público).
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
- [x] Ajustes reorganizados: ⚙ muestra "Fuentes de datos" (nota incluye WSL,
      lista de servidores con estado activo/sin conexión, alta con prueba)
      y "Preferencias" (idioma, presupuesto, exportación) por separado.
- [x] Leyenda de modelos completa (todos los usados, "<1%" para los mínimos).
- [~] Alarmas de uso configurables + avisos de límite/restablecimiento con
      confirmación "Enterado" — implementado (2026-07-11); FALTA probar en
      vivo el ciclo completo (cruce de umbral, 100%, reset, banner).
- [~] Widget flotante (pill) sobre la barra — implementado (2026-07-11);
      FALTA probar en vivo: posición por defecto junto al reloj, arrastre con
      ⠿, persistencia de posición, no robar foco, tema sincronizado.
- [ ] MODO HUB (decidido 2026-07-11, pendiente de implementar tras la semana
      de pruebas): el VPS consolida los datos de todas las máquinas para que
      los totales cuadren en cualquier PC. Diseño acordado: (1) cada meter
      sube su resumen local por SSH a ~/.claude-code-meter/hosts/<hostname>.json
      en el VPS en cada ciclo; (2) meter-export.py devuelve sus logs + los
      resúmenes de los demás hosts, excluyendo el del host que pregunta
      (--exclude-host <hostname>) para no contar doble; (3) opcional: config
      compartida (servidores/presupuesto) guardada también en el hub para que
      una PC nueva herede todo al conectar el VPS.
- [ ] Auto-updater (tauri-plugin-updater)
- [ ] Precios de modelos configurables (JSON externo)

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

- Remoto: `https://github.com/oscarorozcos/claude-code-meter` (privado).
- El desarrollo y las pruebas ocurren en el PC Windows de Oscar; en el VPS vive
  un clon espejo (`/opt/projects/claude-code-meter`) para revisión de código.
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
- Se publicará en GitHub (MIT, releases automáticas por tag). La confianza del
  usuario es prioridad: transparencia total sobre el manejo del token y el
  disclaimer del endpoint no oficial.
