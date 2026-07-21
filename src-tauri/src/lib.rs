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

/// Extrae (accessToken, expiresAt en ms) de un .credentials.json.
fn parse_credentials(raw: &str) -> Option<(String, i64)> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let token = v["claudeAiOauth"]["accessToken"].as_str()?.to_string();
    let exp = v["claudeAiOauth"]["expiresAt"].as_i64().unwrap_or(i64::MAX);
    Some((token, exp))
}

/// Credenciales frescas desde una máquina remota (misma llave SSH que el
/// exportador). El token viaja cifrado por SSH y solo vive en memoria.
fn fetch_remote_credentials(r: &RemoteSource) -> Option<(String, i64)> {
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(&r.host)
        .arg("cat ~/.claude/.credentials.json");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_credentials(std::str::from_utf8(&out.stdout).ok()?)
}

#[tauri::command]
async fn get_quota() -> Result<serde_json::Value, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();

    // 1) Token local vigente. 2) Si falta o venció: token fresco de las máquinas
    // remotas de remotes.json (p. ej. el VPS donde Claude Code se usa a diario).
    // Nunca se llama a la API con token vencido (provoca bloqueos temporales).
    let mut cred = fs::read_to_string(claude_dir().join(".credentials.json"))
        .ok()
        .and_then(|raw| parse_credentials(&raw))
        .filter(|(_, exp)| *exp > now_ms);
    if cred.is_none() {
        for d in wsl_claude_dirs() {
            if let Some(c) = fs::read_to_string(d.join(".credentials.json"))
                .ok()
                .and_then(|raw| parse_credentials(&raw))
                .filter(|(_, exp)| *exp > now_ms)
            {
                cred = Some(c);
                break;
            }
        }
    }
    if cred.is_none() {
        for r in load_remotes() {
            if let Some(c) = fetch_remote_credentials(&r).filter(|(_, exp)| *exp > now_ms) {
                cred = Some(c);
                break;
            }
        }
    }
    // Los errores viajan como códigos ERR_*; el frontend los traduce al idioma activo.
    let (token, _) = cred.ok_or_else(|| "ERR_NO_TOKEN".to_string())?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("user-agent", "claude-code-meter/0.1.0")
        .send()
        .await
        .map_err(|_| "ERR_NET".to_string())?;

    if resp.status().as_u16() == 401 {
        return Err("ERR_TOKEN_EXPIRED".into());
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
        return Err(format!("ERR_RATE_LIMITED:{mins}"));
    }
    if !resp.status().is_success() {
        return Err(format!("ERR_API:{}", resp.status().as_u16()));
    }
    // Devolvemos el JSON crudo; el frontend renderiza los buckets que existan
    // (five_hour, seven_day, seven_day_sonnet/opus/fable...) de forma dinámica,
    // porque la forma exacta varía por plan y puede cambiar.
    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|_| "ERR_BAD_RESPONSE".to_string())?;

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

#[derive(Serialize, Deserialize, Default, Clone)]
struct ModelAgg {
    input: u64,
    output: u64,
    cache_write: u64,
    cache_read: u64,
    cost: f64,
}

#[derive(Serialize, Deserialize)]
struct ProjectAgg {
    name: String,
    cost: f64,
    tokens: u64,
    /// Coste por modelo dentro del proyecto (id de modelo -> USD equiv.)
    #[serde(default)]
    by_model: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize)]
struct DailyAgg {
    date: String, // YYYY-MM-DD (UTC)
    cost: f64,
}

#[derive(Serialize, Deserialize)]
struct LocalStats {
    projects: Vec<ProjectAgg>,
    models: HashMap<String, ModelAgg>,
    cost_today: f64,
    /// Coste de la ventana seleccionada (1/7/30 días; el nombre se conserva
    /// por compatibilidad con el exportador remoto).
    cost_week: f64,
    tokens_week: u64,
    files_scanned: usize,
    entries_deduped: usize,
    /// Serie diaria de los últimos 30 días (para la gráfica de tendencia).
    #[serde(default)]
    daily: Vec<DailyAgg>,
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

// ---------- fuentes remotas opcionales (otras máquinas vía SSH) ----------
// %APPDATA%\com.oscarorozco.claude-code-meter\remotes.json:
//   { "remotes": [ { "name": "vps", "host": "<alias ssh>",
//       "command": "python3 /opt/projects/claude-code-meter/scripts/meter-export.py" } ] }
// Cada fuente devuelve un LocalStats por stdout; se fusiona con lo local y sus
// proyectos se etiquetan "nombre · vps". Sin remotes.json la función no hace nada.

#[derive(Serialize, Deserialize, Clone)]
struct RemoteSource {
    name: String,
    host: String,
    command: String,
}

#[derive(Serialize, Deserialize)]
struct RemotesConfig {
    remotes: Vec<RemoteSource>,
}

fn load_remotes() -> Vec<RemoteSource> {
    fs::read_to_string(app_data_dir().join("remotes.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<RemotesConfig>(&s).ok())
        .map(|c| c.remotes)
        .unwrap_or_default()
}

/// Fuentes remotas configuradas (para el apartado de ajustes del panel).
#[tauri::command]
fn get_remotes() -> Vec<RemoteSource> {
    load_remotes()
}

/// Persiste la lista de fuentes remotas editada desde el panel.
#[tauri::command]
fn save_remotes(remotes: Vec<RemoteSource>) -> Result<(), String> {
    let dir = app_data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let s = serde_json::to_string_pretty(&RemotesConfig { remotes })
        .map_err(|e| e.to_string())?;
    fs::write(dir.join("remotes.json"), s).map_err(|e| e.to_string())
}

/// Prueba la conexión SSH a un host (BatchMode: nunca pide contraseña).
#[tauri::command]
fn test_remote(host: String) -> Result<String, String> {
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(&host)
        .arg("echo ok");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd
        .output()
        .map_err(|e| format!("No pude ejecutar ssh: {e}"))?;
    if out.status.success() {
        Ok("ok".into())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .trim()
            .chars()
            .take(200)
            .collect())
    }
}

/// Ejecuta el exportador remoto por SSH. BatchMode: jamás pide contraseña
/// (requiere llave configurada, la misma que usa VS Code Remote-SSH).
fn fetch_remote(r: &RemoteSource, window_days: u32) -> Option<LocalStats> {
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(&r.host)
        .arg(format!("{} --days {}", r.command, window_days));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sin flash de consola
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice::<LocalStats>(&out.stdout).ok()
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

// ---------- WSL (Claude Code dentro de Windows Subsystem for Linux) ----------
// Muchos usuarios corren Claude Code dentro de WSL; sus logs viven en
// \\wsl.localhost\<distro>\home\<user>\.claude (o /root). Se detectan las
// distros con `wsl.exe -l -q` (salida UTF-16LE) y se leen esas carpetas como
// una fuente local más — cero configuración del usuario.

#[cfg(windows)]
fn wsl_claude_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.args(["-l", "-q"]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let Ok(o) = cmd.output() else { return out };
    if !o.status.success() {
        return out;
    }
    let u16s: Vec<u16> = o
        .stdout
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(&u16s);
    for distro in text
        .lines()
        .map(|l| l.trim().trim_matches('\0'))
        .filter(|l| !l.is_empty())
    {
        let base = PathBuf::from(format!(r"\\wsl.localhost\{distro}"));
        if let Ok(homes) = fs::read_dir(base.join("home")) {
            for h in homes.flatten() {
                let d = h.path().join(".claude");
                if d.is_dir() {
                    out.push(d);
                }
            }
        }
        let root = base.join("root").join(".claude");
        if root.is_dir() {
            out.push(root);
        }
    }
    out
}

#[cfg(not(windows))]
fn wsl_claude_dirs() -> Vec<PathBuf> {
    Vec::new()
}

// ---------- agregación de logs locales (este PC + WSL) ----------

struct ProjSlot {
    display: Option<String>,
    fallback: String,
    suffix: Option<String>,
    cost: f64,
    tokens: u64,
    by_model: HashMap<String, f64>,
}

#[derive(Default)]
struct LocalAgg {
    seen: HashSet<String>,
    projects: HashMap<String, ProjSlot>, // clave: ruta única de la carpeta
    models: HashMap<String, ModelAgg>,
    daily: HashMap<String, f64>, // YYYY-MM-DD -> USD (últimos 30 días)
    cost_today: f64,
    cost_window: f64,
    tokens_window: u64,
    files: usize,
    deduped: usize,
}

/// Escanea un directorio de proyectos de Claude Code y acumula en `agg`.
/// `suffix` etiqueta el origen ("wsl") en el nombre del proyecto.
/// `window_days` es la ventana del gasto por proyecto (1/7/30…).
fn scan_projects_dir(
    projects_dir: &std::path::Path,
    suffix: Option<&str>,
    now: DateTime<Utc>,
    window_days: u32,
    agg: &mut LocalAgg,
) {
    let day_ago = now - Duration::hours(24);
    let window_ago = now - Duration::days(window_days as i64);
    let month_ago = now - Duration::days(30); // serie diaria de la tendencia
    let Ok(entries) = fs::read_dir(projects_dir) else { return };

    for proj in entries.flatten() {
        if !proj.path().is_dir() {
            continue;
        }
        let raw_dir = proj.file_name().to_string_lossy().to_string();
        let slot_key = proj.path().to_string_lossy().to_string();
        agg.projects.entry(slot_key.clone()).or_insert_with(|| ProjSlot {
            display: None,
            // el nombre de carpeta codificado es ambiguo con los guiones; el
            // nombre bonito sale del `cwd` real de las entradas
            fallback: pretty_project(&raw_dir),
            suffix: suffix.map(String::from),
            cost: 0.0,
            tokens: 0,
            by_model: HashMap::new(),
        });

        let Ok(files) = fs::read_dir(proj.path()) else { continue };
        for f in files.flatten() {
            let path = f.path();
            if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
                continue;
            }
            agg.files += 1;
            let Ok(content) = fs::read_to_string(&path) else { continue };

            for line in content.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
                {
                    let slot = agg.projects.get_mut(&slot_key).unwrap();
                    if slot.display.is_none() {
                        if let Some(base) = v["cwd"].as_str().and_then(|c| {
                            let s = c.replace('\\', "/");
                            s.trim_end_matches('/')
                                .rsplit('/')
                                .next()
                                .filter(|x| !x.is_empty())
                                .map(String::from)
                        }) {
                            slot.display = Some(base);
                        }
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
                if key != ":" && !agg.seen.insert(key) {
                    agg.deduped += 1;
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

                let in_window = ts.map(|t| t >= window_ago).unwrap_or(false);
                let in_day = ts.map(|t| t >= day_ago).unwrap_or(false);

                if in_window {
                    agg.cost_window += cost;
                    // tokens "de trabajo": excluimos cache_read (infla ~100x)
                    agg.tokens_window += inp + out + cw;
                    {
                        let slot = agg.projects.get_mut(&slot_key).unwrap();
                        slot.cost += cost;
                        slot.tokens += inp + out + cw;
                        *slot.by_model.entry(model.clone()).or_insert(0.0) += cost;
                    }
                    let m = agg.models.entry(model.clone()).or_default();
                    m.input += inp;
                    m.output += out;
                    m.cache_write += cw;
                    m.cache_read += cr;
                    m.cost += cost;
                }
                if in_day {
                    agg.cost_today += cost;
                }
                // serie diaria (30 días), independiente de la ventana elegida
                if let Some(t) = ts.filter(|t| *t >= month_ago) {
                    *agg.daily.entry(t.format("%Y-%m-%d").to_string()).or_insert(0.0) += cost;
                }
            }
        }
    }
}

/// Agrega todas las fuentes (este PC + WSL + remotos) para una ventana dada.
fn collect_local_stats(window_days: u32) -> LocalStats {
    let now = Utc::now();
    let mut agg = LocalAgg::default();

    // 1) Este PC
    scan_projects_dir(&claude_dir().join("projects"), None, now, window_days, &mut agg);
    // 2) Distros WSL (si existen): misma máquina, cero configuración
    for d in wsl_claude_dirs() {
        scan_projects_dir(&d.join("projects"), Some("wsl"), now, window_days, &mut agg);
    }

    let projects: Vec<ProjectAgg> = agg
        .projects
        .into_values()
        .filter(|s| s.cost > 0.0 || s.tokens > 0)
        .map(|s| {
            let base = s.display.unwrap_or(s.fallback);
            ProjectAgg {
                name: match s.suffix {
                    Some(x) => format!("{base} · {x}"),
                    None => base,
                },
                cost: s.cost,
                tokens: s.tokens,
                by_model: s.by_model,
            }
        })
        .collect();

    let mut daily_map = agg.daily;
    let mut stats = LocalStats {
        projects,
        models: agg.models,
        cost_today: agg.cost_today,
        cost_week: agg.cost_window,
        tokens_week: agg.tokens_window,
        files_scanned: agg.files,
        entries_deduped: agg.deduped,
        daily: Vec::new(),
    };

    // Fusionar fuentes remotas (si remotes.json existe): totales sumados,
    // proyectos etiquetados con su origen, modelos y serie diaria agregados.
    for r in load_remotes() {
        let Some(remote) = fetch_remote(&r, window_days) else { continue };
        stats.cost_today += remote.cost_today;
        stats.cost_week += remote.cost_week;
        stats.tokens_week += remote.tokens_week;
        stats.files_scanned += remote.files_scanned;
        stats.entries_deduped += remote.entries_deduped;
        for p in remote.projects {
            stats.projects.push(ProjectAgg {
                name: format!("{} · {}", p.name, r.name),
                cost: p.cost,
                tokens: p.tokens,
                by_model: p.by_model,
            });
        }
        for (m, a) in remote.models {
            let e = stats.models.entry(m).or_default();
            e.input += a.input;
            e.output += a.output;
            e.cache_write += a.cache_write;
            e.cache_read += a.cache_read;
            e.cost += a.cost;
        }
        for d in remote.daily {
            *daily_map.entry(d.date).or_insert(0.0) += d.cost;
        }
    }
    stats
        .projects
        .sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));

    let mut daily: Vec<DailyAgg> = daily_map
        .into_iter()
        .map(|(date, cost)| DailyAgg { date, cost })
        .collect();
    daily.sort_by(|a, b| a.date.cmp(&b.date));
    stats.daily = daily;

    stats
}

#[tauri::command]
fn get_local_stats(days: Option<u32>) -> Result<LocalStats, String> {
    Ok(collect_local_stats(days.unwrap_or(7).clamp(1, 90)))
}

/// Exporta los datos agregados a CSV o JSON. `dir` vacío = carpeta Descargas.
/// Devuelve la ruta del archivo escrito.
#[tauri::command]
fn export_data(format: String, dir: Option<String>, days: Option<u32>) -> Result<String, String> {
    let stats = collect_local_stats(days.unwrap_or(7).clamp(1, 90));
    let folder = dir
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(dirs::download_dir)
        .ok_or("ERR_NO_EXPORT_DIR")?;
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let ext = if format == "csv" { "csv" } else { "json" };
    let path = folder.join(format!("claude-code-meter-{stamp}.{ext}"));

    let content = if format == "csv" {
        let mut s = String::from("type,name_or_date,cost_usd,tokens\n");
        for p in &stats.projects {
            s.push_str(&format!(
                "project,{},{:.4},{}\n",
                p.name.replace(',', " "),
                p.cost,
                p.tokens
            ));
        }
        for (m, a) in &stats.models {
            s.push_str(&format!(
                "model,{},{:.4},{}\n",
                m,
                a.cost,
                a.input + a.output + a.cache_write
            ));
        }
        for d in &stats.daily {
            s.push_str(&format!("daily,{},{:.4},\n", d.date, d.cost));
        }
        s
    } else {
        serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?
    };
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// ---------- rectángulo real de la barra de tareas (Win32) ----------

#[cfg(windows)]
mod win_taskbar {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, GWL_EXSTYLE,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    pub struct Taskbar {
        pub rect: RECT,
    }

    /// Lee el rectángulo de Shell_TrayWnd (para apoyar panel y widget encima de la barra).
    pub fn query() -> Option<Taskbar> {
        unsafe {
            let tray = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).ok()?;
            let mut rect = RECT::default();
            GetWindowRect(tray, &mut rect).ok()?;
            Some(Taskbar { rect })
        }
    }

    /// Evita que el widget flotante robe el foco al hacer clic.
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
        // Instancia única: si la app ya corre, el segundo arranque solo enfoca el
        // panel (varias instancias duplicando el polling provocan 429). Debe ser
        // el primer plugin registrado.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_panel(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_quota,
            get_local_stats,
            update_tray,
            get_remotes,
            save_remotes,
            test_remote,
            export_data,
            show_panel,
            set_pill_visible,
            get_pill_visible,
            pill_moved
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_panel" => show_main_panel(app),
            "tray_pill" => set_pill_visible_impl(app, !load_pill_config().visible),
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .setup(|app| {
            // Autoarranque con Windows: solo en builds de release (en dev apuntaría
            // al binario de target/debug) y solo la primera vez — si el usuario lo
            // desactiva después en el Administrador de tareas, se respeta.
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;
                let marker = app_data_dir().join("autostart_configured");
                if !marker.exists() && app.autolaunch().enable().is_ok() {
                    let _ = fs::create_dir_all(app_data_dir());
                    let _ = fs::write(&marker, "1");
                }
            }

            // Menú del tray (clic derecho): abrir panel, salir.
            let tray_menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "tray_panel", "Open panel", true, None::<&str>)?,
                    &MenuItem::with_id(app, "tray_pill", "Floating widget", true, None::<&str>)?,
                    &MenuItem::with_id(app, "tray_quit", "Quit", true, None::<&str>)?,
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

            // Widget flotante: sin robo de foco; se muestra si el usuario lo dejó activo.
            {
                use tauri::Manager;
                if let Some(pill) = app.get_webview_window("pill") {
                    #[cfg(windows)]
                    if let Ok(h) = pill.hwnd() {
                        win_taskbar::make_noactivate(h.0 as isize);
                    }
                    if load_pill_config().visible {
                        position_pill(app.handle());
                        let _ = pill.show();
                    }
                }
            }

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

// ---------- widget flotante (pill): pastilla SIEMPRE visible sobre la barra ----------
// No va DENTRO de la barra (Windows 11 lo impide y un overlay tapa los iconos);
// vive justo encima, arrastrable, y recuerda posición y visibilidad.

#[derive(Serialize, Deserialize, Default)]
struct PillConfig {
    visible: bool,
    x: Option<i32>,
    y: Option<i32>,
}

fn pill_config_path() -> PathBuf {
    app_data_dir().join("pill_config.json")
}

fn load_pill_config() -> PillConfig {
    fs::read_to_string(pill_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_pill_config(c: &PillConfig) {
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(s) = serde_json::to_string_pretty(c) {
        let _ = fs::write(pill_config_path(), s);
    }
}

/// Coloca el widget: posición guardada, o esquina inferior derecha encima de la barra.
fn position_pill(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(pill) = app.get_webview_window("pill") else { return };
    let cfg = load_pill_config();
    if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
        let _ = pill.set_position(tauri::PhysicalPosition::new(x, y));
        return;
    }
    if let (Ok(Some(mon)), Ok(size)) = (pill.current_monitor(), pill.outer_size()) {
        let s = mon.size();
        let x = s.width.saturating_sub(size.width + MARGIN_X) as i32;
        #[allow(unused_mut)]
        let mut y = s.height.saturating_sub(size.height + TASKBAR_H + PANEL_GAP) as i32;
        #[cfg(windows)]
        if let Some(tb) = win_taskbar::query() {
            y = (tb.rect.top - size.height as i32 - PANEL_GAP as i32).max(0);
        }
        let _ = pill.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn set_pill_visible_impl(app: &tauri::AppHandle, visible: bool) {
    use tauri::Manager;
    let mut cfg = load_pill_config();
    cfg.visible = visible;
    save_pill_config(&cfg);
    if let Some(pill) = app.get_webview_window("pill") {
        if visible {
            position_pill(app);
            let _ = pill.set_always_on_top(true);
            let _ = pill.show();
        } else {
            let _ = pill.hide();
        }
    }
}

#[tauri::command]
fn set_pill_visible(app: tauri::AppHandle, visible: bool) {
    set_pill_visible_impl(&app, visible);
}

#[tauri::command]
fn get_pill_visible() -> bool {
    load_pill_config().visible
}

/// El widget avisa tras un arrastre; persistimos su nueva posición.
#[tauri::command]
fn pill_moved(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(pill) = app.get_webview_window("pill") {
        if let Ok(p) = pill.outer_position() {
            let mut cfg = load_pill_config();
            cfg.x = Some(p.x);
            cfg.y = Some(p.y);
            save_pill_config(&cfg);
        }
    }
}

/// Abre el panel (clic en el widget flotante).
#[tauri::command]
fn show_panel(app: tauri::AppHandle) {
    show_main_panel(&app);
}

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

