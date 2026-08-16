# MichiClaude — Widget embebido en la taskbar de Windows 11 (Ruta A: Parenting a Shell_TrayWnd)

> Documento de contexto/handoff para asistente de IA. Objetivo: implementar esta funcionalidad en una app Tauri 2 existente.

> **NOTA DE ENCAJE CON EL REPO (2026-08-16):** documento EXTERNO, escrito
> con una foto vieja/imaginada del proyecto. Correcciones contra la
> realidad: NO hay SQLite ni daemon (invariante #4; el escaneo vive en
> `get_local_stats` con `scan_cache.json`); el panel NO es flyout (ventana
> persistente desde 2026-08-14); no hay plan vigente de Azure Trusted
> Signing/winget/Scoop; y la mayor alarma técnica es el invariante
> CRÍTICO de ventanas transparentes (WebView2 deja de pintar al
> redimensionar en vivo) — el spike SIN WebView2 del §4.5 no es opcional,
> es la puerta de entrada. Cuando este doc y CLAUDE.md discrepen, manda
> CLAUDE.md.

---

## 1. Contexto del producto

**MichiClaude** es una app de Windows en **Tauri 2 (Rust backend + HTML/JS frontend)** que vive en el system tray y monitorea el uso de tokens de Claude Code leyendo los archivos JSONL locales (`~/.claude/projects/...`). No consume tokens de API — todo es lectura local. Diferenciador: detecta "fugas de tokens" y mide el efecto de plugins de brevedad con datos reales.

**Stack actual relevante:**
- Tauri 2, Rust estable, frontend HTML/JS vanilla con design system dark navy.
- Ya existe: ícono de tray, ventana anclada al tray (manómetro de uso), ciclo que lee JSONL.
- **Restricción dura: nada de DLL injection en explorer.exe** — rompería la confianza y el pitch de "cero riesgo".

## 2. Qué se quiere construir

Un **mini-widget siempre visible dentro de la barra de tareas de Windows 11** (no un overlay flotante, no solo un tray icon): una franja delgada anclada en el espacio vacío de la taskbar (idealmente a la izquierda del área de notificación / reloj) que muestre en tiempo real:

- % de uso de la sesión de 5h (y opcionalmente la semanal de 7d)
- Countdown al reset
- Color según nivel (ok / advertencia / crítico)
- Clic → abre el panel completo de MichiClaude

## 3. Por qué esta técnica (antecedentes)

- Windows 11 **eliminó las DeskBands** (`IDeskBand`), la API oficial de XP→Win10 para embeber contenido en la taskbar. La taskbar de Win11 es XAML y no hay API pública de reemplazo.
- Herramientas como StartAllBack/ExplorerPatcher lo logran con **inyección de DLL en explorer.exe** — descartado (fragilidad ante updates, flags de Defender/SmartScreen, riesgo reputacional).
- La técnica elegida (Ruta A) es la de **Deskband11** (proyecto de zadjii, dev de Microsoft/Windows Terminal): crear una **ventana propia, transparente, always-on-top, y hacerle `SetParent` al HWND de la taskbar (`Shell_TrayWnd`)**, con clip region ajustada al contenido para no interferir con el resto de la barra. En palabras del autor: "es literalmente solo una ventana encima de la taskbar". Sin injection, sin hooks, solo Win32 público.
- Existe una librería .NET de referencia (**airtaxi/Deskband11Lib**, NuGet para WinUI 3/WPF) que implementa exactamente esto. No es usable directo desde Rust, pero su código fuente documenta la secuencia de llamadas Win32, que es portable.

## 4. Diseño técnico propuesto (Rust + Tauri 2)

### 4.1 Ventana Tauri dedicada

Crear una `WebviewWindow` separada (label p.ej. `taskbar-widget`) con:
- `decorations: false`, `transparent: true`, `shadow: false`, `resizable: false`
- `skip_taskbar: true` (que no aparezca como app en la propia barra)
- Tamaño pequeño y fijo, p.ej. ~140×40 px lógicos (ajustar a la altura real de la taskbar, típicamente 48 px físicos a 100% DPI en Win11)

### 4.2 Parenting con Win32 (crate `windows`)

Desde Rust, con el HWND que expone Tauri (`window.hwnd()`):

```text
1. taskbar_hwnd = FindWindowW("Shell_TrayWnd", null)
2. (opcional, para posicionar) notify_hwnd = FindWindowExW(taskbar_hwnd, null, "TrayNotifyWnd", null)
   → GetWindowRect(notify_hwnd) da el rect del área de notificación;
     el widget se ancla justo a su izquierda.
3. Cambiar estilos del widget:
   - SetWindowLongPtrW(widget_hwnd, GWL_STYLE): quitar WS_POPUP/caption, poner WS_CHILD
   - GWL_EXSTYLE: WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE (que no robe foco);
     considerar WS_EX_LAYERED para transparencia
4. SetParent(widget_hwnd, taskbar_hwnd)
5. SetWindowPos(widget_hwnd, HWND_TOP, x, y, w, h, SWP_SHOWWINDOW | SWP_NOACTIVATE)
   donde x = notify_rect.left - w - margen, y centrado vertical en la taskbar
```

Crates: `windows` (features `Win32_UI_WindowsAndMessaging`, `Win32_Foundation`, `Win32_Graphics_Gdi`). Tauri: `tauri::WebviewWindow::hwnd()` devuelve `HWND` en Windows.

### 4.3 Supervivencia a reinicios de explorer (crítico)

Cuando explorer.exe se reinicia (crash, update, el usuario lo mata), la taskbar se destruye **y la ventana hija muere/queda huérfana con ella**. Solución estándar:

- Registrar el mensaje `RegisterWindowMessageW("TaskbarCreated")` en una ventana oculta propia (message-only window) y, al recibirlo, **recrear/re-parentear** el widget con la secuencia del 4.2.
- Alternativa más simple pero menos elegante: watchdog con timer que verifique `IsWindow(taskbar_hwnd)` cada pocos segundos.

### 4.4 Posicionamiento robusto

- **Reflow de la taskbar:** los íconos centrados de Win11 crecen dinámicamente; por eso el widget debe anclarse **relativo a `TrayNotifyWnd` (derecha)**, no a coordenadas absolutas ni al centro. Re-calcular posición en un timer ligero (p.ej. cada 1–2 s) o al detectar cambios de tamaño del rect.
- **DPI:** usar coordenadas físicas de `GetWindowRect` y convertir con el scale factor del monitor (`GetDpiForWindow`). Tauri maneja DPI en su webview, pero el posicionamiento vía `SetWindowPos` es en píxeles físicos.
- **Multi-monitor:** `Shell_TrayWnd` es la taskbar principal; las secundarias son `Shell_SecondaryTrayWnd`. Fase 1: solo monitor principal.
- **Taskbar en auto-hide:** al ser hija de la taskbar, el widget se oculta/muestra con ella automáticamente (ventaja de esta ruta sobre overlays).

### 4.5 Interacción

- El widget debe recibir clics (abrir el panel principal vía IPC de Tauri / evento al proceso principal) pero **no robar foco** (`WS_EX_NOACTIVATE`).
- Tooltip nativo o el propio HTML del webview para hover.
- Cuidado: la webview de Tauri (WebView2) dentro de una ventana `WS_CHILD` re-parenteada es la parte menos probada de este diseño. **Plan de validación:** primer spike con un rectángulo GDI/ventana nativa mínima antes de meter la webview; si WebView2 da problemas como hija de la taskbar, fallback a dibujar el widget con GDI/Direct2D desde Rust (es solo texto + barra de color).

## 5. Riesgos conocidos y mitigaciones

| Riesgo | Impacto | Mitigación |
|---|---|---|
| Update de Win11 cambia estructura de la taskbar | Widget desaparece o queda mal posicionado | Feature flag: si falla `FindWindow`/parenting, degradar automáticamente a tray icon dinámico (plan B ya diseñado) |
| Explorer restart | Widget muere | Listener de `TaskbarCreated` + recreación (4.3) |
| WebView2 inestable como child de taskbar | Render roto | Spike temprano; fallback GDI/Direct2D (4.5) |
| Íconos centrados invaden el espacio | Solapamiento | Anclar a la derecha, medir `TrayNotifyWnd`, ancho mínimo, opción de ocultar si no hay espacio |
| Percepción de "hack" | Confianza del usuario | Es Win32 público sin injection; documentarlo en README; opción off por default o toggle claro en settings |

## 6. Criterios de aceptación (fase 1)

1. Widget visible dentro de la taskbar principal, a la izquierda del reloj, mostrando `%sesión` + countdown.
2. Sobrevive a `taskkill /f /im explorer.exe && start explorer` (reaparece solo, <5 s).
3. No roba foco al hacer clic; clic abre el panel de MichiClaude.
4. Se comporta bien con taskbar en auto-hide y a 100%, 125% y 150% DPI.
5. Si el parenting falla por cualquier razón, la app degrada a tray icon dinámico sin crashear.

## 7. Referencias

- https://github.com/zadjii/Deskband11 — técnica original (WinUI 3): ventana transparente parented a la taskbar con clip region.
- https://github.com/airtaxi/Deskband11Lib — librería .NET con la implementación de referencia del parenting a `Shell_TrayWnd`.
- https://github.com/niccolo-sabato/claude-usage-widget — competidor que resolvió lo mismo con overlay posicionado (Ruta B, útil para comparar edge cases).
- https://github.com/jens-duttke/usage-monitor-for-claude — referencia del plan B (tray icon dinámico con barras dibujadas).
- Clases de ventana relevantes: `Shell_TrayWnd` (taskbar principal), `Shell_SecondaryTrayWnd` (monitores secundarios), `TrayNotifyWnd` (área de notificación).
