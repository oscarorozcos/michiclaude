// MichiClaude — backend
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
const APP_IDENTIFIER: &str = "com.oscarorozco.michiclaude";

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
        .header("user-agent", "michiclaude/0.1.0")
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
    // la app:  %APPDATA%\com.oscarorozco.michiclaude\quota_debug.json
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
    /// El modelo no aparece en ninguna tabla de precios ni es una familia
    /// conocida: su coste sale de la tarifa por defecto y la UI lo marca como
    /// estimación en vez de darlo por firme.
    #[serde(default)]
    estimated: bool,
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
/// con API key es gasto real.
///
/// La tarifa depende de la VERSIÓN, no solo de la familia: Opus bajó de
/// $15/$75 a $5/$25 a partir de la 4.5 (las 3, 4.0 y 4.1 se quedaron en la
/// tarifa vieja). Por eso se lee el número de versión del id — así una
/// versión nueva de una familia conocida hereda la tarifa correcta sola.
/// Verificado contra la doc oficial el 2026-07-26; hasta entonces se cobraba
/// $15/$75 a todo Opus/Fable (tarifa del difunto Opus 4.1), lo que inflaba
/// los costes de Opus ~3x. Esta tabla es solo el RESPALDO: la fuente
/// preferente serán los precios descargados (ver pendiente en CLAUDE.md).
///
/// La escritura de caché cuesta 1.25x el input y la lectura 0.1x en todos
/// los modelos, así que se derivan en vez de repetirse.
fn price_table(model: &str) -> (f64, f64, f64, f64) {
    let m = model.to_lowercase();
    // versión del id, ignorando la fecha del snapshot (8 dígitos)
    let mut nums = m
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty() && t.len() != 8)
        .filter_map(|t| t.parse::<u32>().ok());
    let major = nums.next().unwrap_or(0);
    let minor = nums.next().unwrap_or(0);

    let (inp, out) = if m.contains("fable") || m.contains("mythos") {
        (10.0, 50.0)
    } else if m.contains("opus") {
        if major > 4 || (major == 4 && minor >= 5) {
            (5.0, 25.0)
        } else {
            (15.0, 75.0) // Opus 3 / 4.0 / 4.1
        }
    } else if m.contains("haiku") {
        (1.0, 5.0)
    } else {
        (3.0, 15.0) // sonnet y desconocidos
    };
    (inp, out, inp * 1.25, inp * 0.1)
}

// ---------- precios dinámicos (cascada de fuentes públicas) ----------
// Anthropic NO publica sus tarifas en ningún endpoint (verificado 2026-07-26:
// /v1/models expone capacidades y límites, jamás precios), así que se descargan
// de las tablas públicas de la comunidad, en cascada por confiabilidad:
//   1) LiteLLM    — el estándar de facto (es lo que usa ccusage); se actualiza
//                   el día del lanzamiento e incluye tarifas introductorias.
//   2) models.dev — open source, esquema más limpio, comunidad menor.
//   3) OpenRouter — los datos más frescos porque facturan con ellos, pero es
//                   una empresa comercial: último recurso de red.
// Si las tres fallan se usa el caché en disco y, en último término, la tabla
// embebida `price_table()`. Es cascada de RESPALDO, no verificación cruzada:
// en cuanto una fuente responde con datos, se para.
//
// PRIVACIDAD: son GET anónimos a un JSON público. No viaja NADA del usuario
// (ni token, ni ids, ni telemetría) y el usuario puede apagarlo en Preferencias.

const LITELLM_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const MODELSDEV_URL: &str = "https://models.dev/api.json";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/models";

#[derive(Serialize, Deserialize, Clone, Copy)]
struct PriceEntry {
    input: f64,
    output: f64,
    cache_write: f64,
    cache_read: f64,
}

#[derive(Serialize, Deserialize, Default)]
struct PricesCache {
    #[serde(default)]
    fetched_at: String,
    #[serde(default)]
    source: String,
    /// Último INTENTO, haya salido bien o mal. Permite distinguir "aún no se
    /// ha intentado" de "se intentó y no hubo red" (firewall corporativo,
    /// proxy…) en vez de dejar al usuario con un mensaje ambiguo.
    #[serde(default)]
    last_try: String,
    #[serde(default)]
    prices: HashMap<String, PriceEntry>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct PricesConfig {
    /// Descarga automática cada 24 h (apagable desde Preferencias).
    #[serde(default = "default_true")]
    auto: bool,
    /// URLs configurables: si faltan se usan las constantes de arriba.
    #[serde(default)]
    litellm_url: Option<String>,
    #[serde(default)]
    modelsdev_url: Option<String>,
    #[serde(default)]
    openrouter_url: Option<String>,
}

impl Default for PricesConfig {
    fn default() -> Self {
        Self {
            auto: true,
            litellm_url: None,
            modelsdev_url: None,
            openrouter_url: None,
        }
    }
}

fn prices_config_path() -> PathBuf {
    app_data_dir().join("prices_config.json")
}

fn prices_cache_path() -> PathBuf {
    app_data_dir().join("prices_cache.json")
}

fn load_prices_config() -> PricesConfig {
    fs::read_to_string(prices_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn load_prices_cache() -> PricesCache {
    fs::read_to_string(prices_cache_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Tabla viva en memoria; se carga del caché al primer uso y la refresca el
/// hilo de descarga. Vacía = solo tabla embebida.
static PRICES: std::sync::OnceLock<std::sync::RwLock<HashMap<String, PriceEntry>>> =
    std::sync::OnceLock::new();

fn prices_map() -> &'static std::sync::RwLock<HashMap<String, PriceEntry>> {
    PRICES.get_or_init(|| std::sync::RwLock::new(load_prices_cache().prices))
}

/// Clave normalizada para casar el id del log con el de las tablas públicas:
/// minúsculas, sin prefijo de proveedor ("anthropic/"), sin variante entre
/// corchetes ("[1m]") y sin la fecha del snapshot final.
fn price_key(model: &str) -> String {
    let lower = model.to_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    let base = base.split('[').next().unwrap_or(base);
    let mut s = base.trim().to_string();
    if let Some(i) = s.rfind('-') {
        let tail = &s[i + 1..];
        if tail.len() == 8 && tail.chars().all(|c| c.is_ascii_digit()) {
            s.truncate(i);
        }
    }
    s
}

fn price_lookup(model: &str) -> Option<PriceEntry> {
    let key = price_key(model);
    let map = prices_map().read().ok()?;
    map.get(&key).copied()
}

/// ¿La familia del modelo es una que la tabla embebida sabe cobrar? Si no,
/// el coste sale de la tarifa por defecto (Sonnet) y la UI lo marca como
/// estimación en vez de presentarlo como un dato firme.
fn family_known(model: &str) -> bool {
    let m = model.to_lowercase();
    ["fable", "mythos", "opus", "haiku", "sonnet"]
        .iter()
        .any(|f| m.contains(f))
}

fn price_is_estimated(model: &str) -> bool {
    price_lookup(model).is_none() && !family_known(model)
}

/// LiteLLM: mapa plano id -> { litellm_provider, *_cost_per_token }.
/// Los precios vienen POR TOKEN; aquí se pasan a USD por millón.
fn parse_litellm(v: &serde_json::Value) -> HashMap<String, PriceEntry> {
    let mut out = HashMap::new();
    let Some(obj) = v.as_object() else { return out };
    for (k, m) in obj {
        if m["litellm_provider"].as_str() != Some("anthropic") {
            continue;
        }
        let inp = m["input_cost_per_token"].as_f64().unwrap_or(0.0) * 1e6;
        let outp = m["output_cost_per_token"].as_f64().unwrap_or(0.0) * 1e6;
        if inp <= 0.0 || outp <= 0.0 {
            continue;
        }
        out.insert(
            price_key(k),
            PriceEntry {
                input: inp,
                output: outp,
                cache_write: m["cache_creation_input_token_cost"]
                    .as_f64()
                    .map(|x| x * 1e6)
                    .unwrap_or(inp * 1.25),
                cache_read: m["cache_read_input_token_cost"]
                    .as_f64()
                    .map(|x| x * 1e6)
                    .unwrap_or(inp * 0.1),
            },
        );
    }
    out
}

/// models.dev: { "anthropic": { "models": { id: { cost: {...} } } } }.
/// Ya viene en USD por millón.
fn parse_modelsdev(v: &serde_json::Value) -> HashMap<String, PriceEntry> {
    let mut out = HashMap::new();
    let Some(models) = v["anthropic"]["models"].as_object() else { return out };
    for (k, m) in models {
        let c = &m["cost"];
        let inp = c["input"].as_f64().unwrap_or(0.0);
        let outp = c["output"].as_f64().unwrap_or(0.0);
        if inp <= 0.0 || outp <= 0.0 {
            continue;
        }
        out.insert(
            price_key(k),
            PriceEntry {
                input: inp,
                output: outp,
                cache_write: c["cache_write"].as_f64().unwrap_or(inp * 1.25),
                cache_read: c["cache_read"].as_f64().unwrap_or(inp * 0.1),
            },
        );
    }
    out
}

/// OpenRouter: { "data": [ { id: "anthropic/...", pricing: { ... } } ] }.
/// Los precios son cadenas y por token.
fn parse_openrouter(v: &serde_json::Value) -> HashMap<String, PriceEntry> {
    let num = |x: &serde_json::Value| -> Option<f64> {
        x.as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| x.as_f64())
    };
    let mut out = HashMap::new();
    let Some(arr) = v["data"].as_array() else { return out };
    for it in arr {
        let Some(id) = it["id"].as_str() else { continue };
        if !id.starts_with("anthropic/") {
            continue;
        }
        let p = &it["pricing"];
        let (Some(inp), Some(outp)) = (num(&p["prompt"]), num(&p["completion"])) else {
            continue;
        };
        let (inp, outp) = (inp * 1e6, outp * 1e6);
        if inp <= 0.0 || outp <= 0.0 {
            continue;
        }
        out.insert(
            price_key(id),
            PriceEntry {
                input: inp,
                output: outp,
                cache_write: num(&p["input_cache_write"])
                    .map(|x| x * 1e6)
                    .unwrap_or(inp * 1.25),
                cache_read: num(&p["input_cache_read"])
                    .map(|x| x * 1e6)
                    .unwrap_or(inp * 0.1),
            },
        );
    }
    out
}

/// Recorre la cascada y devuelve (fuente, precios) de la PRIMERA que responda
/// con datos. Nunca propaga errores: sin red, simplemente no hay actualización.
async fn fetch_prices(cfg: &PricesConfig) -> Option<(String, HashMap<String, PriceEntry>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok()?;
    let sources: [(&str, String); 3] = [
        (
            "litellm",
            cfg.litellm_url.clone().unwrap_or(LITELLM_URL.into()),
        ),
        (
            "models.dev",
            cfg.modelsdev_url.clone().unwrap_or(MODELSDEV_URL.into()),
        ),
        (
            "openrouter",
            cfg.openrouter_url.clone().unwrap_or(OPENROUTER_URL.into()),
        ),
    ];
    for (name, url) in sources {
        let Ok(resp) = client
            .get(&url)
            .header("user-agent", "michiclaude/0.1.0")
            .send()
            .await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(v) = resp.json::<serde_json::Value>().await else {
            continue;
        };
        let map = match name {
            "litellm" => parse_litellm(&v),
            "models.dev" => parse_modelsdev(&v),
            _ => parse_openrouter(&v),
        };
        if !map.is_empty() {
            return Some((name.to_string(), map));
        }
    }
    None
}

fn save_prices_cache(c: &PricesCache) {
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(s) = serde_json::to_string_pretty(c) {
        let _ = fs::write(prices_cache_path(), s);
    }
}

/// Refresca los precios si toca (o si `force`). Devuelve la fuente usada.
/// Con `app` avisa al panel (`prices:updated`) para que repinte el estado y
/// recalcule los costes al instante, sin esperar al siguiente ciclo.
async fn refresh_prices(app: Option<tauri::AppHandle>, force: bool) -> Option<String> {
    let cfg = load_prices_config();
    if !cfg.auto && !force {
        return None;
    }
    let mut cached = load_prices_cache();
    if !force {
        // caché de menos de 24 h: nada que hacer
        if let Ok(t) = DateTime::parse_from_rfc3339(&cached.fetched_at) {
            if Utc::now().signed_duration_since(t.with_timezone(&Utc)) < Duration::hours(24) {
                return None;
            }
        }
    }
    let now = Utc::now().to_rfc3339();
    match fetch_prices(&cfg).await {
        Some((source, prices)) => {
            if let Ok(mut w) = prices_map().write() {
                *w = prices.clone();
            }
            save_prices_cache(&PricesCache {
                fetched_at: now.clone(),
                source: source.clone(),
                last_try: now,
                prices,
            });
            if let Some(a) = app {
                use tauri::Emitter;
                let _ = a.emit("prices:updated", serde_json::json!({"ok": true}));
            }
            Some(source)
        }
        None => {
            // sin red: se conserva lo que hubiera y solo se anota el intento,
            // para poder decirle al usuario que se intentó y no se pudo
            cached.last_try = now;
            save_prices_cache(&cached);
            // se avisa igual: el panel muestra el aviso junto a los costes sin
            // esperar a que el usuario entre en Preferencias
            if let Some(a) = app {
                use tauri::Emitter;
                let _ = a.emit("prices:updated", serde_json::json!({"ok": false}));
            }
            None
        }
    }
}

/// Estado para Preferencias: si está activo, de dónde salieron los precios,
/// cuándo y cuántos modelos se conocen.
#[tauri::command]
fn get_prices_status() -> serde_json::Value {
    let cfg = load_prices_config();
    let cache = load_prices_cache();
    serde_json::json!({
        "auto": cfg.auto,
        "source": cache.source,
        "fetched_at": cache.fetched_at,
        "last_try": cache.last_try,
        "count": cache.prices.len(),
    })
}

#[tauri::command]
fn set_prices_auto(app: tauri::AppHandle, auto: bool) -> Result<(), String> {
    let mut cfg = load_prices_config();
    cfg.auto = auto;
    let dir = app_data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let s = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(prices_config_path(), s).map_err(|e| e.to_string())?;
    // al reactivarlo, descargar ya: esperar al ciclo de 6 h se vería como que
    // el interruptor no hizo nada
    if auto {
        tauri::async_runtime::spawn(async move {
            let _ = refresh_prices(Some(app), true).await;
        });
    }
    Ok(())
}

/// Botón "actualizar ahora": ignora la ventana de 24 h y el interruptor.
#[tauri::command]
async fn refresh_prices_now(app: tauri::AppHandle) -> Result<String, String> {
    refresh_prices(Some(app), true)
        .await
        .ok_or_else(|| "ERR_PRICES_FETCH".to_string())
}

fn cost_of(model: &str, inp: u64, out: u64, cw: u64, cr: u64) -> f64 {
    let (pi, po, pcw, pcr) = price_for(model);
    (inp as f64 * pi + out as f64 * po + cw as f64 * pcw + cr as f64 * pcr) / 1_000_000.0
}

/// Precio efectivo: primero la tabla descargada, si no la embebida.
fn price_for(model: &str) -> (f64, f64, f64, f64) {
    match price_lookup(model) {
        Some(p) => (p.input, p.output, p.cache_write, p.cache_read),
        None => price_table(model),
    }
}

// ---------- fuentes remotas opcionales (otras máquinas vía SSH) ----------
// %APPDATA%\com.oscarorozco.michiclaude\remotes.json:
//   { "remotes": [ { "name": "vps", "host": "<alias ssh>",
//       "command": "python3 /opt/projects/michiclaude/scripts/meter-export.py" } ] }
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

/// Ruta canónica del exportador en el servidor. Solo se reinstala en los
/// remotos que apunten aquí: si el usuario puso su propia ruta, no se le
/// escribe nada en su máquina.
const REMOTE_SCRIPT_PATH: &str = "~/.michiclaude/meter-export.py";

/// El exportador viaja DENTRO del binario. Así el usuario no tiene que
/// copiarlo a mano ni saber dónde está, y cada actualización de MichiClaude
/// lo mantiene en sincronía con el backend (invariante 1).
const REMOTE_SCRIPT: &str = include_str!("../../scripts/meter-export.py");

/// Sube el exportador al servidor por SSH (lo escribe desde stdin, sin
/// necesitar scp ni permisos extra). Idempotente: sobrescribe siempre.
fn upload_exporter(host: &str) -> Result<(), String> {
    use std::io::Write;
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg(host)
        .arg("mkdir -p ~/.michiclaude && cat > ~/.michiclaude/meter-export.py && chmod +x ~/.michiclaude/meter-export.py")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("No pude ejecutar ssh: {e}"))?;
    if let Some(mut si) = child.stdin.take() {
        si.write_all(REMOTE_SCRIPT.as_bytes())
            .map_err(|e| format!("No pude enviar el script: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .trim()
            .chars()
            .take(200)
            .collect())
    }
}

/// Busca en el servidor un Python que sirva (3.7+, que es lo que usa el
/// exportador). Sin esto, un servidor sin `python3` se daba de alta como
/// "conectado" y luego no aparecía ningún dato: un fallo silencioso que el
/// usuario no tenía forma de diagnosticar.
fn detect_python(host: &str) -> Option<String> {
    let probe = "for p in python3 python3.13 python3.12 python3.11 python3.10 \
python3.9 python3.8 python3.7 python; do \
if command -v \"$p\" >/dev/null 2>&1 && \"$p\" -c \
'import sys;raise SystemExit(0 if sys.version_info>=(3,7) else 1)' 2>/dev/null; \
then echo \"$p\"; exit 0; fi; done; exit 1";
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg(host)
        .arg(probe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let found = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if found.is_empty() { None } else { Some(found) }
}

/// Alta de servidor: busca Python, deja el exportador instalado y devuelve el
/// comando ya resuelto para guardarlo.
#[tauri::command]
fn install_remote(host: String) -> Result<String, String> {
    let py = detect_python(&host).ok_or_else(|| "ERR_NO_PYTHON".to_string())?;
    upload_exporter(&host)?;
    Ok(format!("{py} {REMOTE_SCRIPT_PATH}"))
}

/// Ejecuta el exportador remoto por SSH. BatchMode: jamás pide contraseña
/// (requiere llave configurada, la misma que usa VS Code Remote-SSH).
fn fetch_remote(r: &RemoteSource, window_days: u32) -> Option<LocalStats> {
    use std::io::Write;
    // Los precios frescos se le pasan al exportador por stdin: así hay UNA sola
    // fuente de verdad y su tabla embebida queda solo como respaldo. Un
    // exportador viejo ignora el flag desconocido y sigue funcionando.
    let prices = prices_map()
        .read()
        .ok()
        .filter(|m| !m.is_empty())
        .and_then(|m| serde_json::to_string(&*m).ok());

    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(&r.host)
        .arg(format!(
            "{} --days {}{}",
            r.command,
            window_days,
            if prices.is_some() { " --prices-stdin" } else { "" }
        ))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sin flash de consola
    }
    let mut child = cmd.spawn().ok()?;
    if let Some(mut si) = child.stdin.take() {
        if let Some(json) = &prices {
            let _ = si.write_all(json.as_bytes());
        }
        // cerrar stdin siempre: si no, el exportador se queda esperando
    }
    let out = child.wait_with_output().ok()?;
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
// ---------- caché de escaneo (lectura incremental) ----------
// Releer todos los .jsonl en cada ciclo cuesta cada vez más porque el historial
// solo crece. Dos ideas, ninguna cambia un número (verificadas en el exportador
// contra una copia congelada de los logs: salida idéntica byte a byte):
//   1) Todos los agregados están acotados en el tiempo (ventana elegida, 24 h,
//      30 días de tendencia): un archivo cuya última escritura sea anterior a
//      la ventana más amplia no puede aportar nada y se salta sin abrirlo.
//   2) De los recientes se cachea el PARSEO indexado por tamaño+mtime.
// Se guardan tokens y timestamp, nunca el coste: así un cambio de precios se
// aplica a todo el historial al instante. Es un caché reconstruible — si se
// borra o no se entiende, se recalcula desde los logs.
const SCAN_CACHE_VERSION: u32 = 1;
const SCAN_SKIP_MARGIN_DAYS: i64 = 2;

/// Entrada ya parseada. Nombres cortos: se serializan miles.
#[derive(Serialize, Deserialize, Clone)]
struct CachedEntry {
    #[serde(rename = "t")]
    ts: Option<i64>, // epoch en segundos
    #[serde(rename = "m")]
    model: String,
    #[serde(rename = "i")]
    inp: u64,
    #[serde(rename = "o")]
    out: u64,
    #[serde(rename = "w")]
    cw: u64,
    #[serde(rename = "r")]
    cr: u64,
    #[serde(rename = "k")]
    key: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct CachedFile {
    len: u64,
    mtime: i64,
    display: Option<String>,
    entries: Vec<CachedEntry>,
    /// Duplicados internos ya descartados, para que el contador de
    /// diagnóstico siga cuadrando con el escaneo completo.
    #[serde(default)]
    dups: usize,
}

#[derive(Serialize, Deserialize, Default)]
struct ScanCache {
    #[serde(default)]
    version: u32,
    /// Hasta dónde atrás retiene entradas este caché (epoch). Si una ejecución
    /// necesita más historial (ventana mayor), se descarta y se reconstruye.
    #[serde(default)]
    retained_from: i64,
    #[serde(default)]
    files: HashMap<String, CachedFile>,
}

fn scan_cache_path() -> PathBuf {
    app_data_dir().join("scan_cache.json")
}

fn load_scan_cache(need_from: i64) -> HashMap<String, CachedFile> {
    fs::read_to_string(scan_cache_path())
        .ok()
        .and_then(|s| serde_json::from_str::<ScanCache>(&s).ok())
        .filter(|c| c.version == SCAN_CACHE_VERSION && c.retained_from <= need_from)
        .map(|c| c.files)
        .unwrap_or_default()
}

fn save_scan_cache(files: HashMap<String, CachedFile>, retained_from: i64) {
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(s) = serde_json::to_string(&ScanCache {
        version: SCAN_CACHE_VERSION,
        retained_from,
        files,
    }) {
        let _ = fs::write(scan_cache_path(), s);
    }
}

/// Parsea un .jsonl a entradas compactas, deduplicando dentro del archivo.
/// Devuelve (nombre del cwd, entradas, duplicados internos).
fn parse_jsonl_file(
    path: &std::path::Path,
    keep_after: i64,
) -> (Option<String>, Vec<CachedEntry>, usize) {
    let Ok(content) = fs::read_to_string(path) else {
        return (None, Vec::new(), 0);
    };
    let mut display: Option<String> = None;
    let mut entries = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut dups = 0usize;

    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if display.is_none() {
            if let Some(base) = v["cwd"].as_str().and_then(|c| {
                let s = c.replace('\\', "/");
                s.trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .filter(|x| !x.is_empty())
                    .map(String::from)
            }) {
                display = Some(base);
            }
        }
        let msg = &v["message"];
        let usage = &msg["usage"];
        if !usage.is_object() {
            continue;
        }
        // Deduplicación: mismo message.id + requestId = misma petición
        let key = format!(
            "{}:{}",
            msg["id"].as_str().unwrap_or(""),
            v["requestId"].as_str().unwrap_or("")
        );
        if key != ":" && !seen.insert(key.clone()) {
            dups += 1;
            continue;
        }
        let model = msg["model"].as_str().unwrap_or("unknown").to_string();
        // "<synthetic>" = placeholders de error de Claude Code, no son peticiones
        if model == "<synthetic>" {
            continue;
        }
        let ts = v["timestamp"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc).timestamp());
        // fuera de toda ventana posible: ni se guarda
        if ts.map(|t| t < keep_after).unwrap_or(false) {
            continue;
        }
        entries.push(CachedEntry {
            ts,
            model,
            inp: usage["input_tokens"].as_u64().unwrap_or(0),
            out: usage["output_tokens"].as_u64().unwrap_or(0),
            cw: usage["cache_creation_input_tokens"].as_u64().unwrap_or(0),
            cr: usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
            key,
        });
    }
    (display, entries, dups)
}

fn scan_projects_dir(
    projects_dir: &std::path::Path,
    suffix: Option<&str>,
    now: DateTime<Utc>,
    window_days: u32,
    agg: &mut LocalAgg,
    cache_in: &HashMap<String, CachedFile>,
    cache_out: &mut HashMap<String, CachedFile>,
) {
    let day_ago = now - Duration::hours(24);
    let window_ago = now - Duration::days(window_days as i64);
    let month_ago = now - Duration::days(30); // serie diaria de la tendencia
    // ventana más amplia de esta ejecución: la elegida o los 30 días de la
    // tendencia. Nada anterior entra en ningún cálculo.
    let keep_after = (now
        - Duration::days(window_days.max(30) as i64 + SCAN_SKIP_MARGIN_DAYS))
        .timestamp();
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
            let Ok(meta) = f.metadata() else { continue };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            // (1) escrito antes de la ventana más amplia: no puede aportar nada
            if mtime != 0 && mtime < keep_after {
                continue;
            }
            agg.files += 1;

            // (2) ¿sigue igual que la última vez? entonces se reutiliza el parseo
            let fkey = path.to_string_lossy().to_string();
            let cached = cache_in
                .get(&fkey)
                .filter(|c| c.len == meta.len() && c.mtime == mtime)
                .cloned();
            let entry = match cached {
                Some(c) => c,
                None => {
                    let (display, entries, dups) = parse_jsonl_file(&path, keep_after);
                    CachedFile { len: meta.len(), mtime, display, entries, dups }
                }
            };

            {
                let slot = agg.projects.get_mut(&slot_key).unwrap();
                if slot.display.is_none() {
                    slot.display = entry.display.clone();
                }
            }
            agg.deduped += entry.dups; // los internos; los cruzados, abajo

            // (3) agregación: siempre con los precios y la ventana de AHORA
            for e in &entry.entries {
                if e.key != ":" && !agg.seen.insert(e.key.clone()) {
                    agg.deduped += 1;
                    continue;
                }
                let Some(ts_s) = e.ts else { continue };
                let Some(ts) = DateTime::from_timestamp(ts_s, 0) else { continue };
                let cost = cost_of(&e.model, e.inp, e.out, e.cw, e.cr);

                if ts >= window_ago {
                    agg.cost_window += cost;
                    // tokens "de trabajo": excluimos cache_read (infla ~100x)
                    agg.tokens_window += e.inp + e.out + e.cw;
                    {
                        let slot = agg.projects.get_mut(&slot_key).unwrap();
                        slot.cost += cost;
                        slot.tokens += e.inp + e.out + e.cw;
                        *slot.by_model.entry(e.model.clone()).or_insert(0.0) += cost;
                    }
                    let m = agg.models.entry(e.model.clone()).or_default();
                    m.input += e.inp;
                    m.output += e.out;
                    m.cache_write += e.cw;
                    m.cache_read += e.cr;
                    m.cost += cost;
                    m.estimated = price_is_estimated(&e.model);
                }
                if ts >= day_ago {
                    agg.cost_today += cost;
                }
                // serie diaria (30 días), independiente de la ventana elegida
                if ts >= month_ago {
                    *agg.daily.entry(ts.format("%Y-%m-%d").to_string()).or_insert(0.0) += cost;
                }
            }
            cache_out.insert(fkey, entry);
        }
    }
}

/// Agrega todas las fuentes (este PC + WSL + remotos) para una ventana dada.
fn collect_local_stats(window_days: u32) -> LocalStats {
    let now = Utc::now();
    let mut agg = LocalAgg::default();
    // Caché de parseo compartido por todas las fuentes locales. Si esta
    // ejecución necesita más historial del guardado, load_scan_cache lo
    // descarta y se reconstruye en vez de devolver de menos.
    let keep_after = (now
        - Duration::days(window_days.max(30) as i64 + SCAN_SKIP_MARGIN_DAYS))
        .timestamp();
    let cache_in = load_scan_cache(keep_after);
    let mut cache_out: HashMap<String, CachedFile> = HashMap::new();

    // 1) Este PC
    scan_projects_dir(
        &claude_dir().join("projects"), None, now, window_days, &mut agg,
        &cache_in, &mut cache_out,
    );
    // 2) Distros WSL (si existen): misma máquina, cero configuración
    for d in wsl_claude_dirs() {
        scan_projects_dir(
            &d.join("projects"), Some("wsl"), now, window_days, &mut agg,
            &cache_in, &mut cache_out,
        );
    }
    // solo lo visto en esta pasada: los archivos borrados o ya fuera de
    // ventana desaparecen del caché por sí solos
    save_scan_cache(cache_out, keep_after);

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
            e.estimated = e.estimated || a.estimated;
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
    let path = folder.join(format!("michiclaude-{stamp}.{ext}"));

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
        FindWindowW, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, SetWindowPos,
        GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
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

    /// Vuelve a poner la ventana al frente, SIN pasar por Tauri.
    ///
    /// Por qué hace falta: `set_always_on_top(true)` de Tauri no llega al
    /// sistema si su estado interno ya dice "esta ventana es topmost". Windows
    /// puede degradarla por su cuenta (otra app se activa, cambia el
    /// escritorio, se conecta un monitor), Tauri no se entera y las
    /// re-afirmaciones se convierten en no-ops: el gatito se queda detrás para
    /// siempre. SetWindowPos con HWND_TOPMOST siempre reinserta la ventana
    /// arriba de la banda topmost, esté como esté el estado cacheado.
    /// SWP_NOACTIVATE es imprescindible: el widget nunca debe robar el foco.
    pub fn force_topmost(hwnd_raw: isize) {
        unsafe {
            let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
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
            install_remote,
            export_data,
            show_panel,
            set_pill_visible,
            get_pill_visible,
            get_pill_style,
            set_pill_style,
            toggle_pill_card,
            is_dev,
            get_pill_layer,
            set_pill_layer,
            get_prices_status,
            set_prices_auto,
            refresh_prices_now,
            hover_card,
            set_notif_visible,
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
                .tooltip("MichiClaude")
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
                }
                // ni el gatito ni sus globos (hover/notificación) roban foco
                for label in ["cat", "card", "notif", "pcard"] {
                    if let Some(w) = app.get_webview_window(label) {
                        #[cfg(windows)]
                        if let Ok(h) = w.hwnd() {
                            win_taskbar::make_noactivate(h.0 as isize);
                        }
                    }
                }
                if load_pill_config().visible {
                    set_pill_visible_impl(app.handle(), true);
                }
            }

            // El exportador viaja dentro del binario, así que tras cada
            // actualización de MichiClaude hay que refrescarlo en los
            // servidores o quedaría desincronizado con el backend. Solo se
            // toca a los que usan NUESTRA ruta: si el usuario puso la suya,
            // no se le escribe nada. En hilo aparte y en silencio: es una
            // comodidad, no debe entorpecer el arranque ni molestar si el
            // servidor está apagado.
            std::thread::spawn(|| {
                for r in load_remotes() {
                    if r.command.contains(REMOTE_SCRIPT_PATH) {
                        let _ = upload_exporter(&r.host);
                    }
                }
            });

            // Precios: intento al arrancar y luego cada 6 h (refresh_prices ya
            // respeta la ventana de 24 h del caché y el interruptor del
            // usuario). En hilo aparte para no retrasar el arranque ni
            // bloquear las estadísticas si la red va lenta.
            {
                let h = app.handle().clone();
                std::thread::spawn(move || loop {
                    let _ = tauri::async_runtime::block_on(refresh_prices(Some(h.clone()), false));
                    std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                });
            }

            // "Curación" continua de la capa: Windows degrada el always-on-top
            // cuando se activan otras apps, y el gatito o sus globos quedaban
            // tapados hasta el siguiente ciclo de cuota (3 min). Se re-afirma
            // cada 2 s, pero SOLO en modo "top": en "normal"/"bottom" volver a
            // aplicarla re-elevaría la ventana y estorbaría al usuario.
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let cfg = load_pill_config();
                    if !cfg.visible || cfg.layer == "normal" || cfg.layer == "bottom" {
                        continue;
                    }
                    // con el panel abierto no se re-eleva nada: el gatito
                    // taparía el panel (también es alwaysOnTop)
                    use tauri::Manager;
                    let panel_open = handle
                        .get_webview_window("main")
                        .and_then(|w| w.is_visible().ok())
                        .unwrap_or(false);
                    if !panel_open {
                        reassert_layers(&handle);
                    }
                });
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
            // El widget visible (pastilla o gatito) persiste su posición al
            // arrastrarse; ambos comparten la misma posición guardada.
            tauri::WindowEvent::Moved(_) => {
                if matches!(window.label(), "pill" | "cat")
                    && window.is_visible().unwrap_or(false)
                {
                    if let Ok(p) = window.outer_position() {
                        let mut cfg = load_pill_config();
                        cfg.x = Some(p.x);
                        cfg.y = Some(p.y);
                        save_pill_config(&cfg);
                    }
                    // el globo acompaña al widget al arrastrarlo, sea el
                    // gatito o la pastilla, y se recoloca solo si el nuevo
                    // sitio no le deja espacio (misma lógica de pose)
                    let visible = window
                        .app_handle()
                        .get_webview_window("notif")
                        .and_then(|n| n.is_visible().ok())
                        .unwrap_or(false);
                    if visible {
                        let cfg = load_pill_config();
                        place_balloon(
                            window.app_handle(),
                            "notif",
                            notif_dx(&cfg),
                            notif_overlap(&cfg),
                        );
                    }
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error al iniciar MichiClaude");
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
    // "plain" (solo pastilla) o "cat" (gatito animado encima de la pastilla).
    #[serde(default)]
    style: String,
    // capa en pantalla: "top" (siempre al frente, default) | "normal" |
    // "bottom" (pegado al fondo del escritorio, estilo Rainmeter).
    #[serde(default)]
    layer: String,
}

/// Aplica la capa elegida a una ventana del widget. Windows a veces
/// "degrada" el siempre-visible, así que esto se re-afirma periódicamente
/// (update_tray) y cada vez que un globo aparece — no solo al mostrar.
fn apply_layer(w: &tauri::WebviewWindow, layer: &str) {
    match layer {
        "bottom" => {
            let _ = w.set_always_on_top(false);
            let _ = w.set_always_on_bottom(true);
        }
        "normal" => {
            let _ = w.set_always_on_bottom(false);
            let _ = w.set_always_on_top(false);
        }
        _ => {
            let _ = w.set_always_on_bottom(false);
            let _ = w.set_always_on_top(true);
            // ...y además por Win32 directo, porque la llamada de Tauri se
            // ignora si su estado interno ya cree que la ventana es topmost
            // (Windows la degrada sin avisarle). Ver force_topmost().
            #[cfg(windows)]
            if let Ok(h) = w.hwnd() {
                win_taskbar::force_topmost(h.0 as isize);
            }
        }
    }
}

/// Re-afirma la capa elegida en TODAS las ventanas del widget: el gatito (o
/// la pastilla) y sus globos de información y de alarma van siempre en la
/// misma capa — el usuario espera que se comporten como una sola pieza.
/// El orden importa: la última en aplicarse queda arriba dentro de su capa,
/// así que los globos se re-elevan por encima del gatito.
fn reassert_layers(app: &tauri::AppHandle) {
    use tauri::Manager;
    let cfg = load_pill_config();
    let widget = if cfg.style == "cat" { "cat" } else { "pill" };
    for label in [widget, "card", "notif", "pcard"] {
        if let Some(w) = app.get_webview_window(label) {
            apply_layer(&w, &cfg.layer);
        }
    }
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

/// Coloca el gatito (modo mascota): posición guardada, o esquina inferior
/// derecha encima de la barra. El gatito SUSTITUYE a la pastilla — es un
/// widget independiente y arrastrable a cualquier parte del escritorio.
fn position_cat(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(cat) = app.get_webview_window("cat") else { return };
    let cfg = load_pill_config();
    if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
        let _ = cat.set_position(tauri::PhysicalPosition::new(x, y));
        return;
    }
    if let (Ok(Some(mon)), Ok(size)) = (cat.current_monitor(), cat.outer_size()) {
        let s = mon.size();
        let x = s.width.saturating_sub(size.width + MARGIN_X) as i32;
        #[allow(unused_mut)]
        let mut y = s.height.saturating_sub(size.height + TASKBAR_H + PANEL_GAP) as i32;
        #[cfg(windows)]
        if let Some(tb) = win_taskbar::query() {
            y = (tb.rect.top - size.height as i32 - PANEL_GAP as i32).max(0);
        }
        let _ = cat.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

/// Globo de diálogo del gatito: al pasar el mouse por el gatito se muestra
/// la ventana 'card' (burbuja cómic) arriba a la derecha, con la cola
/// apuntando al gato; al salir el mouse se oculta. Ventana FIJA — nunca se
/// redimensiona nada (el resize en vivo rompía el pintado de WebView2).
#[tauri::command]
fn hover_card(app: tauri::AppHandle, hovering: bool) {
    use tauri::Manager;
    let cfg = load_pill_config();
    let Some(card) = app.get_webview_window("card") else { return };
    if !hovering || !cfg.visible || cfg.style != "cat" {
        let _ = card.hide();
        return;
    }
    place_balloon(&app, "card", 79.0, 70.0);
    // prioridad: nunca dos globos a la vez — la notificación se esconde
    // mientras el de información está abierto; al plegarse, el gatito la
    // vuelve a pedir (emite notif:ready y el panel la re-muestra)
    if let Some(n) = app.get_webview_window("notif") {
        let _ = n.hide();
    }
    apply_layer(&card, &cfg.layer);
    let _ = card.show();
    reassert_layers(&app); // el gatito no debe quedarse atrás del globo
}

/// Coloca un globo (información o aviso) junto al widget que esté puesto
/// —gatito o pastilla—:
/// - pose vertical automática: ARRIBA del gato si cabe en el monitor,
///   ABAJO con la cola volteada si no (widget pegado al borde superior);
/// - X preferida sujetada a los límites del monitor ACTUAL del widget,
///   usando su ORIGEN + tamaño — en multi-monitor el globo acompaña al
///   widget (el clamp anterior contra [0, ancho] lo devolvía al monitor 1);
/// - la cola se reposiciona (evento balloon:pose hacia esa ventana) para
///   apuntar SIEMPRE al widget aunque el globo se haya corrido.
fn place_balloon(app: &tauri::AppHandle, label: &str, prefer_dx: f64, overlap_up: f64) {
    use tauri::{Emitter, Manager};
    // El ancla es el widget que esté puesto: el gatito o la pastilla. La cola
    // apunta a la cabeza del gato (62% de su ancho) o al centro de la pastilla.
    let cfg = load_pill_config();
    let (anchor_win, tail_ratio) = if cfg.style == "cat" {
        ("cat", 0.62)
    } else {
        ("pill", 0.50)
    };
    let (Some(anchor), Some(w)) =
        (app.get_webview_window(anchor_win), app.get_webview_window(label))
    else {
        return;
    };
    let (Ok(p), Ok(ks), Ok(ws)) =
        (anchor.outer_position(), anchor.outer_size(), w.outer_size())
    else {
        return;
    };
    let scale = anchor.scale_factor().unwrap_or(1.0);
    let (mx0, my0, mx1, my1) = match anchor.current_monitor() {
        Ok(Some(m)) => {
            let mp = m.position();
            let ms = m.size();
            (mp.x, mp.y, mp.x + ms.width as i32, mp.y + ms.height as i32)
        }
        _ => (i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2),
    };
    // pose vertical: la altura efectiva arriba descuenta el solape de la cola
    let up_h = ws.height as i32 - (overlap_up * scale).round() as i32;
    let pose_up = p.y - my0 >= up_h;
    let y = if pose_up {
        p.y - up_h
    } else {
        (p.y + ks.height as i32 - (10.0 * scale).round() as i32)
            .min(my1 - ws.height as i32)
            .max(my0)
    };
    let mut x = p.x + (prefer_dx * scale).round() as i32;
    x = x.min(mx1 - ws.width as i32).max(mx0);
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    let anchor = p.x as f64 + ks.width as f64 * tail_ratio;
    let wlog = ws.width as f64 / scale;
    let tailx = ((anchor - x as f64) / scale).round().clamp(24.0, wlog - 64.0);
    let _ = app.emit_to(label, "balloon:pose", serde_json::json!({
        "pose": if pose_up { "up" } else { "down" },
        "tailx": tailx,
    }));
}

/// Desplazamiento horizontal del globo respecto al widget: el gatito lo lleva
/// a su izquierda (para no taparle la cara) y la pastilla lo lleva centrado.
fn notif_dx(cfg: &PillConfig) -> f64 {
    if cfg.style == "cat" { -194.0 } else { -21.0 }
}

/// Cuánto se mete la punta de la cola dentro de la ventana del widget. El
/// gatito tiene márgenes transparentes de sobra y admite 40 px; la cápsula
/// mide 44 px con 6 de margen, así que con ese valor la cola le caía encima
/// del texto — solo debe rozar su borde.
fn notif_overlap(cfg: &PillConfig) -> f64 {
    // Cápsula: la punta del popover no cae en el borde de la ventana sino a
    // unos 5 px (es un cuadrado girado 45° anclado a 8 px), así que se suma
    // esa diferencia para que roce el borde de la cápsula y no lo muerda.
    if cfg.style == "cat" { 40.0 } else { 12.0 }
}

/// Pliega el detalle de la pastilla y devuelve la cápsula a su sitio.
fn close_pill_card(app: &tauri::AppHandle, cfg: &PillConfig) {
    use tauri::{Emitter, Manager};
    if let Some(c) = app.get_webview_window("pcard") {
        let _ = c.hide();
    }
    if cfg.visible && cfg.style != "cat" {
        if let Some(pill) = app.get_webview_window("pill") {
            let _ = pill.show();
        }
    }
    let _ = app.emit_to("pill", "pcard:closed", ());
}

/// Globo de aviso del widget: el PANEL decide cuándo mostrarlo y se cierra
/// con su ✕ o al abrir el panel — NUNCA solo. Sirve para los dos widgets
/// (gatito y pastilla): un toast de Windows se va a los pocos segundos y si
/// el usuario no estaba delante, no se entera (2026-07-27).
#[tauri::command]
fn set_notif_visible(app: tauri::AppHandle, visible: bool) {
    use tauri::Manager;
    let Some(w) = app.get_webview_window("notif") else { return };
    let cfg = load_pill_config();
    if visible && cfg.visible {
        // un aviso y un detalle a la vez se tapan: el globo tiene
        // prioridad, así que el detalle se pliega (lo mismo que ya hacía el
        // globo de información del gatito con la notificación)
        close_pill_card(&app, &cfg);
        place_balloon(&app, "notif", notif_dx(&cfg), notif_overlap(&cfg));
        // misma capa que el gatito: widget y globos se comportan como una
        // sola pieza (petición de Oscar 2026-07-26; antes la alarma se
        // forzaba al frente y rompía la coherencia con el ajuste elegido)
        apply_layer(&w, &cfg.layer);
        let _ = w.show();
        reassert_layers(&app); // el gatito no debe quedarse atrás del globo
    } else {
        let _ = w.hide();
    }
}

/// Despliega o pliega el detalle de la PASTILLA. La maqueta crece al hacer
/// clic, pero una ventana transparente no se puede redimensionar en vivo sin
/// que WebView2 deje de pintar, así que son dos ventanas: al desplegar se
/// oculta la pastilla y se muestra `pcard` con la cabecera idéntica EN SU
/// MISMO SITIO, y parece que creció.
/// Si no cabe hacia abajo (el widget suele vivir pegado a la barra de
/// tareas), la caja se ancla por el borde inferior y crece hacia ARRIBA: la
/// cabecera se queda donde estaba y las filas salen encima (pose "up").
#[tauri::command]
fn toggle_pill_card(app: tauri::AppHandle, open: bool) {
    use tauri::{Emitter, Manager};
    let Some(w) = app.get_webview_window("pcard") else { return };
    let cfg = load_pill_config();
    let Some(pill) = app.get_webview_window("pill") else { return };
    if !open || !cfg.visible || cfg.style == "cat" {
        let _ = w.hide();
        if cfg.visible && cfg.style != "cat" {
            let _ = pill.show();
        }
        let _ = app.emit_to("pill", "pcard:closed", ());
        return;
    }
    let (Ok(p), Ok(ps), Ok(ws)) = (pill.outer_position(), pill.outer_size(), w.outer_size())
    else {
        return;
    };
    let (mx0, my0, mx1, my1) = match pill.current_monitor() {
        Ok(Some(m)) => {
            let mp = m.position();
            let ms = m.size();
            (mp.x, mp.y, mp.x + ms.width as i32, mp.y + ms.height as i32)
        }
        _ => (i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2),
    };
    let down = p.y + ws.height as i32 <= my1;
    let y = if down {
        p.y
    } else {
        (p.y + ps.height as i32 - ws.height as i32).max(my0)
    };
    let x = p.x.min(mx1 - ws.width as i32).max(mx0);
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = app.emit_to("pcard", "balloon:pose", serde_json::json!({
        "pose": if down { "down" } else { "up" },
    }));
    apply_layer(&w, &cfg.layer);
    if let Some(n) = app.get_webview_window("notif") {
        let _ = n.hide();  // nunca globo y detalle a la vez
    }
    let _ = w.show();
    let _ = pill.hide();   // la cabecera del detalle la sustituye
}

/// ¿Es una compilación de desarrollo (`npm run dev`)? El panel lo usa para
/// enseñar el simulador de estados del gatito, que no debe existir en release.
#[tauri::command]
fn is_dev() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
fn get_pill_layer() -> String {
    let l = load_pill_config().layer;
    if l == "normal" || l == "bottom" { l } else { "top".into() }
}

/// Cambia la capa del widget (al frente / normal / fondo) y la aplica al
/// instante a las ventanas visibles.
#[tauri::command]
fn set_pill_layer(app: tauri::AppHandle, layer: String) {
    let mut cfg = load_pill_config();
    cfg.layer = match layer.as_str() {
        "normal" => "normal".into(),
        "bottom" => "bottom".into(),
        _ => "top".into(),
    };
    save_pill_config(&cfg);
    reassert_layers(&app);
}

fn set_pill_visible_impl(app: &tauri::AppHandle, visible: bool) {
    use tauri::{Emitter, Manager};
    let mut cfg = load_pill_config();
    cfg.visible = visible;
    save_pill_config(&cfg);
    // el widget es UNO: pastilla (estilo plain) o gatito (estilo cat)
    let cat_mode = cfg.style == "cat";
    if let Some(pill) = app.get_webview_window("pill") {
        if visible && !cat_mode {
            position_pill(app);
            apply_layer(&pill, &cfg.layer);
            let _ = pill.show();
        } else {
            let _ = pill.hide();
        }
    }
    if let Some(cat) = app.get_webview_window("cat") {
        if visible && cat_mode {
            position_cat(app);
            apply_layer(&cat, &cfg.layer);
            let _ = cat.show();
        } else {
            let _ = cat.hide();
        }
    }
    // los globos (hover y notificación) nunca sobreviven a un cambio de
    // visibilidad; el panel re-muestra la notificación si sigue pendiente
    for label in ["card", "notif", "pcard"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
    // la pastilla olvida que estaba desplegada (si no, vuelve con la
    // flecha al revés y el primer clic no haría nada visible)
    let _ = app.emit_to("pill", "pcard:closed", ());
}

#[tauri::command]
fn set_pill_visible(app: tauri::AppHandle, visible: bool) {
    set_pill_visible_impl(&app, visible);
}

#[tauri::command]
fn get_pill_visible() -> bool {
    load_pill_config().visible
}

#[tauri::command]
fn get_pill_style() -> String {
    let s = load_pill_config().style;
    if s == "cat" { "cat".into() } else { "plain".into() }
}

/// Cambia el estilo del widget (pastilla sola / con gatito): guarda la
/// preferencia y redimensiona la ventana conservando el borde inferior
/// (para que siga pegada encima de la barra) y la posición horizontal.
#[tauri::command]
fn set_pill_style(app: tauri::AppHandle, style: String) {
    use tauri::Manager;
    let mut cfg = load_pill_config();
    let new_style = if style == "cat" { "cat" } else { "plain" };
    // al alternar pastilla ↔ gatito (alturas distintas) se conserva el borde
    // INFERIOR de la posición guardada, para que el widget no "salte"
    if cfg.style != new_style {
        if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
            let (from, to) = if new_style == "cat" { ("pill", "cat") } else { ("cat", "pill") };
            if let (Some(fw), Some(tw)) =
                (app.get_webview_window(from), app.get_webview_window(to))
            {
                if let (Ok(fs), Ok(ts)) = (fw.outer_size(), tw.outer_size()) {
                    cfg.x = Some(x);
                    cfg.y = Some((y + fs.height as i32 - ts.height as i32).max(0));
                }
            }
        }
    }
    cfg.style = new_style.into();
    save_pill_config(&cfg);
    // muestra/oculta la ventana que corresponda (pastilla O gatito)
    set_pill_visible_impl(&app, cfg.visible);
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
    if rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err("buffer RGBA de tamaño inesperado".into());
    }
    // "curación" periódica: Windows a veces degrada el siempre-visible del
    // widget (quedaba detrás de otras apps); cada ciclo se re-afirma la capa
    reassert_layers(&app);
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

