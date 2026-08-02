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
        // aquí la distro no importa: solo se busca un token vigente
        for (_distro, d) in wsl_claude_dirs() {
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

#[derive(Serialize, Deserialize, Clone)]
struct ProjectAgg {
    name: String,
    cost: f64,
    tokens: u64,
    /// Coste por modelo dentro del proyecto (id de modelo -> USD equiv.)
    #[serde(default)]
    by_model: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DailyAgg {
    date: String, // YYYY-MM-DD (UTC)
    cost: f64,
}

/// Una fila del reporte: un hecho por fila, y todas las columnas aplican a
/// todas. Sustituye al CSV viejo, que metía tres tablas distintas en una
/// (proyectos, modelos y días) con una columna `name_or_date` que a veces
/// era un nombre y a veces una fecha (2026-07-29).
#[derive(Serialize, Deserialize, Clone)]
struct ExportRow {
    date: String,
    project: String,
    model: String,
    /// Lo rellena QUIEN LEE, con el nombre que el usuario dio al servidor: el
    /// exportador remoto no sabe cómo se llama a sí mismo, así que sus filas
    /// llegan sin este campo. Sin `default`, serde daba por inválida la
    /// respuesta entera y las filas del servidor se perdían sin avisar
    /// (visto 2026-07-29 en las capturas de Oscar: solo salían las locales).
    #[serde(default)]
    origin: String,
    cost: f64,
    tokens: u64,
}

#[derive(Serialize, Deserialize, Clone)]
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
    /// Filas del reporte. Solo se llenan cuando alguien exporta: calcularlas
    /// en cada refresco del panel sería trabajo tirado, y engordarían la foto
    /// que se sube al hub.
    #[serde(default)]
    rows: Vec<ExportRow>,
    /// Modo hub: resúmenes que OTRAS máquinas dejaron en ese servidor. Llegan
    /// sin fusionar para que la etiqueta la ponga quien lee. Vacío con un
    /// exportador viejo, que ni siquiera devuelve la clave.
    #[serde(default)]
    hosts: Vec<HubHost>,
    /// Analizador de fugas: solo se llenan bajo el flag --findings (patrón
    /// want_rows). El serde(default) es OBLIGATORIO: un exportador viejo no
    /// devuelve la clave y sin él se invalidaría la respuesta ENTERA (la
    /// misma mordida que ExportRow.origin el 2026-07-29).
    #[serde(default)]
    findings: Vec<Finding>,
}

/// Un hallazgo del analizador de fugas. Campos planos con default para que
/// cada `kind` llene solo los suyos y un exportador de otra versión no rompa
/// nada: reread (file/count/session), inflate (session/turns), mech (count),
/// mcp_unused (server). `origin` lo pone QUIEN LEE con el nombre que el
/// usuario dio al servidor — igual que en ExportRow.
#[derive(Serialize, Deserialize, Clone, Default)]
struct Finding {
    kind: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    server: String,
    #[serde(default)]
    session: String,
    #[serde(default)]
    count: u64,
    #[serde(default)]
    turns: u64,
    #[serde(default)]
    tokens: u64,
    #[serde(default)]
    cost: f64,
    #[serde(default)]
    estimated: bool,
    #[serde(default)]
    origin: String,
}

/// Una máquina ajena vista a través del servidor.
#[derive(Serialize, Deserialize, Clone)]
struct HubHost {
    #[serde(default)]
    id: String,
    machine: String,
    stats: LocalStats,
    /// Cuándo se escribió su resumen. No se descarta nada por antigüedad: la
    /// app no puede distinguir "se fue" de "está de vacaciones".
    #[serde(default)]
    seen_at: String,
    /// false = esa máquina no había subido la ventana pedida y se sirvió otra.
    /// Se conserva para poder avisarlo en la interfaz en vez de dar por buena
    /// una cifra que no es de la ventana elegida.
    #[serde(default)]
    window_exact: bool,
}

/// Lo que cada máquina deja en el hub: su identidad y su foto. Se sobreescribe
/// entera en cada ciclo — el hub no acumula historia, la foto ya trae dentro
/// la serie de los últimos 30 días.
#[derive(Serialize, Deserialize, Clone)]
struct HubSnapshot {
    id: String,
    machine: String,
    /// La ventana que tenía puesta esta máquina. Se conserva como respaldo
    /// para un exportador que no sepa de `windows`.
    stats: LocalStats,
    /// Una foto por cada ventana del selector (1/7/15/30). El SERVIDOR elige
    /// la que le piden: quien lee no puede recortar un resumen ajeno, porque
    /// el desglose por proyecto ya viene sumado.
    #[serde(default)]
    windows: HashMap<String, LocalStats>,
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
    /// Qué se sube a ESTE servidor: "all" (es mío) o "picked" (solo lo
    /// marcado). Va por servidor y no global porque una misma persona puede
    /// tener su VPS, donde sube todo, y el de un equipo, donde solo comparte
    /// el proyecto común. Ver docs/hub-modo-equipo.md.
    #[serde(default = "share_all")]
    share: String,
    /// Proyectos marcados cuando share == "picked". Nacen APAGADOS: el error
    /// hacia "todo" deja tus datos en el servidor de otra gente y no se puede
    /// deshacer; el error hacia "nada" solo enseña de menos.
    #[serde(default)]
    shared: Vec<String>,
}

fn share_all() -> String {
    "all".into()
}

/// Identidad de esta máquina en el hub. El NOMBRE es el del archivo que se
/// deja en el servidor (legible por una persona); el ID distingue "soy yo
/// otra vez" de "somos dos máquinas con el mismo nombre", que si no se
/// pisarían el archivo en cada ciclo sin que nadie se entere.
#[derive(Serialize, Deserialize, Clone)]
struct HubIdentity {
    id: String,
    machine: String,
}

fn hub_identity_path() -> PathBuf {
    app_data_dir().join("hub_identity.json")
}

/// Nombre de la máquina tal como lo conoce el sistema, como valor de partida.
fn host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "michiclaude".into())
}

/// Cómo se llama el origen "esta máquina" en el reporte. El panel lo manda
/// traducido antes de exportar; si no, queda en inglés.
static LOCAL_LABEL: std::sync::OnceLock<std::sync::Mutex<String>> = std::sync::OnceLock::new();

fn local_label() -> String {
    LOCAL_LABEL
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Local".into())
}

/// Solo lo que puede vivir en un nombre de archivo, para no depender de cómo
/// se llame la PC del usuario (espacios, acentos, barras…).
fn safe_name(n: &str) -> String {
    let out: String = n
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let out = out.trim_matches('-').to_lowercase();
    if out.is_empty() { "michiclaude".into() } else { out.chars().take(48).collect() }
}

/// Se crea una vez y se queda. El id no pretende ser criptográfico: solo
/// tiene que ser distinto entre instalaciones.
fn hub_identity() -> HubIdentity {
    if let Some(id) = fs::read_to_string(hub_identity_path())
        .ok()
        .and_then(|s| serde_json::from_str::<HubIdentity>(&s).ok())
    {
        return id;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let ident = HubIdentity {
        id: format!("{:x}-{:x}", nanos, std::process::id()),
        machine: host_name(),
    };
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(txt) = serde_json::to_string_pretty(&ident) {
        let _ = fs::write(hub_identity_path(), txt);
    }
    ident
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
/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
#[tauri::command]
async fn test_remote(host: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || test_remote_impl(host))
        .await
        .map_err(|e| e.to_string())?
}

fn test_remote_impl(host: String) -> Result<String, String> {
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
        // Saltos de línea de Unix SIEMPRE. Git entrega el archivo con CRLF al
        // clonar en Windows e include_str! lo embebe así; el script funciona
        // igual porque se ejecuta como `python3 archivo`, pero se sube con
        // permiso de ejecución y quien pruebe `./meter-export.py` en el
        // servidor se topa con un intérprete llamado "python3\r" y un error
        // que no dice nada (visto 2026-07-28).
        si.write_all(REMOTE_SCRIPT.replace("\r\n", "\n").as_bytes())
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
/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
#[tauri::command]
async fn install_remote(host: String, python: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || install_remote_impl(host, python))
        .await
        .map_err(|e| e.to_string())?
}

/// Comprueba que ESE binario concreto sirve. Sin esto se guardaría un comando
/// roto y el servidor aparecería "conectado" sin devolver ningún dato — el
/// fallo silencioso que ya nos mordió una vez.
fn verify_python(host: &str, py: &str) -> bool {
    let probe = format!(
        "command -v {py} >/dev/null 2>&1 && {py} -c \
'import sys;raise SystemExit(0 if sys.version_info>=(3,7) else 1)'"
    );
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg(host)
        .arg(probe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// `python` = la ruta que escribió el usuario cuando la detección falló. Se
/// pregunta ESO y no el comando entero porque es un dato que sí puede saber
/// (`which python3` en su servidor); el comando lo arma la app, que además
/// necesita subir el lector — cosa que no ocurriría si el usuario diera un
/// comando propio (2026-07-29).
fn install_remote_impl(host: String, python: Option<String>) -> Result<String, String> {
    let py = match python.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        Some(p) => {
            if !verify_python(&host, &p) {
                return Err("ERR_BAD_PYTHON".into());
            }
            p
        }
        None => detect_python(&host).ok_or_else(|| "ERR_NO_PYTHON".to_string())?,
    };
    upload_exporter(&host)?;
    Ok(format!("{py} {REMOTE_SCRIPT_PATH}"))
}

/// Deja el resumen de ESTA máquina en el servidor, en
/// `~/.michiclaude/hosts/<máquina>.json`. Con eso el servidor deja de ser
/// solo una fuente y pasa a ser el punto de encuentro donde cada máquina
/// deja su foto. Ver docs/hub-modo-equipo.md.
///
/// La comprobación del id se hace EN EL SERVIDOR, dentro del mismo comando:
/// si el archivo ya existe y trae otro id, no se sobreescribe y se sale con
/// código 3. Así una segunda máquina con el mismo nombre no borra los datos
/// de la primera en silencio, y no cuesta una conexión extra por ciclo.
fn upload_summary(
    r: &RemoteSource,
    stats: &LocalStats,
    windows: &HashMap<String, LocalStats>,
) -> Result<(), String> {
    let me = hub_identity();
    let file = safe_name(&me.machine);
    let id = me.id.clone();
    let payload = HubSnapshot {
        id: me.id,
        machine: me.machine,
        stats: stats.clone(),
        windows: windows.clone(),
    };
    // to_string (NO pretty) para que el id salga sin espacios —`"id":"..."`—
    // que es la cadena exacta que busca el grep del servidor.
    let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let guard = format!(
        "mkdir -p ~/.michiclaude/hosts && f=~/.michiclaude/hosts/{file}.json; \
if [ -f \"$f\" ] && ! grep -q '\"id\":\"{id}\"' \"$f\"; then exit 3; fi; cat > \"$f\""
    );
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg(&r.host)
        .arg(&guard)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .ok_or("ERR_SSH_STDIN")?
            .write_all(json.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    match st.code() {
        Some(0) => Ok(()),
        Some(3) => Err(format!("ERR_HUB_NAME_TAKEN:{file}")),
        _ => Err("ERR_HUB_UPLOAD".into()),
    }
}

/// Ejecuta el exportador remoto por SSH. BatchMode: jamás pide contraseña
/// (requiere llave configurada, la misma que usa VS Code Remote-SSH).
fn fetch_remote(
    r: &RemoteSource,
    window_days: u32,
    want_rows: bool,
    want_findings: bool,
) -> Option<LocalStats> {
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
            "{} --days {} --exclude-host {}{}{}",
            r.command,
            window_days,
            hub_identity().id,
            if prices.is_some() { " --prices-stdin" } else { "" },
            if want_rows { " --rows" } else { "" }
        ) + if want_findings { " --findings" } else { "" })
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
fn wsl_claude_dirs() -> Vec<(String, PathBuf)> {
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
                    out.push((distro.to_string(), d));
                }
            }
        }
        let root = base.join("root").join(".claude");
        if root.is_dir() {
            out.push((distro.to_string(), root));
        }
    }
    out
}

#[cfg(not(windows))]
fn wsl_claude_dirs() -> Vec<(String, PathBuf)> {
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
    /// (fecha, proyecto, modelo, origen) -> (USD, tokens). Solo si want_rows.
    rows: HashMap<(String, String, String, String), (f64, u64)>,
    want_rows: bool,
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
                    if agg.want_rows {
                        let slot = agg.projects.get(&slot_key).unwrap();
                        let base = slot.display.clone().unwrap_or_else(|| slot.fallback.clone());
                        let origin = slot.suffix.clone().unwrap_or_else(|| local_label());
                        let k = (
                            ts.format("%Y-%m-%d").to_string(),
                            base,
                            e.model.clone(),
                            origin,
                        );
                        let r = agg.rows.entry(k).or_insert((0.0, 0));
                        r.0 += cost;
                        r.1 += e.inp + e.out + e.cw;
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
/// Solo lo de ESTA máquina (este PC + sus distros WSL), sin tocar la red.
/// Se separó de collect_local_stats para poder calcular varias ventanas de
/// una tirada: el hub las necesita todas, porque quien lee un resumen ajeno
/// no puede recortarlo a otra ventana —el desglose por proyecto ya viene
/// sumado— y enseñaría el número de otra semana sin avisar (2026-07-28).
fn collect_own_stats(window_days: u32, want_rows: bool) -> (LocalStats, HashMap<String, f64>) {
    let now = Utc::now();
    let mut agg = LocalAgg { want_rows, ..Default::default() };
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
    // Sufijo "wsl-<distro>" (p. ej. "wsl-Ubuntu"). Dos cosas en una: sin el
    // nombre, Ubuntu y Debian caían bajo la misma etiqueta y no había forma
    // de distinguirlas; sin el prefijo, un "Ubuntu" suelto en la columna
    // Origen parece OTRA máquina en vez del Linux de este mismo PC
    // (2026-07-29, idea de Oscar).
    for (distro, d) in wsl_claude_dirs() {
        let tag = format!("wsl-{distro}");
        scan_projects_dir(
            &d.join("projects"), Some(&tag), now, window_days, &mut agg,
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

    // sin `mut`: aquí solo se construyen; quien los modifica es
    // collect_local_stats, que los recibe por valor
    let daily_map = agg.daily;
    let stats = LocalStats {
        projects,
        models: agg.models,
        cost_today: agg.cost_today,
        cost_week: agg.cost_window,
        tokens_week: agg.tokens_window,
        files_scanned: agg.files,
        entries_deduped: agg.deduped,
        daily: Vec::new(),
        rows: agg
            .rows
            .into_iter()
            .map(|((date, project, model, origin), (cost, tokens))| ExportRow {
                date, project, model, origin, cost, tokens,
            })
            .collect(),
        hosts: Vec::new(),   // se rellena al leer los servidores, más abajo
        findings: Vec::new(), // solo los llena get_findings, bajo demanda
    };
    (stats, daily_map)
}

/// Ventanas que se suben al hub. Tienen que ser las MISMAS que ofrece el
/// selector del panel: si el usuario elige una que nadie subió, el resumen
/// ajeno caería a otra y enseñaría un número que no es de esa ventana.
const HUB_WINDOWS: [u32; 4] = [1, 7, 15, 30];

fn collect_local_stats(window_days: u32) -> LocalStats {
    let (mut stats, mut daily_map) = collect_own_stats(window_days, false);

    // Subir la foto de ESTA máquina al hub, antes de fusionar nada. Tiene que
    // ser lo local a secas: si se subiera lo ya fusionado, las máquinas se
    // harían eco entre ellas y los totales se multiplicarían solos.
    let remotes = load_remotes();
    if !remotes.is_empty() {
        let daily: Vec<DailyAgg> = {
            let mut d: Vec<DailyAgg> = daily_map
                .iter()
                .map(|(date, cost)| DailyAgg { date: date.clone(), cost: *cost })
                .collect();
            d.sort_by(|a, b| a.date.cmp(&b.date));
            d
        };
        // Una foto por ventana. La ya calculada se reaprovecha; las demás
        // salen del caché de parseo, así que cuestan décimas de segundo.
        let mut windows: HashMap<String, LocalStats> = HashMap::new();
        for w in HUB_WINDOWS {
            let mut st = if w == window_days {
                stats.clone()
            } else {
                collect_own_stats(w, false).0
            };
            st.daily = daily.clone();
            windows.insert(w.to_string(), st);
        }
        let mut mine = windows
            .get(&window_days.to_string())
            .cloned()
            .unwrap_or_else(|| stats.clone());
        mine.daily = daily;
        let mut errs: Vec<String> = Vec::new();
        for r in &remotes {
            // La red nunca bloquea los datos locales: si falla, se anota y ya.
            if let Err(e) = upload_summary(r, &mine, &windows) {
                errs.push(format!("{}: {}", r.name, e));
            }
        }
        // Sin interfaz todavía (eso es la fase 2): por ahora queda el rastro,
        // igual que quota_debug.json, para poder diagnosticar.
        let _ = fs::write(
            app_data_dir().join("hub_debug.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "at": Utc::now().to_rfc3339(),
                "machine": hub_identity().machine,
                "uploaded_to": remotes.len() - errs.len(),
                "errors": errs,
            }))
            .unwrap_or_default(),
        );
    }

    // Fusionar fuentes remotas (si remotes.json existe): totales sumados,
    // proyectos etiquetados con su origen, modelos y serie diaria agregados.
    for r in remotes {
        let Some(remote) = fetch_remote(&r, window_days, false, false) else { continue };
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
        // Modo hub: las demás máquinas que dejaron su resumen en ese
        // servidor. Cada una se etiqueta con SU nombre, no con el del
        // servidor: el VPS es el punto de encuentro, no el origen.
        let me = hub_identity().id;
        for h in remote.hosts {
            // Cinturón y tirantes: el exportador ya nos excluye con
            // --exclude-host, pero si alguno viejo lo ignorase nos
            // devolvería lo nuestro y lo contaríamos dos veces.
            if !me.is_empty() && h.id == me {
                continue;
            }
            stats.cost_today += h.stats.cost_today;
            stats.cost_week += h.stats.cost_week;
            stats.tokens_week += h.stats.tokens_week;
            stats.files_scanned += h.stats.files_scanned;
            stats.entries_deduped += h.stats.entries_deduped;
            for p in h.stats.projects {
                stats.projects.push(ProjectAgg {
                    name: format!("{} · {}", p.name, h.machine),
                    cost: p.cost,
                    tokens: p.tokens,
                    by_model: p.by_model,
                });
            }
            for (m, a) in h.stats.models {
                let e = stats.models.entry(m).or_default();
                e.input += a.input;
                e.output += a.output;
                e.cache_write += a.cache_write;
                e.cache_read += a.cache_read;
                e.cost += a.cost;
                e.estimated = e.estimated || a.estimated;
            }
            for d in h.stats.daily {
                *daily_map.entry(d.date).or_insert(0.0) += d.cost;
            }
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

/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
#[tauri::command]
async fn get_local_stats(days: Option<u32>) -> Result<LocalStats, String> {
    tauri::async_runtime::spawn_blocking(move || get_local_stats_impl(days))
        .await
        .map_err(|e| e.to_string())?
}

fn get_local_stats_impl(days: Option<u32>) -> Result<LocalStats, String> {
    Ok(collect_local_stats(days.unwrap_or(7).clamp(1, 90)))
}

/// Filas del reporte de TODAS las fuentes. Solo la usa el export: el panel no
/// necesita este detalle y calcularlo en cada ciclo sería trabajo tirado.
fn collect_export_rows(window_days: u32) -> Vec<ExportRow> {
    let (mine, _) = collect_own_stats(window_days, true);
    let mut rows = mine.rows;
    for r in load_remotes() {
        let Some(rem) = fetch_remote(&r, window_days, true, false) else { continue };
        // el origen lo pone quien lee, con el nombre que el usuario le dio al
        // servidor — el exportador remoto no sabe cómo se llama a sí mismo
        rows.extend(rem.rows.into_iter().map(|mut x| {
            x.origin = r.name.clone();
            x
        }));
    }
    // fecha descendente, y dentro de cada día lo más caro primero
    rows.sort_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then(b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal))
    });
    rows
}

// ---------------------------------------------------------------------------
// Analizador de fugas — réplica EXACTA de scan_findings en meter-export.py.
// Mantener AMBOS en sincronía, como la agregación (invariante #1). Diseño y
// reglas en docs/analizador-fugas.md: solo hallazgos estructurales medibles,
// nada que exija adivinar qué tan difícil era una tarea; costos MEDIDOS y,
// donde entre la heurística chars/4, el hallazgo va con estimated:true.
// Pasada APARTE sin caché (necesita detalle por línea que el scan_cache no
// guarda) y solo bajo demanda: el ciclo del panel nunca la paga.
// ---------------------------------------------------------------------------

const REREAD_MIN: u64 = 3;
const REREAD_MIN_TOKENS: u64 = 2000;
const INFLATE_MIN_GROWTH: u64 = 50_000;
const INFLATE_MIN_TURNS: u64 = 10;
const MECH_MIN: u64 = 5;
const CACHEBREAK_MIN_PREV: u64 = 20_000;
const CACHEBREAK_MIN_TOKENS: u64 = 300_000;
const SUB_MIN_TOKENS: u64 = 50_000;
const HOOKNOISE_MIN_FIRES: u64 = 15;
const HOOKNOISE_MIN_TOKENS: u64 = 10_000;
const CLAUDEMD_MIN_LINES: usize = 5;
const CLAUDEMD_MAX_TOKENS: usize = 400;
const MAX_FINDINGS: usize = 12;

/// Comandos deterministas: turnos donde Claude no piensa, solo ejecuta.
/// Pareja del MECH_RE del exportador — sin crate regex (invariante #4), así
/// que va con recortes de string. La lista es CORTA a propósito: un falso
/// positivo aquí cuesta la credibilidad del detector entero.
fn is_mech_cmd(cmd: &str) -> bool {
    let mut s = cmd.trim_start();
    // prefijo opcional "cd <ruta> && " o "cd <ruta> ; "
    if s.starts_with("cd ") {
        if let Some(i) = s.find("&&") {
            s = s[i + 2..].trim_start();
        } else if let Some(i) = s.find(';') {
            s = s[i + 1..].trim_start();
        }
    }
    s == "git"
        || s.starts_with("git ")
        || s.starts_with("pytest")
        || s.starts_with("cargo check")
        || s.starts_with("cargo fmt")
        || s.starts_with("cargo clippy")
        || s.starts_with("npm test")
        || s.starts_with("npm ci")
        || s.starts_with("npm install")
}

/// Skills propias del usuario (~/.claude/skills). Los plugins NO se cuentan:
/// la carpeta de marketplaces es el catálogo ENTERO cacheado (docenas de
/// skills que nadie instaló) y contarla fabricaría hallazgos falsos.
fn skills_installed() -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(home) = dirs::home_dir() else { return out };
    let Ok(entries) = fs::read_dir(home.join(".claude").join("skills")) else {
        return out;
    };
    for e in entries.flatten() {
        if e.path().join("SKILL.md").is_file() {
            out.insert(e.file_name().to_string_lossy().to_lowercase());
        }
    }
    out
}

/// Skills con uso registrado por el PROPIO Claude Code (skillUsage de
/// ~/.claude.json) dentro de la ventana. Complementa a los logs: cubre las
/// invocadas por la herramienta Skill aunque el log ya se haya borrado.
fn skills_used_at(window_ago: &DateTime<Utc>) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(home) = dirs::home_dir() else { return out };
    let Ok(raw) = fs::read_to_string(home.join(".claude.json")) else { return out };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { return out };
    if let Some(m) = v["skillUsage"].as_object() {
        for (name, u) in m {
            if u["lastUsedAt"].as_f64().unwrap_or(0.0) / 1000.0
                >= window_ago.timestamp() as f64
            {
                out.insert(name.split(':').last().unwrap_or(name).to_lowercase());
            }
        }
    }
    out
}

/// Nombres de skill dentro de <command-name>…</command-name> — pareja del
/// SKILL_CMD_RE del exportador, sin crate regex (invariante #4).
fn command_names(text: &str, out: &mut HashSet<String>) {
    let mut rest = text;
    while let Some(i) = rest.find("<command-name>") {
        rest = &rest[i + 14..];
        let Some(j) = rest.find("</command-name>") else { break };
        let name = rest[..j].trim().trim_start_matches('/');
        if let Some(first) = name.split_whitespace().next() {
            let n = first.split(':').last().unwrap_or(first).to_lowercase();
            if !n.is_empty() {
                out.insert(n);
            }
        }
        rest = &rest[j..];
    }
}

/// Filtro de identificadores del CLAUDE.md — pareja de _md_token_ok del
/// exportador. Descarta lo corto (falsos verdes por subcadena), los
/// patrones con comodines, las URLs y lo que no lleva letras.
fn md_token_ok(tok: &str) -> bool {
    let n = tok.chars().count();
    if !(4..=80).contains(&n) {
        return false;
    }
    if tok.starts_with("http://") || tok.starts_with("https://") {
        return false;
    }
    if tok.chars().any(|c| "<>{}*$|`\"".contains(c) || c.is_whitespace()) {
        return false;
    }
    tok.chars().any(|c| c.is_alphabetic())
}

/// Identificadores verificables de una línea de CLAUDE.md — pareja de
/// _md_line_tokens del exportador: lo que va entre backticks más palabras
/// con pinta de ruta o de archivo.ext. Una línea sin nada verificable
/// queda GRIS (sin opinión), nunca roja.
fn md_line_tokens(line: &str) -> Vec<String> {
    let mut toks: Vec<String> = Vec::new();
    for (i, seg) in line.split('`').enumerate() {
        if i % 2 == 1 {
            if let Some(first) = seg.trim().split_whitespace().next() {
                toks.push(first.to_string());
            }
        } else {
            for w in seg.split_whitespace() {
                let w = w
                    .trim_start_matches(|c| "(\"'«“[".contains(c))
                    .trim_end_matches(|c| ".,;:!?)\"'»”]".contains(c));
                let cs: Vec<char> = w.chars().collect();
                let dotted = (1..cs.len().saturating_sub(1)).any(|j| {
                    cs[j] == '.' && cs[j - 1].is_alphanumeric() && cs[j + 1].is_alphanumeric()
                });
                if w.contains('/') || w.contains('\\') || dotted {
                    toks.push(w.to_string());
                }
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    for tok in toks {
        let tl = tok.to_lowercase();
        if md_token_ok(&tl) && !out.contains(&tl) {
            out.push(tl);
        }
    }
    out
}

/// CLAUDE.md global + el de cada proyecto con actividad en la ventana —
/// pareja de _claude_mds del exportador. El cwd real sale de las primeras
/// líneas de los .jsonl (el nombre de la carpeta de logs viene aplanado y
/// no se puede revertir sin ambigüedad); los cwd de WSL no resuelven desde
/// Windows y se saltan solos al fallar la lectura. Dedup por ruta real: un
/// symlink de carpeta renombrada haría analizar dos veces el mismo archivo.
fn claude_mds(pdirs: &[PathBuf], skip_before: i64) -> Vec<(String, Option<String>, String)> {
    use std::io::BufRead;
    let mut mds = Vec::new();
    let mut vistos: HashSet<PathBuf> = HashSet::new();
    let g = claude_dir().join("CLAUDE.md");
    if let Ok(texto) = fs::read_to_string(&g) {
        vistos.insert(fs::canonicalize(&g).unwrap_or_else(|_| g.clone()));
        mds.push((g.to_string_lossy().to_string(), None, texto));
    }
    for pdir in pdirs {
        let Ok(projs) = fs::read_dir(pdir) else { continue };
        let mut dirs: Vec<PathBuf> = projs.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for ppath in dirs {
            if !ppath.is_dir() {
                continue;
            }
            let proj_name = ppath
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let Ok(files) = fs::read_dir(&ppath) else { continue };
            let mut jsonls: Vec<PathBuf> = files
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
                .collect();
            jsonls.sort();
            let mut cwd: Option<String> = None;
            for fp in jsonls {
                if let Ok(md) = fs::metadata(&fp) {
                    if let Ok(mt) = md.modified() {
                        let secs = mt
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        if secs < skip_before {
                            continue;
                        }
                    }
                }
                let Ok(fh) = fs::File::open(&fp) else { continue };
                for line in std::io::BufReader::new(fh).lines().take(20).flatten() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(c) = v["cwd"].as_str() {
                            if !c.is_empty() {
                                cwd = Some(c.to_string());
                                break;
                            }
                        }
                    }
                }
                if cwd.is_some() {
                    break;
                }
            }
            let Some(c) = cwd else { continue };
            let p = PathBuf::from(&c).join("CLAUDE.md");
            let rp = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            if vistos.contains(&rp) {
                continue;
            }
            if let Ok(texto) = fs::read_to_string(&p) {
                vistos.insert(rp);
                mds.push((p.to_string_lossy().to_string(), Some(proj_name), texto));
            }
        }
    }
    mds
}

/// Servidores MCP dados de alta en ~/.claude.json (global y por proyecto).
fn mcp_servers_configured() -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(home) = dirs::home_dir() else { return out };
    let Ok(raw) = fs::read_to_string(home.join(".claude.json")) else { return out };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { return out };
    if let Some(m) = v["mcpServers"].as_object() {
        out.extend(m.keys().cloned());
    }
    if let Some(projs) = v["projects"].as_object() {
        for p in projs.values() {
            if let Some(m) = p["mcpServers"].as_object() {
                out.extend(m.keys().cloned());
            }
        }
    }
    out
}

#[derive(Default)]
struct SessFindings {
    first_cr: Option<u64>,
    last_cr: u64,
    turns: u64,
    cr_cost: f64,
    reads: HashMap<String, u64>,
    read_chars: HashMap<String, u64>,
    models: HashMap<String, u64>,
    /// nombre de la CARPETA de logs — se usa para CASAR con el detector de
    /// CLAUDE.md, así que no se toca
    proj: String,
    /// nombre real del proyecto (del `cwd`), solo para enseñar
    disp: String,
    /// hilo principal en orden (epoch, modelo, cache_read, cache_write)
    /// para el detector de rupturas de caché
    cb: Vec<(i64, String, u64, u64)>,
    /// timestamps de compactaciones: ahí reescribir es el ahorro, no la fuga
    compacts: Vec<i64>,
    /// hookName -> (disparos, chars de `content`) para el detector de
    /// hooks ruidosos
    hooks: HashMap<String, (u64, u64)>,
}

/// Corre los detectores sobre las fuentes locales (este PC + WSL) en la
/// ventana pedida. Los hallazgos de servidores llegan aparte, por
/// fetch_remote con --findings, y el origen lo etiqueta quien lee.
fn scan_local_findings(window_days: u32) -> Vec<Finding> {
    let now = Utc::now();
    let window_ago = now - Duration::days(window_days as i64);
    let skip_before = (window_ago - Duration::days(2)).timestamp();

    let mut sessions: HashMap<String, SessFindings> = HashMap::new();
    let mut pend: HashMap<String, (String, String)> = HashMap::new();
    let mut mcp_used: HashSet<String> = HashSet::new();
    let mut skills_used: HashSet<String> = HashSet::new();
    let mut seen: HashSet<String> = HashSet::new();
    let (mut mech_count, mut mech_tokens, mut mech_cost) = (0u64, 0u64, 0f64);
    let (mut sub_count, mut sub_tokens, mut sub_cost) = (0u64, 0u64, 0f64);

    let mut dirs_to_scan = vec![claude_dir().join("projects")];
    for (_distro, d) in wsl_claude_dirs() {
        dirs_to_scan.push(d.join("projects"));
    }

    // CLAUDE.md sin respaldo: identificadores por línea, a buscar en el
    // texto CRUDO de los logs de la ventana. Solo con 7+ días, como skills:
    // "no lo mencionaste HOY" no dice nada. El tope de búsqueda va por
    // archivo y en orden de lectura; las líneas cuyos identificadores no
    // entraron quedan grises (sin opinión), nunca rojas.
    let mut md_meta: Vec<(String, Option<String>)> = Vec::new();
    let mut md_lines: Vec<(usize, usize, usize, Vec<String>)> = Vec::new();
    let mut md_pending: HashSet<String> = HashSet::new();
    let mut md_found: HashSet<String> = HashSet::new();
    if window_days >= 7 {
        for (ruta, pj, texto) in claude_mds(&dirs_to_scan, skip_before) {
            let idx = md_meta.len();
            md_meta.push((ruta, pj));
            let mut added: HashSet<String> = HashSet::new();
            for (ln_no, ln) in texto.lines().enumerate() {
                let mut keep: Vec<String> = Vec::new();
                for tok in md_line_tokens(ln) {
                    if md_pending.contains(&tok) || added.contains(&tok) {
                        keep.push(tok);
                    } else if added.len() < CLAUDEMD_MAX_TOKENS {
                        added.insert(tok.clone());
                        keep.push(tok);
                    }
                }
                if !keep.is_empty() {
                    md_lines.push((idx, ln_no + 1, ln.chars().count(), keep));
                }
            }
            md_pending.extend(added);
        }
    }

    for pdir in dirs_to_scan {
        let Ok(projs) = fs::read_dir(&pdir) else { continue };
        for proj in projs.flatten() {
            let ppath = proj.path();
            if !ppath.is_dir() {
                continue;
            }
            let proj_name = proj.file_name().to_string_lossy().to_string();
            let Ok(files) = fs::read_dir(&ppath) else { continue };
            for f in files.flatten() {
                let fp = f.path();
                if fp.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                // demasiado viejo para la ventana: ni se abre (mismo margen
                // de 2 días que el exportador)
                if let Ok(md) = f.metadata() {
                    if let Ok(mt) = md.modified() {
                        let secs = mt
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        if secs < skip_before {
                            continue;
                        }
                    }
                }
                let Ok(text) = fs::read_to_string(&fp) else { continue };
                // identificadores del CLAUDE.md contra el texto crudo, con
                // eliminación temprana: el encontrado deja de buscarse
                if !md_pending.is_empty() {
                    let low = text.to_lowercase();
                    md_pending.retain(|tok| {
                        if low.contains(tok.as_str()) {
                            md_found.insert(tok.clone());
                            false
                        } else {
                            true
                        }
                    });
                }
                for line in text.lines() {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                        continue;
                    };
                    let sid = v["sessionId"].as_str().unwrap_or("").to_string();
                    // una compactación reescribe el contexto A PROPÓSITO: se
                    // marca para no contarla como ruptura de caché (estas
                    // líneas tampoco traen usage, así que va antes del filtro)
                    if v["isCompactSummary"].as_bool().unwrap_or(false)
                        || v["subtype"].as_str() == Some("compact_boundary")
                    {
                        if let Some(cts) = v["timestamp"]
                            .as_str()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        {
                            sessions
                                .entry(sid.clone())
                                .or_default()
                                .compacts
                                .push(cts.timestamp());
                        }
                    }
                    // /comandos del usuario: quedan como <command-name> en el
                    // mensaje (estas líneas tampoco traen usage)
                    if line.contains("<command-name>") {
                        let in_window = v["timestamp"]
                            .as_str()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|d| d.with_timezone(&Utc) >= window_ago)
                            .unwrap_or(false);
                        if in_window {
                            if let Some(s) = v["message"]["content"].as_str() {
                                command_names(s, &mut skills_used);
                            } else if let Some(arr) = v["message"]["content"].as_array() {
                                for b in arr {
                                    if let Some(t2) = b["text"].as_str() {
                                        command_names(t2, &mut skills_used);
                                    }
                                }
                            }
                        }
                    }
                    // salida de hooks: cada disparo queda como attachment
                    // hook_success y su `content` es EXACTAMENTE lo que entró
                    // al contexto en ese turno (verificado con un log real
                    // 2026-07-30). Dedup por uuid: las reanudaciones copian
                    // las líneas viejas al archivo nuevo.
                    if v["type"].as_str() == Some("attachment") {
                        let a = &v["attachment"];
                        if a["type"].as_str() == Some("hook_success") {
                            let in_window = v["timestamp"]
                                .as_str()
                                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                                .map(|d| d.with_timezone(&Utc) >= window_ago)
                                .unwrap_or(false);
                            let uuid = v["uuid"].as_str().unwrap_or("");
                            if in_window && !uuid.is_empty() && seen.insert(uuid.to_string())
                            {
                                let hname =
                                    a["hookName"].as_str().unwrap_or("?").to_string();
                                let chars =
                                    a["content"].as_str().map(|s| s.len() as u64).unwrap_or(0);
                                let st = sessions.entry(sid.clone()).or_default();
                                let e = st.hooks.entry(hname).or_insert((0, 0));
                                e.0 += 1;
                                e.1 += chars;
                            }
                        }
                        continue; // los attachments nunca traen usage
                    }
                    // resultados de lecturas: se MIDE lo que viajó de verdad
                    // (va ANTES del filtro de usage: estas líneas no lo traen)
                    if let Some(blocks) = v["message"]["content"].as_array() {
                        for b in blocks {
                            if b["type"].as_str() != Some("tool_result") {
                                continue;
                            }
                            let Some(id) = b["tool_use_id"].as_str() else { continue };
                            let Some((s2, path)) = pend.remove(id) else { continue };
                            let n = if let Some(s) = b["content"].as_str() {
                                s.len() as u64
                            } else if let Some(arr) = b["content"].as_array() {
                                arr.iter()
                                    .filter_map(|x| x["text"].as_str())
                                    .map(|t| t.len() as u64)
                                    .sum()
                            } else {
                                0
                            };
                            let st = sessions.entry(s2).or_default();
                            *st.read_chars.entry(path).or_insert(0) += n;
                        }
                    }
                    let usage = &v["message"]["usage"];
                    if !usage.is_object() {
                        continue;
                    }
                    let key = format!(
                        "{}:{}",
                        v["message"]["id"].as_str().unwrap_or(""),
                        v["requestId"].as_str().unwrap_or("")
                    );
                    if key != ":" && !seen.insert(key) {
                        continue;
                    }
                    let model = v["message"]["model"].as_str().unwrap_or("unknown").to_string();
                    if model == "<synthetic>" {
                        continue;
                    }
                    let Some(ts) = v["timestamp"]
                        .as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|d| d.with_timezone(&Utc))
                    else {
                        continue;
                    };
                    if ts < window_ago {
                        continue;
                    }
                    let inp = usage["input_tokens"].as_u64().unwrap_or(0);
                    let out_t = usage["output_tokens"].as_u64().unwrap_or(0);
                    let cw = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                    let cr = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                    {
                        let st = sessions.entry(sid.clone()).or_default();
                        if st.proj.is_empty() {
                            st.proj = proj_name.clone();
                        }
                        if st.disp.is_empty() {
                            // Para ENSEÑAR, el `cwd` real de la sesión: el
                            // nombre de la carpeta de logs codifica la ruta
                            // entera con guiones y al recortarla se parten
                            // los nombres compuestos ("test-agente" ->
                            // "agente"). OJO: `proj` NO se toca, que es lo
                            // que casa con el detector de CLAUDE.md.
                            if let Some(b) = cwd_name(&v) {
                                st.disp = b;
                            }
                        }
                        st.turns += 1;
                        *st.models.entry(model.clone()).or_insert(0) += 1;
                        if st.first_cr.is_none() {
                            st.first_cr = Some(cr);
                        }
                        st.last_cr = cr;
                        // MEDIDO: lo que costó releer el contexto en este turno
                        st.cr_cost += cost_of(&model, 0, 0, 0, cr);
                        // hilo principal para el detector de rupturas; los
                        // subagentes llevan SU contexto y mezclarlos
                        // fabricaría rupturas que no existieron
                        if !v["isSidechain"].as_bool().unwrap_or(false) {
                            st.cb.push((ts.timestamp(), model.clone(), cr, cw));
                        } else {
                            // subagentes: costo MEDIDO de su propio usage —
                            // ya está dentro del total, pero ahí es invisible
                            sub_count += 1;
                            sub_tokens += inp + out_t + cw;
                            sub_cost += cost_of(&model, inp, out_t, cw, cr);
                        }
                    }
                    let empty = Vec::new();
                    let uses: Vec<&serde_json::Value> = v["message"]["content"]
                        .as_array()
                        .unwrap_or(&empty)
                        .iter()
                        .filter(|b| b["type"].as_str() == Some("tool_use"))
                        .collect();
                    let mut all_mech = !uses.is_empty();
                    for b in &uses {
                        let name = b["name"].as_str().unwrap_or("");
                        if let Some(rest) = name.strip_prefix("mcp__") {
                            mcp_used
                                .insert(rest.split("__").next().unwrap_or(rest).to_string());
                        }
                        if name == "Skill" {
                            if let Some(sk) = b["input"]["skill"].as_str() {
                                skills_used.insert(
                                    sk.split(':').last().unwrap_or(sk).to_lowercase(),
                                );
                            }
                        }
                        if name == "Read" {
                            if let Some(p) = b["input"]["file_path"].as_str() {
                                let st = sessions.entry(sid.clone()).or_default();
                                *st.reads.entry(p.to_string()).or_insert(0) += 1;
                                if let Some(id) = b["id"].as_str() {
                                    pend.insert(id.to_string(), (sid.clone(), p.to_string()));
                                }
                            }
                        }
                        if name != "Bash"
                            || !is_mech_cmd(b["input"]["command"].as_str().unwrap_or(""))
                        {
                            all_mech = false;
                        }
                    }
                    if all_mech {
                        mech_count += 1;
                        mech_tokens += inp + out_t + cw;
                        mech_cost += cost_of(&model, inp, out_t, cw, cr);
                    }
                }
            }
        }
    }

    let mut findings: Vec<Finding> = Vec::new();
    // hookName -> (disparos, chars, costo) sumado entre sesiones
    let mut hooks_g: HashMap<String, (u64, u64, f64)> = HashMap::new();
    // (proyecto, precio de input del modelo dominante) por sesión de la
    // ventana, para el costo piso del detector de CLAUDE.md
    let mut sess_pi: Vec<(String, f64)> = Vec::new();
    for (sid, s) in &sessions {
        if s.models.is_empty() {
            continue;
        }
        let top_model = s
            .models
            .iter()
            .max_by_key(|(_, c)| **c)
            .map(|(m, _)| m.clone())
            .unwrap_or_default();
        let pi = price_for(&top_model).0;
        sess_pi.push((s.proj.clone(), pi));
        // los disparos se acumulan por hook GLOBAL, pero el costo se valora
        // con el modelo dominante de la sesión donde ocurrieron
        for (hname, (nf, nch)) in &s.hooks {
            let g = hooks_g.entry(hname.clone()).or_insert((0, 0, 0.0));
            g.0 += nf;
            g.1 += nch;
            g.2 += *nch as f64 / 4.0 * pi / 1_000_000.0;
        }
        let sid8: String = sid.chars().take(8).collect();
        // archivos releídos: el contenido se APILA en la conversación, no se
        // reemplaza. Tokens ~ chars/4 de lo devuelto tras la primera lectura;
        // el costo es el PISO (una ingesta a precio de input) — la realidad
        // es mayor porque además se relee en cada turno posterior.
        for (path, n) in &s.reads {
            if *n < REREAD_MIN {
                continue;
            }
            let chars = s.read_chars.get(path).copied().unwrap_or(0);
            let stacked = chars * (*n - 1) / *n / 4;
            if stacked < REREAD_MIN_TOKENS {
                continue;
            }
            findings.push(Finding {
                kind: "reread".into(),
                file: path.clone(),
                project: sdisp(s),
                count: *n,
                tokens: stacked,
                cost: stacked as f64 * pi / 1_000_000.0,
                estimated: true,
                session: sid8.clone(),
                ..Default::default()
            });
        }
        let growth = s.last_cr.saturating_sub(s.first_cr.unwrap_or(0));
        if growth >= INFLATE_MIN_GROWTH && s.turns >= INFLATE_MIN_TURNS {
            findings.push(Finding {
                kind: "inflate".into(),
                project: sdisp(s),
                session: sid8.clone(),
                turns: s.turns,
                tokens: growth,
                cost: s.cr_cost,
                ..Default::default()
            });
        }
        // rupturas de caché: turnos donde el prefijo cacheado se PERDIÓ
        // (cache_read cae a menos de la mitad) y la conversación se
        // reescribió a precio de escritura (1.25x input) en vez de leerse
        // a 0.1x. Causas típicas: pausa mayor al TTL del caché o cambio de
        // modelo (cada modelo tiene el suyo). El costo es MEDIDO: tokens
        // que ya estaban escritos, cobrados otra vez a tarifa de escritura.
        let mut cb = s.cb.clone();
        cb.sort_by_key(|t| t.0);
        let (mut breaks, mut rew_tok, mut rew_cost) = (0u64, 0u64, 0f64);
        for w in cb.windows(2) {
            let prev = w[0].2 + w[0].3;
            let (ts_i, m_i, cr_i, cw_i) = (w[1].0, &w[1].1, w[1].2, w[1].3);
            if prev < CACHEBREAK_MIN_PREV || cr_i * 2 >= prev {
                continue;
            }
            if s.compacts.iter().any(|c| (ts_i - *c).abs() < 120) {
                continue;
            }
            let rew = cw_i.min(prev); // PISO: solo lo que ya estaba escrito
            breaks += 1;
            rew_tok += rew;
            rew_cost += rew as f64 * price_for(m_i).2 / 1_000_000.0;
        }
        if rew_tok >= CACHEBREAK_MIN_TOKENS {
            findings.push(Finding {
                kind: "cachebreak".into(),
                project: sdisp(s),
                session: sid8,
                count: breaks,
                tokens: rew_tok,
                cost: rew_cost,
                ..Default::default()
            });
        }
    }
    if mech_count >= MECH_MIN {
        findings.push(Finding {
            kind: "mech".into(),
            count: mech_count,
            tokens: mech_tokens,
            cost: mech_cost,
            ..Default::default()
        });
    }
    // subagentes: una tarjeta con el costo agregado de la ventana. No juzga
    // si valieron la pena — solo hace VISIBLE un gasto que hoy se mezcla
    // con el total de la conversación principal.
    if sub_tokens >= SUB_MIN_TOKENS {
        findings.push(Finding {
            kind: "subagents".into(),
            count: sub_count,
            tokens: sub_tokens,
            cost: sub_cost,
            ..Default::default()
        });
    }
    // hooks ruidosos: la salida de un hook entra al contexto en CADA disparo
    // (tamaño × turnos). Tokens ~ chars/4 (heurística → "~") y costo PISO a
    // precio de input — la realidad es mayor porque además se relee en los
    // turnos posteriores. No juzga si el hook sirve: mide lo que cuesta
    // cargarlo, igual que skills_unused y mcp_unused.
    let mut hnames: Vec<&String> = hooks_g.keys().collect();
    hnames.sort();
    for hname in hnames {
        let (nf, nch, hcost) = hooks_g[hname];
        let tok = nch / 4;
        if nf < HOOKNOISE_MIN_FIRES || tok < HOOKNOISE_MIN_TOKENS {
            continue;
        }
        findings.push(Finding {
            kind: "hooks_noise".into(),
            file: hname.clone(),
            count: nf,
            tokens: tok,
            cost: hcost,
            estimated: true,
            ..Default::default()
        });
    }
    let mut unused: Vec<String> = mcp_servers_configured()
        .into_iter()
        .filter(|s| !mcp_used.contains(s))
        .collect();
    unused.sort();
    for server in unused {
        findings.push(Finding { kind: "mcp_unused".into(), server, ..Default::default() });
    }
    // skills instaladas y sin usar en la ventana: UNA tarjeta agregada (una
    // por skill inundaría el reporte). Solo con ventana de 7+ días: "no
    // usaste tu skill HOY" no dice nada y devalúa a las demás tarjetas.
    if window_days >= 7 {
        let used_cfg = skills_used_at(&window_ago);
        let mut sk_unused: Vec<String> = skills_installed()
            .into_iter()
            .filter(|s| !skills_used.contains(s) && !used_cfg.contains(s))
            .collect();
        sk_unused.sort();
        if !sk_unused.is_empty() {
            let shown = if sk_unused.len() > 8 {
                format!("{} …", sk_unused[..8].join(", "))
            } else {
                sk_unused.join(", ")
            };
            findings.push(Finding {
                kind: "skills_unused".into(),
                count: sk_unused.len() as u64,
                file: shown,
                ..Default::default()
            });
        }
    }
    // líneas de CLAUDE.md sin respaldo: NINGUNA de sus menciones aparece en
    // los logs de la ventana. Costo PISO, nunca "líneas × turnos" (la trampa
    // documentada: tras el primer turno está cacheado): esas líneas entran
    // al contexto UNA vez por sesión — tokens ~ chars/4 ("~") por sesión de
    // la ventana, al precio de input del modelo dominante de cada una.
    // Limitación asumida (dirección segura del error): si el CLAUDE.md se
    // leyó o editó en la ventana, sus líneas viajan en los logs y salen
    // verdes — el detector calla en vez de arriesgar un falso rojo.
    for (idx, (ruta, pj)) in md_meta.iter().enumerate() {
        let reds: Vec<(usize, usize)> = md_lines
            .iter()
            .filter(|(i2, _, _, toks)| {
                *i2 == idx && toks.iter().all(|t| !md_found.contains(t))
            })
            .map(|(_, ln_no, ch, _)| (*ln_no, *ch))
            .collect();
        if reds.len() < CLAUDEMD_MIN_LINES {
            continue;
        }
        let tok_est = (reds.iter().map(|(_, ch)| *ch as u64).sum::<u64>()) / 4;
        let mut cost = 0.0;
        for (sproj, pi2) in &sess_pi {
            if let Some(p) = pj {
                if sproj != p {
                    continue;
                }
            }
            cost += tok_est as f64 * pi2 / 1_000_000.0;
        }
        if cost <= 0.0 {
            continue; // sin sesiones en la ventana, ese CLAUDE.md no viajó
        }
        let mut nums = reds
            .iter()
            .take(6)
            .map(|(ln_no, _)| format!("L{}", ln_no))
            .collect::<Vec<_>>()
            .join(", ");
        if reds.len() > 6 {
            nums.push_str(" …");
        }
        findings.push(Finding {
            kind: "claudemd".into(),
            count: reds.len() as u64,
            file: format!("{} · {}", ruta, nums),
            project: pj.clone().unwrap_or_default(),
            tokens: tok_est,
            cost,
            estimated: true,
            ..Default::default()
        });
    }
    findings.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
    findings.truncate(MAX_FINDINGS);
    findings
}

/// Analizador de fugas: detectores locales (este PC + WSL) más los de cada
/// servidor vía --findings, con el origen etiquetado por quien lee. Async +
/// spawn_blocking obligatorio (invariante 10ter: SSH y escaneo de disco).
#[tauri::command]
async fn get_findings(days: Option<u32>) -> Result<Vec<Finding>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let window_days = days.unwrap_or(7).clamp(1, 90);
        let mut out = scan_local_findings(window_days);
        for r in load_remotes() {
            let Some(rem) = fetch_remote(&r, window_days, false, true) else { continue };
            for mut f in rem.findings {
                f.origin = r.name.clone();
                out.push(f);
            }
        }
        out.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(MAX_FINDINGS);
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Coach de sesión activa (docs/consejos-coach.md §3-§4, nivel 2 de frescura):
// el panel sondea cada ciclo la COLA de los logs tocados hace poco y evalúa
// un catálogo CORTO de reglas medibles. Sin hooks: MichiClaude sigue afuera
// mirando archivos. SOLO fuentes locales de esta máquina — el consejo es
// para la sesión que tienes en el teclado, no para un servidor remoto (y el
// exportador no participa: divergencia documentada, como el WSL en Python).
// La lectura es INCREMENTAL por offset: cada archivo se parsea entero una
// sola vez y de ahí en adelante solo los bytes añadidos, así el sondeo de
// 3 min cuesta casi nada aunque la sesión pese cientos de MB.
// El anti-spam (una vez por sesión por regla + tope diario) vive en el
// FRONTEND: aquí solo se reportan los hechos medidos actuales.
// ---------------------------------------------------------------------------

const COACH_ACTIVE_MIN: i64 = 30; // minutos sin tocar el log = sesión dormida
const COACH_CTX_HIGH: u64 = 120_000; // tokens de contexto para sugerir /compact
const COACH_GAP_MIN: i64 = 6; // minutos de pausa para avisar del caché vencido
const COACH_GAP_CTX: u64 = 30_000; // ...solo si hay contexto que valga la pena
const COACH_REREAD: u32 = 3; // lecturas del mismo archivo en la sesión
const COACH_SUM_QUIET: i64 = 10; // minutos quieta = sesión terminada: resumen
const COACH_SUM_MIN_TURNS: u64 = 5; // por debajo no hay nada que resumir
// Aviso al celular de "tu agente terminó": antes que el resumen (que es una
// tarjeta para cuando vuelvas) porque este es para cuando NO estás — cinco
// minutos de silencio ya significan que la tarea acabó, y esperar diez es
// tenerte esperando de más.
const COACH_DONE_QUIET: i64 = 5;
const COACH_DONE_TURNS: u64 = 5; // un chat corto no vale una notificación

#[derive(Default)]
struct CoachSess {
    offset: u64,
    last_ctx: u64,      // cache_read+cache_write del último turno principal
    first_turn: i64,    // epoch del primer turno con usage (para la duración)
    last_turn: i64,     // epoch del último turno con usage
    turns: u64,
    cmds: u64,          // tool_use Bash (comandos ejecutados)
    reads: HashMap<String, u32>,
    edits: HashSet<String>, // archivos tocados con Edit/Write/NotebookEdit
    tool_ids: HashSet<String>, // dedup de tool_use (reanudaciones copian líneas)
    title: String,      // ai-title del log — SOLO display, campo interno
    proj: String,       // nombre real del proyecto, del `cwd` de la sesión
    cost: f64,          // costo MEDIDO de la sesión (usage × tarifa por turno)
    gaps: u64,          // pausas ≥6 min con contexto grande (caché reescrito)
    done: bool,         // el resumen ya se emitió: una vez por sesión
    notified: bool,     // el aviso de "terminó" ya salió: una vez por sesión
}

/// Una fuga detectada al CIERRE de la sesión (mini-auditoría del coach):
/// hechos medidos en memoria, nunca re-escaneo de disco. `kind` casa con
/// las fichas del catálogo (reread→attach, ctx→compact, gap→cache).
#[derive(Serialize, Clone)]
struct CoachLeak {
    kind: String,
    file: String,
    n: u64,
}

/// Mini-auditoría de la sesión que acaba de cerrar: solo reglas que ya
/// sabemos medir al vuelo. Subagentes/hooks siguen siendo de la pasada
/// diaria de Hallazgos, que mira ventanas de días.
fn coach_leaks(st: &CoachSess) -> Vec<CoachLeak> {
    let mut out = Vec::new();
    if let Some((f, n)) = st
        .reads
        .iter()
        .filter(|(_, n)| **n >= COACH_REREAD)
        .max_by_key(|(_, n)| **n)
    {
        let base = f.replace('\\', "/");
        let base = base.rsplit('/').next().unwrap_or(f).to_string();
        out.push(CoachLeak { kind: "reread".into(), file: base, n: *n as u64 });
    }
    if st.last_ctx >= COACH_CTX_HIGH {
        out.push(CoachLeak { kind: "ctx".into(), file: String::new(), n: st.last_ctx / 1000 });
    } else if st.last_ctx >= COACH_GAP_CTX {
        // Cerró con contexto grande (sin llegar a los 120k del /compact):
        // el usuario está lejos —por eso hay push— y el TTL del caché es de
        // minutos, así que para cuando lo lea ya venció. SIN esta rama el
        // push decía "terminó" a los 5 min y el panel sacaba el consejo del
        // caché al minuto siguiente (la regla viva pide 6 min de pausa):
        // dos historias distintas por 60 segundos (lo cazó Oscar 2026-08-02
        // en su segunda prueba real).
        out.push(CoachLeak { kind: "cache".into(), file: String::new(), n: st.last_ctx / 1000 });
    }
    if st.gaps > 0 {
        out.push(CoachLeak { kind: "gap".into(), file: String::new(), n: st.gaps });
    }
    out
}

#[derive(Serialize, Clone, Default)]
struct CoachHit {
    rule: String,    // id de la ficha de Consejos — o "sum" para el resumen
    session: String, // sid corto, para el "una vez por sesión" del frontend
    value: u64,      // el dato medido (en "sum": minutos de duración)
    project: String, // carpeta de logs: con varias sesiones abiertas (VPS +
                     // local) el usuario necesita saber a CUÁL aplicar el
                     // consejo (lo pidió Oscar al validar, 2026-07-31)
    title: String,   // solo "sum": el ai-title (vacío = respaldo al proyecto)
    cmds: u64,       // solo "sum": comandos ejecutados
    edits: u64,      // solo "sum": archivos editados distintos
    turns: u64,      // "sum"/"done": turnos de la sesión
    cost: f64,       // "sum"/"done": costo medido de la sesión (equiv. API)
    leaks: Vec<CoachLeak>, // "sum"/"done": mini-auditoría al cierre
}

static COACH_STATE: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, CoachSess>>> =
    std::sync::OnceLock::new();

/// Nombre a ENSEÑAR de una sesión de hallazgos: el real del `cwd` y, si el
/// log no lo trajo, el de la carpeta de logs (que puede venir recortado).
fn sdisp(s: &SessFindings) -> String {
    if s.disp.is_empty() {
        s.proj.clone()
    } else {
        s.disp.clone()
    }
}

/// Última carpeta del `cwd` de una línea del log (el nombre real del
/// proyecto). None si la línea no lo trae.
fn cwd_name(v: &serde_json::Value) -> Option<String> {
    let c = v["cwd"].as_str()?;
    let s = c.replace('\\', "/");
    s.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|x| !x.is_empty())
        .map(String::from)
}

/// Nombre del proyecto LISTO PARA ENSEÑAR: el real del `cwd` y, mientras el
/// log no lo haya traído, el de la carpeta de logs recortado como último
/// recurso. Sale de aquí ya resuelto para que el panel no tenga que
/// adivinarlo: ese recorte es el que convertía "test-agente" en "agente".
fn pname(st: &CoachSess, dir: &str) -> String {
    if !st.proj.is_empty() {
        return st.proj.clone();
    }
    if let Some(i) = dir.find("projects-") {
        return dir[i + 9..].to_string();
    }
    let t = dir.trim_start_matches('-');
    t.rsplit('-').next().filter(|s| !s.is_empty()).unwrap_or(t).to_string()
}

fn coach_scan() -> Vec<CoachHit> {
    let mut hits: Vec<CoachHit> = Vec::new();
    let now = Utc::now().timestamp();
    let states = COACH_STATE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let Ok(mut states) = states.lock() else { return hits };
    let pdir = claude_dir().join("projects");
    let Ok(projs) = fs::read_dir(&pdir) else { return hits };
    for proj in projs.flatten() {
        let ppath = proj.path();
        if !ppath.is_dir() {
            continue;
        }
        let proj_name = proj.file_name().to_string_lossy().to_string();
        let Ok(files) = fs::read_dir(&ppath) else { continue };
        for f in files.flatten() {
            let fp = f.path();
            if fp.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(md) = f.metadata() else { continue };
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            // solo sesiones vivas; las dormidas ni se abren
            if now - mtime > COACH_ACTIVE_MIN * 60 {
                continue;
            }
            let size = md.len();
            let st = states.entry(fp.clone()).or_default();
            if size < st.offset {
                *st = CoachSess::default(); // el archivo se truncó: de cero
            }
            if size > st.offset {
                use std::io::{Read, Seek};
                let Ok(mut fh) = fs::File::open(&fp) else { continue };
                if fh.seek(std::io::SeekFrom::Start(st.offset)).is_err() {
                    continue;
                }
                let mut buf = Vec::with_capacity((size - st.offset) as usize);
                if fh.read_to_end(&mut buf).is_err() {
                    continue;
                }
                // solo líneas COMPLETAS: lo que siga a la última \n se relee
                // en el próximo ciclo, cuando ya esté cerrado
                let cut = match buf.iter().rposition(|b| *b == b'\n') {
                    Some(i) => i + 1,
                    None => continue,
                };
                st.offset += cut as u64;
                let text = String::from_utf8_lossy(&buf[..cut]);
                for line in text.lines() {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                        continue;
                    };
                    let ts = v["timestamp"]
                        .as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|d| d.timestamp());
                    // Nombre REAL del proyecto, del `cwd` de la propia sesión.
                    // El nombre de la CARPETA de logs no sirve: codifica la
                    // ruta entera cambiando cada separador por "-", así que
                    // "…\Claude\test-agente" queda como
                    // "C--Users-oscar-Claude-test-agente" y no hay forma de
                    // saber dónde acaba la ruta y empieza el nombre — al
                    // recortarlo salía "agente" (visto por Oscar 2026-08-01
                    // en el aviso al celular y en la ficha del coach).
                    if st.proj.is_empty() {
                        if let Some(base) = cwd_name(&v) {
                            st.proj = base;
                        }
                    }
                    // título de la sesión: Claude Code lo escribe él mismo en
                    // el log (campo interno — SOLO display, nunca lógica)
                    if v["type"].as_str() == Some("ai-title") {
                        if let Some(t2) = v["aiTitle"].as_str() {
                            if !t2.trim().is_empty() {
                                st.title = t2.trim().to_string();
                            }
                        }
                    }
                    let usage = &v["message"]["usage"];
                    if usage.is_object() && !v["isSidechain"].as_bool().unwrap_or(false) {
                        let cr = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                        let cw = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                        let ctx = cr + cw;
                        // pausa larga con contexto grande ANTES de este turno:
                        // el caché venció y la conversación se reescribió — se
                        // cuenta aquí para la mini-auditoría del cierre
                        if let Some(t2) = ts {
                            if st.last_turn > 0
                                && t2 - st.last_turn >= COACH_GAP_MIN * 60
                                && st.last_ctx >= COACH_GAP_CTX
                            {
                                st.gaps += 1;
                            }
                        }
                        if ctx > 0 {
                            st.last_ctx = ctx;
                        }
                        st.turns += 1;
                        // costo MEDIDO del turno, con la misma tarifa que el
                        // resto del panel (tabla descargada → embebida)
                        let model = v["message"]["model"].as_str().unwrap_or("unknown");
                        st.cost += cost_of(
                            model,
                            usage["input_tokens"].as_u64().unwrap_or(0),
                            usage["output_tokens"].as_u64().unwrap_or(0),
                            cw,
                            cr,
                        );
                        if let Some(t2) = ts {
                            if st.first_turn == 0 {
                                st.first_turn = t2;
                            }
                            st.last_turn = t2;
                        }
                    }
                    if let Some(blocks) = v["message"]["content"].as_array() {
                        for b in blocks {
                            if b["type"].as_str() != Some("tool_use") {
                                continue;
                            }
                            let id = b["id"].as_str().unwrap_or("");
                            if id.is_empty() || !st.tool_ids.insert(id.to_string()) {
                                continue; // repetido: reanudaciones copian líneas
                            }
                            match b["name"].as_str().unwrap_or("") {
                                "Read" => {
                                    if let Some(p) = b["input"]["file_path"].as_str() {
                                        *st.reads.entry(p.to_string()).or_insert(0) += 1;
                                    }
                                }
                                "Bash" => st.cmds += 1,
                                "Edit" | "Write" | "NotebookEdit" => {
                                    if let Some(p) = b["input"]["file_path"].as_str() {
                                        st.edits.insert(p.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            // reglas sobre el estado acumulado; el sid corto sale del nombre
            // del archivo, que en Claude Code es el uuid de la sesión
            let sid: String = fp
                .file_stem()
                .map(|s| s.to_string_lossy().chars().take(8).collect())
                .unwrap_or_default();
            if st.last_ctx >= COACH_CTX_HIGH {
                hits.push(CoachHit {
                    rule: "compact".into(),
                    session: sid.clone(),
                    value: st.last_ctx / 1000, // se enseña en k
                    project: pname(st, &proj_name),
                    ..Default::default()
                });
            }
            let gap_min = (now - st.last_turn) / 60;
            if st.last_turn > 0 && gap_min >= COACH_GAP_MIN && st.last_ctx >= COACH_GAP_CTX {
                hits.push(CoachHit {
                    rule: "cache".into(),
                    session: sid.clone(),
                    value: gap_min.max(0) as u64,
                    project: pname(st, &proj_name),
                    ..Default::default()
                });
            }
            if let Some((_, n)) = st
                .reads
                .iter()
                .filter(|(_, n)| **n >= COACH_REREAD)
                .max_by_key(|(_, n)| **n)
            {
                hits.push(CoachHit {
                    rule: "attach".into(),
                    session: sid.clone(),
                    value: *n as u64,
                    project: pname(st, &proj_name),
                    ..Default::default()
                });
            }
            // resumen de sesión (docs/consejos-coach.md §8): la sesión que
            // ESTUVO viva se quedó quieta — una tarjeta-espejo con lo medido.
            // Solo si hubo trabajo de verdad; una vez por sesión (done). Si
            // MichiClaude no estuvo abierto durante la sesión no hay estado
            // acumulado y no hay resumen — limitación asumida de la v1.
            let quiet_min = now.saturating_sub(mtime) / 60;
            // "tu agente terminó": va ANTES que el resumen y por otro canal
            // (el celular). El frontend decide si empujarlo y vuelve a
            // deduplicar: este estado vive en memoria, así que al reiniciar
            // la app una sesión recién callada podría reaparecer aquí.
            if !st.notified && quiet_min >= COACH_DONE_QUIET && st.turns >= COACH_DONE_TURNS {
                st.notified = true;
                let mins = ((st.last_turn - st.first_turn) / 60).max(1) as u64;
                hits.push(CoachHit {
                    rule: "done".into(),
                    session: sid.clone(),
                    value: mins,
                    project: pname(st, &proj_name),
                    title: st.title.clone(),
                    cmds: st.cmds,
                    edits: st.edits.len() as u64,
                    turns: st.turns,
                    cost: st.cost,
                    leaks: coach_leaks(st),
                });
            }
            if !st.done && quiet_min >= COACH_SUM_QUIET && st.turns >= COACH_SUM_MIN_TURNS {
                st.done = true;
                let mins = ((st.last_turn - st.first_turn) / 60).max(1) as u64;
                hits.push(CoachHit {
                    rule: "sum".into(),
                    session: sid,
                    value: mins,
                    project: pname(st, &proj_name),
                    title: st.title.clone(),
                    cmds: st.cmds,
                    edits: st.edits.len() as u64,
                    turns: st.turns,
                    cost: st.cost,
                    leaks: coach_leaks(st),
                });
            }
        }
    }
    hits
}

/// Sondeo del coach: async + spawn_blocking (invariante 10ter — toca disco).
#[tauri::command]
async fn get_coach() -> Result<Vec<CoachHit>, String> {
    tauri::async_runtime::spawn_blocking(coach_scan)
        .await
        .map_err(|e| e.to_string())
}

/// Escapa un campo de CSV: comillas alrededor y las internas duplicadas. Antes
/// se sustituían las comas por espacios, que mutila el dato en vez de citarlo.
fn csv_field(v: &str) -> String {
    format!("\"{}\"", v.replace('"', "\"\""))
}

/// Exporta los datos agregados a CSV o JSON. `dir` vacío = carpeta Descargas.
/// Devuelve la ruta del archivo escrito.
/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
#[tauri::command]
async fn export_data(
    format: String,
    dir: Option<String>,
    days: Option<u32>,
    headers: Option<Vec<String>>,
    local: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_data_impl(format, dir, days, headers, local)
    })
        .await
        .map_err(|e| e.to_string())?
}

/// Último archivo exportado. Se guarda AQUÍ y no se acepta una ruta desde el
/// panel: abrir lo que diga el frontend sería abrir lo que diga cualquiera
/// que consiga hablarle. Solo se puede abrir lo que esta misma app escribió.
static LAST_EXPORT: std::sync::OnceLock<std::sync::Mutex<Option<PathBuf>>> =
    std::sync::OnceLock::new();

/// Abre el explorador con el último archivo exportado seleccionado.
#[tauri::command]
fn open_export() {
    let Some(p) = LAST_EXPORT
        .get()
        .and_then(|m| m.lock().ok())
        .and_then(|g| g.clone())
    else {
        return;
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(p.parent().unwrap_or(&p))
            .spawn();
    }
}

fn export_data_impl(
    format: String,
    dir: Option<String>,
    days: Option<u32>,
    headers: Option<Vec<String>>,
    local: Option<String>,
) -> Result<String, String> {
    if let Some(l) = local.filter(|s| !s.trim().is_empty()) {
        if let Ok(mut g) = LOCAL_LABEL
            .get_or_init(|| std::sync::Mutex::new(String::new()))
            .lock()
        {
            *g = l;
        }
    }
    let rows = collect_export_rows(days.unwrap_or(7).clamp(1, 90));
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
        let h = headers.filter(|v| v.len() == 6).unwrap_or_else(|| {
            ["Date", "Project", "Model", "Source", "Estimated cost (USD)", "Tokens"]
                .iter()
                .map(|x| x.to_string())
                .collect()
        });
        // BOM: sin él, Excel abre el .csv como texto de Windows y un "·" se
        // ve como "Â·". Tres bytes que evitan que todos los acentos salgan
        // rotos (2026-07-29).
        let mut s = String::from("\u{feff}");
        s.push_str(&h.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(","));
        s.push('\n');
        for r in &rows {
            s.push_str(&format!(
                "{},{},{},{},{:.4},{}\n",
                csv_field(&r.date),
                csv_field(&r.project),
                csv_field(&r.model),
                csv_field(&r.origin),
                r.cost,
                r.tokens
            ));
        }
        s
    } else {
        // MISMOS datos que el CSV, más lo único que un CSV no puede llevar:
        // de cuándo es y de qué ventana. Quien lo procese con un script no
        // tiene que adivinarlo por el nombre del archivo.
        serde_json::to_string_pretty(&serde_json::json!({
            "generated_at": Utc::now().to_rfc3339(),
            "window_days": days.unwrap_or(7).clamp(1, 90),
            "cost_note": "equiv. API, solo Claude Code",
            "rows": rows,
        }))
        .map_err(|e| e.to_string())?
    };
    fs::write(&path, content).map_err(|e| e.to_string())?;
    if let Ok(mut g) = LAST_EXPORT
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
    {
        *g = Some(path.clone());
    }
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_quota,
            get_local_stats,
            get_coach,
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
            drag_pill_from_card,
            open_faq_issue,
            save_hub_config,
            load_hub_config,
            check_update,
            install_update,
            open_releases,
            open_export,
            is_dev,
            get_pill_layer,
            set_pill_layer,
            get_prices_status,
            set_prices_auto,
            refresh_prices_now,
            hover_card,
            set_notif_visible,
            set_tray_menu,
            get_findings,
            pill_moved,
            get_ntfy,
            save_ntfy,
            ntfy_push,
            ntfy_qr,
            ntfy_regen
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
                let cfg = load_pill_config();
                // Solo el par que corresponde al estilo elegido; el otro se
                // crea si el usuario cambia de widget. Ahí va el make_noactivate
                // de esas cuatro ventanas (ver ensure_widget_windows).
                ensure_widget_windows(app.handle(), &cfg.style);
                // el globo de avisos sale con los DOS estilos, así que sigue
                // declarado en tauri.conf.json y se le aplica aquí
                if let Some(w) = app.get_webview_window("notif") {
                    #[cfg(windows)]
                    if let Ok(h) = w.hwnd() {
                        win_taskbar::make_noactivate(h.0 as isize);
                    }
                }
                if cfg.visible {
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

/// Abre un issue de GitHub PRE-LLENADO con las búsquedas sin ficha de la
/// pestaña Consejos (faqMisses, docs/consejos-coach.md §9). La BASE de la
/// URL es una constante; título y cuerpo llegan del panel YA
/// percent-encodados (encodeURIComponent) y aquí solo se valida que lo
/// estén — cualquier otro carácter descarta la apertura entera. En
/// Windows se lanza con rundll32 (NO con `cmd /C start`: cmd re-parsea la
/// línea y el `&` de la query la partiría en dos comandos).
#[tauri::command]
fn open_faq_issue(title: String, body: String) {
    let enc_ok = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "%-_.!~*'()".contains(c))
    };
    if !enc_ok(&title) || !enc_ok(&body) {
        return;
    }
    let url = format!("{}?title={}&body={}", ISSUES_URL, title, body);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
}

// ---------- Avisos al celular (ntfy) ----------
// Push opcional al teléfono vía ntfy (por defecto el servidor público
// ntfy.sh; cambiable a mano en ntfy_config.json, como las URLs de precios —
// el self-host sale gratis). Reglas fijas: APAGADO por defecto; el topic es
// la contraseña del canal (aleatorio criptográfico, jamás se loggea fuera de
// su config); los TEXTOS llegan del panel ya traducidos — Rust no redacta
// avisos (la regla del menú del tray, invariante #10) —; y por este canal
// viajan SOLO porcentajes y horas de reset: nunca nombres de proyecto,
// rutas ni dólares. Se publica en JSON (POST a la raíz del servidor) a
// propósito: los headers HTTP no aguantan UTF-8 y los avisos van en 8
// idiomas.

#[derive(Serialize, Deserialize, Clone)]
struct NtfyConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    topic: String,
    #[serde(default = "ntfy_default_server")]
    server: String,
    #[serde(default)]
    alarms: bool,
    /// Avisar cuando una sesión larga de Claude Code termina (queda quieta).
    #[serde(default)]
    done: bool,
    /// Incluir el NOMBRE del proyecto en ese aviso. Apagado por defecto y a
    /// propósito: los canales de ntfy son públicos, y la regla general es que
    /// por ahí no viajan nombres. Quien tenga su canal solo para él puede
    /// asumirlo; la casilla lo advierte.
    #[serde(default)]
    names: bool,
}

fn ntfy_default_server() -> String {
    "https://ntfy.sh".into()
}

impl Default for NtfyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            topic: String::new(),
            server: ntfy_default_server(),
            alarms: false,
            done: false,
            names: false,
        }
    }
}

fn ntfy_config_path() -> PathBuf {
    app_data_dir().join("ntfy_config.json")
}

fn load_ntfy_config() -> NtfyConfig {
    fs::read_to_string(ntfy_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Topic nuevo: "michi-" + 12 símbolos [a-z0-9] del CSPRNG (~62 bits). En
/// ntfy el topic ES la contraseña — con SystemTime sería adivinable.
fn ntfy_new_topic() -> Result<String, String> {
    let mut buf = [0u8; 12];
    getrandom::getrandom(&mut buf).map_err(|e| e.to_string())?;
    const CS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let tail: String = buf
        .iter()
        .map(|b| CS[(*b as usize) % CS.len()] as char)
        .collect();
    Ok(format!("michi-{tail}"))
}

#[tauri::command]
fn get_ntfy() -> NtfyConfig {
    load_ntfy_config()
}

/// Guarda la config; al activar por primera vez inventa el topic. Devuelve
/// la config final para que el panel pinte topic y QR sin releer.
#[tauri::command]
fn save_ntfy(mut cfg: NtfyConfig) -> Result<NtfyConfig, String> {
    if cfg.enabled && cfg.topic.is_empty() {
        cfg.topic = ntfy_new_topic()?;
    }
    if cfg.server.trim().is_empty() {
        cfg.server = ntfy_default_server();
    }
    let s = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    let dir = app_data_dir();
    let _ = fs::create_dir_all(&dir);
    fs::write(ntfy_config_path(), s).map_err(|e| e.to_string())?;
    Ok(cfg)
}

/// Canal NUEVO, para cuando el actual se filtró — en ntfy el topic es la
/// contraseña, y un QR visible en una captura de pantalla basta para
/// regalarla. Se genera otro topic y el viejo queda muerto: quien lo
/// tuviera deja de recibir, y el teléfono propio debe re-escanear.
#[tauri::command]
fn ntfy_regen() -> Result<NtfyConfig, String> {
    let mut cfg = load_ntfy_config();
    cfg.topic = ntfy_new_topic()?;
    let s = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    let dir = app_data_dir();
    let _ = fs::create_dir_all(&dir);
    fs::write(ntfy_config_path(), s).map_err(|e| e.to_string())?;
    Ok(cfg)
}

/// Deja el último fallo en ntfy_debug.json (código y hora — el topic jamás
/// se escribe ahí) y devuelve el mismo código para que el panel lo traduzca.
fn ntfy_fail(code: String) -> String {
    let dir = app_data_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(
        dir.join("ntfy_debug.json"),
        format!(
            "{{\"last_error\":\"{}\",\"at\":\"{}\"}}",
            code,
            chrono::Utc::now().to_rfc3339()
        ),
    );
    code
}

/// El POST genérico. `at` = timestamp Unix (segundos) para entrega
/// PROGRAMADA: ntfy retiene el mensaje y lo entrega a esa hora aunque la PC
/// esté APAGADA — es lo que convierte "límite alcanzado" en "puedes apagar,
/// yo te aviso". Máximo 3 días en ntfy.sh; ese guard vive en el panel.
/// Async por la regla 10ter: es red, no puede congelar la interfaz.
#[tauri::command]
async fn ntfy_push(
    title: String,
    body: String,
    priority: u8,
    at: Option<i64>,
) -> Result<(), String> {
    let cfg = load_ntfy_config();
    if !cfg.enabled || cfg.topic.is_empty() {
        return Err("ERR_NTFY_OFF".into());
    }
    let mut payload = serde_json::json!({
        "topic": cfg.topic,
        "title": title,
        "message": body,
        "priority": priority.clamp(1, 5),
    });
    if let Some(ts) = at {
        payload["delay"] = serde_json::Value::String(ts.to_string());
    }
    let client = reqwest::Client::new();
    let resp = client
        .post(cfg.server.trim_end_matches('/'))
        .json(&payload)
        .send()
        .await
        .map_err(|_| ntfy_fail("ERR_NET".into()))?;
    if !resp.status().is_success() {
        return Err(ntfy_fail(format!("ERR_NTFY:{}", resp.status().as_u16())));
    }
    Ok(())
}

/// Matriz del QR con el enlace de suscripción (ntfy://host/topic — el enlace
/// profundo que la app ntfy abre ya suscrita; la app NO trae escáner, se usa
/// la cámara normal). El panel la pinta en un canvas: ni PNG ni dependencia
/// de imagen. Siempre sobre fondo claro al pintarlo: un QR invertido no lo
/// leen todas las cámaras.
#[tauri::command]
fn ntfy_qr() -> Result<serde_json::Value, String> {
    let cfg = load_ntfy_config();
    if cfg.topic.is_empty() {
        return Err("ERR_NTFY_OFF".into());
    }
    let host = cfg
        .server
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let link = format!("ntfy://{}/{}", host, cfg.topic);
    let code = qrcode::QrCode::new(link.as_bytes()).map_err(|e| e.to_string())?;
    let cells: String = code
        .to_colors()
        .iter()
        .map(|c| if *c == qrcode::Color::Dark { '1' } else { '0' })
        .collect();
    Ok(serde_json::json!({ "size": code.width(), "cells": cells }))
}

/// Guarda los ajustes del panel en el hub, para que otra PC los herede.
/// Se escribe en TODOS los servidores configurados: si mañana uno no está,
/// los ajustes siguen en el otro.
///
/// A propósito NO es automático. Una sincronización de ida y vuelta en cada
/// ciclo acabaría pisando en un lado lo que acabas de cambiar en el otro, y
/// unos ajustes que cambian solos son peores que unos que no se comparten.
/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
#[tauri::command]
async fn save_hub_config(json: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || save_hub_config_impl(json))
        .await
        .map_err(|e| e.to_string())?
}

fn save_hub_config_impl(json: String) -> Result<usize, String> {
    let remotes = load_remotes();
    if remotes.is_empty() {
        return Err("ERR_NO_REMOTES".into());
    }
    let mut ok = 0usize;
    for r in &remotes {
        if ssh_write_file(&r.host, "~/.michiclaude/config.json", &json).is_ok() {
            ok += 1;
        }
    }
    if ok == 0 { Err("ERR_HUB_SAVE".into()) } else { Ok(ok) }
}

/// Trae los ajustes guardados. Gana el primer servidor que responda: con un
/// hub personal todos tienen lo mismo, y pedirle al usuario que elija cuál
/// sería preguntarle algo que no puede saber.
/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
#[tauri::command]
async fn load_hub_config() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || load_hub_config_impl())
        .await
        .map_err(|e| e.to_string())?
}

fn load_hub_config_impl() -> Result<String, String> {
    for r in load_remotes() {
        let mut cmd = std::process::Command::new("ssh");
        cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
            .arg(&r.host)
            .arg("cat ~/.michiclaude/config.json");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if txt.starts_with('{') {
                    return Ok(txt);
                }
            }
        }
    }
    Err("ERR_HUB_NO_CONFIG".into())
}

/// Escribe un archivo en el servidor por SSH, mandando el contenido por
/// stdin: así no hay que citar nada ni cabe en la línea de comandos.
fn ssh_write_file(host: &str, path: &str, content: &str) -> Result<(), String> {
    use std::io::Write;
    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("~");
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg(host)
        .arg(format!("mkdir -p {dir} && cat > {path}"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or("ERR_SSH_STDIN")?
        .write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    if child.wait().map_err(|e| e.to_string())?.success() {
        Ok(())
    } else {
        Err("ERR_SSH_WRITE".into())
    }
}

/// Dirección de descargas. Va ESCRITA AQUÍ a propósito y nunca sale de un
/// archivo descargado: un aviso manipulado podría cambiar el texto, pero
/// jamás a dónde lleva el botón.
const RELEASES_URL: &str = "https://github.com/oscarorozcos/michiclaude/releases/latest";
/// Base del issue pre-llenado de faqMisses. CONSTANTE, como RELEASES_URL:
/// el destino de un botón jamás sale de datos externos.
const ISSUES_URL: &str = "https://github.com/oscarorozcos/michiclaude/issues/new";

/// ¿Hay versión nueva? Devuelve su número, o nada. Un fallo de red no es un
/// error que merezca molestar: se devuelve `None` y se reintenta otro día.
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let up = app.updater().map_err(|e| e.to_string())?;
    match up.check().await {
        Ok(Some(u)) => Ok(Some(u.version.clone())),
        Ok(None) => Ok(None),
        Err(_) => Ok(None),
    }
}

/// Descarga e instala. Si la FIRMA no cuadra —el caso de "se perdió la llave
/// y se generó otra"— falla aquí, y el panel enseña el aviso de descarga
/// manual en vez de dejar al usuario congelado en una versión vieja sin
/// enterarse. La app se reinicia sola al terminar.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let up = app.updater().map_err(|e| e.to_string())?;
    let Some(u) = up.check().await.map_err(|_| "ERR_UPD_CHECK".to_string())? else {
        return Err("ERR_UPD_NONE".into());
    };
    u.download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| "ERR_UPD_INSTALL".to_string())?;
    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

/// Abre la página de descargas en el navegador. La URL es la constante de
/// arriba: no se acepta ninguna que venga de fuera.
#[tauri::command]
fn open_releases() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", RELEASES_URL])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(RELEASES_URL).spawn();
    }
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

/// Traduce el menú del icono de bandeja (el del clic derecho).
///
/// Era el ÚNICO texto visible de la app que se quedaba en inglés, y no por
/// olvido: lo construye Rust al arrancar, mientras que el idioma elegido vive
/// en el localStorage del PANEL. Así que lo manda el panel — `applyI18n()`
/// llama aquí con las tres etiquetas ya traducidas, al cargar y cada vez que
/// se cambia de idioma. Lo vio Oscar el 2026-07-29 teniendo la app en español.
///
/// El menú se reconstruye entero en vez de guardar los tres items en el
/// estado para irles cambiando el texto: son tres líneas y ocurre dos veces
/// por sesión. Los ids NO cambian, que son los que enrutan `on_menu_event`.
#[tauri::command]
fn set_tray_menu(app: tauri::AppHandle, open: String, widget: String, quit: String) {
    use tauri::menu::{Menu, MenuItem};
    let (Ok(a), Ok(b), Ok(c)) = (
        MenuItem::with_id(&app, "tray_panel", open, true, None::<&str>),
        MenuItem::with_id(&app, "tray_pill", widget, true, None::<&str>),
        MenuItem::with_id(&app, "tray_quit", quit, true, None::<&str>),
    ) else {
        return;
    };
    let Ok(menu) = Menu::with_items(&app, &[&a, &b, &c]) else { return };
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_menu(Some(menu));
    }
}

/// Crea, solo si hacen falta, las dos ventanas del estilo de widget elegido.
///
/// La pastilla y el gatito son EXCLUYENTES: con el gatito puesto, `pill` y
/// `pcard` no se muestran JAMÁS, y al revés. Declarándolas las cuatro en
/// tauri.conf.json se creaban todas al arrancar, y cada WebView2 cuesta un
/// piso de ~57 MB aunque esté vacía y oculta: eran ~115 MB para no pintar
/// nada (medido en el Windows de Oscar el 2026-07-29 — ver la sección de
/// consumo de recursos en CLAUDE.md).
///
/// OJO, esto NO choca con la regla de "nunca redimensionar una ventana
/// transparente": esa prohíbe cambiar el TAMAÑO de una ventana viva, que es
/// lo que deja a WebView2 sin pintar. Aquí cada ventana nace con su tamaño
/// fijo y no se toca más.
///
/// Los valores son los mismos que tenían en tauri.conf.json. Si se cambia el
/// tamaño de una ventana del widget, se cambia AQUÍ (ya no está en el json).
fn ensure_widget_windows(app: &tauri::AppHandle, style: &str) {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    let wins: [(&str, &str, &str, f64, f64); 2] = if style == "cat" {
        [
            ("cat", "cat.html", "MichiClaude — cat", 210.0, 157.0),
            ("card", "card.html", "MichiClaude — card", 294.0, 322.0),
        ]
    } else {
        [
            // 280→250→280 el mismo 2026-07-31: los huecos semanal/por-modelo
            // salieron de la cápsula y VOLVIERON a las horas (Oscar los
            // extrañó), así que el ancho original volvió con ellos.
            ("pill", "pill.html", "MichiClaude — widget", 280.0, 56.0),
            ("pcard", "pcard.html", "MichiClaude — detalle", 280.0, 300.0),
        ]
    };
    for (label, url, title, w, h) in wins {
        if app.get_webview_window(label).is_some() {
            continue;
        }
        let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(w, h)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .focused(false)
            .visible(false)
            .build();
        match built {
            // el widget NUNCA roba el foco: sin esto, mostrarlo sacaría al
            // usuario de lo que estuviera escribiendo
            Ok(_win) => {
                #[cfg(windows)]
                if let Ok(hw) = _win.hwnd() {
                    win_taskbar::make_noactivate(hw.0 as isize);
                }
            }
            // que falle una ventana del widget no debe tumbar la app: el
            // panel y el icono de la bandeja siguen sirviendo igual
            Err(e) => eprintln!("MichiClaude: no se pudo crear la ventana {label}: {e}"),
        }
    }
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
/// ASYNC OBLIGATORIO, y por un motivo distinto al del invariante 10ter (que
/// es sobre operaciones lentas): este comando CREA una ventana, y hacerlo
/// desde un comando SÍNCRONO congela la app entera en Windows. La ventana
/// nueva necesita que el bucle de eventos avance para nacer, pero ese bucle
/// está detenido esperando a que el comando termine — se esperan mutuamente.
/// Siendo async, Tauri lo ejecuta fuera del hilo de la interfaz, el bucle
/// sigue vivo y la ventana se crea. Pasó en vivo el 2026-07-29: al cambiar a
/// la pastilla se quedaba el gatito en pantalla y el panel dejaba de
/// responder. En `setup()` la misma llamada sí puede ser síncrona, porque
/// ahí el bucle de eventos todavía no ha arrancado.
#[tauri::command]
async fn set_pill_style(app: tauri::AppHandle, style: String) {
    use tauri::Manager;
    let mut cfg = load_pill_config();
    let new_style = if style == "cat" { "cat" } else { "plain" };
    // El par del estilo nuevo puede no existir todavía (se crean bajo
    // demanda). Hay que crearlo ANTES de medir su alto unas líneas más
    // abajo: sin ventana no hay `outer_size`, y el widget aparecería
    // desplazado en vertical al cambiar de estilo.
    ensure_widget_windows(&app, new_style);
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
    // Y se liberan las del estilo que se acaba de abandonar: si solo se
    // ocultaran, quien probara los dos widgets acabaría con las cuatro
    // cargadas —los ~115 MB que este diseño evita— hasta reiniciar la app.
    // `destroy` en vez de `close` porque no queremos pasar por el evento de
    // cierre; el panel (`main`) sigue existiendo, así que la app no se cae
    // por quedarse sin ventanas. Vuelven a crearse solas si el usuario
    // cambia de opinión.
    let stale = if new_style == "cat" { ["pill", "pcard"] } else { ["cat", "card"] };
    for label in stale {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.destroy();
        }
    }
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

/// El asa de la cabecera del detalle arrastra DE VERDAD (2026-07-31: la
/// cabecera gemela parecía rota — su gatito no abría el panel y su asa no
/// movía nada). El truco: plegar, mostrar la pastilla — que quedó
/// EXACTAMENTE bajo la cabecera, porque el despliegue no la mueve — y
/// pasarle el arrastre del sistema en el mismo gesto, con el botón aún
/// apretado. La posición se persiste unos instantes después, como hace
/// pill_moved con su temporizador. Comando SÍNCRONO a propósito: toca
/// ventanas y debe correr en el hilo principal (invariante 10ter).
#[tauri::command]
fn drag_pill_from_card(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    let Some(card) = app.get_webview_window("pcard") else { return };
    let Some(pill) = app.get_webview_window("pill") else { return };
    let _ = card.hide();
    let _ = app.emit_to("pill", "pcard:closed", ());
    let _ = pill.show();
    let _ = pill.start_dragging();
    std::thread::spawn(move || {
        // el arrastre sigue vivo al volver este comando; se guarda la
        // posición varias veces para cubrir gestos de hasta ~2.5 s
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Ok(p) = pill.outer_position() {
                let mut cfg = load_pill_config();
                cfg.x = Some(p.x);
                cfg.y = Some(p.y);
                save_pill_config(&cfg);
            }
        }
    });
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

