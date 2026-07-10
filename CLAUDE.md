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

## Arquitectura

```
src/index.html          # Frontend del panel: HTML+CSS+JS vanilla, un solo archivo,
                        # sin frameworks, sin bundler, sin dependencias npm de runtime.
src/bar.html            # Frontend de la franja compacta (widget sobre la barra)
src-tauri/src/main.rs   # Entry point (windows_subsystem = "windows")
src-tauri/src/lib.rs    # Backend: comandos, tray, ventanas, Win32
src-tauri/tauri.conf.json
src-tauri/capabilities/default.json
app-icon.png            # Fuente de iconos (npm run icons los genera)
.github/workflows/release.yml  # Compila y publica instalador en tags v*
```

### Fuentes de datos (dos, independientes)

**A) Cuota real — comando `get_quota` (Rust):**
1. Lee el token OAuth de `~/.claude/.credentials.json`
   (campo `claudeAiOauth.accessToken`). Respeta `CLAUDE_CONFIG_DIR` si existe.
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
5. Agrega: por proyecto (7 días), por modelo (7 días), coste hoy y semana.

### Ventanas

- **Panel** (`main`): flyout sin decoraciones, transparente, alwaysOnTop,
  skipTaskbar. Se abre con clic en el tray; se oculta al perder foco (excepto
  durante arrastre); ✕ oculta a bandeja; arrastrable desde el encabezado.
- **Franja compacta** (`bar`): overlay posicionado SOBRE la barra de tareas
  (junto al área de bandeja) usando Win32 (`Shell_TrayWnd`/`TrayNotifyWnd` vía
  crate `windows`), porque Windows 11 no permite incrustar ventanas reales en
  la barra (DeskBands deprecado). Persistente, no roba foco
  (`WS_EX_NOACTIVATE`), re-aserta always-on-top cada 5 s, clic abre el panel,
  clic derecho: menú ocultar/salir con preferencia persistida
  (`bar_config.json`).
- **Tray icon**: base robusta de la app; clic izquierdo muestra el panel,
  clic derecho menú (abrir panel / mostrar-ocultar franja / salir).

## INVARIANTES — no romper nunca

1. `get_quota` y `get_local_stats`: no cambiar firmas; no eliminar la
   deduplicación ni la exclusión de `cache_read`.
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
10. Mensajes de UI en español, tono claro y accionable.

## Comportamiento ya validado — no regresionar

- Panel con datos reales (gauge sesión, buckets dinámicos incl. Fable,
  proyección con burn rate, gasto por proyecto con dedup, split de modelos con
  nombres bonitos, totales hoy/semana).
- Arrastre del panel desde el encabezado (con guarda anti-blur durante drag).
- ✕ oculta a bandeja; clic en tray reabre; flyout se oculta al perder foco.
- Estados de error legibles con punto rojo en la línea de estado.
- Notificaciones de umbral: máx. una por umbral por ventana de sesión
  (`maybeNotify`, clave en localStorage).
- La franja nunca llama al endpoint: recibe datos del panel vía evento
  `quota:update` y pide el último conocido con `bar:ready` al (re)cargar.

## Estado actual / pendientes conocidos

- [x] Panel completo con datos reales
- [x] Buckets semanales dinámicos (incluido Fable)
- [x] Fila "claude.ai / otros"
- [x] Fix de arrastre y flyout
- [x] `cargo check` limpio (verificado 2026-07-10; la sesión que se cortó a
      mitad de editar Cargo.toml no dejó nada roto)
- [~] Franja compacta sobre la barra de tareas — código completo (bar.html,
      módulo `win_taskbar`, `reposition_bar`, menú contextual, persistencia).
      FALTA probarla en vivo en Windows 11: posición junto a la bandeja,
      que no robe foco, auto-hide de la barra, DPI/multi-monitor.
- [~] Bug de render vacío de la franja: mitigado (estado "loading", handshake
      `bar:ready` → reemisión del último payload, backoff 5→10→20→40 s en el
      panel). FALTA confirmar en vivo que no reaparece.
- [ ] Lectura incremental de .jsonl por offset (hoy: escaneo completo por ciclo)
- [ ] Auto-updater (tauri-plugin-updater) y autostart (tauri-plugin-autostart)
- [ ] Tema claro
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
