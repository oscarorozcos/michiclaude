// Claude Code Meter — backend
// Dos fuentes de datos:
//   1) Cuota real (5h / semanal, compartida entre claude.ai y Claude Code)
//      -> endpoint OAuth con el token de ~/.claude/.credentials.json
//   2) Detalle local ($/proyecto, modelo más usado)
//      -> parseo de ~/.claude/projects/**/*.jsonl con deduplicación

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

// ---------- rutas ----------

/// Debe coincidir con `identifier` en tauri.conf.json.
const APP_IDENTIFIER: &str = "com.oscarorozco.claude-code-meter";

/// Directorio de datos de la app (Windows: %APPDATA%\<identifier>).
fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_IDENTIFIER)
}

fn claude_dir() -> PathBuf {
    // Respeta CLAUDE_CONFIG_DIR si el usuario movió su config
    if let Ok(d) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    dirs::home_dir().unwrap_or_default().join(".claude")
}

// ---------- 1) Cuota real (endpoint OAuth, no oficial) ----------

#[tauri::command]
async fn get_quota() -> Result<serde_json::Value, String> {
    let cred_path = claude_dir().join(".credentials.json");
    let raw = fs::read_to_string(&cred_path).map_err(|_| {
        "No encontré ~/.claude/.credentials.json. Inicia sesión en Claude Code primero.".to_string()
    })?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Credenciales ilegibles: {e}"))?;
    let token = v["claudeAiOauth"]["accessToken"]
        .as_str()
        .ok_or("Token OAuth no encontrado en las credenciales.")?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|e| format!("Sin conexión con la API: {e}"))?;

    if resp.status().as_u16() == 401 {
        return Err("Token expirado. Abre Claude Code (o ejecuta `claude update`) para refrescarlo.".into());
    }
    if !resp.status().is_success() {
        return Err(format!("La API respondió {}", resp.status()));
    }
    // Devolvemos el JSON crudo; el frontend renderiza los buckets que existan
    // (five_hour, seven_day, seven_day_sonnet/opus/fable...) de forma dinámica,
    // porque la forma exacta varía por plan y puede cambiar.
    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Respuesta ilegible: {e}"))?;

    // Debug: volcamos la respuesta cruda a quota_debug.json para inspeccionar qué
    // buckets reales devuelve el endpoint. Los nombres de modelo (p. ej. "Fable")
    // viven en el array `limits[].scope.model.display_name`, no en las claves
    // seven_day_* (que suelen venir null). Se escribe en el directorio de datos de
    // la app:  %APPDATA%\com.oscarorozco.claude-code-meter\quota_debug.json
    // (con respaldo junto al ejecutable y, en último caso, el directorio actual).
    if let Ok(pretty) = serde_json::to_string_pretty(&data) {
        let dir = app_data_dir();
        let _ = fs::create_dir_all(&dir);
        let primary = dir.join("quota_debug.json");
        if fs::write(&primary, &pretty).is_err() {
            let fallback = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("quota_debug.json")));
            match fallback {
                Some(alt) if fs::write(&alt, &pretty).is_ok() => {}
                _ => {
                    let _ = fs::write("quota_debug.json", &pretty);
                }
            }
        }
    }

    Ok(data)
}

// ---------- 2) Detalle local desde los .jsonl ----------

#[derive(Serialize, Default, Clone)]
struct ModelAgg {
    input: u64,
    output: u64,
    cache_write: u64,
    cache_read: u64,
    cost: f64,
}

#[derive(Serialize)]
struct ProjectAgg {
    name: String,
    cost: f64,
    tokens: u64,
}

#[derive(Serialize)]
struct LocalStats {
    projects: Vec<ProjectAgg>,
    models: HashMap<String, ModelAgg>,
    cost_today: f64,
    cost_week: f64,
    tokens_week: u64,
    files_scanned: usize,
    entries_deduped: usize,
}

/// Precios API por MTok: (input, output, cache_write, cache_read).
/// Con suscripción Pro/Max el coste es *nocional* (equivalente API);
/// con API key es gasto real. Ajustable sin recompilar en una versión futura.
fn price_for(model: &str) -> (f64, f64, f64, f64) {
    let m = model.to_lowercase();
    if m.contains("opus") || m.contains("fable") || m.contains("mythos") {
        (15.0, 75.0, 18.75, 1.5)
    } else if m.contains("haiku") {
        (1.0, 5.0, 1.25, 0.1)
    } else {
        // sonnet y desconocidos
        (3.0, 15.0, 3.75, 0.3)
    }
}

fn cost_of(model: &str, inp: u64, out: u64, cw: u64, cr: u64) -> f64 {
    let (pi, po, pcw, pcr) = price_for(model);
    (inp as f64 * pi + out as f64 * po + cw as f64 * pcw + cr as f64 * pcr) / 1_000_000.0
}

/// Decodifica el nombre de carpeta de proyecto de Claude Code
/// ("C--Users-oscar-mi-proyecto" -> "mi-proyecto")
fn pretty_project(dir_name: &str) -> String {
    dir_name
        .rsplit('-')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(dir_name)
        .to_string()
}

#[tauri::command]
fn get_local_stats() -> Result<LocalStats, String> {
    let projects_dir = claude_dir().join("projects");
    let now = Utc::now();
    let day_ago = now - Duration::hours(24);
    let week_ago = now - Duration::days(7);

    let mut seen: HashSet<String> = HashSet::new();
    // Clave: nombre de carpeta codificado; el nombre bonito sale del `cwd` real
    // que traen las entradas (el nombre codificado es ambiguo con los guiones).
    let mut display_names: HashMap<String, String> = HashMap::new();
    let mut per_project: HashMap<String, (f64, u64)> = HashMap::new();
    let mut per_model: HashMap<String, ModelAgg> = HashMap::new();
    let mut cost_today = 0.0;
    let mut cost_week = 0.0;
    let mut tokens_week: u64 = 0;
    let mut files_scanned = 0usize;
    let mut deduped = 0usize;

    let entries = match fs::read_dir(&projects_dir) {
        Ok(e) => e,
        Err(_) => {
            return Ok(LocalStats {
                projects: vec![],
                models: HashMap::new(),
                cost_today: 0.0,
                cost_week: 0.0,
                tokens_week: 0,
                files_scanned: 0,
                entries_deduped: 0,
            })
        }
    };

    for proj in entries.flatten() {
        if !proj.path().is_dir() {
            continue;
        }
        let raw_dir = proj.file_name().to_string_lossy().to_string();

        let files = match fs::read_dir(proj.path()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        for f in files.flatten() {
            let path = f.path();
            if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
                continue;
            }
            files_scanned += 1;
            let Ok(content) = fs::read_to_string(&path) else { continue };

            for line in content.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
                if !display_names.contains_key(&raw_dir) {
                    if let Some(base) = v["cwd"]
                        .as_str()
                        .and_then(|c| c.rsplit(['\\', '/']).next())
                        .filter(|s| !s.is_empty())
                    {
                        display_names.insert(raw_dir.clone(), base.to_string());
                    }
                }
                let msg = &v["message"];
                let usage = &msg["usage"];
                if !usage.is_object() {
                    continue;
                }

                // Deduplicación: mismo message.id + requestId = misma petición
                // (reanudaciones y streaming duplican entradas en los .jsonl)
                let key = format!(
                    "{}:{}",
                    msg["id"].as_str().unwrap_or(""),
                    v["requestId"].as_str().unwrap_or("")
                );
                if key != ":" && !seen.insert(key) {
                    deduped += 1;
                    continue;
                }

                let inp = usage["input_tokens"].as_u64().unwrap_or(0);
                let out = usage["output_tokens"].as_u64().unwrap_or(0);
                let cw = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                let cr = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                let model = msg["model"].as_str().unwrap_or("unknown").to_string();
                // "<synthetic>" = mensajes placeholder de error de Claude Code,
                // no son peticiones reales al modelo.
                if model == "<synthetic>" {
                    continue;
                }
                let cost = cost_of(&model, inp, out, cw, cr);

                let ts: Option<DateTime<Utc>> = v["timestamp"]
                    .as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&Utc));

                let in_week = ts.map(|t| t >= week_ago).unwrap_or(false);
                let in_day = ts.map(|t| t >= day_ago).unwrap_or(false);

                if in_week {
                    cost_week += cost;
                    // tokens "de trabajo": excluimos cache_read (infla ~100x)
                    tokens_week += inp + out + cw;
                    let e = per_project.entry(raw_dir.clone()).or_insert((0.0, 0));
                    e.0 += cost;
                    e.1 += inp + out + cw;
                    let m = per_model.entry(model.clone()).or_default();
                    m.input += inp;
                    m.output += out;
                    m.cache_write += cw;
                    m.cache_read += cr;
                    m.cost += cost;
                }
                if in_day {
                    cost_today += cost;
                }
            }
        }
    }

    let mut projects: Vec<ProjectAgg> = per_project
        .into_iter()
        .map(|(raw, (cost, tokens))| ProjectAgg {
            name: display_names
                .get(&raw)
                .cloned()
                .unwrap_or_else(|| pretty_project(&raw)),
            cost,
            tokens,
        })
        .collect();
    projects.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));

    Ok(LocalStats {
        projects,
        models: per_model,
        cost_today,
        cost_week,
        tokens_week,
        files_scanned,
        entries_deduped: deduped,
    })
}

// ---------- config persistente de la franja ----------

#[derive(Serialize, Deserialize)]
struct BarConfig {
    bar_visible: bool,
}
impl Default for BarConfig {
    fn default() -> Self {
        BarConfig { bar_visible: true }
    }
}

/// (junto al ejecutable, directorio de datos). Se intenta el primero y, si no se
/// puede, el segundo — cumple "un JSON de config junto al ejecutable" con respaldo.
fn bar_config_paths() -> (PathBuf, PathBuf) {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("bar_config.json")));
    let data = app_data_dir().join("bar_config.json");
    (exe.unwrap_or_else(|| data.clone()), data)
}

fn load_bar_config() -> BarConfig {
    let (exe, data) = bar_config_paths();
    for p in [exe, data] {
        if let Ok(s) = fs::read_to_string(&p) {
            if let Ok(c) = serde_json::from_str::<BarConfig>(&s) {
                return c;
            }
        }
    }
    BarConfig::default()
}

fn save_bar_config(c: &BarConfig) {
    if let Ok(s) = serde_json::to_string_pretty(c) {
        let (exe, data) = bar_config_paths();
        if fs::write(&exe, &s).is_ok() {
            return;
        }
        let _ = fs::create_dir_all(app_data_dir());
        let _ = fs::write(&data, &s);
    }
}

// ---------- rectángulo real de la barra de tareas (Win32) ----------

#[cfg(windows)]
mod win_taskbar {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, FindWindowW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
        SetWindowLongPtrW, GWL_EXSTYLE, SM_CYSCREEN, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    pub struct Taskbar {
        pub rect: RECT,
        pub tray_left: Option<i32>,
        pub hidden: bool,
    }

    /// Lee Shell_TrayWnd (barra) y TrayNotifyWnd (área de bandeja) + estado auto-hide.
    pub fn query() -> Option<Taskbar> {
        unsafe {
            let tray = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).ok()?;
            let mut rect = RECT::default();
            GetWindowRect(tray, &mut rect).ok()?;

            let tray_left = FindWindowExW(Some(tray), None, w!("TrayNotifyWnd"), PCWSTR::null())
                .ok()
                .and_then(|h| {
                    let mut r = RECT::default();
                    if GetWindowRect(h, &mut r).is_ok() {
                        Some(r.left)
                    } else {
                        None
                    }
                });

            let mut abd = APPBARDATA {
                cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                ..Default::default()
            };
            let state = SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32;
            let cy = GetSystemMetrics(SM_CYSCREEN);
            let h = rect.bottom - rect.top;
            // Auto-hide "guardada": alto ~0 o borde superior pegado al fondo (deslizada).
            let hidden = h <= 2 || ((state & ABS_AUTOHIDE) != 0 && rect.top >= cy - 4);

            Some(Taskbar { rect, tray_left, hidden })
        }
    }

    /// Evita que la franja robe el foco al hacer clic (se comporta como la barra).
    pub fn make_noactivate(hwnd_raw: isize) {
        unsafe {
            let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let add = (WS_EX_NOACTIVATE.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | add);
        }
    }
}

// ---------- app + tray ----------

pub fn run() {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
        Manager,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_quota,
            get_local_stats,
            show_panel,
            open_bar_menu
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "bar_hide" | "tray_bar_hide" => set_bar_visible(app, false),
            "tray_bar_show" => set_bar_visible(app, true),
            "tray_panel" => show_main_panel(app),
            "bar_quit" | "tray_quit" => app.exit(0),
            _ => {}
        })
        .setup(|app| {
            // Franja: quitarle la activación (no roba foco) y colocarla en la barra.
            if let Some(bar) = app.get_webview_window("bar") {
                #[cfg(windows)]
                if let Ok(h) = bar.hwnd() {
                    win_taskbar::make_noactivate(h.0 as isize);
                }
                let _ = &bar;
            }
            reposition_bar(app.handle());

            // Menú del tray (clic derecho): abrir panel, mostrar/ocultar franja, salir.
            let tray_menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "tray_panel", "Abrir panel", true, None::<&str>)?,
                    &MenuItem::with_id(app, "tray_bar_show", "Mostrar franja", true, None::<&str>)?,
                    &MenuItem::with_id(app, "tray_bar_hide", "Ocultar franja", true, None::<&str>)?,
                    &MenuItem::with_id(app, "tray_quit", "Salir", true, None::<&str>)?,
                ],
            )?;

            // Icono en la bandeja; clic izquierdo abre el panel, clic derecho el menú.
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Claude Code Meter")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        show_main_panel(tray.app_handle());
                    }
                })
                .build(app)?;

            // Reposiciona la franja cada 5 s (resolución/DPI/auto-hide) y re-asegura el
            // always-on-top: la barra de tareas también es topmost y puede taparla.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                reposition_bar(&handle);
            });

            Ok(())
        })
        .on_window_event(|window, event| match event {
            // La ✕ (o el aspa nativa) oculta el panel a la bandeja en vez de cerrar.
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
            // Cambio de DPI/monitor: recoloca la franja dentro de la barra.
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                if window.label() == "bar" {
                    reposition_bar(window.app_handle());
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error al iniciar Claude Code Meter");
}

// Márgenes (px físicos aprox.).
const MARGIN_X: u32 = 16;
const TASKBAR_H: u32 = 56; // respaldo si no hay rect real de la barra
const PANEL_GAP: u32 = 8;
/// Ancho lógico objetivo de la franja (se escala por DPI). El alto lo dicta la barra.
const BAR_W_LOGICAL: f64 = 205.0;
const BAR_MARGIN_V: f64 = 8.0; // margen vertical dentro de la barra

/// Muestra el panel (invocado desde el clic en la franja o el tray).
#[tauri::command]
fn show_panel(app: tauri::AppHandle) {
    show_main_panel(&app);
}

/// Menú contextual de la franja (clic derecho): ocultar franja / salir.
#[tauri::command]
fn open_bar_menu(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    use tauri::menu::{Menu, MenuItem};
    let hide = MenuItem::with_id(&app, "bar_hide", "Ocultar franja", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(&app, "bar_quit", "Salir", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(&app, &[&hide, &quit]).map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())?;
    Ok(())
}

/// Cambia (y persiste) la visibilidad de la franja.
fn set_bar_visible(app: &tauri::AppHandle, visible: bool) {
    use tauri::Manager;
    let mut cfg = load_bar_config();
    cfg.bar_visible = visible;
    save_bar_config(&cfg);
    if visible {
        reposition_bar(app);
    } else if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
}

/// Coloca, muestra y enfoca el panel principal (encima de la barra de tareas).
fn show_main_panel(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        position_panel(&w);
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// El panel se apoya arriba a la derecha, justo encima de la barra de tareas.
fn position_panel(w: &tauri::WebviewWindow) {
    if let (Ok(Some(monitor)), Ok(size)) = (w.current_monitor(), w.outer_size()) {
        let screen = monitor.size();
        let x = screen.width.saturating_sub(size.width + MARGIN_X) as i32;

        #[cfg(windows)]
        if let Some(tb) = win_taskbar::query() {
            let y = (tb.rect.top - size.height as i32 - PANEL_GAP as i32).max(0);
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
            return;
        }

        let y = screen.height.saturating_sub(size.height + TASKBAR_H) as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

/// Coloca la franja DENTRO de la barra de tareas (o esquina inferior derecha si no
/// hay Win32), re-asegurando el always-on-top. Respeta la preferencia guardada.
fn reposition_bar(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(bar) = app.get_webview_window("bar") else {
        return;
    };
    if !load_bar_config().bar_visible {
        let _ = bar.hide();
        return;
    }

    #[cfg(windows)]
    if let Some(tb) = win_taskbar::query() {
        if tb.hidden {
            let _ = bar.hide(); // barra auto-hide oculta -> franja oculta también
            return;
        }
        let scale = bar.scale_factor().unwrap_or(1.0);
        let tb_h = (tb.rect.bottom - tb.rect.top).max(1);
        let margin = (BAR_MARGIN_V * scale) as i32;
        let bar_h = (tb_h - 2 * margin).clamp((24.0 * scale) as i32, tb_h);
        let bar_w = (BAR_W_LOGICAL * scale) as i32;
        let anchor = tb.tray_left.unwrap_or(tb.rect.right - (220.0 * scale) as i32);
        let x = (anchor - bar_w - margin).max(tb.rect.left);
        let y = tb.rect.top + (tb_h - bar_h) / 2;
        let _ = bar.set_size(tauri::PhysicalSize::new(bar_w.max(1) as u32, bar_h.max(1) as u32));
        let _ = bar.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = bar.set_always_on_top(true); // re-aserción
        if !bar.is_visible().unwrap_or(true) {
            let _ = bar.show();
        }
        return;
    }

    // Fallback sin Win32: esquina inferior derecha, siempre encima.
    if let (Ok(Some(monitor)), Ok(size)) = (bar.current_monitor(), bar.outer_size()) {
        let s = monitor.size();
        let x = s.width.saturating_sub(size.width + 220) as i32;
        let y = s.height.saturating_sub(size.height + 8) as i32;
        let _ = bar.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = bar.set_always_on_top(true);
        let _ = bar.show();
    }
}
