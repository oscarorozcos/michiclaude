// Claude Code Meter — backend
// Dos fuentes de datos:
//   1) Cuota real (5h / semanal, compartida entre claude.ai y Claude Code)
//      -> endpoint OAuth con el token de ~/.claude/.credentials.json
//   2) Detalle local ($/proyecto, modelo más usado)
//      -> parseo de ~/.claude/projects/**/*.jsonl con deduplicación

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
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

    // Si el token ya venció según las credenciales locales, no llamamos a la API
    // (peticiones repetidas con token vencido provocan bloqueos temporales).
    if let Some(exp) = v["claudeAiOauth"]["expiresAt"].as_i64() {
        if exp < chrono::Utc::now().timestamp_millis() {
            return Err(
                "Token expirado. Usa Claude Code en ESTE PC (cualquier consulta) para refrescarlo."
                    .into(),
            );
        }
    }

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("user-agent", "claude-code-meter/0.1.0")
        .send()
        .await
        .map_err(|e| format!("Sin conexión con la API: {e}"))?;

    if resp.status().as_u16() == 401 {
        return Err("Token expirado. Abre Claude Code (o ejecuta `claude update`) para refrescarlo.".into());
    }
    if resp.status().as_u16() == 429 {
        // Respetar el Retry-After del servidor si viene; si no, 5 min.
        let mins = resp
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok())
            .map(|secs| ((secs / 60.0).ceil() as u64).max(1))
            .unwrap_or(5);
        // Cuerpo del error a quota_debug.json para diagnóstico.
        let body = resp.text().await.unwrap_or_default();
        let dir = app_data_dir();
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(
            dir.join("quota_debug.json"),
            format!("HTTP 429 (retry-after: {mins} min)\n{body}"),
        );
        return Err(format!(
            "La API limitó las peticiones (429). Reintento en {mins} min — tu cuota no se ve afectada."
        ));
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

// ---------- rectángulo real de la barra de tareas (Win32) ----------

#[cfg(windows)]
mod win_taskbar {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect};

    pub struct Taskbar {
        pub rect: RECT,
    }

    /// Lee el rectángulo de Shell_TrayWnd (para apoyar el panel encima de la barra).
    pub fn query() -> Option<Taskbar> {
        unsafe {
            let tray = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).ok()?;
            let mut rect = RECT::default();
            GetWindowRect(tray, &mut rect).ok()?;
            Some(Taskbar { rect })
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
        // Instancia única: si la app ya corre, el segundo arranque solo enfoca el
        // panel (varias instancias duplicando el polling provocan 429). Debe ser
        // el primer plugin registrado.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_panel(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_quota,
            get_local_stats,
            update_tray
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_panel" => show_main_panel(app),
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .setup(|app| {
            // Menú del tray (clic derecho): abrir panel, salir.
            let tray_menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "tray_panel", "Abrir panel", true, None::<&str>)?,
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
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error al iniciar Claude Code Meter");
}

// Márgenes (px físicos aprox.).
const MARGIN_X: u32 = 16;
const TASKBAR_H: u32 = 56; // respaldo si no hay rect real de la barra
const PANEL_GAP: u32 = 8;

/// Redibuja el icono del tray con el % actual (RGBA renderizado por el panel en
/// un canvas) y actualiza el tooltip. Así el número vive junto al reloj, como
/// los medidores de batería/CPU — la vía nativa en Windows 11.
#[tauri::command]
fn update_tray(
    app: tauri::AppHandle,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    tooltip: String,
) -> Result<(), String> {
    use tauri::Manager;
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err("buffer RGBA de tamaño inesperado".into());
    }
    let tray = app
        .tray_by_id("main-tray")
        .ok_or("icono de bandeja no encontrado")?;
    let icon = tauri::image::Image::new_owned(rgba, width, height);
    tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;
    tray.set_tooltip(Some(&tooltip)).map_err(|e| e.to_string())?;
    Ok(())
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

