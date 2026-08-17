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
    /// Turnos ÚTILES del usuario (mensajes humanos reales) en la ventana.
    /// Motor del reporte: tokens ÷ uturns = rendimiento. 0 = "sin datos"
    /// (exportador viejo o proyecto sin turnos): la UI NUNCA divide entre 0
    /// ni pinta un rendimiento con esto vacío (invariante #8).
    #[serde(default)]
    uturns: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct DailyAgg {
    date: String, // YYYY-MM-DD (UTC)
    cost: f64,
    /// Tokens "de trabajo" del día (input+output+cache_write, cache_read
    /// fuera — el mismo criterio de siempre). Con `default` un exportador
    /// viejo manda solo cost y estos quedan honestamente en 0.
    #[serde(default)]
    tokens: u64,
    /// Turnos útiles del usuario ese día (ver ProjectAgg::uturns).
    #[serde(default)]
    uturns: u64,
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
    /// Turnos útiles del usuario en la ventana (suma de todas las fuentes).
    /// tokens_week ÷ uturns_week = tokens por turno útil, la métrica del
    /// reporte. 0 = sin datos, nunca dividir (invariante #8).
    #[serde(default)]
    uturns_week: u64,
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
    /// % de desperdicio estructural del exportador: viaja junto a findings
    /// porque nace de la MISMA pasada. Exportador viejo = sin clave =
    /// ceros = ese origen queda fuera del numerador Y del denominador.
    #[serde(default)]
    waste: Waste,
    /// Integridad detectada EN EL SERVIDOR durante esta pasada (pieza 1).
    /// Quien lee les pone el origen, como a las filas del export. Vacío con
    /// un exportador viejo.
    #[serde(default)]
    integrity: Vec<IntegrityEvent>,
    /// true cuando había resúmenes de OTRAS máquinas del hub y se dejaron
    /// fuera por haber pedido un rango de fechas: sus fotos son de ventanas
    /// que terminan hoy y no se pueden recortar a un periodo pasado. El
    /// panel lo dice en pantalla — callarlo sería enseñar un total
    /// incompleto sin avisar (invariante #8).
    #[serde(default)]
    hub_skipped: bool,
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
    /// Última actividad (epoch) de la sesión que originó el hallazgo: el
    /// panel ordena los MÁS RECIENTES arriba (Oscar 2026-08-02). Los
    /// hallazgos "de estado" (mcp, skills, claudemd, mech) no tienen hora
    /// y se van abajo. serde(default): un exportador viejo manda 0.
    #[serde(default)]
    ts: i64,
}

/// % de desperdicio estructural (docs/presion-y-rendimiento.md §fórmula):
/// numerador = hallazgos de la CLASE estructural ANTES del tope de 12;
/// denominador = costo total de la MISMA pasada (mismo escaneo, misma
/// ventana — nunca se cruzan corridas). `items` lleva esas tarjetas SIN
/// recortar para que el panel descuente las ignoradas (fndIgnore) con su
/// misma clave. TODO con default: un exportador viejo no manda la clave
/// (o manda {}) y eso es "sin datos" (sessions=0), jamás un 0%.
#[derive(Serialize, Deserialize, Clone, Default)]
struct Waste {
    #[serde(default)]
    struct_cost: f64,
    #[serde(default)]
    struct_tokens: u64,
    #[serde(default)]
    total_cost: f64,
    #[serde(default)]
    sessions: u64,
    #[serde(default)]
    days: u32,
    #[serde(default)]
    end: i64,
    #[serde(default)]
    estimated: bool,
    #[serde(default)]
    items: Vec<Finding>,
}

/// Respuesta de get_findings: las tarjetas más el waste de la misma pasada.
#[derive(Serialize, Clone, Default)]
struct FindingsPack {
    findings: Vec<Finding>,
    waste: Waste,
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
    /// Techo de contexto del modelo en tokens (0 = la fuente no lo dijo).
    /// Viaja en la MISMA tabla que los precios porque las tres fuentes lo
    /// publican ahí: ni una descarga nueva ni una dependencia nueva. Es el
    /// denominador del manómetro de presión — ver `ctx_for()`.
    /// Aditivo: un caché en disco de una versión anterior no lo trae, se
    /// queda en 0 y el respaldo embebido responde hasta la próxima descarga.
    #[serde(default)]
    ctx: u64,
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

/// Punto entre dígitos → guión. OpenRouter escribe la versión con PUNTO
/// ("claude-opus-4.8") donde LiteLLM, models.dev y los propios logs usan
/// GUIÓN ("claude-opus-4-8"). Sin esto, la tercera fuente de la cascada solo
/// casaba 6 de sus modelos: si LiteLLM y models.dev caían, ocho modelos
/// vigentes —Opus 4.5/4.6/4.7/4.8, Sonnet 4.5/4.6, Haiku 4.5, Opus 4.1— se
/// quedaban sin precio Y sin techo, en silencio (cazado 2026-08-08 auditando
/// las tres fuentes). Solo entre dígitos: "anthropic.claude-opus-5" no se toca.
fn dots_to_dashes(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, ch) in c.iter().enumerate() {
        let between_digits = *ch == '.'
            && i > 0
            && c[i - 1].is_ascii_digit()
            && c.get(i + 1).map_or(false, |n| n.is_ascii_digit());
        out.push(if between_digits { '-' } else { *ch });
    }
    out
}

/// Clave normalizada para casar el id del log con el de las tablas públicas:
/// minúsculas, sin prefijo de proveedor ("anthropic/"), sin variante entre
/// corchetes ("[1m]"), con la versión en guiones y sin la fecha del snapshot.
/// La usan las DOS partes (guardar y buscar), así que cualquier normalización
/// nueva tiene que ir aquí dentro o los dos lados dejan de casar.
fn price_key(model: &str) -> String {
    let lower = model.to_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    let base = base.split('[').next().unwrap_or(base);
    let mut s = dots_to_dashes(base.trim());
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

/// Techo de contexto por defecto cuando no hay tabla descargada. 200k era el
/// techo de TODOS los modelos hasta Opus/Sonnet 4.6, que saltaron a 1M.
const CTX_FALLBACK: u64 = 200_000;
const CTX_1M: u64 = 1_000_000;

/// Respaldo embebido del techo de contexto, hermano de `price_table()`: decide
/// por VERSIÓN y no por lista de modelos (invariante #6), así una versión nueva
/// de una familia conocida hereda el techo correcto sola.
///
/// En la duda devuelve 200k A PROPÓSITO: quedarse corto hace que el manómetro
/// avise ANTES de tiempo (molesto), mientras que pasarse haría que no avisara
/// nunca (el usuario choca con el muro sin previo aviso). El fallo seguro de un
/// avisador es avisar de más.
fn ctx_table(model: &str) -> u64 {
    let m = model.to_lowercase();
    // La variante de contexto largo se marca en el propio id ("…[1m]"), y
    // `price_key()` la recorta antes de buscar en la tabla — así que se mira
    // aquí, sobre el id crudo, o se perdería.
    if m.contains("[1m]") {
        return CTX_1M;
    }
    let mut nums = m
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty() && t.len() != 8)
        .filter_map(|t| t.parse::<u32>().ok());
    let major = nums.next().unwrap_or(0);
    let minor = nums.next().unwrap_or(0);
    if m.contains("fable") || m.contains("mythos") {
        return CTX_1M;
    }
    if m.contains("haiku") {
        return CTX_FALLBACK;
    }
    // Opus y Sonnet saltaron a 1M en la 4.6; la 4.5 y anteriores siguen en 200k
    if (m.contains("opus") || m.contains("sonnet")) && (major > 4 || (major == 4 && minor >= 6)) {
        return CTX_1M;
    }
    CTX_FALLBACK
}

/// Techo de contexto REAL del modelo: primero la tabla descargada (la misma
/// cascada de los precios, que ya trae el dato), y si no lo dijo, el respaldo
/// embebido. Nunca devuelve 0 — es un denominador.
fn ctx_for(model: &str) -> u64 {
    if model.to_lowercase().contains("[1m]") {
        return CTX_1M;
    }
    match price_lookup(model) {
        Some(p) if p.ctx > 0 => p.ctx,
        _ => ctx_table(model),
    }
}

/// Escalones de ventana que existen de verdad. NO es una lista de modelos
/// (invariante #6): son magnitudes, y solo se usan cuando la realidad medida
/// contradice a la tabla.
const CTX_LADDER: [u64; 4] = [200_000, 1_000_000, 2_000_000, 5_000_000];

/// Techo que se le manda al panel, corregido con la EVIDENCIA de esta máquina.
///
/// Las tablas pueden equivocarse por abajo: `claude-sonnet-4-5` figura como
/// 200k en LiteLLM y como 1M en models.dev (es de 200k con un beta de 1M), así
/// que el número depende de qué fuente respondiera ese día. Pero si una sesión
/// YA superó el techo que dice la tabla, la tabla está demostrablemente mal:
/// los tokens medidos ganan. Se sube entonces al primer escalón por encima de
/// lo visto, en vez de devolver lo visto a secas — devolver `seen` dejaría el
/// manómetro clavado en 100%, que es justo el bug del que venimos.
fn ctx_full(model: &str, seen: u64) -> u64 {
    let base = ctx_for(model);
    if seen <= base {
        return base;
    }
    CTX_LADDER.iter().copied().find(|s| *s > seen).unwrap_or(seen)
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
                ctx: m["max_input_tokens"].as_u64().unwrap_or(0),
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
                // models.dev lo pone en `limit.context`; se acepta también un
                // `context` suelto por si el esquema cambia de sitio.
                ctx: m["limit"]["context"]
                    .as_u64()
                    .or_else(|| m["context"].as_u64())
                    .unwrap_or(0),
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
                ctx: it["context_length"]
                    .as_u64()
                    .or_else(|| it["top_provider"]["context_length"].as_u64())
                    .unwrap_or(0),
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
        // Cuántos de esos modelos traen además el TECHO de contexto. Viaja en
        // la misma tabla y por la misma cascada que los precios, así que se
        // enseña en el mismo sitio: si una fuente deja de publicarlo, este
        // número baja y se ve, en vez de degradarse en silencio.
        "ctx_count": cache.prices.values().filter(|p| p.ctx > 0).count(),
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

/// Uso de disco de los logs de UN servidor (exportador `--du`). SOLO
/// LECTURA: desde MichiClaude no se borra nada por SSH (Oscar 2026-08-15) —
/// el panel enseña ruta, peso y edad, y un comando ACOTADO POR EDAD para
/// que el usuario decida allá. Exportador viejo (sin --du) devuelve el JSON
/// normal → no parsea como RemoteDu → None, y el panel dice "actualiza".
#[derive(Serialize, Deserialize, Default, Clone)]
struct RemoteDu {
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    files: u64,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    old_files: u64,
    #[serde(default)]
    old_bytes: u64,
    #[serde(default)]
    oldest: i64,
}

#[tauri::command]
async fn get_remote_du() -> Result<Vec<RemoteDu>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out = Vec::new();
        for r in load_remotes() {
            let mut cmd = std::process::Command::new("ssh");
            cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
                .arg(&r.host)
                .arg(format!("{} --du", r.command));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            let Ok(o) = cmd.output() else { continue };
            if !o.status.success() {
                continue;
            }
            // un exportador viejo ignora --du y devuelve el resumen normal:
            // sin la clave "path" no es un informe de disco y se descarta
            let Ok(mut du) = serde_json::from_slice::<RemoteDu>(&o.stdout) else { continue };
            if du.path.is_empty() {
                continue;
            }
            du.name = r.name.clone();
            out.push(du);
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ruta canónica del exportador en el servidor. Solo se reinstala en los
/// remotos que apunten aquí: si el usuario puso su propia ruta, no se le
/// escribe nada en su máquina.
const REMOTE_SCRIPT_PATH: &str = "~/.michiclaude/meter-export.py";

/// El exportador viaja DENTRO del binario. Así el usuario no tiene que
/// copiarlo a mano ni saber dónde está, y cada actualización de MichiClaude
/// lo mantiene en sincronía con el backend (invariante 1).
const REMOTE_SCRIPT: &str = include_str!("../../scripts/meter-export.py");

/// El relevo para Linux viaja igual que el exportador: embebido y re-subido
/// solo. Es PYTHON a propósito — en Linux la PTY vive en la stdlib, y el VPS
/// no tiene toolchain de Rust; compilar michi.exe para Linux exigiría tocar
/// el workflow (invariante #9). Réplica de relevo/src/main.rs sin la rama
/// win32-input-mode (eso es ConPTY de Windows).
const REMOTE_RELEVO_PATH: &str = "~/.michiclaude/michi-relevo.py";
const REMOTE_RELEVO: &str = include_str!("../../scripts/michi-relevo.py");
/// Lanzador para `claudeCode.claudeProcessWrapper`: es lo que la extensión de
/// VS Code ejecuta en lugar de `claude`. Viaja con los otros dos.
const REMOTE_WRAP_PATH: &str = "~/.michiclaude/michi-wrap.sh";
const REMOTE_WRAP: &str = include_str!("../../scripts/michi-wrap.sh");

/// Sube un script al servidor por SSH escribiéndolo desde stdin (sin scp ni
/// permisos extra). Idempotente: sobrescribe siempre.
fn upload_script(host: &str, path: &str, body: &str) -> Result<(), String> {
    use std::io::Write;
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg(host)
        .arg(format!(
            "mkdir -p ~/.michiclaude && cat > {path} && chmod +x {path}"
        ))
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
        si.write_all(body.replace("\r\n", "\n").as_bytes())
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

/// Los dos scripts que viajan al servidor. El exportador es imprescindible
/// (sin él no hay datos); el relevo es cortesía — si su subida falla no
/// tumba el alta, la sesión simplemente no será remediable desde el panel.
fn upload_exporter(host: &str) -> Result<(), String> {
    upload_script(host, REMOTE_SCRIPT_PATH, REMOTE_SCRIPT)?;
    let _ = upload_script(host, REMOTE_RELEVO_PATH, REMOTE_RELEVO);
    let _ = upload_script(host, REMOTE_WRAP_PATH, REMOTE_WRAP);
    Ok(())
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
    end_ts: Option<i64>,
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
            "{} --days {} --exclude-host {}{}{}{}",
            r.command,
            window_days,
            hub_identity().id,
            // el exportador VIEJO ignora un flag que no conoce y sigue
            // devolviendo su ventana de siempre: nunca rompe, como mucho
            // manda datos hasta hoy en vez de hasta el fin del rango
            end_ts.map(|t| format!(" --end {t}")).unwrap_or_default(),
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
    uturns: u64,
}

/// Valor de un día en la serie de tendencia: (USD, tokens de trabajo,
/// turnos útiles). Antes era solo el USD; el reporte necesita los tres
/// para calcular rendimiento por periodo sin re-escanear.
type DayCell = (f64, u64, u64);

#[derive(Default)]
struct LocalAgg {
    seen: HashSet<String>,
    /// Dedup de turnos de usuario por uuid: igual que los de usage, un
    /// turno puede repetirse entre archivos (continuaciones de sesión).
    useen: HashSet<String>,
    projects: HashMap<String, ProjSlot>, // clave: ruta única de la carpeta
    models: HashMap<String, ModelAgg>,
    daily: HashMap<String, DayCell>, // YYYY-MM-DD -> (USD, tok, uturns), 30 días
    /// (fecha, proyecto, modelo, origen) -> (USD, tokens). Solo si want_rows.
    rows: HashMap<(String, String, String, String), (f64, u64)>,
    want_rows: bool,
    cost_today: f64,
    cost_window: f64,
    tokens_window: u64,
    uturns_window: u64,
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
/// v2 (2026-08-06): el caché guarda también los turnos de usuario (uturns).
/// Un caché v1 no los trae y devolvería 0 EN SILENCIO para archivos sin
/// cambios — el bump fuerza una reconstrucción completa única, que es
/// exactamente para lo que el caché es reconstruible por diseño.
// v3: user_turn_text excluye isCompactSummary (2026-08-14)
const SCAN_CACHE_VERSION: u32 = 3;
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

/// Turno de usuario ya parseado: cuándo y su uuid (para la dedup global).
#[derive(Serialize, Deserialize, Clone)]
struct CachedUTurn {
    #[serde(rename = "t")]
    ts: i64,
    #[serde(rename = "u", default)]
    id: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct CachedFile {
    len: u64,
    mtime: i64,
    display: Option<String>,
    entries: Vec<CachedEntry>,
    /// Turnos ÚTILES del usuario (mensajes humanos reales; fuera meta,
    /// sidechain, resultados de herramienta y comandos locales).
    #[serde(default)]
    uturns: Vec<CachedUTurn>,
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

// ---------------------------------------------------------------------------
// INTEGRIDAD DE LAS FUENTES (docs/adr-multiharness-y-persistencia.md, pieza 1)
// Los .jsonl NO son nuestros: un limpiador de disco (conversation-reclaim y
// parientes) o el propio usuario pueden recortarlos o borrarlos. Sin darse
// cuenta, MichiClaude leería menos y diría "bajó el consumo" — la mentira que
// prohíbe el invariante #8. El detector es PASIVO y casi gratis: el caché de
// escaneo ya guarda tamaño+mtime por archivo, así que un archivo que ENCOGIÓ
// (o que desapareció de una raíz que sí pudimos leer) se ve solo.
//
// Lo que NO detecta, y es a propósito (invariante #4, sin hashes ni offsets):
// una reescritura del mismo tamaño exacto. Un recorte real siempre encoge.
// Falsos positivos evitados por diseño:
//   - El ARCHIVADOR propio mueve archivos ≥365d, y el caché solo guarda los
//     de los últimos ~32 días: cero solape, nunca se acusa a la app misma.
//   - WSL apagado (o un servidor caído) deja su raíz ILEGIBLE: solo se
//     comparan las raíces que se pudieron leer en ESTA pasada.
//   - Caché nuevo o invalidado (bump de versión) = sin línea base = silencio.
// Local y privado: no viaja al hub ni a ntfy.
// ---------------------------------------------------------------------------

/// Un hecho de integridad ya agregado por (tipo, origen) de una pasada.
#[derive(Serialize, Deserialize, Clone, Default)]
struct IntegrityEvent {
    // TODOS con default: estos hechos llegan también del exportador remoto y
    // un campo ausente invalidaría la respuesta ENTERA (la mordida de
    // ExportRow.origin y Finding.ts, invariante #1).
    /// cuándo se DETECTÓ (epoch); el recorte pudo ser antes
    #[serde(default)]
    t: i64,
    /// "truncated" (encogió) | "vanished" (desapareció)
    #[serde(default)]
    kind: String,
    /// archivos afectados
    #[serde(default)]
    n: u64,
    /// bytes perdidos
    #[serde(default)]
    b: u64,
    /// "" = este PC · "wsl-<distro>" · nombre del servidor
    #[serde(default)]
    o: String,
}

#[derive(Serialize, Deserialize, Default)]
struct IntegrityLog {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    events: Vec<IntegrityEvent>,
}

const INTEGRITY_KEEP_DAYS: i64 = 400;
const INTEGRITY_MAX: usize = 200;
/// Dos detecciones idénticas tan seguidas son la MISMA (el ciclo llama al
/// escaneo varias veces por las ventanas del hub): se funden en una.
const INTEGRITY_MERGE_SECS: i64 = 120;

fn integrity_path() -> PathBuf {
    app_data_dir().join("integrity.json")
}

/// Anota hechos nuevos. Idempotente en la práctica: el caché se guarda con el
/// tamaño nuevo tras la pasada, así que un mismo recorte se ve UNA vez.
fn log_integrity(evs: Vec<IntegrityEvent>) {
    if evs.is_empty() {
        return;
    }
    let mut h: IntegrityLog = fs::read_to_string(integrity_path())
        .ok()
        .and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or_default();
    h.version = 1;
    for e in evs {
        // cinturón y tirantes contra el doble conteo entre ventanas del hub
        if let Some(last) = h
            .events
            .iter_mut()
            .rev()
            .find(|x| x.kind == e.kind && x.o == e.o)
        {
            if e.t - last.t < INTEGRITY_MERGE_SECS {
                last.n = last.n.max(e.n);
                last.b = last.b.max(e.b);
                continue;
            }
        }
        h.events.push(e);
    }
    let cutoff = Utc::now().timestamp() - INTEGRITY_KEEP_DAYS * 86_400;
    h.events.retain(|x| x.t >= cutoff);
    let n = h.events.len();
    if n > INTEGRITY_MAX {
        h.events.drain(0..n - INTEGRITY_MAX);
    }
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(s) = serde_json::to_string(&h) {
        let _ = fs::write(integrity_path(), s);
    }
}

/// Hechos de integridad de los últimos `days` días. Los usa el Reporte para
/// marcar una comparación como NO CONCLUYENTE (pieza 2) en vez de cantar una
/// mejora que igual solo fue un borrado.
#[tauri::command]
async fn get_integrity(days: Option<u32>) -> Result<Vec<IntegrityEvent>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = i64::from(days.unwrap_or(90).clamp(1, 400));
        let cutoff = Utc::now().timestamp() - d * 86_400;
        let h: IntegrityLog = fs::read_to_string(integrity_path())
            .ok()
            .and_then(|x| serde_json::from_str(&x).ok())
            .unwrap_or_default();
        Ok(h.events.into_iter().filter(|x| x.t >= cutoff).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// CUADERNITO DIARIO (pieza 3). La serie diaria se recalcula desde los .jsonl
// en cada ciclo; si mañana los recortan, los días viejos DESAPARECEN de la
// gráfica. Aquí se apunta cada día ya FUSIONADO (local + WSL + servidores +
// hub), en unos KB, con el mismo patrón que quota_history.json.
// REGLA: es RESPALDO, no jefe. Lo vivo manda siempre; el cuadernito solo
// rellena los días que el escaneo ya no puede ver — y cuando lo hace, se
// dice. Así un arreglo retroactivo (como el de uturns del 2026-08-14) sigue
// corrigiendo la historia en vez de quedar fosilizado.
// Local y privado: no viaja al hub ni a ntfy.
// ---------------------------------------------------------------------------

const DAILY_HIST_KEEP_DAYS: i64 = 400;

#[derive(Serialize, Deserialize, Default)]
struct DailyHist {
    #[serde(default)]
    version: u32,
    /// "YYYY-MM-DD" -> (coste, tokens, turnos útiles)
    #[serde(default)]
    days: HashMap<String, DailyAgg>,
}

fn daily_hist_path() -> PathBuf {
    app_data_dir().join("daily_history.json")
}

fn log_daily_history(daily: &[DailyAgg]) {
    if daily.is_empty() {
        return;
    }
    let mut h: DailyHist = fs::read_to_string(daily_hist_path())
        .ok()
        .and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or_default();
    h.version = 1;
    for d in daily {
        h.days.insert(d.date.clone(), d.clone());
    }
    let cutoff = (Utc::now() - Duration::days(DAILY_HIST_KEEP_DAYS))
        .format("%Y-%m-%d")
        .to_string();
    h.days.retain(|k, _| k.as_str() >= cutoff.as_str());
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(s) = serde_json::to_string(&h) {
        let _ = fs::write(daily_hist_path(), s);
    }
}

/// El cuadernito, ordenado por fecha. El panel lo usa para rellenar los días
/// que el escaneo ya no ve (y decirlo) y para congelar el "antes" de una
/// marca de arreglo (pieza 4).
#[tauri::command]
async fn get_daily_history(days: Option<u32>) -> Result<Vec<DailyAgg>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = i64::from(days.unwrap_or(90).clamp(1, 400));
        let cutoff = (Utc::now() - Duration::days(d)).format("%Y-%m-%d").to_string();
        let h: DailyHist = fs::read_to_string(daily_hist_path())
            .ok()
            .and_then(|x| serde_json::from_str(&x).ok())
            .unwrap_or_default();
        let mut out: Vec<DailyAgg> = h
            .days
            .into_values()
            .filter(|x| x.date.as_str() >= cutoff.as_str())
            .collect();
        out.sort_by(|a, b| a.date.cmp(&b.date));
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
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

/// ¿Es un turno ÚTIL del usuario? Un mensaje HUMANO real: fuera los meta,
/// los de subagente, los resultados de herramienta (llegan con rol user
/// pero los escribe la máquina) y los envoltorios de comandos locales.
/// Es el denominador de "tokens por turno útil" — sesgo aquí = métrica
/// mentirosa, por eso los filtros son deliberadamente conservadores.
fn is_user_turn(v: &serde_json::Value) -> bool {
    user_turn_text(v).is_some()
}

/// El TEXTO de un turno humano real (None = turno maquinal). Es la MISMA
/// lógica de `is_user_turn` — refactorizada 2026-08-11 para devolver el
/// texto, porque la evidencia del análisis local (docs/analisis-local.md)
/// necesita los mensajes y no solo contarlos. El bool la envuelve: UNA
/// implementación, cero divergencia (réplica exacta en meter-export.py,
/// invariante #1).
fn user_turn_text(v: &serde_json::Value) -> Option<String> {
    // isCompactSummary: los resúmenes de continuación de una compactación
    // viajan con rol user pero los escribe la máquina — contaban como turno
    // útil e inflaban el denominador del rendimiento (cazado 2026-08-14).
    // Cambio de semántica => SCAN_CACHE_VERSION 3 en los dos lados.
    if v["type"].as_str() != Some("user")
        || v["isSidechain"].as_bool().unwrap_or(false)
        || v["isMeta"].as_bool().unwrap_or(false)
        || v["isCompactSummary"].as_bool().unwrap_or(false)
        || v.get("toolUseResult").map_or(false, |x| !x.is_null())
    {
        return None;
    }
    let msg = &v["message"];
    if msg["role"].as_str() != Some("user") {
        return None;
    }
    let text: String = match &msg["content"] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => {
            // un turno con tool_result en el content también es maquinal
            if a.iter().any(|b| b["type"].as_str() == Some("tool_result")) {
                return None;
            }
            a.iter()
                .filter(|b| b["type"].as_str() == Some("text"))
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => return None,
    };
    let t = text.trim();
    // envoltorios que Claude Code o el IDE inyectan con rol user sin marcar
    // isMeta (el <ide_… se vio en logs reales del VPS, 2026-08-06). Lista
    // explícita y conservadora: ante la duda, mejor contar de menos que
    // inflar el denominador y abaratar el rendimiento en falso.
    if t.is_empty()
        || t.starts_with("<command-")
        || t.starts_with("<local-command")
        || t.starts_with("<ide_")
        || t.starts_with("<system-reminder")
        || t.starts_with("[Request interrupted")
    {
        return None;
    }
    Some(t.to_string())
}

/// Parsea un .jsonl a entradas compactas, deduplicando dentro del archivo.
/// Devuelve (nombre del cwd, entradas, turnos de usuario, duplicados internos).
fn parse_jsonl_file(
    path: &std::path::Path,
    keep_after: i64,
) -> (Option<String>, Vec<CachedEntry>, Vec<CachedUTurn>, usize) {
    let Ok(content) = fs::read_to_string(path) else {
        return (None, Vec::new(), Vec::new(), 0);
    };
    let mut display: Option<String> = None;
    let mut entries = Vec::new();
    let mut uturns: Vec<CachedUTurn> = Vec::new();
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
        // turnos de usuario: no traen usage, se recogen aparte ANTES del
        // filtro. La ventana se aplica igual que a las entradas con costo.
        if is_user_turn(&v) {
            if let Some(ts) = v["timestamp"]
                .as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc).timestamp())
            {
                if ts >= keep_after {
                    uturns.push(CachedUTurn {
                        ts,
                        id: v["uuid"].as_str().unwrap_or("").to_string(),
                    });
                }
            }
            continue;
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
    (display, entries, uturns, dups)
}

/// Los .jsonl de una carpeta de proyecto: los planos MÁS los transcripts de
/// subagentes. Claude Code moderno (visto en v2.1.221, 2026-08-04) ya no
/// escribe los turnos del subagente en el archivo de la sesión con
/// isSidechain:true — los pone en <sesión>/subagents/agent-*.jsonl. Sin
/// entrar ahí, ni el costo por proyecto ni el detector de subagentes los ven.
fn project_jsonls(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(files) = fs::read_dir(dir) else { return out };
    for f in files.flatten() {
        let p = f.path();
        if p.is_dir() {
            if let Ok(subs) = fs::read_dir(p.join("subagents")) {
                for s in subs.flatten() {
                    let sp = s.path();
                    if sp.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        out.push(sp);
                    }
                }
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(p);
        }
    }
    out
}

/// Devuelve `true` si la raíz se pudo LEER. Importa para la integridad: con
/// WSL apagado (o un disco desconectado) la raíz es ilegible y sus archivos
/// "faltan" sin haberse borrado — solo se juzgan las raíces leídas.
fn scan_projects_dir(
    projects_dir: &std::path::Path,
    suffix: Option<&str>,
    now: DateTime<Utc>,
    end: DateTime<Utc>,
    window_days: u32,
    agg: &mut LocalAgg,
    cache_in: &HashMap<String, CachedFile>,
    cache_out: &mut HashMap<String, CachedFile>,
    intg: &mut Vec<IntegrityEvent>,
) -> bool {
    // "hoy" y la tendencia van con AHORA; la ventana, con el final elegido
    let day_ago = now - Duration::hours(24);
    let window_ago = end - Duration::days(window_days as i64);
    let month_ago = now - Duration::days(30); // serie diaria de la tendencia
    // ventana más amplia de esta ejecución: la elegida (que puede estar en el
    // pasado) o los 30 días de la tendencia. Nada anterior entra en ningún
    // cálculo.
    let keep_after = std::cmp::min(
        (now - Duration::days(30 + SCAN_SKIP_MARGIN_DAYS)).timestamp(),
        (end - Duration::days(window_days as i64 + SCAN_SKIP_MARGIN_DAYS)).timestamp(),
    );
    let Ok(entries) = fs::read_dir(projects_dir) else { return false };
    // recorte detectado en ESTA raíz: (archivos, bytes perdidos)
    let mut shrunk: (u64, u64) = (0, 0);

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
            uturns: 0,
        });

        for path in project_jsonls(&proj.path()) {
            let Ok(meta) = fs::metadata(&path) else { continue };
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
            let prev = cache_in.get(&fkey);
            // INTEGRIDAD: el archivo ENCOGIÓ = alguien lo recortó por fuera.
            // Solo se observa (el parseo relee el archivo entero, así que no
            // hay offset que corregir): se anota para poder decirlo.
            if let Some(p) = prev {
                if meta.len() < p.len {
                    shrunk.0 += 1;
                    shrunk.1 += p.len - meta.len();
                }
            }
            let cached = prev
                .filter(|c| c.len == meta.len() && c.mtime == mtime)
                .cloned();
            let entry = match cached {
                Some(c) => c,
                None => {
                    let (display, entries, uturns, dups) = parse_jsonl_file(&path, keep_after);
                    CachedFile { len: meta.len(), mtime, display, entries, uturns, dups }
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

                if ts >= window_ago && ts <= end {
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
                    let c = agg
                        .daily
                        .entry(ts.format("%Y-%m-%d").to_string())
                        .or_insert((0.0, 0, 0));
                    c.0 += cost;
                    c.1 += e.inp + e.out + e.cw; // mismo criterio: cache_read fuera
                }
            }
            // turnos útiles del usuario: dedup global por uuid (también
            // cruzan archivos, como las entradas con usage) y las MISMAS
            // ventanas que el resto — elegida para el total, 30 días para
            // la serie del reporte.
            for u in &entry.uturns {
                if !u.id.is_empty() && !agg.useen.insert(u.id.clone()) {
                    continue;
                }
                let Some(ts) = DateTime::from_timestamp(u.ts, 0) else { continue };
                if ts >= window_ago && ts <= end {
                    agg.uturns_window += 1;
                    agg.projects.get_mut(&slot_key).unwrap().uturns += 1;
                }
                if ts >= month_ago {
                    agg.daily
                        .entry(ts.format("%Y-%m-%d").to_string())
                        .or_insert((0.0, 0, 0))
                        .2 += 1;
                }
            }
            cache_out.insert(fkey, entry);
        }
    }
    if shrunk.0 > 0 {
        intg.push(IntegrityEvent {
            t: Utc::now().timestamp(),
            kind: "truncated".into(),
            n: shrunk.0,
            b: shrunk.1,
            o: suffix.unwrap_or("").to_string(),
        });
    }
    true
}

/// Agrega todas las fuentes (este PC + WSL + remotos) para una ventana dada.
/// Solo lo de ESTA máquina (este PC + sus distros WSL), sin tocar la red.
/// Se separó de collect_local_stats para poder calcular varias ventanas de
/// una tirada: el hub las necesita todas, porque quien lee un resumen ajeno
/// no puede recortarlo a otra ventana —el desglose por proyecto ya viene
/// sumado— y enseñaría el número de otra semana sin avisar (2026-07-28).
/// `end_ts` = final del periodo (epoch). None = ahora, que es el
/// comportamiento de siempre ("últimos N días"). Con un valor, la ventana
/// se desplaza al pasado y cubre [end - window_days, end]: así un RANGO DE
/// FECHAS se expresa con los mismos dos datos de siempre (ancho + final) y
/// no hace falta un camino paralelo por todo el motor (2026-08-05).
/// OJO: "hoy" (cost_today) y la serie diaria de 30 días siguen anclados a
/// AHORA a propósito — no son la ventana elegida, y moverlos convertiría
/// "Hoy" en "el último día del rango", que no es lo que dice la etiqueta.
fn collect_own_stats(
    window_days: u32,
    want_rows: bool,
    end_ts: Option<i64>,
) -> (LocalStats, HashMap<String, DayCell>) {
    let now = Utc::now();
    let end = end_ts
        .and_then(|t| DateTime::from_timestamp(t, 0))
        .unwrap_or(now);
    let mut agg = LocalAgg { want_rows, ..Default::default() };
    // Caché de parseo compartido por todas las fuentes locales. Si esta
    // ejecución necesita más historial del guardado, load_scan_cache lo
    // descarta y se reconstruye en vez de devolver de menos. Con un rango
    // antiguo hay que retroceder hasta SU principio, no solo 30 días.
    let keep_after = std::cmp::min(
        (now - Duration::days(30 + SCAN_SKIP_MARGIN_DAYS)).timestamp(),
        (end - Duration::days(window_days as i64 + SCAN_SKIP_MARGIN_DAYS)).timestamp(),
    );
    let cache_in = load_scan_cache(keep_after);
    let mut cache_out: HashMap<String, CachedFile> = HashMap::new();
    // integridad: hechos de esta pasada y raíces que SÍ se pudieron leer
    let mut intg: Vec<IntegrityEvent> = Vec::new();
    let mut roots_ok: Vec<(String, String)> = Vec::new(); // (ruta, origen)

    // 1) Este PC
    let own_root = claude_dir().join("projects");
    if scan_projects_dir(
        &own_root, None, now, end, window_days, &mut agg,
        &cache_in, &mut cache_out, &mut intg,
    ) {
        roots_ok.push((own_root.to_string_lossy().into_owned(), String::new()));
    }
    // 2) Distros WSL (si existen): misma máquina, cero configuración
    // Sufijo "wsl-<distro>" (p. ej. "wsl-Ubuntu"). Dos cosas en una: sin el
    // nombre, Ubuntu y Debian caían bajo la misma etiqueta y no había forma
    // de distinguirlas; sin el prefijo, un "Ubuntu" suelto en la columna
    // Origen parece OTRA máquina en vez del Linux de este mismo PC
    // (2026-07-29, idea de Oscar).
    for (distro, d) in wsl_claude_dirs() {
        let tag = format!("wsl-{distro}");
        let root = d.join("projects");
        if scan_projects_dir(
            &root, Some(&tag), now, end, window_days, &mut agg,
            &cache_in, &mut cache_out, &mut intg,
        ) {
            roots_ok.push((root.to_string_lossy().into_owned(), tag));
        }
    }
    // INTEGRIDAD (pieza 1, la mitad de los DESAPARECIDOS): lo que estaba en
    // el caché y ya no está en disco. Solo cuenta si su raíz se pudo LEER en
    // esta pasada (con WSL apagado sus archivos "faltan" sin haberse borrado)
    // y solo si de verdad no existe (un archivo que simplemente envejeció
    // fuera de la ventana sigue en su sitio y no se toca).
    {
        let mut gone: HashMap<String, (u64, u64)> = HashMap::new();
        for (fkey, c) in &cache_in {
            if cache_out.contains_key(fkey) {
                continue;
            }
            let Some((_, origin)) = roots_ok.iter().find(|(r, _)| fkey.starts_with(r.as_str()))
            else {
                continue; // raíz ilegible o no escaneada: no se juzga
            };
            if std::path::Path::new(fkey).exists() {
                continue; // sigue ahí; solo salió de la ventana
            }
            let e = gone.entry(origin.clone()).or_insert((0, 0));
            e.0 += 1;
            e.1 += c.len;
        }
        for (origin, (n, b)) in gone {
            intg.push(IntegrityEvent {
                t: Utc::now().timestamp(),
                kind: "vanished".into(),
                n,
                b,
                o: origin,
            });
        }
    }
    log_integrity(intg);
    // solo lo visto en esta pasada: los archivos borrados o ya fuera de
    // ventana desaparecen del caché por sí solos
    save_scan_cache(cache_out, keep_after);

    let projects: Vec<ProjectAgg> = agg
        .projects
        .into_values()
        .filter(|s| s.cost > 0.0 || s.tokens > 0 || s.uturns > 0)
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
                uturns: s.uturns,
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
        uturns_week: agg.uturns_window,
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
        waste: Waste::default(), // ídem: nace en la pasada de findings
        integrity: Vec::new(),   // lo llena el exportador; aquí, nunca
        hub_skipped: false,   // lo enciende collect_local_stats si toca
    };
    (stats, daily_map)
}

/// Ventanas que se suben al hub. Tienen que ser las MISMAS que ofrece el
/// selector del panel: si el usuario elige una que nadie subió, el resumen
/// ajeno caería a otra y enseñaría un número que no es de esa ventana.
const HUB_WINDOWS: [u32; 4] = [1, 7, 15, 30];

fn collect_local_stats(window_days: u32, end_ts: Option<i64>) -> LocalStats {
    let (mut stats, mut daily_map) = collect_own_stats(window_days, false, end_ts);

    // Subir la foto de ESTA máquina al hub, antes de fusionar nada. Tiene que
    // ser lo local a secas: si se subiera lo ya fusionado, las máquinas se
    // harían eco entre ellas y los totales se multiplicarían solos.
    // CON RANGO DE FECHAS no se sube nada: las fotos del hub son de ventanas
    // que TERMINAN HOY (HUB_WINDOWS) y subir ahí un periodo del pasado
    // envenenaría lo que leen las demás máquinas.
    let remotes = load_remotes();
    if !remotes.is_empty() && end_ts.is_none() {
        let daily: Vec<DailyAgg> = {
            let mut d: Vec<DailyAgg> = daily_map
                .iter()
                .map(|(date, c)| DailyAgg {
                    date: date.clone(),
                    cost: c.0,
                    tokens: c.1,
                    uturns: c.2,
                })
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
                collect_own_stats(w, false, None).0
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
        let Some(remote) = fetch_remote(&r, window_days, false, false, end_ts) else { continue };
        // INTEGRIDAD del servidor (pieza 1): el exportador la detecta con SU
        // propio caché y la manda; aquí solo se etiqueta con el nombre que el
        // usuario le dio y se guarda en el registro local.
        if !remote.integrity.is_empty() {
            let evs = remote
                .integrity
                .iter()
                .map(|e| IntegrityEvent { o: r.name.clone(), ..e.clone() })
                .collect();
            log_integrity(evs);
        }
        stats.cost_today += remote.cost_today;
        stats.cost_week += remote.cost_week;
        stats.tokens_week += remote.tokens_week;
        stats.uturns_week += remote.uturns_week;
        stats.files_scanned += remote.files_scanned;
        stats.entries_deduped += remote.entries_deduped;
        for p in remote.projects {
            stats.projects.push(ProjectAgg {
                name: format!("{} · {}", p.name, r.name),
                cost: p.cost,
                tokens: p.tokens,
                by_model: p.by_model,
                uturns: p.uturns,
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
            let c = daily_map.entry(d.date).or_insert((0.0, 0, 0));
            c.0 += d.cost;
            c.1 += d.tokens;
            c.2 += d.uturns;
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
            // CON RANGO DE FECHAS estas fotos no valen: son ventanas que
            // terminan HOY y nadie puede recortarlas a un periodo pasado.
            // Sumarlas mezclaría dos periodos distintos en la misma cifra;
            // el panel avisa de que faltan (hub_skipped).
            if end_ts.is_some() {
                stats.hub_skipped = true;
                continue;
            }
            stats.cost_today += h.stats.cost_today;
            stats.cost_week += h.stats.cost_week;
            stats.tokens_week += h.stats.tokens_week;
            stats.uturns_week += h.stats.uturns_week;
            stats.files_scanned += h.stats.files_scanned;
            stats.entries_deduped += h.stats.entries_deduped;
            for p in h.stats.projects {
                stats.projects.push(ProjectAgg {
                    name: format!("{} · {}", p.name, h.machine),
                    cost: p.cost,
                    tokens: p.tokens,
                    by_model: p.by_model,
                    uturns: p.uturns,
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
                let c = daily_map.entry(d.date).or_insert((0.0, 0, 0));
                c.0 += d.cost;
                c.1 += d.tokens;
                c.2 += d.uturns;
            }
        }
    }
    stats
        .projects
        .sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));

    let mut daily: Vec<DailyAgg> = daily_map
        .into_iter()
        .map(|(date, c)| DailyAgg { date, cost: c.0, tokens: c.1, uturns: c.2 })
        .collect();
    daily.sort_by(|a, b| a.date.cmp(&b.date));
    // CUADERNITO (pieza 3): la serie ya FUSIONADA se apunta en disco propio.
    // Solo en el camino normal: con un rango al pasado las fotos del hub se
    // descartan (hub_skipped) y lo remoto llega de otra ventana — apuntar eso
    // grabaría un día incompleto encima de uno bueno.
    if end_ts.is_none() {
        log_daily_history(&daily);
    }
    stats.daily = daily;

    stats
}

/// Envuelto para que el trabajo NO corra en el hilo principal. Tauri ejecuta
/// los comandos síncronos en el mismo hilo que dibuja la ventana, así que un
/// SSH de dos segundos congelaba el panel entero — se notaba al cambiar de
/// pestaña, que dispara este comando (2026-07-28).
/// `end` (epoch, opcional) mueve el FINAL de la ventana al pasado para
/// servir un rango de fechas; sin él, todo funciona como siempre. Es
/// aditivo a propósito: las llamadas que solo mandan `days` no cambian
/// (invariante #1).
#[tauri::command]
async fn get_local_stats(days: Option<u32>, end: Option<i64>) -> Result<LocalStats, String> {
    tauri::async_runtime::spawn_blocking(move || get_local_stats_impl(days, end))
        .await
        .map_err(|e| e.to_string())?
}

fn get_local_stats_impl(days: Option<u32>, end: Option<i64>) -> Result<LocalStats, String> {
    Ok(collect_local_stats(days.unwrap_or(7).clamp(1, 90), end))
}

/// Filas del reporte de TODAS las fuentes. Solo la usa el export: el panel no
/// necesita este detalle y calcularlo en cada ciclo sería trabajo tirado.
fn collect_export_rows(window_days: u32) -> Vec<ExportRow> {
    let (mine, _) = collect_own_stats(window_days, true, None);
    let mut rows = mine.rows;
    for r in load_remotes() {
        let Some(rem) = fetch_remote(&r, window_days, true, false, None) else { continue };
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
// chars que Claude Code CARGA de un CLAUDE.md; lo que sobre no se lee
// nunca (detector 10)
const CLAUDEMD_LOAD_LIMIT: usize = 40_000;
const ACOMPACT_MIN: u64 = 3; // auto-compacts por proyecto en la ventana
// Pegado masivo: umbral por MENSAJE calibrado con logs reales (2026-08-14:
// mediana tecleada 290 chars — 5k es ~17× eso). Réplica de meter-export.py.
const PASTE_MIN_CHARS: usize = 5000;
const PASTE_MIN_COUNT: u64 = 3;
const PASTE_MIN_TOKENS: u64 = 10_000;
const MAX_FINDINGS: usize = 12;
// % de desperdicio estructural: la CLASE que entra al numerador — una línea
// de factura por detector (input: claudemd/hooks_noise; cache_write:
// cachebreak), suma disjunta por construcción. claudemdsize NUNCA; los de
// costo 0 (mcp/skills) entran solos el día que se midan; lo conductual
// (inflate/reread/mech/subagents) fuera. Réplica en meter-export.py
// (WASTE_KINDS, invariante #1).
const WASTE_KINDS: [&str; 5] =
    ["claudemd", "hooks_noise", "cachebreak", "mcp_unused", "skills_unused"];
const WASTE_MAX_ITEMS: usize = 100;

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
    /// última actividad de la sesión (epoch) — viaja en Finding.ts
    last_ts: i64,
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
    /// auto-compacts de la ventana: (n, preTokens sumados, último epoch) —
    /// detector 11; solo trigger != manual
    ac: (u64, u64, i64),
    /// pegotes de la ventana: (n, chars sumados, último epoch) — detector 12
    pb: (u64, u64, i64),
}

/// Corre los detectores sobre las fuentes locales (este PC + WSL) en la
/// ventana pedida. Los hallazgos de servidores llegan aparte, por
/// fetch_remote con --findings, y el origen lo etiqueta quien lee.
/// `end_ts` = final del periodo (epoch). None = ahora. Igual que en
/// collect_own_stats: con él la ventana se desplaza al pasado y cubre
/// [end - window_days, end], que es como se sirve un rango de fechas.
fn scan_local_findings(window_days: u32, end_ts: Option<i64>) -> (Vec<Finding>, Waste) {
    let now = Utc::now();
    let end = end_ts
        .and_then(|t| DateTime::from_timestamp(t, 0))
        .unwrap_or(now);
    let window_ago = end - Duration::days(window_days as i64);
    let skip_before = (window_ago - Duration::days(2)).timestamp();

    let mut sessions: HashMap<String, SessFindings> = HashMap::new();
    let mut pend: HashMap<String, (String, String)> = HashMap::new();
    let mut mcp_used: HashSet<String> = HashSet::new();
    let mut skills_used: HashSet<String> = HashSet::new();
    let mut seen: HashSet<String> = HashSet::new();
    let (mut mech_count, mut mech_tokens, mut mech_cost) = (0u64, 0u64, 0f64);
    let (mut sub_count, mut sub_tokens, mut sub_cost) = (0u64, 0u64, 0f64);
    // denominador del waste: TODO lo gastado en la ventana, de esta MISMA
    // pasada (mismo escaneo = numerador y denominador nunca se desfasan).
    // Subagentes incluidos.
    let mut total_cost = 0f64;
    // última actividad de cada detector agregado — sin ella la tarjeta cae
    // al fondo del reporte aunque sea la más fresca (ordenamiento por ts)
    let (mut mech_ts, mut sub_ts): (i64, i64) = (0, 0);

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
    // (ruta, proyecto, chars) — detector 10
    let mut md_big: Vec<(String, Option<String>, usize)> = Vec::new();
    if window_days >= 7 {
        for (ruta, pj, texto) in claude_mds(&dirs_to_scan, skip_before) {
            if texto.len() > CLAUDEMD_LOAD_LIMIT {
                md_big.push((ruta.clone(), pj.clone(), texto.len()));
            }
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
            for fp in project_jsonls(&ppath) {
                // demasiado viejo para la ventana: ni se abre (mismo margen
                // de 2 días que el exportador)
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
                            // detector 11 — frecuencia de auto-compacts: solo
                            // las AUTOMÁTICAS (una manual la hiciste tú; las
                            // del relevo entran como manual y quedan fuera
                            // solas). Dedup por uuid: las reanudaciones
                            // copian la línea al archivo nuevo. preTokens =
                            // contexto al compactar — la compactación NO trae
                            // usage, así que es PISO ("~").
                            if v["type"].as_str() == Some("system")
                                && v["subtype"].as_str() == Some("compact_boundary")
                            {
                                let cm = &v["compactMetadata"];
                                let cx = cts.with_timezone(&Utc);
                                let uuid = v["uuid"].as_str().unwrap_or("");
                                if cm["trigger"].as_str() != Some("manual")
                                    && cx >= window_ago
                                    && cx <= end
                                    && !uuid.is_empty()
                                    && seen.insert(uuid.to_string())
                                {
                                    let st = sessions.entry(sid.clone()).or_default();
                                    st.ac.0 += 1;
                                    st.ac.1 += cm["preTokens"].as_u64().unwrap_or(0);
                                    st.ac.2 = st.ac.2.max(cts.timestamp());
                                }
                            }
                        }
                    }
                    // detector 12 — pegado masivo: un mensaje HUMANO
                    // anormalmente grande casi siempre lleva un bloque
                    // pegado. user_turn_text ya filtra lo maquinal (tool
                    // results, resúmenes de compactación, <ide_…, comandos).
                    // chars(), no bytes: réplica exacta del len() de Python.
                    // Dedup por uuid: las reanudaciones copian la línea.
                    if let Some(ptxt) = user_turn_text(&v) {
                        if ptxt.chars().count() >= PASTE_MIN_CHARS {
                            let uuid = v["uuid"].as_str().unwrap_or("");
                            let pcts = v["timestamp"]
                                .as_str()
                                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                                .map(|d| d.with_timezone(&Utc));
                            if let Some(cx) = pcts {
                                if cx >= window_ago
                                    && cx <= end
                                    && !uuid.is_empty()
                                    && seen.insert(uuid.to_string())
                                {
                                    let st = sessions.entry(sid.clone()).or_default();
                                    st.pb.0 += 1;
                                    st.pb.1 += ptxt.chars().count() as u64;
                                    st.pb.2 = st.pb.2.max(cx.timestamp());
                                }
                            }
                        }
                    }
                    // /comandos del usuario: quedan como <command-name> en el
                    // mensaje (estas líneas tampoco traen usage)
                    if line.contains("<command-name>") {
                        let in_window = v["timestamp"]
                            .as_str()
                            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                            .map(|d| { let x=d.with_timezone(&Utc); x >= window_ago && x <= end })
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
                                .map(|d| { let x=d.with_timezone(&Utc); x >= window_ago && x <= end })
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
                    if ts < window_ago || ts > end {
                        continue;
                    }
                    let inp = usage["input_tokens"].as_u64().unwrap_or(0);
                    let out_t = usage["output_tokens"].as_u64().unwrap_or(0);
                    let cw = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                    let cr = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                    total_cost += cost_of(&model, inp, out_t, cw, cr);
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
                        // los subagentes llevan SU contexto y SU sessionId es
                        // el de la sesión MADRE (verificado 2026-08-04 con un
                        // transcript real): si tocaran el estado de la sesión,
                        // su cache_read chico rompería first/last_cr
                        // (infladas) y fabricaría rupturas que no existieron.
                        // Solo suman a su propia tarjeta; sus tool_use de
                        // abajo sí cuentan (un MCP invocado por el subagente
                        // ES un MCP usado).
                        if !v["isSidechain"].as_bool().unwrap_or(false) {
                            st.turns += 1;
                            st.last_ts = st.last_ts.max(ts.timestamp());
                            *st.models.entry(model.clone()).or_insert(0) += 1;
                            if st.first_cr.is_none() {
                                st.first_cr = Some(cr);
                            }
                            st.last_cr = cr;
                            // MEDIDO: lo que costó releer el contexto en este turno
                            st.cr_cost += cost_of(&model, 0, 0, 0, cr);
                            // hilo principal en orden para el detector de rupturas
                            st.cb.push((ts.timestamp(), model.clone(), cr, cw));
                        } else {
                            // subagentes: costo MEDIDO de su propio usage —
                            // ya está dentro del total, pero ahí es invisible
                            sub_count += 1;
                            sub_tokens += inp + out_t + cw;
                            sub_cost += cost_of(&model, inp, out_t, cw, cr);
                            sub_ts = sub_ts.max(ts.timestamp());
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
                            if b["input"]["file_path"].as_str().is_some() {
                                let p = read_key(&b["input"]); // archivo + rango
                                let st = sessions.entry(sid.clone()).or_default();
                                *st.reads.entry(p.clone()).or_insert(0) += 1;
                                if let Some(id) = b["id"].as_str() {
                                    pend.insert(id.to_string(), (sid.clone(), p));
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
                        mech_ts = mech_ts.max(ts.timestamp());
                    }
                }
            }
        }
    }

    let mut findings: Vec<Finding> = Vec::new();
    // hookName -> (disparos, chars, costo, última actividad) sumado entre sesiones
    let mut hooks_g: HashMap<String, (u64, u64, f64, i64)> = HashMap::new();
    // proyecto -> (n, preTokens, costo, último epoch) — detector 11
    let mut acomp_g: HashMap<String, (u64, u64, f64, i64)> = HashMap::new();
    // proyecto -> (n, chars, costo, último epoch) — detector 12
    let mut paste_g: HashMap<String, (u64, u64, f64, i64)> = HashMap::new();
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
        // auto-compacts por PROYECTO ("7 veces esta semana en X" es el
        // hábito; por sesión sería confeti). Costo PISO: releer el contexto
        // que había (preTokens) una vez a precio de input del modelo
        // dominante — el costo real no es medible (sin usage).
        if s.ac.0 > 0 {
            let g = acomp_g.entry(sdisp(s)).or_insert((0, 0, 0.0, 0));
            g.0 += s.ac.0;
            g.1 += s.ac.1;
            g.2 += s.ac.1 as f64 * pi / 1_000_000.0;
            g.3 = g.3.max(s.ac.2);
        }
        // pegotes por PROYECTO. Costo PISO: una ingesta de lo pegado
        // (chars/4, "~") al input del modelo dominante — la realidad es
        // mayor porque viaja en cada turno posterior.
        if s.pb.0 > 0 {
            let g = paste_g.entry(sdisp(s)).or_insert((0, 0, 0.0, 0));
            g.0 += s.pb.0;
            g.1 += s.pb.1;
            g.2 += s.pb.1 as f64 / 4.0 * pi / 1_000_000.0;
            g.3 = g.3.max(s.pb.2);
        }
        // los disparos se acumulan por hook GLOBAL, pero el costo se valora
        // con el modelo dominante de la sesión donde ocurrieron
        for (hname, (nf, nch)) in &s.hooks {
            let g = hooks_g.entry(hname.clone()).or_insert((0, 0, 0.0, 0));
            g.0 += nf;
            g.1 += nch;
            g.2 += *nch as f64 / 4.0 * pi / 1_000_000.0;
            g.3 = g.3.max(s.last_ts);
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
                ts: s.last_ts,
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
                ts: s.last_ts,
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
                ts: s.last_ts,
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
            ts: mech_ts,
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
            ts: sub_ts,
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
        let (nf, nch, hcost, hts) = hooks_g[hname];
        let tok = nch / 4;
        if nf < HOOKNOISE_MIN_FIRES || tok < HOOKNOISE_MIN_TOKENS {
            continue;
        }
        findings.push(Finding {
            kind: "hooks_noise".into(),
            file: hname.clone(),
            ts: hts,
            count: nf,
            tokens: tok,
            cost: hcost,
            estimated: true,
            ..Default::default()
        });
    }
    // detector 11 — auto-compacts frecuentes: el salvavidas del ~94% no se
    // toca ni se sugiere apagar; la tarjeta señala el HÁBITO (llegar ahí
    // seguido) y su piso. El fix es entrar antes: /compact al 80%.
    let mut apjs: Vec<&String> = acomp_g.keys().collect();
    apjs.sort();
    for pj in apjs {
        let (n, pre, acost, ats) = acomp_g[pj];
        if n < ACOMPACT_MIN {
            continue;
        }
        findings.push(Finding {
            kind: "acompact".into(),
            project: pj.clone(),
            ts: ats,
            count: n,
            tokens: pre,
            cost: acost,
            estimated: true,
            ..Default::default()
        });
    }
    // detector 12 — pegado masivo: el fix no regaña (a veces pegar ES lo
    // correcto): si es un archivo del proyecto, mencionar la ruta sale
    // más barato que pegarlo.
    let mut ppjs: Vec<&String> = paste_g.keys().collect();
    ppjs.sort();
    for pj in ppjs {
        let (n, nch, pcost, pts) = paste_g[pj];
        let tok = nch / 4;
        if n < PASTE_MIN_COUNT || tok < PASTE_MIN_TOKENS {
            continue;
        }
        findings.push(Finding {
            kind: "paste".into(),
            project: pj.clone(),
            ts: pts,
            count: n,
            tokens: tok,
            cost: pcost,
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
    // detector 10 — CLAUDE.md más grande de lo que Claude Code CARGA (40k
    // chars): lo que sobra no se lee NUNCA — reglas al fondo del archivo no
    // llegan al modelo, en silencio. No es fuga de dinero sino de
    // INSTRUCCIONES: tarjeta de estado (costo 0 — el costo del tramo
    // cargado ya lo mide el detector de líneas); tokens ~ del tramo sin
    // leer. Nos pasó en carne propia el 2026-08-04 (118.8k chars, semanas
    // sin ver el aviso amarillo de la terminal).
    for (ruta, pj, sz) in md_big {
        findings.push(Finding {
            kind: "claudemdsize".into(),
            count: (sz / 1000) as u64,
            file: ruta,
            project: pj.unwrap_or_default(),
            tokens: ((sz - CLAUDEMD_LOAD_LIMIT) / 4) as u64,
            cost: 0.0,
            estimated: true,
            ..Default::default()
        });
    }
    findings.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
    // % de desperdicio estructural: el numerador se arma ANTES del tope de
    // 12 (el tope ordena por costo y decapita justo a los estructurales,
    // que son los baratos). Es un PISO y el copy lo dice ("al menos") —
    // invariante #8. Réplica exacta del bloque waste de meter-export.py.
    let struct_items: Vec<Finding> = findings
        .iter()
        .filter(|f| WASTE_KINDS.contains(&f.kind.as_str()))
        .take(WASTE_MAX_ITEMS)
        .cloned()
        .collect();
    let waste = Waste {
        struct_cost: struct_items.iter().map(|f| f.cost).sum(),
        struct_tokens: struct_items.iter().map(|f| f.tokens).sum(),
        total_cost,
        sessions: sessions.values().filter(|s| !s.models.is_empty()).count() as u64,
        days: window_days,
        end: end.timestamp(),
        estimated: struct_items.iter().any(|f| f.estimated && f.cost > 0.0),
        items: struct_items,
    };
    findings.truncate(MAX_FINDINGS);
    (findings, waste)
}

/// Analizador de fugas: detectores locales (este PC + WSL) más los de cada
/// servidor vía --findings, con el origen etiquetado por quien lee. Async +
/// spawn_blocking obligatorio (invariante 10ter: SSH y escaneo de disco).
#[tauri::command]
async fn get_findings(days: Option<u32>, end: Option<i64>) -> Result<FindingsPack, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let window_days = days.unwrap_or(7).clamp(1, 90);
        let (mut out, mut waste) = scan_local_findings(window_days, end);
        for r in load_remotes() {
            let Some(rem) = fetch_remote(&r, window_days, false, true, end) else { continue };
            for mut f in rem.findings {
                f.origin = r.name.clone();
                out.push(f);
            }
            // Fusión de razones: SUMA de numeradores y de denominadores por
            // separado — jamás promedio de porcentajes (media de razones ≠
            // razón de sumas). Si el SSH falló, el `continue` de arriba ya
            // sacó a ese origen de los DOS lados a la vez; un exportador
            // viejo manda waste en ceros = mismo efecto.
            waste.struct_cost += rem.waste.struct_cost;
            waste.struct_tokens += rem.waste.struct_tokens;
            waste.total_cost += rem.waste.total_cost;
            waste.sessions += rem.waste.sessions;
            waste.estimated |= rem.waste.estimated;
            for mut it in rem.waste.items {
                if waste.items.len() >= WASTE_MAX_ITEMS {
                    break;
                }
                it.origin = r.name.clone();
                waste.items.push(it);
            }
        }
        out.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(MAX_FINDINGS);
        Ok(FindingsPack { findings: out, waste })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Coach de sesión activa (docs/consejos-coach.md §3-§4, nivel 2 de frescura):
// el panel sondea cada ciclo la COLA de los logs tocados hace poco y evalúa
// un catálogo CORTO de reglas medibles. Sin hooks: MichiClaude sigue afuera
// mirando archivos. Desde 2026-08-05 cubre la máquina ENTERA (Windows +
// WSL) y los SERVIDORES por SSH: Oscar trabaja por SSH dentro del VPS y esa
// sesión está tan "en el teclado" como una local — la vieja suposición de
// que lo remoto es de otra persona no aguantó. El exportador replica este
// motor bajo --coach (con estado incremental propio en el servidor) y
// get_coach fusiona etiquetando el origen, como las filas del export.
// La lectura es INCREMENTAL por offset: cada archivo se parsea entero una
// sola vez y de ahí en adelante solo los bytes añadidos, así el sondeo de
// 3 min cuesta casi nada aunque la sesión pese cientos de MB.
// El anti-spam (una vez por sesión por regla + tope diario) vive en el
// FRONTEND: aquí solo se reportan los hechos medidos actuales.
// ---------------------------------------------------------------------------

const COACH_ACTIVE_MIN: i64 = 30; // minutos sin tocar el log = sesión dormida
// % del techo del modelo para sugerir /compact. Antes era 120k FIJO: con un
// techo de 1M avisaba al 12% — mismo bug que tuvo el manómetro. El techo sale
// de ctx_full() (evidencia de la máquina incluida); con modelo desconocido
// ctx_for() cae a 200k y el umbral queda en 120k, el comportamiento de antes.
const COACH_CTX_PCT: u64 = 60;
const COACH_GAP_MIN: i64 = 6; // minutos de pausa para avisar del caché vencido
const COACH_GAP_CTX: u64 = 30_000; // ...solo si hay contexto que valga la pena
const COACH_REREAD: u32 = 3; // lecturas del mismo archivo en la sesión
// Capturas de pantalla miradas en la sesión (Read sobre una IMAGEN): NO son
// relecturas — el archivo se regenera entre una y otra (revision.png ×19 en
// sparky-site, 2026-08-15, disparó "attach" y el consejo no aplicaba) — pero
// cada una entra al contexto y viaja en todos los turnos siguientes. Regla
// aparte "shots" con su propio consejo. Réplica en el exportador.
const COACH_SHOTS: u32 = 10;
const IMG_EXT: [&str; 7] = [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp", ".svg"];

fn is_image_path(p: &str) -> bool {
    let l = p.to_ascii_lowercase();
    IMG_EXT.iter().any(|e| l.ends_with(e))
}

/// Clave de una lectura para contar RELECTURAS: archivo + rango. Leer
/// trozos DISTINTOS de un archivo grande (lib.rs en 6 tandas de 1000
/// líneas, 2026-08-15) no apila copias — es justo lo que la ficha
/// recomienda — y contaba como 6 relecturas. Sin offset/limit la clave es
/// la ruta a secas. Réplica en el exportador (Hallazgos y coach).
fn read_key(inp: &serde_json::Value) -> String {
    let p = inp["file_path"].as_str().unwrap_or("").to_string();
    let off = inp["offset"].as_i64();
    let lim = inp["limit"].as_i64();
    if off.is_none() && lim.is_none() {
        return p;
    }
    let o = off.unwrap_or(0);
    let end = lim.map(|l| (o + l).to_string()).unwrap_or_default();
    format!("{}#L{}-{}", p, o, end)
}
const COACH_SUM_QUIET: i64 = 10; // minutos quieta = sesión terminada: resumen
const COACH_SUM_MIN_TURNS: u64 = 5; // por debajo no hay nada que resumir
// Aviso al celular de "tu agente terminó": antes que el resumen (que es una
// tarjeta para cuando vuelvas) porque este es para cuando NO estás — cinco
// minutos de silencio ya significan que la tarea acabó, y esperar diez es
// tenerte esperando de más.
const COACH_DONE_QUIET: i64 = 5;
const COACH_DONE_TURNS: u64 = 5; // un chat corto no vale una notificación
// "Claude está esperando tu aprobación": la sesión quieta con una
// herramienta lanzada y SIN resultado no terminó — está detenida
// esperando un clic. Confundirla con "terminó" fue el falso positivo de
// la prueba de Oscar del 2026-08-02 (push de 'terminó · 6 min' con el
// permiso en pantalla). Tres minutos bastan: quien sigue frente a la
// terminal aprueba en segundos.
const COACH_ASK_QUIET: i64 = 3;
// Manómetro de presión de contexto (docs/remediacion.md etapa 1): el hit
// "press" reporta el contexto de la sesión BAJO LOS DEDOS — quieta menos de
// estos minutos. No es ficha ni aviso: el frontend lo aparta (como done/ask)
// y solo lo dibuja en el widget; sin compuertas anti-spam.
const PRESS_QUIET_MAX: i64 = 10;

#[derive(Default)]
struct CoachSess {
    offset: u64,
    last_ctx: u64,      // cache_read+cache_write del último turno principal
    first_turn: i64,    // epoch del primer turno con usage (para la duración)
    last_turn: i64,     // epoch del último turno con usage
    turns: u64,
    cmds: u64,          // tool_use Bash (comandos ejecutados)
    reads: HashMap<String, u32>,
    shots: HashMap<String, u32>, // capturas (imágenes leídas) por ruta — aparte
    edits: HashSet<String>, // archivos tocados con Edit/Write/NotebookEdit
    tool_ids: HashSet<String>, // dedup de tool_use (reanudaciones copian líneas)
    title: String,      // ai-title del log — SOLO display, campo interno
    umsgs: Vec<String>, // últimos 3 mensajes HUMANOS truncados a 300 chars:
                        // la evidencia del análisis local (docs/
                        // analisis-local.md); mismo filtro que los turnos
                        // útiles (user_turn_text)
    proj: String,       // nombre real del proyecto, del `cwd` de la sesión
    scwd: String,       // el `cwd` COMPLETO (barras normalizadas): la identidad
                        // con la que el panel casa esta sesión con un relevo
                        // (etapa 3b) — el nombre suelto no basta, dos carpetas
                        // distintas pueden llamarse igual
    cost: f64,          // costo MEDIDO de la sesión (usage × tarifa por turno)
    gaps: u64,          // pausas ≥6 min con contexto grande (caché reescrito)
    done: bool,         // el resumen ya se emitió: una vez por sesión
    notified: bool,     // el aviso de "terminó" ya salió: una vez por sesión
    pending_tool: bool, // hay tool_use sin tool_result: espera una aprobación
    asked: bool,        // el aviso de "te está esperando" ya salió (se rearma
                        // cuando el log vuelve a crecer)
    // Señales del clasificador de tarea viva (docs/remediacion.md etapa 1b).
    // Aquí solo HECHOS medidos; el veredicto Alive/Boundary/Uncertain lo pone
    // el panel (una sola implementación, en JS).
    todos_open: u64,    // pendientes (status != completed) del ÚLTIMO TodoWrite
    todos_total: u64,   // tareas totales de esa misma lista
    trail: Vec<String>, // últimos archivos tocados (Read/Edit/Write), tope 20
    commit_clean: bool, // hubo `git commit` y NADA se editó después
    model: String,      // modelo del último turno: de él sale el TECHO de
                        // contexto del manómetro (ctx_for), que va de 200k a
                        // 1M según el modelo — no es una constante
    ctx_seen: u64,      // contexto MÁXIMO visto en la sesión: evidencia medida
                        // que corrige a la tabla cuando esta se queda corta
    // Último `compact_boundary` AUTOMÁTICO visto en el log (los manuales no
    // avisan: los hiciste tú). Es la pieza clave para el chat de la extensión
    // de VS Code, donde no se puede inyectar nada: Claude Code compacta solo
    // al llegar al límite, y Michi al menos te lo cuenta y te enseña a
    // elegir el momento tú. `acomp_done` evita re-avisar el mismo.
    acomp_ts: i64,
    acomp_pre: u64,     // tokens que llevaba la sesión al compactarse
    acomp_done: i64,
}

/// Una fuga detectada al CIERRE de la sesión (mini-auditoría del coach):
/// hechos medidos en memoria, nunca re-escaneo de disco. `kind` casa con
/// las fichas del catálogo (reread→attach, ctx→compact, gap→cache).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
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
    let shots: u32 = st.shots.values().sum();
    if shots >= COACH_SHOTS {
        out.push(CoachLeak { kind: "shots".into(), file: String::new(), n: shots as u64 });
    }
    if st.last_ctx >= ctx_full(&st.model, st.ctx_seen) * COACH_CTX_PCT / 100 {
        out.push(CoachLeak { kind: "ctx".into(), file: String::new(), n: st.last_ctx / 1000 });
    } else if st.last_ctx >= COACH_GAP_CTX {
        // Cerró con contexto grande (sin llegar al umbral del /compact):
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

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct CoachHit {
    rule: String,    // id de la ficha de Consejos — o "sum" para el resumen
    session: String, // sid corto, para el "una vez por sesión" del frontend
    value: u64,      // el dato medido (en "sum": minutos de duración)
    project: String, // carpeta de logs: con varias sesiones abiertas (VPS +
                     // local) el usuario necesita saber a CUÁL aplicar el
                     // consejo (lo pidió Oscar al validar, 2026-07-31)
    title: String,   // "sum" y "press": el ai-title (vacío = respaldo al
                     // proyecto; en "press" es el ancla del tema para el
                     // análisis local)
    cmds: u64,       // solo "sum": comandos ejecutados
    edits: u64,      // solo "sum": archivos editados distintos
    turns: u64,      // "sum"/"done": turnos de la sesión
    cost: f64,       // "sum"/"done": costo medido de la sesión (equiv. API)
    leaks: Vec<CoachLeak>, // "sum"/"done": mini-auditoría al cierre
    quiet: u64,      // solo "press": minutos desde el último toque del log —
                     // con varias sesiones vivas el panel elige la más fresca
                     // (aditivo con default, invariante #1)
    // Señales del clasificador (solo "press", aditivas): hechos crudos para
    // que el panel calcule el veredicto y la tarjeta de intención.
    topen: u64,      // pendientes abiertos del último TodoWrite
    ttotal: u64,     // tareas totales de esa lista (0 = nunca hubo lista)
    cont: u64,       // continuidad de archivos: Jaccard % (últimos 10 vs 10
                     // previos del rastro); 0 = sin rastro suficiente
    gclean: bool,    // git commit reciente sin ediciones después
    full: u64,       // solo "press": techo de contexto del modelo de ESTA
                     // sesión, en tokens. El motor manda el denominador junto
                     // al dato porque solo él sabe qué modelo corrió: va de
                     // 200k (Haiku, modelos ≤4.5) a 1M (Opus/Sonnet 4.6+).
                     // Aditivo: un exportador viejo manda 0 y el panel cae a
                     // su constante de siempre.
    scwd: String,    // solo "press": el `cwd` completo de la sesión, para
                     // casarla con un relevo abierto en esa misma carpeta
                     // (etapa 3b). Aditivo: un exportador viejo manda "" y
                     // el panel simplemente no encuentra pareja
    msgs: Vec<String>, // solo "press": últimos mensajes humanos truncados —
                     // la evidencia del análisis local (docs/
                     // analisis-local.md). Aditivo: un exportador viejo no
                     // lo manda y el panel simplemente no analiza
    /// De qué máquina viene el consejo: vacío = esta (el panel enseña
    /// "local"); con nombre = el que el usuario le dio al servidor. Lo pone
    /// get_coach al fusionar, nunca el exportador (el mismo patrón que el
    /// origen de las filas del export: lo etiqueta quien lee).
    origin: String,
    /// "attach"/"shots": nombre (sin ruta) del archivo más releído / más
    /// capturado — para que la ficha diga QUÉ leyó Claude. Aditivo: un
    /// exportador viejo no lo manda y la ficha lo omite.
    file: String,
}

/// Nombre sin ruta (barras de Windows normalizadas).
fn basename(p: &str) -> String {
    let n = p.replace('\\', "/");
    n.rsplit('/').next().unwrap_or(p).to_string()
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
    // Ventana de cristal: el estado de cada sesión viva y sus compuertas se
    // vuelca a coach_debug.json en cada sondeo. Sin esto, "no me llegó el
    // push" es imposible de diagnosticar a distancia (2026-08-03).
    let mut dbg: Vec<serde_json::Value> = Vec::new();
    let now = Utc::now().timestamp();
    let states = COACH_STATE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let Ok(mut states) = states.lock() else { return hits };
    // esta máquina ENTERA: Windows y sus distros WSL (mismo teclado; el
    // coach las ignoraba y una sesión en Ubuntu no aconsejaba nada —
    // pedido de Oscar 2026-08-05)
    let mut pdirs = vec![claude_dir().join("projects")];
    for (_distro, d) in wsl_claude_dirs() {
        pdirs.push(d.join("projects"));
    }
    for pdir in pdirs {
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
                    if st.scwd.is_empty() {
                        if let Some(c) = v["cwd"].as_str().filter(|c| !c.trim().is_empty()) {
                            st.scwd = c.replace('\\', "/").trim_end_matches('/').to_string();
                        }
                    }
                    // Compactación: Claude Code deja un `compact_boundary` con
                    // el disparador y los tokens de antes. Solo interesan las
                    // AUTOMÁTICAS — una manual la hiciste tú y avisarte sería
                    // ruido. Detector de auto-compacts (pendiente de
                    // presion-y-rendimiento.md, adelantado el 2026-08-08
                    // porque es LA pieza para el chat de la extensión de VS
                    // Code, donde no se puede inyectar nada).
                    if v["type"].as_str() == Some("system")
                        && v["subtype"].as_str() == Some("compact_boundary")
                    {
                        let cm = &v["compactMetadata"];
                        if cm["trigger"].as_str() != Some("manual") {
                            st.acomp_ts = ts.unwrap_or(0);
                            st.acomp_pre = cm["preTokens"].as_u64().unwrap_or(0);
                        }
                        // El contexto acaba de VACIARSE, y da igual quién
                        // compactara (por eso va FUERA del if de arriba). Sin
                        // esta línea el manómetro seguía marcando lo de antes
                        // hasta el siguiente turno —hasta 10 min— y el
                        // automático disparaba un /compact redundante sobre una
                        // sesión recién compactada: el "No messages to compact"
                        // que vio Oscar el 2026-08-08. Cuánto quedó NO se sabe
                        // hasta el próximo turno: 0 = "sin medida", el hit
                        // `press` exige > 0 y no se emite (invariante #8, antes
                        // que enseñar una cifra que ya es mentira). `ctx_seen`
                        // NO se toca: es el máximo histórico, la evidencia
                        // medida del techo real (ver ctx_full).
                        st.last_ctx = 0;
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
                    // evidencia del análisis local: los últimos 3 mensajes
                    // HUMANOS (docs/analisis-local.md). Truncado por CHARS
                    // (por bytes partiría un UTF-8 y reventaría el JSON).
                    if let Some(txt) = user_turn_text(&v) {
                        st.umsgs.push(txt.chars().take(300).collect());
                        if st.umsgs.len() > 3 {
                            let cut = st.umsgs.len() - 3;
                            st.umsgs.drain(..cut);
                        }
                    }
                    let usage = &v["message"]["usage"];
                    if usage.is_object() && !v["isSidechain"].as_bool().unwrap_or(false) {
                        // Un turno NUEVO del hilo principal demuestra que la
                        // sesión no está detenida esperando un permiso:
                        // limpia el pendiente aunque algún tool_result
                        // anterior viniera con una forma rara y no se haya
                        // visto (si ESTE mismo mensaje trae tool_use, el
                        // bucle de bloques lo vuelve a poner). Sin esta
                        // línea, un pendiente fantasma bloqueaba "terminó"
                        // y el recibo para siempre (2026-08-03).
                        st.pending_tool = false;
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
                            // el MÁXIMO visto es evidencia dura del techo real
                            // de esta máquina: ninguna tabla puede afirmar una
                            // ventana menor que lo que ya se usó (ver ctx_full)
                            if ctx > st.ctx_seen {
                                st.ctx_seen = ctx;
                            }
                        }
                        st.turns += 1;
                        // costo MEDIDO del turno, con la misma tarifa que el
                        // resto del panel (tabla descargada → embebida)
                        let model = v["message"]["model"].as_str().unwrap_or("unknown");
                        // El modelo del ÚLTIMO turno decide el techo de
                        // contexto del manómetro (`full` del hit "press"). Se
                        // guarda el id crudo, no el techo ya resuelto: así una
                        // tabla de precios recién descargada corrige la cuenta
                        // en el siguiente sondeo sin arrastrar un número viejo.
                        st.model = model.to_string();
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
                        // los SUBAGENTES llevan sus propias herramientas: sus
                        // tool_use no significan que el hilo principal esté
                        // esperando un permiso — se excluyen del pendiente
                        let side = v["isSidechain"].as_bool().unwrap_or(false);
                        for b in blocks {
                            // ¿la sesión quedó esperando una aprobación? El
                            // orden del archivo da el estado final: cada
                            // tool_use la deja "esperando" y su tool_result
                            // la libera. Si lo último fue un tool_use suelto,
                            // Claude está detenido esperando un clic.
                            match b["type"].as_str() {
                                Some("tool_use") if !side => st.pending_tool = true,
                                Some("tool_result") if !side => st.pending_tool = false,
                                _ => {}
                            }
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
                                        if is_image_path(p) {
                                            *st.shots.entry(p.to_string()).or_insert(0) += 1;
                                        } else {
                                            // archivo + rango: trozos distintos no son relectura
                                            *st.reads.entry(read_key(&b["input"])).or_insert(0) += 1;
                                        }
                                        st.trail.push(p.to_string());
                                    }
                                }
                                "Bash" => {
                                    st.cmds += 1;
                                    // señal de cierre: un commit deja la mesa
                                    // limpia — hasta que algo se edite después
                                    if b["input"]["command"]
                                        .as_str()
                                        .map(|c| c.contains("git commit"))
                                        .unwrap_or(false)
                                    {
                                        st.commit_clean = true;
                                    }
                                }
                                "Edit" | "Write" | "NotebookEdit" => {
                                    if let Some(p) = b["input"]["file_path"].as_str() {
                                        st.edits.insert(p.to_string());
                                        st.trail.push(p.to_string());
                                    }
                                    st.commit_clean = false;
                                }
                                // la señal REINA del clasificador: la propia
                                // lista de tareas que Claude Code mantiene
                                "TodoWrite" => {
                                    if let Some(td) = b["input"]["todos"].as_array() {
                                        st.todos_total = td.len() as u64;
                                        st.todos_open = td
                                            .iter()
                                            .filter(|x| {
                                                x["status"].as_str().unwrap_or("")
                                                    != "completed"
                                            })
                                            .count()
                                            as u64;
                                    }
                                }
                                _ => {}
                            }
                            // rastro acotado: solo los últimos 20 archivos
                            if st.trail.len() > 20 {
                                let cut = st.trail.len() - 20;
                                st.trail.drain(..cut);
                            }
                        }
                    }
                }
                // el log creció (aprobaron, o siguió solo): el aviso de
                // espera se rearma para el próximo atasco
                st.asked = false;
            }
            // reglas sobre el estado acumulado; el sid corto sale del nombre
            // del archivo, que en Claude Code es el uuid de la sesión
            let sid: String = fp
                .file_stem()
                .map(|s| s.to_string_lossy().chars().take(8).collect())
                .unwrap_or_default();
            if st.last_ctx >= ctx_full(&st.model, st.ctx_seen) * COACH_CTX_PCT / 100 {
                hits.push(CoachHit {
                    rule: "compact".into(),
                    session: sid.clone(),
                    value: st.last_ctx / 1000, // se enseña en k
                    project: pname(st, &proj_name),
                    // aditivo (2026-08-16): con el cwd la ficha caliente puede
                    // casar un relevo (relayFor) y ofrecer el botón "Aplicar"
                    scwd: st.scwd.clone(),
                    // y el título dice DE QUÉ SESIÓN habla (2026-08-17): con
                    // varias sesiones en la misma carpeta, "proyecto · local"
                    // no distingue nada y la ficha parecía repetirse
                    title: st.title.clone(),
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
                    scwd: st.scwd.clone(), // aditivo, mismo motivo que "compact"
                    title: st.title.clone(), // de qué sesión habla la ficha
                    ..Default::default()
                });
            }
            if let Some((f, n)) = st
                .reads
                .iter()
                .filter(|(_, n)| **n >= COACH_REREAD)
                .max_by_key(|(_, n)| **n)
            {
                // el nombre del archivo viaja (aditivo): la ficha dice QUÉ
                // releyó Claude — sin él el usuario creía que la regla
                // hablaba de lo que ÉL pegaba (Oscar 2026-08-15)
                hits.push(CoachHit {
                    rule: "attach".into(),
                    session: sid.clone(),
                    value: *n as u64,
                    project: pname(st, &proj_name),
                    file: basename(f),
                    ..Default::default()
                });
            }
            let shots: u32 = st.shots.values().sum();
            if shots >= COACH_SHOTS {
                let top = st.shots.iter().max_by_key(|(_, n)| **n).map(|(f, _)| basename(f));
                hits.push(CoachHit {
                    rule: "shots".into(),
                    session: sid.clone(),
                    value: shots as u64,
                    project: pname(st, &proj_name),
                    file: top.unwrap_or_default(),
                    ..Default::default()
                });
            }
            // resumen de sesión (docs/consejos-coach.md §8): la sesión que
            // ESTUVO viva se quedó quieta — una tarjeta-espejo con lo medido.
            // Solo si hubo trabajo de verdad; una vez por sesión (done). Si
            // MichiClaude no estuvo abierto durante la sesión no hay estado
            // acumulado y no hay resumen — limitación asumida de la v1.
            let quiet_min = now.saturating_sub(mtime) / 60;
            // manómetro (docs/remediacion.md etapa 1): la presión de contexto
            // de la sesión que se está trabajando AHORA. Dato puro en cada
            // sondeo — el % y los colores los pone el frontend.
            if st.last_ctx > 0 && st.last_turn > 0 && quiet_min < PRESS_QUIET_MAX {
                // continuidad de archivos: Jaccard de los últimos 10 contra
                // los 10 previos del rastro (¿sigue en los MISMOS archivos?)
                let cont = if st.trail.len() >= 12 {
                    let n = st.trail.len();
                    let a: HashSet<&String> = st.trail[n - 10..].iter().collect();
                    let b: HashSet<&String> =
                        st.trail[n.saturating_sub(20)..n - 10].iter().collect();
                    let i = a.intersection(&b).count();
                    let u = a.union(&b).count();
                    if u > 0 { (i * 100 / u) as u64 } else { 0 }
                } else {
                    0
                };
                hits.push(CoachHit {
                    rule: "press".into(),
                    session: sid.clone(),
                    value: st.last_ctx,
                    project: pname(st, &proj_name),
                    quiet: quiet_min.max(0) as u64,
                    topen: st.todos_open,
                    ttotal: st.todos_total,
                    cont,
                    gclean: st.commit_clean,
                    full: ctx_full(&st.model, st.ctx_seen),
                    scwd: st.scwd.clone(),
                    title: st.title.clone(),
                    msgs: st.umsgs.clone(),
                    ..Default::default()
                });
            }
            // Auto-compact reciente y aún sin avisar: una tarjeta que explica
            // por qué el manómetro bajó de golpe y enseña a elegir el momento.
            // Los 30 min evitan revivir compactaciones viejas si el estado se
            // reconstruye desde cero (offset 0 relee el archivo entero).
            if st.acomp_ts > 0 && st.acomp_ts != st.acomp_done && now - st.acomp_ts < 30 * 60 {
                st.acomp_done = st.acomp_ts;
                hits.push(CoachHit {
                    rule: "acomp".into(),
                    session: sid.clone(),
                    value: st.acomp_pre / 1000,
                    project: pname(st, &proj_name),
                    ..Default::default()
                });
            }
            // "Claude está esperando tu aprobación": quieta con una
            // herramienta sin resultado NO es terminada — es la sesión
            // detenida en un permiso. Va al celular (regla `ask`, no es
            // ficha) y BLOQUEA el "terminó" y el resumen: anunciar el final
            // con el permiso en pantalla fue el falso positivo de la prueba
            // de Oscar (2026-08-02). `asked` se rearma si el log crece.
            if st.pending_tool
                && !st.asked
                && quiet_min >= COACH_ASK_QUIET
                && st.turns >= 1
            {
                st.asked = true;
                hits.push(CoachHit {
                    rule: "ask".into(),
                    session: sid.clone(),
                    value: quiet_min as u64,
                    project: pname(st, &proj_name),
                    turns: st.turns,
                    ..Default::default()
                });
            }
            // "tu agente terminó": va ANTES que el resumen y por otro canal
            // (el celular). El frontend decide si empujarlo y vuelve a
            // deduplicar: este estado vive en memoria, así que al reiniciar
            // la app una sesión recién callada podría reaparecer aquí.
            if !st.notified
                && !st.pending_tool
                && quiet_min >= COACH_DONE_QUIET
                && st.turns >= COACH_DONE_TURNS
            {
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
                    ..Default::default()
                });
            }
            if !st.done
                && !st.pending_tool
                && quiet_min >= COACH_SUM_QUIET
                && st.turns >= COACH_SUM_MIN_TURNS
            {
                st.done = true;
                let mins = ((st.last_turn - st.first_turn) / 60).max(1) as u64;
                hits.push(CoachHit {
                    rule: "sum".into(),
                    session: sid.clone(),
                    value: mins,
                    project: pname(st, &proj_name),
                    title: st.title.clone(),
                    cmds: st.cmds,
                    edits: st.edits.len() as u64,
                    turns: st.turns,
                    cost: st.cost,
                    leaks: coach_leaks(st),
                    ..Default::default()
                });
            }
            dbg.push(serde_json::json!({
                "sid": sid,
                "proj": pname(st, &proj_name),
                "turns": st.turns,
                "quiet_min": quiet_min,
                "ctx": st.last_ctx,
                "pending": st.pending_tool,
                "asked": st.asked,
                "notified": st.notified,
                "sum_done": st.done,
                "gaps": st.gaps,
                "cost": (st.cost * 100.0).round() / 100.0,
            }));
        }
    }
    }
    let _ = fs::write(
        app_data_dir().join("coach_debug.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "at": Utc::now().to_rfc3339(),
            "sessions": dbg,
            "hits": hits
                .iter()
                .map(|h| format!("{}|{}", h.rule, h.session))
                .collect::<Vec<_>>(),
        }))
        .unwrap_or_default(),
    );
    hits
}

/// El coach de UN servidor: `--coach` le pide al exportador SOLO los
/// consejos de sus sesiones vivas (atajo barato: sin agregación de gasto).
/// Falla → None y silencio, como fetch_remote: la red nunca rompe el sondeo.
fn fetch_remote_coach(r: &RemoteSource) -> Option<Vec<CoachHit>> {
    use std::io::Write;
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct CoachOut {
        coach: Vec<CoachHit>,
    }
    let prices = prices_map()
        .read()
        .ok()
        .filter(|m| !m.is_empty())
        .and_then(|m| serde_json::to_string(&*m).ok());
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(&r.host)
        .arg(format!(
            "{} --coach{}",
            r.command,
            if prices.is_some() { " --prices-stdin" } else { "" }
        ))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().ok()?;
    if let Some(mut si) = child.stdin.take() {
        if let Some(json) = &prices {
            let _ = si.write_all(json.as_bytes());
        }
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice::<CoachOut>(&out.stdout).ok().map(|c| c.coach)
}

/// Sondeo del coach: async + spawn_blocking (invariante 10ter — toca disco
/// y SSH). Local + WSL con el motor de siempre; cada servidor aporta los
/// suyos vía --coach y quien lee les pone el origen (el exportador viejo
/// ignora el flag y devuelve el JSON grande sin clave `coach`: cero hits,
/// cero errores — se degrada solo).
#[tauri::command]
async fn get_coach() -> Result<Vec<CoachHit>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut hits = coach_scan();
        for r in load_remotes() {
            let Some(remote) = fetch_remote_coach(&r) else { continue };
            for mut h in remote {
                h.origin = r.name.clone();
                hits.push(h);
            }
        }
        hits
    })
    .await
    .map_err(|e| e.to_string())
}

// ---------- análisis local (IA) — docs/analisis-local.md, LEERLO ----------
// Un modelo local chico sugiere /clear o /compact cuando el clasificador
// determinista quedó en `unsure`. SOLO pinta una insignia en la tarjeta
// MANUAL de intención: jamás toca las compuertas del automático (el
// auto-/clear sigue exigiendo Boundary determinista). llama-server arranca
// bajo demanda en 127.0.0.1 y SE MATA al terminar: la app no gana procesos
// residentes. HTTP a mano sobre TcpStream: reqwest no trae la feature
// `blocking` y el patrón de la casa es async fn → spawn_blocking
// (invariante 10ter); cero dependencias nuevas (invariante #4).

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct AiConfig {
    enabled: bool,
    server: String, // ruta de llama-server; vacía = el del PATH
    model: String,  // ruta del .gguf — obligatoria para analizar
    port: u16,      // 0 = 8791; solo se escucha en 127.0.0.1
    /// ruta del GGUF de EMBEDDINGS (etapa 2, 2026-08-13); vacía = el que
    /// bajó la descarga guiada (ai_emb_file). Sin archivo, el peldaño se
    /// salta en silencio y decide el 2B — fail-quiet, regla #4.
    emb: String,
}

fn ai_config_path() -> PathBuf {
    app_data_dir().join("ai_config.json")
}

fn load_ai_config() -> AiConfig {
    fs::read_to_string(ai_config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn ai_get_config() -> AiConfig {
    load_ai_config()
}

#[tauri::command]
fn ai_set_config(cfg: AiConfig) -> Result<(), String> {
    let _ = fs::create_dir_all(app_data_dir());
    let s = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(ai_config_path(), s).map_err(|e| e.to_string())
}

/// Mata a llama-server pase lo que pase: el guard vive lo que dura el
/// análisis y su Drop cubre TODOS los caminos de salida, incluidos los `?`.
struct AiChild(std::process::Child);
impl Drop for AiChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

const AI_HEALTH_WAIT_MS: u64 = 45_000; // carga fría del GGUF desde disco
const AI_ANSWER_WAIT_MS: u64 = 60_000; // prefill + decode en CPU
const AI_PORT: u16 = 8791;

/// La forma se FUERZA, no se pide (lección central de
/// la investigación de modelos, fuera del repo). OJO CON EL MECANISMO:
/// `grammar` (GBNF) solo
/// lo acepta el endpoint NATIVO `/completion`; en el de chat se IGNORA en
/// silencio — el modelo contesta en prosa y el parseo muere con BADOUT (así
/// falló la primera prueba real, 2026-08-12). Aquí la vía correcta es
/// `response_format` con esquema: llama-server lo convierte él mismo a
/// gramática, así que los `enum` se cumplen al muestrear, no al validar.
fn ai_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "rec": {"type": "string", "enum": ["clear", "compact", "unsure"]},
            "reason": {"type": "string",
                "enum": ["tema_nuevo", "tema_cruzado", "tarea_viva", "cierre", "na"]}
        },
        "required": ["rec", "reason"],
        "additionalProperties": false
    })
}

/// Rastro del análisis: la petición y la respuesta CRUDA del último intento.
/// Misma familia que quota_debug.json / wrap_debug.txt — un fallo que solo
/// dice "no se pudo leer" obliga a adivinar, y eso ya costó una ronda.
/// Se sobrescribe (no crece) y vive en la carpeta de datos de la app: la
/// evidencia es local, como todo lo demás de esta función.
fn ai_dbg(req: &str, resp: &str) {
    let _ = fs::create_dir_all(app_data_dir());
    let cut = |s: &str, n: usize| -> String {
        s.chars().take(n).collect::<String>()
    };
    let _ = fs::write(
        app_data_dir().join("ai_debug.txt"),
        format!(
            "--- PETICIÓN ---\n{}\n\n--- RESPUESTA CRUDA ---\n{}\n",
            cut(req, 4000),
            cut(resp, 4000)
        ),
    );
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct AiVerdict {
    rec: String,
    reason: String,
    /// Qué peldaño de la escalera decidió (etapa 2, 2026-08-13): "emb" =
    /// embeddings, "llm" = el 2B. Aditivo con default: el JSON del modelo
    /// no lo trae y el panel viejo lo ignora. Es EL dato de la auditoría
    /// de la etapa 2 — el flowLog lo enseña.
    via: String,
    /// Similitud coseno tema↔reciente cuando decidieron los embeddings.
    sim: f32,
}

/// POST/GET HTTP/1.1 mínimo contra 127.0.0.1 (std puro). `Connection: close`
/// y leer hasta EOF. Devuelve (línea de estado, cuerpo): el cuerpo se
/// des-chunkea si hace falta — a nivel de BYTES, que es la unidad de los
/// tamaños de chunk (por chars se descuadraría con UTF-8 multibyte).
fn ai_http(port: u16, req: &str, body: Option<&str>, timeout_ms: u64) -> Result<(String, String), String> {
    use std::io::{Read, Write};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut s = std::net::TcpStream::connect_timeout(
        &addr,
        std::time::Duration::from_millis(2_000),
    )
    .map_err(|_| "ERR_AI_TIMEOUT".to_string())?;
    let _ = s.set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)));
    let _ = s.set_write_timeout(Some(std::time::Duration::from_millis(5_000)));
    let b = body.unwrap_or("");
    let msg = format!(
        "{req} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}",
        b.len()
    );
    s.write_all(msg.as_bytes()).map_err(|_| "ERR_AI_TIMEOUT".to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|_| "ERR_AI_TIMEOUT".to_string())?;
    let cut = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "ERR_AI_BADOUT".to_string())?;
    let head = String::from_utf8_lossy(&raw[..cut]).to_string();
    let mut payload = raw[cut + 4..].to_vec();
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        let mut out = Vec::new();
        let mut rest: &[u8] = &payload;
        loop {
            let Some(nl) = rest.windows(2).position(|w| w == b"\r\n") else { break };
            let n = usize::from_str_radix(
                String::from_utf8_lossy(&rest[..nl]).trim(),
                16,
            )
            .unwrap_or(0);
            if n == 0 {
                break;
            }
            let start = nl + 2;
            if rest.len() < start + n {
                break; // chunk truncado: se queda lo leído
            }
            out.extend_from_slice(&rest[start..start + n]);
            rest = &rest[(start + n + 2).min(rest.len())..];
        }
        payload = out;
    }
    let status = head.lines().next().unwrap_or("").to_string();
    Ok((status, String::from_utf8_lossy(&payload).to_string()))
}

// --- etapa 2: el peldaño de EMBEDDINGS (2026-08-13, docs/analisis-local.md
// §"Etapa 2"). Similitud coseno entre EL TEMA (título + mensajes viejos) y
// LO RECIENTE (el último mensaje): los casos claros se deciden en 1-3 s en
// vez de los 10-26 s del 2B, que queda solo para la banda media. Umbrales
// del diseño: <0.45 = tema_nuevo (clear), >0.65 = tema_cruzado (compact).
// FAIL-QUIET en cadena (regla #4): sin GGUF, server que no arranca, salida
// rara o banda media → None, y decide el 2B como en la v1 — este peldaño
// solo puede ACELERAR, nunca cambiar lo que existe.
const EMB_NEW: f32 = 0.45; // por debajo: el tema cambió — clear
const EMB_CROSS: f32 = 0.65; // por encima: sigue el hilo — compact
const AI_EMB_PORT_OFF: u16 = 1; // puerto del 2B + 1: nunca chocan
const AI_EMB_WAIT_MS: u64 = 20_000; // carga fría del e5 (126 MB, no 1.3 GB)

/// Vector f32 desde el JSON de /v1/embeddings.
fn emb_vec(v: &serde_json::Value) -> Option<Vec<f32>> {
    let a = v.as_array()?;
    if a.is_empty() {
        return None;
    }
    Some(a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect())
}

fn emb_cos(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return None;
    }
    Some(dot / (na * nb))
}

/// Solo la MEDIDA: arranca el server de embeddings, calcula el coseno y
/// devuelve la similitud (None = no se pudo medir). La decisión vive en
/// ai_emb_verdict — separadas para que la similitud pueda viajar con el
/// veredicto del 2B cuando cae en banda media (auditoría de la etapa 2).
/// Rastro PROPIO del peldaño (`emb_debug.txt`, se sobrescribe). El
/// fail-quiet de la escalera se traga la causa A PROPÓSITO (regla #4), pero
/// tragarse el DIAGNÓSTICO fue un error: en la primera corrida real salió
/// "(modelo)" a secas y no había forma de saber si fue banda media o un
/// server que ni arrancó — y ai_debug.txt no sirve porque el 2B lo pisa.
fn emb_dbg(msg: &str) {
    let _ = fs::write(
        app_data_dir().join("emb_debug.txt"),
        format!("{}\n{msg}\n", Utc::now()),
    );
}

fn ai_emb_sim(cfg: &AiConfig, server: &str, title: &str, msgs: &[String]) -> Option<f32> {
    let emb = ai_emb_path(cfg);
    if !emb.is_file() {
        emb_dbg(&format!("sin GGUF de embeddings: {}", emb.display()));
        return None;
    }
    // el TEMA = título + mensajes viejos; LO RECIENTE = el último mensaje.
    // Con un solo mensaje el tema es el título a secas — sigue funcionando.
    let recent = msgs.last()?.trim().to_string();
    if recent.is_empty() {
        emb_dbg("sin mensaje reciente");
        return None;
    }
    let mut theme = title.trim().to_string();
    for m in &msgs[..msgs.len().saturating_sub(1)] {
        theme.push_str(" · ");
        theme.push_str(m);
    }
    if theme.trim().is_empty() || theme.trim() == "·" {
        return None;
    }
    let port = (if cfg.port == 0 { AI_PORT } else { cfg.port }) + AI_EMB_PORT_OFF;
    let port_s = port.to_string();
    // el stderr del server va a emb_server.log: ahí dice llama-server por
    // qué no carga un GGUF o no reconoce un flag — el oro del diagnóstico
    let slog = fs::File::create(app_data_dir().join("emb_server.log"));
    let mut cmd = std::process::Command::new(server);
    cmd.args([
        "-m",
        emb.to_str()?,
        "--host",
        "127.0.0.1",
        "--port",
        port_s.as_str(),
        // modelo de embeddings puro: sin este flag llama-server lo trata de
        // chat y el endpoint no existe. El pooling NO se pisa: el GGUF
        // oficial de gemma trae el suyo en metadatos (calibrado así en el
        // banco del 2026-08-13 — mismos flags que aquí).
        "--embeddings",
        "-c",
        "1024",
        "-t",
        "4",
        "-ngl",
        "0",
        "--no-mmap",
    ])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(match slog {
        Ok(f) => std::process::Stdio::from(f),
        Err(_) => std::process::Stdio::null(),
    });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            emb_dbg(&format!("no arrancó «{server}»: {e}"));
            return None;
        }
    };
    let _guard = AiChild(child); // muere también en todos los caminos de error
    let t0 = std::time::Instant::now();
    loop {
        if t0.elapsed().as_millis() as u64 > AI_EMB_WAIT_MS {
            emb_dbg("health: 20 s sin contestar — mira emb_server.log");
            return None;
        }
        if let Ok((status, _)) = ai_http(port, "GET /health", None, 2_000) {
            if status.contains(" 200") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    // SIN prefijos, a propósito: en el banco del 2026-08-13 gemma separó
    // MEJOR sin ellos (tema nuevo 0.15-0.25, continuación ~0.53, mismo tema
    // 0.84) y esa distribución calza con los umbrales 0.45/0.65 del diseño.
    // El "task: sentence similarity | query:" de su ficha comprimía todo
    // hacia la banda media (más invocaciones del 2B, más lento).
    let body = serde_json::json!({
        "input": [theme.as_str(), recent.as_str()]
    })
    .to_string();
    let (status, payload) = match ai_http(port, "POST /v1/embeddings", Some(&body), 15_000) {
        Ok(v) => v,
        Err(e) => {
            emb_dbg(&format!("http /v1/embeddings: {e}"));
            return None;
        }
    };
    let head: String = payload.chars().take(400).collect();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload.trim()) else {
        emb_dbg(&format!("respuesta ilegible ({status}): {head}"));
        return None;
    };
    let (Some(a), Some(b)) = (
        emb_vec(&v["data"][0]["embedding"]),
        emb_vec(&v["data"][1]["embedding"]),
    ) else {
        emb_dbg(&format!("sin vectores ({status}): {head}"));
        return None;
    };
    let Some(sim) = emb_cos(&a, &b) else {
        emb_dbg("coseno imposible (vectores vacíos o de largos distintos)");
        return None;
    };
    // al rastro de siempre (se sobrescribe): la evidencia y la similitud,
    // no los vectores — 768 floats no le sirven a nadie para depurar
    ai_dbg(
        &format!("[emb] tema: {theme}\n[emb] reciente: {recent}"),
        &format!("sim={sim:.3} (clear<{EMB_NEW} · compact>{EMB_CROSS})"),
    );
    emb_dbg(&format!("ok · sim={sim:.3}"));
    Some(sim)
}

/// El veredicto por embeddings. Devuelve además la similitud medida
/// (-1.0 = no se pudo medir): en banda media el veredicto es None pero la
/// similitud VIAJA con la respuesta del 2B — sin eso, "(modelo)" en el
/// flowLog no distingue "el peldaño midió y no decidió" de "el peldaño
/// falló", y el ai_debug.txt del 2B pisa el rastro del emb (visto en la
/// primera corrida real: 31 s y ninguna forma de saber por qué).
fn ai_emb_verdict(
    cfg: &AiConfig,
    server: &str,
    title: &str,
    msgs: &[String],
) -> (Option<AiVerdict>, f32) {
    let Some(sim) = ai_emb_sim(cfg, server, title, msgs) else {
        return (None, -1.0);
    };
    if sim < EMB_NEW {
        (
            Some(AiVerdict {
                rec: "clear".into(),
                reason: "tema_nuevo".into(),
                via: "emb".into(),
                sim,
            }),
            sim,
        )
    } else if sim > EMB_CROSS {
        (
            Some(AiVerdict {
                rec: "compact".into(),
                reason: "tema_cruzado".into(),
                via: "emb".into(),
                sim,
            }),
            sim,
        )
    } else {
        (None, sim) // banda media: la zona gris sigue siendo del 2B
    }
}

/// El análisis en sí (síncrono; el comando lo envuelve en spawn_blocking).
fn ai_intent_impl(title: String, msgs: Vec<String>, cont: u64) -> Result<AiVerdict, String> {
    let cfg = load_ai_config();
    if !cfg.enabled {
        return Err("ERR_AI_OFF".into());
    }
    if cfg.model.trim().is_empty() || !std::path::Path::new(cfg.model.trim()).is_file() {
        return Err("ERR_AI_MODEL".into());
    }
    if msgs.is_empty() {
        return Err("ERR_AI_BADOUT".into()); // sin evidencia no hay qué juzgar
    }
    let server = if cfg.server.trim().is_empty() {
        "llama-server".to_string()
    } else {
        cfg.server.trim().to_string()
    };
    // La escalera de la etapa 2: los embeddings deciden los casos claros en
    // segundos; None (sin modelo, fallo o banda media) = el 2B, como en la v1.
    // La similitud medida (o -1.0) acompaña también al veredicto del 2B.
    let (embv, emb_sim) = ai_emb_verdict(&cfg, &server, &title, &msgs);
    if let Some(v) = embv {
        return Ok(v);
    }
    let port = if cfg.port == 0 { AI_PORT } else { cfg.port };
    // Flags de la investigación de modelos (§3), medidos en CPU sin GPU:
    // -ngl 0 (la iGPU comparte la RAM), --no-mmap (GGML_ASSERT con memoria
    // fragmentada), sin razonamiento (12x), temp 0 (clasificación, no prosa).
    let port_s = port.to_string();
    let mut cmd = std::process::Command::new(&server);
    cmd.args([
        "-m", cfg.model.trim(),
        "--host", "127.0.0.1",
        "--port", port_s.as_str(),
        "-c", "2048",
        "-t", "4",
        "-ngl", "0",
        "--no-mmap",
        "--reasoning-budget", "0",
        "--temp", "0",
    ])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let child = cmd.spawn().map_err(|_| "ERR_AI_START".to_string())?;
    let _guard = AiChild(child);
    // esperar a que el modelo cargue: /health contesta 503 mientras tanto
    let t0 = std::time::Instant::now();
    loop {
        if t0.elapsed().as_millis() as u64 > AI_HEALTH_WAIT_MS {
            return Err("ERR_AI_TIMEOUT".into());
        }
        if let Ok((status, _)) = ai_http(port, "GET /health", None, 3_000) {
            if status.contains(" 200") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    // Prompt en inglés (el 2B sigue mejor las instrucciones en inglés); la
    // evidencia va verbatim en su idioma. El SESGO ASIMÉTRICO vive aquí y en
    // el render del panel: en la duda jamás clear (regla #3 del diseño).
    let ev: String = msgs
        .iter()
        .enumerate()
        .map(|(i, m)| format!("{}. \"{}\"\n", i + 1, m.replace('"', "'")))
        .collect();
    let prompt = format!(
        "You classify a coding session to recommend /clear (wipe the context) \
         or /compact (summarize, keeping the thread).\n\
         Session topic: \"{}\"\n\
         Most recent user messages (newest LAST):\n{}\
         File continuity with earlier work: {}%.\n\
         Question: does the NEWEST message need the earlier conversation?\n\
         - It starts an unrelated new topic -> rec=clear, reason=tema_nuevo\n\
         - It builds on or references earlier work -> rec=compact, reason=tema_cruzado\n\
         - A task still seems unfinished -> rec=compact, reason=tarea_viva\n\
         - Work seems done and nothing new started -> rec=unsure, reason=cierre\n\
         - When in doubt NEVER answer clear: answer compact or unsure.\n\
         Answer ONLY the JSON. /no_think",
        title.replace('"', "'"),
        ev,
        cont
    );
    let body = serde_json::json!({
        "messages": [{"role": "user", "content": prompt}],
        "response_format": {"type": "json_object", "schema": ai_schema()},
        // Qwen3.5 RAZONA por defecto y el `--reasoning-budget 0` del servidor
        // es solo un default: lo pisa la plantilla de chat. Sin esto el modelo
        // gastó su presupuesto entero en "Thinking Process:" y dejó `content`
        // VACÍO (finish_reason: length) — así falló la primera prueba real
        // (2026-08-12). Ya estaba avisado en la investigación §3, que
        // además da el cinturón: el `/no_think` del final del prompt.
        "chat_template_kwargs": {"enable_thinking": false},
        // 64 y no 40: el JSON son ~20 tokens, pero un margen barato evita
        // volver a quedarse a medias si el modelo escribe algo delante
        "max_tokens": 64,
        "temperature": 0,
    })
    .to_string();
    let (_, payload) =
        ai_http(port, "POST /v1/chat/completions", Some(&body), AI_ANSWER_WAIT_MS)?;
    ai_dbg(&body, &payload);
    let v: serde_json::Value =
        serde_json::from_str(payload.trim()).map_err(|_| "ERR_AI_BADOUT".to_string())?;
    if !v["error"].is_null() {
        return Err("ERR_AI_BADOUT".into()); // el servidor rechazó la petición
    }
    let msg = &v["choices"][0]["message"];
    // con algunos modelos el texto útil cae en `reasoning_content` si el
    // separador de razonamiento se activa: se mira el segundo antes de rendirse
    let mut content = msg["content"].as_str().unwrap_or("");
    if content.trim().is_empty() {
        content = msg["reasoning_content"].as_str().unwrap_or("");
    }
    // el objeto puede venir con algo delante o detrás: se recorta al primer
    // {...} (llaves ASCII, así que los índices caen en frontera de carácter)
    let slice = match (content.find('{'), content.rfind('}')) {
        (Some(i), Some(j)) if j > i => &content[i..=j],
        _ => content.trim(),
    };
    let out: AiVerdict =
        serde_json::from_str(slice).map_err(|_| "ERR_AI_BADOUT".to_string())?;
    match out.rec.as_str() {
        "clear" | "compact" | "unsure" => Ok(AiVerdict {
            via: "llm".into(), // qué peldaño decidió, para la auditoría
            sim: emb_sim,      // lo que midió el emb aunque no decidiera
            ..out
        }),
        _ => Err("ERR_AI_BADOUT".into()),
    }
    // _guard cae aquí: llama-server muere también en los caminos de error
}

/// async + spawn_blocking (invariante 10ter): el análisis tarda 10-45 s y el
/// hilo de la UI no puede congelarse mientras tanto.
#[tauri::command]
async fn ai_intent(title: String, msgs: Vec<String>, cont: u64) -> Result<AiVerdict, String> {
    tauri::async_runtime::spawn_blocking(move || ai_intent_impl(title, msgs, cont))
        .await
        .map_err(|e| e.to_string())?
}

// --- descarga guiada (un clic, sin rutas) --------------------------------
// URLs y huellas SHA-256 FIJAS en el binario — la regla del updater: jamás
// salen de algo descargado. Pineadas a un build concreto de llama.cpp y al
// GGUF exacto de la investigación de modelos. Cada archivo tiene su fuente
// original Y un espejo en los Releases de este repo (release `modelos-v1`,
// prerelease para que el updater no lo vea): si la fuente original muere o
// cambia el archivo, la descarga cae sola al espejo. La MISMA huella valida
// ambas fuentes (el espejo es copia byte a byte); al cambiar de build o de
// modelo, actualizar las SEIS constantes juntas Y subir las copias nuevas a
// un release `modelos-v2`. Es la ÚNICA conexión de la app que no va a
// api.anthropic.com: GitHub y Hugging Face, una vez, opt-in y anunciada en
// la propia interfaz (ai_dl_note).
const AI_LS_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b10362/llama-b10362-bin-win-cpu-x64.zip";
const AI_LS_URL_MIRROR: &str = "https://github.com/oscarorozcos/michiclaude/releases/download/modelos-v1/llama-b10362-bin-win-cpu-x64.zip";
const AI_LS_SHA: &str = "a9d95d26cf00664f2902f73cb0fd9b167a3a1f252294bb2f8b236305f57d6363";
const AI_MODEL_URL: &str =
    "https://huggingface.co/unsloth/Qwen3.5-2B-MTP-GGUF/resolve/main/Qwen3.5-2B-UD-Q4_K_XL.gguf";
const AI_MODEL_URL_MIRROR: &str =
    "https://github.com/oscarorozcos/michiclaude/releases/download/modelos-v1/Qwen3.5-2B-UD-Q4_K_XL.gguf";
const AI_MODEL_SHA: &str = "9f7b15d04cf2d5878c8122a7c181dbc09f050cd66080ce3374576e734ccb0910";
// El peldaño de EMBEDDINGS de la etapa 2 (2026-08-13): EmbeddingGemma-300M
// en Q8_0 (~319 MB), GGUF OFICIAL de ggml-org. NO es el e5-small del diseño:
// las conversiones comunitarias de e5 salieron ROTAS (sin token_type_count —
// ni cargan en llama.cpp moderno — y con el tokenizer dañado: las
// similitudes no separaban clases, banco del 2026-08-13 en la bitácora).
// Gemma se calibró en el mismo banco: SIN prefijo, los umbrales del diseño
// (0.45/0.65) calzan con lo medido. Espejo verificado (descarga anónima,
// huella idéntica). Las constantes de la descarga guiada son NUEVE —
// siguen actualizándose JUNTAS.
const AI_EMB_URL: &str =
    "https://huggingface.co/ggml-org/embeddinggemma-300M-GGUF/resolve/main/embeddinggemma-300M-Q8_0.gguf";
const AI_EMB_URL_MIRROR: &str =
    "https://github.com/oscarorozcos/michiclaude/releases/download/modelos-v1/embeddinggemma-300M-Q8_0.gguf";
const AI_EMB_SHA: &str = "b5ce9d77a3fc4b3b39ccb5643c36777911cc4eb46a66962eadfa3f5f60490d63";

fn ai_dir() -> PathBuf {
    app_data_dir().join("ai")
}
fn ai_model_file() -> PathBuf {
    ai_dir().join("Qwen3.5-2B-UD-Q4_K_XL.gguf")
}
fn ai_emb_file() -> PathBuf {
    ai_dir().join("embeddinggemma-300M-Q8_0.gguf")
}

/// La ruta EFECTIVA del GGUF de embeddings: la manual si existe, si no la
/// descargada. El e5 roto del primer intento (2026-08-13) se IGNORA aunque
/// siga configurado — apuntar a un modelo que no carga es no tener modelo,
/// y sin esto la config vieja de quien lo descargó ese día bloquearía la
/// descarga del bueno para siempre.
fn ai_emb_path(cfg: &AiConfig) -> PathBuf {
    let manual = cfg.emb.trim();
    if manual.is_empty() || manual.ends_with("multilingual-e5-small-q8_0.gguf") {
        ai_emb_file()
    } else {
        PathBuf::from(manual)
    }
}

/// Busca llama-server dentro de lo descomprimido: el zip de llama.cpp ha
/// cambiado de forma entre builds (a veces raíz, a veces subcarpeta), así
/// que se busca el exe donde haya caído en vez de suponer la ruta.
fn find_ls(dir: &std::path::Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p
                    .file_name()
                    .map_or(false, |n| n == "llama-server.exe" || n == "llama-server")
                {
                    return Some(p);
                }
            }
        }
    }
    None
}

#[derive(Serialize, Clone, Default)]
struct AiSetupStatus {
    server: bool,
    model: bool,
    emb: bool,
}

/// ¿Qué falta por descargar? Cuenta tanto lo configurado a mano (rutas del
/// usuario) como lo nuestro: el botón solo aparece si de verdad falta algo.
#[tauri::command]
fn ai_setup_status() -> AiSetupStatus {
    let cfg = load_ai_config();
    let server = (!cfg.server.trim().is_empty()
        && std::path::Path::new(cfg.server.trim()).is_file())
        || find_ls(&ai_dir().join("bin")).is_some();
    let model = (!cfg.model.trim().is_empty()
        && std::path::Path::new(cfg.model.trim()).is_file())
        || ai_model_file().is_file();
    let emb = ai_emb_path(&cfg).is_file();
    AiSetupStatus { server, model, emb }
}

/// Descarga con progreso a eventos `ai:dl` hacia el panel. Un `.part` viejo
/// no se reanuda: se rehace entero (la verificación es por huella completa).
async fn ai_download(
    app: &tauri::AppHandle,
    phase: &str,
    url: &str,
    dest: &std::path::Path,
) -> Result<(), String> {
    use std::io::Write;
    use tauri::Emitter;
    let _ = fs::remove_file(dest);
    let mut resp = reqwest::get(url).await.map_err(|_| "ERR_AI_DL".to_string())?;
    if !resp.status().is_success() {
        return Err("ERR_AI_DL".into());
    }
    let total = resp.content_length().unwrap_or(0);
    let mut f = fs::File::create(dest).map_err(|_| "ERR_AI_DL".to_string())?;
    let mut got: u64 = 0;
    let mut last: u64 = 200; // imposible: fuerza el primer evento
    while let Some(chunk) = resp.chunk().await.map_err(|_| "ERR_AI_DL".to_string())? {
        f.write_all(&chunk).map_err(|_| "ERR_AI_DL".to_string())?;
        got += chunk.len() as u64;
        let pct = if total > 0 { got * 100 / total } else { 0 };
        if pct != last {
            last = pct;
            let _ = app.emit_to(
                "main",
                "ai:dl",
                serde_json::json!({ "phase": phase, "pct": pct }),
            );
        }
    }
    Ok(())
}

/// Verifica la huella SHA-256 de lo descargado. En Windows con Get-FileHash
/// (viene con el sistema; misma decisión que la etapa 2 de remediación:
/// PowerShell antes que una dependencia nueva — invariante #4). Si no casa,
/// el archivo se BORRA: medio archivo corrupto no puede quedarse esperando.
fn ai_check_sha(p: &std::path::Path, want: &str) -> Result<(), String> {
    #[cfg(windows)]
    let got = {
        let mut cmd = std::process::Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-FileHash -Algorithm SHA256 -LiteralPath \"{}\").Hash.ToLower()",
                p.display()
            ),
        ]);
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let out = cmd.output().map_err(|_| "ERR_AI_DL".to_string())?;
        String::from_utf8_lossy(&out.stdout).trim().to_lowercase()
    };
    #[cfg(not(windows))]
    let got = {
        // el producto es Windows; en el espejo de desarrollo vale sha256sum
        let out = std::process::Command::new("sha256sum")
            .arg(p)
            .output()
            .map_err(|_| "ERR_AI_DL".to_string())?;
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase()
    };
    if got != want {
        let _ = fs::remove_file(p);
        return Err("ERR_AI_SHA".into());
    }
    Ok(())
}

/// Descarga con respaldo: intenta cada URL en orden y valida la huella de lo
/// que llegue. Que la fuente original responda pero con OTRO archivo (lo
/// reemplazaron río arriba) también manda al espejo: el fallo de huella es
/// tan definitivo como el de red. Devuelve el último error si ninguna fuente
/// entrega el archivo correcto.
async fn ai_fetch(
    app: &tauri::AppHandle,
    phase: &str,
    urls: &[&str],
    sha: &str,
    dest: &std::path::Path,
) -> Result<(), String> {
    let mut last = "ERR_AI_DL".to_string();
    for url in urls {
        match ai_download(app, phase, url, dest).await {
            Ok(()) => match ai_check_sha(dest, sha) {
                Ok(()) => return Ok(()),
                Err(e) => last = e,
            },
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Descomprime el zip del binario (Expand-Archive: viene con Windows).
fn ai_unzip(zip: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    let _ = fs::create_dir_all(dest);
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -Force -LiteralPath \"{}\" -DestinationPath \"{}\"",
                zip.display(),
                dest.display()
            ),
        ]);
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let out = cmd.output().map_err(|_| "ERR_AI_DL".to_string())?;
        if !out.status.success() {
            return Err("ERR_AI_DL".into());
        }
    }
    #[cfg(not(windows))]
    {
        let ok = std::process::Command::new("unzip")
            .arg("-o")
            .arg(zip)
            .arg("-d")
            .arg(dest)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            return Err("ERR_AI_DL".into());
        }
    }
    Ok(())
}

/// El botón "Descargar": trae SOLO lo que falte (respeta rutas manuales que
/// ya funcionen), verifica huellas, rellena la config y enciende el
/// análisis. Idempotente: se puede reintentar tras un fallo a media bajada.
#[tauri::command]
async fn ai_setup(app: tauri::AppHandle) -> Result<AiConfig, String> {
    let dir = ai_dir();
    let _ = fs::create_dir_all(&dir);
    let st = ai_setup_status();
    if !st.server {
        let zip = dir.join("llama-cpu.zip");
        ai_fetch(&app, "server", &[AI_LS_URL, AI_LS_URL_MIRROR], AI_LS_SHA, &zip).await?;
        ai_unzip(&zip, &dir.join("bin"))?;
        let _ = fs::remove_file(&zip);
        if find_ls(&dir.join("bin")).is_none() {
            return Err("ERR_AI_DL".into());
        }
    }
    if !st.model {
        let part = dir.join("model.part");
        ai_fetch(&app, "model", &[AI_MODEL_URL, AI_MODEL_URL_MIRROR], AI_MODEL_SHA, &part).await?;
        fs::rename(&part, ai_model_file()).map_err(|_| "ERR_AI_DL".to_string())?;
    }
    // el e5 ROTO del primer intento de la etapa 2 (2026-08-13, ni cargaba):
    // si quedó en disco, fuera — 126 MB huérfanos que ya no referencia nada
    let _ = fs::remove_file(dir.join("multilingual-e5-small-q8_0.gguf"));
    // etapa 2: el modelo de embeddings (~319 MB). Mismo trato que los otros
    // dos: solo si falta, huella verificada, y el fallo deja todo como
    // estaba (el análisis funciona sin él — el 2B decide solo).
    if !st.emb {
        let part = dir.join("emb.part");
        ai_fetch(&app, "emb", &[AI_EMB_URL, AI_EMB_URL_MIRROR], AI_EMB_SHA, &part).await?;
        fs::rename(&part, ai_emb_file()).map_err(|_| "ERR_AI_DL".to_string())?;
    }
    let mut cfg = load_ai_config();
    cfg.enabled = true;
    if cfg.server.trim().is_empty() || !std::path::Path::new(cfg.server.trim()).is_file() {
        if let Some(exe) = find_ls(&dir.join("bin")) {
            cfg.server = exe.display().to_string();
        }
    }
    if cfg.model.trim().is_empty() || !std::path::Path::new(cfg.model.trim()).is_file() {
        cfg.model = ai_model_file().display().to_string();
    }
    // la ruta del e5 roto se pisa con la buena (ai_emb_path ya lo ignora,
    // pero la config no debe seguir enseñando un camino muerto)
    if cfg.emb.trim().is_empty()
        || cfg.emb.trim().ends_with("multilingual-e5-small-q8_0.gguf")
        || !std::path::Path::new(cfg.emb.trim()).is_file()
    {
        cfg.emb = ai_emb_file().display().to_string();
    }
    let s = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(ai_config_path(), s).map_err(|e| e.to_string())?;
    Ok(cfg)
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

// ---------- histórico de cuota (motor del reporte) ----------
// El panel sondea la cuota cada 3 min y hasta hoy la TIRABA. El reporte
// ("¿te duró más o menos?") necesita memoria: cuántas veces se topó el
// límite y cuándo se acabó el semanal. Guardadito local y PRIVADO —
// solo porcentajes y horas de reset; nunca sale de la máquina, no viaja
// por ntfy ni en las fotos del hub (misma regla de privacidad de siempre).

const QUOTA_HIST_DAYS: i64 = 90;
/// Una foto por ciclo: el ciclo normal es de 180 s; el margen absorbe
/// re-renders (cambio de idioma) y dobles llamadas sin ensuciar la serie.
const QUOTA_HIST_MIN_GAP: i64 = 150;

#[derive(Serialize, Deserialize, Clone, Default)]
struct QuotaSnap {
    t: i64, // epoch de la foto
    #[serde(default)]
    s: Option<u32>, // % de la sesión de 5 h (None = el endpoint no lo trajo)
    #[serde(default)]
    w: Option<u32>, // % del semanal global
    #[serde(default)]
    sr: Option<i64>, // reset de sesión (epoch)
    #[serde(default)]
    wr: Option<i64>, // reset semanal (epoch)
}

#[derive(Serialize, Deserialize, Default)]
struct QuotaHist {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    snaps: Vec<QuotaSnap>,
}

fn quota_hist_path() -> PathBuf {
    app_data_dir().join("quota_history.json")
}

fn log_quota_impl(s: Option<u32>, w: Option<u32>, sr: Option<i64>, wr: Option<i64>) {
    let now = Utc::now().timestamp();
    let mut h: QuotaHist = fs::read_to_string(quota_hist_path())
        .ok()
        .and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or_default();
    if h.snaps.last().map_or(false, |l| now - l.t < QUOTA_HIST_MIN_GAP) {
        return;
    }
    h.version = 1;
    h.snaps.push(QuotaSnap { t: now, s, w, sr, wr });
    let cutoff = now - QUOTA_HIST_DAYS * 86_400;
    h.snaps.retain(|x| x.t >= cutoff);
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(x) = serde_json::to_string(&h) {
        let _ = fs::write(quota_hist_path(), x);
    }
}

/// Lo llama el panel tras cada lectura BUENA del endpoint (nunca con datos
/// simulados ni de error). Fallar en silencio es correcto: el histórico es
/// un lujo del reporte, jamás debe estorbar al ciclo de cuota.
#[tauri::command]
async fn log_quota(s: Option<u32>, w: Option<u32>, sr: Option<i64>, wr: Option<i64>) {
    let _ = tauri::async_runtime::spawn_blocking(move || log_quota_impl(s, w, sr, wr)).await;
}

#[tauri::command]
async fn get_quota_history(days: Option<u32>) -> Result<Vec<QuotaSnap>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = i64::from(days.unwrap_or(90).clamp(1, 90));
        let cutoff = Utc::now().timestamp() - d * 86_400;
        let h: QuotaHist = fs::read_to_string(quota_hist_path())
            .ok()
            .and_then(|x| serde_json::from_str(&x).ok())
            .unwrap_or_default();
        Ok(h.snaps.into_iter().filter(|x| x.t >= cutoff).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- Remediación etapa 2: automático out-of-band ----------
// docs/remediacion.md — SOLO LOCAL: nada de matar procesos ni mover
// archivos por SSH; WSL queda para la etapa 4 (sus procesos viven dentro
// de la VM y desde Win32 ni se ven). Tres piezas: MCPs zombies (detectar
// + cerrar con anti-reciclaje de PID), archivar JSONL ≥365 días y el
// registro de acciones. El desbloqueo progresivo (primera vez SIEMPRE
// manual) vive en el frontend; aquí solo se ejecuta y se registra.

/// Registro de acciones aplicadas (candado de confianza: el usuario puede
/// auditar TODO lo que Michi hizo). d1/d2 son datos crudos que el panel
/// traduce (invariante #10: Rust no redacta textos).
#[derive(Serialize, Deserialize, Clone, Default)]
struct RemAction {
    ts: i64,
    /// "zombie" | "archive"
    kind: String,
    /// true = la aplicó el modo automático; false = clic del usuario
    auto: bool,
    ok: bool,
    /// zombie: nombre del MCP · archive: nº de archivos
    #[serde(default)]
    d1: String,
    /// zombie: ejecutable · archive: MB movidos
    #[serde(default)]
    d2: String,
    /// relay + /clear con red: NOMBRE del archivo de copia (sin ruta), para
    /// que el panel pueda ofrecer "ver la copia". Solo el nombre a
    /// propósito: la carpeta la pone el backend y así una ruta del panel no
    /// puede abrir nada de fuera (misma regla que LAST_EXPORT).
    #[serde(default)]
    file: String,
    /// Dónde vive la copia: "" = esta máquina, nombre de servidor SSH, o
    /// `wsl-<distro>` (2026-08-13, visor de copias). Con esto las remotas
    /// también se pueden VER: el visor la trae por SSH con `read_handoff`.
    #[serde(default)]
    origin: String,
}

fn actions_log_path() -> PathBuf {
    app_data_dir().join("actions_log.json")
}

/// Añade una entrada al registro (tope 200 — es una bitácora, no un
/// histórico infinito). Nunca viaja a ntfy ni al hub: contiene nombres.
fn log_action(kind: &str, auto: bool, ok: bool, d1: String, d2: String) {
    log_action_file(kind, auto, ok, d1, d2, String::new(), String::new())
}

/// Igual, pero apuntando además el archivo de copia y dónde vive (ver
/// RemAction.file / RemAction.origin).
#[allow(clippy::too_many_arguments)]
fn log_action_file(
    kind: &str,
    auto: bool,
    ok: bool,
    d1: String,
    d2: String,
    file: String,
    origin: String,
) {
    let mut list: Vec<RemAction> = fs::read_to_string(actions_log_path())
        .ok()
        .and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or_default();
    list.push(RemAction {
        ts: Utc::now().timestamp(),
        kind: kind.into(),
        auto,
        ok,
        d1,
        d2,
        file,
        origin,
    });
    let skip = list.len().saturating_sub(200);
    let list: Vec<_> = list.into_iter().skip(skip).collect();
    let _ = fs::create_dir_all(app_data_dir());
    if let Ok(j) = serde_json::to_string(&list) {
        let _ = fs::write(actions_log_path(), j);
    }
}

/// Carpeta de las copias que deja el /clear con red. La MISMA que usa el
/// relevo (`<datos>/handoff`), calculada aquí para no fiarse de nadie.
fn handoff_dir() -> PathBuf {
    app_data_dir().join("handoff")
}

/// Abre la copia que guardó un /clear con red.
///
/// SEGURIDAD, misma regla que `open_export`: el panel manda un NOMBRE de
/// archivo, nunca una ruta. Aquí se rechaza cualquier nombre con separadores
/// o `..` y se compone la ruta contra `handoff_dir()`, así que solo puede
/// abrirse algo que esta misma app escribió en su propia carpeta. Sin esto,
/// "abrir lo que diga el frontend" sería abrir lo que diga cualquiera que
/// consiga hablarle.
#[tauri::command]
fn open_handoff(name: String) -> Result<(), String> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains(':')
    {
        return Err("ERR_HANDOFF_NAME".into());
    }
    let p = handoff_dir().join(&name);
    if !p.is_file() {
        return Err("ERR_HANDOFF_GONE".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // /select deja el archivo señalado en el explorador; abrirlo con la
        // app asociada sería ejecutar lo que decida el sistema para .md.
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|_| "ERR_HANDOFF_OPEN".to_string())?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(p.parent().unwrap_or(&p))
            .spawn()
            .map_err(|_| "ERR_HANDOFF_OPEN".to_string())?;
    }
    Ok(())
}

/// El contenido de una copia del /clear con red, para el VISOR del panel
/// (2026-08-13). Mismas reglas de nombre que `open_handoff`, y MÁS duras:
/// en las remotas el nombre viaja dentro de un comando ssh, así que solo se
/// acepta [A-Za-z0-9._-] — nada que un shell pueda interpretar. `origin`
/// decide dónde vivir la lectura: "" = esta máquina, nombre de servidor =
/// SSH (`cat` de su handoff/), `wsl-<distro>` = sistema de archivos de la
/// distro. Tope de 4 MB: es un visor, no un descargador.
#[tauri::command]
async fn read_handoff(name: String, origin: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if name.is_empty()
            || name.contains("..")
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        {
            return Err("ERR_HANDOFF_NAME".to_string());
        }
        const CAP: usize = 4_000_000;
        fn capped(mut s: String) -> String {
            if s.len() > CAP {
                let mut i = CAP;
                while !s.is_char_boundary(i) {
                    i -= 1;
                }
                s.truncate(i);
                s.push_str("\n… [recortado]");
            }
            s
        }
        if !origin.is_empty() {
            if let Some(r) = load_remotes().into_iter().find(|r| r.name == origin) {
                let s = ssh_out(&r.host, &format!("cat ~/.michiclaude/handoff/{name}"), "15")
                    .ok_or("ERR_HANDOFF_GONE")?;
                if s.is_empty() {
                    return Err("ERR_HANDOFF_GONE".to_string());
                }
                return Ok(capped(s));
            }
            // WSL: la carpeta handoff es hermana del buzón del relevo
            for (d, dir) in wsl_relay_dirs() {
                if wsl_origin(&d) != origin {
                    continue;
                }
                let Some(h) = dir.parent().map(|p| p.join("handoff")) else {
                    continue;
                };
                let p = h.join(&name);
                if let Ok(s) = fs::read_to_string(&p) {
                    return Ok(capped(s));
                }
            }
            return Err("ERR_HANDOFF_GONE".to_string());
        }
        let p = handoff_dir().join(&name);
        fs::read_to_string(&p)
            .map(capped)
            .map_err(|_| "ERR_HANDOFF_GONE".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// El .jsonl de la sesión que un /clear acaba de borrar de la vista, para el
/// visor (globo post-/clear, docs/remediacion.md §"El globo post-/clear").
/// Claude Code NO borra el transcript al hacer /clear: sigue en
/// `projects/*/<sid>.jsonl`, así que el "seguir viendo lo anterior" no
/// necesita que exista copia handoff — este comando lo localiza y lo trae.
///
/// SEGURIDAD, misma familia que `read_handoff`: el frontend NUNCA manda una
/// ruta. Manda un `sid` (charset [A-Za-z0-9-], el uuid que el relevo vio en
/// modo chat) o, sin sid (terminal), un `cwd` + `ts` con los que ESTE lado
/// busca la sesión: archivo cuyo cwd (leído de su cabecera) casa y que ya
/// existía antes del /clear. La ruta se compone siempre aquí, contra las
/// mismas raíces del coach (local + WSL) o por ssh con el sid validado.
/// Solo lectura, tope 4 MB. Un sid corto (el de 8 chars de los hits del
/// coach) se trata como "sin sid": jamás casaría un nombre de archivo.
#[tauri::command]
async fn read_cleared(sid: String, cwd: String, ts: i64, origin: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !sid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err("ERR_SESSION_NAME".to_string());
        }
        let sid = if sid.len() >= 36 { sid } else { String::new() };
        const CAP: usize = 4_000_000;
        fn capped(mut s: String) -> String {
            if s.len() > CAP {
                let mut i = CAP;
                while !s.is_char_boundary(i) {
                    i -= 1;
                }
                s.truncate(i);
                s.push_str("\n… [recortado]");
            }
            s
        }
        // misma normalización que cwdKey() del frontend
        fn cwd_key(p: &str) -> String {
            p.replace('\\', "/").trim_end_matches('/').to_lowercase()
        }
        // saca el valor de una clave string de un fragmento de JSON crudo,
        // deshaciendo los escapes: las cabeceras pueden pasar del tamaño de
        // línea razonable y un parse entero del head no es fiable
        fn raw_str(head: &str, key: &str) -> Option<String> {
            let pat = format!("\"{key}\":\"");
            let i = head.find(&pat)? + pat.len();
            let mut out = String::new();
            let mut esc = false;
            for c in head[i..].chars() {
                if esc {
                    out.push(c);
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    return Some(out);
                } else {
                    out.push(c);
                }
            }
            None
        }
        if !origin.is_empty() && !origin.starts_with("wsl-") {
            // Servidor SSH: el disco es suyo, así que la BÚSQUEDA la hace él
            // con el exportador (`--cleared-stdin`, réplica de esta misma
            // función — invariante #1). Las señas van por STDIN igual que
            // los precios: son datos del usuario (un cwd con espacios o
            // comillas) y en la línea de comandos habría shell que los
            // interpretara. Un exportador VIEJO no conoce el flag, cae al
            // camino normal y devuelve JSON de gasto: no empieza por "{"…
            // con líneas jsonl, así que se descarta y el visor dice GONE —
            // se degrada solo, como --coach.
            let Some(r) = load_remotes().into_iter().find(|r| r.name == origin) else {
                return Err("ERR_SESSION_GONE".to_string());
            };
            let payload = serde_json::json!({"sid": &sid, "cwd": &cwd, "ts": ts}).to_string();
            let mut cmd = std::process::Command::new("ssh");
            cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"])
                .arg(&r.host)
                .arg(format!("{} --cleared-stdin", r.command))
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            }
            let mut child = cmd.spawn().map_err(|_| "ERR_SESSION_GONE".to_string())?;
            if let Some(mut si) = child.stdin.take() {
                use std::io::Write;
                let _ = si.write_all(payload.as_bytes());
                // cerrar stdin siempre: si no, el exportador espera para siempre
            }
            let out = child
                .wait_with_output()
                .map_err(|_| "ERR_SESSION_GONE".to_string())?;
            let s = String::from_utf8_lossy(&out.stdout).to_string();
            // la respuesta buena son líneas jsonl (cada una un objeto); un
            // exportador viejo devolvería el JSON de gasto en UNA línea
            if !out.status.success() || s.trim().is_empty() || !s.trim_start().starts_with('{') {
                return Err("ERR_SESSION_GONE".to_string());
            }
            if s.lines().filter(|l| l.trim_start().starts_with('{')).count() < 2 {
                return Err("ERR_SESSION_GONE".to_string());
            }
            return Ok(capped(s));
        }
        // esta máquina: mismas raíces que el coach (local + distros WSL);
        // con origin "wsl-<distro>" solo esa distro
        let mut roots: Vec<PathBuf> = Vec::new();
        if origin.is_empty() {
            roots.push(claude_dir().join("projects"));
        }
        for (d, p) in wsl_claude_dirs() {
            if origin.is_empty() || wsl_origin(&d) == origin {
                roots.push(p.join("projects"));
            }
        }
        // 1) con sid: el nombre del archivo ES el sid (regla de
        // session_jsonl del relevo: no se reproduce la transformación de
        // carpetas de Claude Code, se busca por nombre)
        if !sid.is_empty() {
            for root in &roots {
                let Ok(projs) = fs::read_dir(root) else { continue };
                for proj in projs.flatten() {
                    let p = proj.path().join(format!("{sid}.jsonl"));
                    if p.is_file() {
                        return fs::read_to_string(&p)
                            .map(capped)
                            .map_err(|_| "ERR_SESSION_GONE".to_string());
                    }
                }
            }
            return Err("ERR_SESSION_GONE".to_string());
        }
        // 2) terminal (sin sid): candidata = sesión del MISMO cwd que calló
        // justo antes del /clear (mtime <= ts+120 por relojes) y que ya
        // vivía de antes (primer timestamp < ts-30) — eso excluye a la
        // recién nacida del propio /clear, que también ronda ese minuto.
        let want = cwd_key(&cwd);
        if want.is_empty() || ts <= 0 {
            return Err("ERR_SESSION_GONE".to_string());
        }
        let mut best: Option<(i64, PathBuf)> = None;
        for root in &roots {
            let Ok(projs) = fs::read_dir(root) else { continue };
            for proj in projs.flatten() {
                let ppath = proj.path();
                if !ppath.is_dir() {
                    continue;
                }
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
                    if mtime > ts + 120 || ts - mtime > 6 * 3600 {
                        continue;
                    }
                    if best.as_ref().is_some_and(|(m, _)| *m >= mtime) {
                        continue; // ya hay una más cercana al /clear
                    }
                    // cabecera: cwd y primer timestamp viven en las
                    // primeras líneas; 16 KB bastan y no cuesta abrir todo
                    let Ok(fh) = fs::File::open(&fp) else { continue };
                    let mut buf = vec![0u8; 16384];
                    let n = {
                        use std::io::Read as _;
                        let mut fh = fh;
                        fh.read(&mut buf).unwrap_or(0)
                    };
                    let head = String::from_utf8_lossy(&buf[..n]);
                    let Some(c) = raw_str(&head, "cwd") else { continue };
                    if cwd_key(&c) != want {
                        continue;
                    }
                    let born = raw_str(&head, "timestamp")
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|d| d.timestamp())
                        .unwrap_or(i64::MAX);
                    if born >= ts - 30 {
                        continue; // nació con (o después de) el /clear: es la nueva
                    }
                    best = Some((mtime, fp));
                }
            }
        }
        let Some((_, fp)) = best else {
            return Err("ERR_SESSION_GONE".to_string());
        };
        fs::read_to_string(&fp)
            .map(capped)
            .map_err(|_| "ERR_SESSION_GONE".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_action_log() -> Result<Vec<RemAction>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut list: Vec<RemAction> = fs::read_to_string(actions_log_path())
            .ok()
            .and_then(|x| serde_json::from_str(&x).ok())
            .unwrap_or_default();
        list.reverse(); // la más reciente primero
        Ok(list)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- relevo: descubrimiento (docs/remediacion.md etapa 3b) ----------
// El relevo `michi claude` (crate APARTE en relevo/, binario michi.exe) envuelve
// a Claude Code en una ConPTY y deja su estado en
// %APPDATA%\<app>\relevo\<pid>.json cada 500 ms. Aquí SOLO SE LEE: saber qué
// sesiones tienen relevo y en qué estado están. Inyectar es la etapa 3c — y
// aunque el panel aún no pueda pedir nada, quien decide siempre es el relevo
// (`attend()` vuelve a comprobar R1-R3 en el instante de escribir).

fn relay_dir() -> PathBuf {
    app_data_dir().join("relevo")
}

/// Sesión viva = estado refrescado hace menos de esto. Un relevo matado de
/// golpe deja su archivo atrás; la frescura es lo único fiable. MISMA regla
/// que `michi status` — si cambia, cambia en los dos lados.
const RELAY_FRESH: i64 = 15;
/// Un archivo que lleva un día sin tocarse es basura de un relevo muerto de
/// golpe: uno vivo escribe cada 500 ms. Se borra al pasar (es nuestra propia
/// carpeta de datos, y así no crece para siempre).
const RELAY_STALE: i64 = 24 * 3600;

/// Una sesión de Claude Code con relevo. Espejo de lo que escribe el relevo,
/// sin el bloque `diag` (cuentas de teclas para `michi status --debug`) ni
/// nada que huela a contenido: el relevo NUNCA escribe lo tecleado.
#[derive(Serialize, Default, Clone)]
struct Relay {
    /// versión del formato de estado del relevo. ≥2 = sabe hacer la red
    /// /export antes de un /clear; el panel NO pide la red a uno viejo
    /// (la ignoraría y borraría sin copia — fail-closed).
    v: u32,
    pid: u32,
    /// ruta completa donde se lanzó — es la identidad con la que se casa
    /// con la sesión de los logs
    cwd: String,
    /// última carpeta del cwd: lo que se enseña
    project: String,
    started: i64,
    ts: i64,
    /// listo para recibir un comando AHORA (R1-R3 en verde)
    ready: bool,
    /// por qué no: ERR_RELAY_* — lo traduce el panel (invariante #10)
    why: String,
    /// hay texto sin enviar en el prompt (R1)
    typed: bool,
    idle_in: u64,
    idle_out: u64,
    /// el usuario aplicó él mismo uno de los dos comandos de la lista
    /// blanca (para el desbloqueo progresivo de la 3c)
    user_cmd: String,
    user_cmd_ts: i64,
    /// "terminal" (ConPTY/PTY) o "chat" (proxy stream-json de la extensión
    /// de VS Code). Cambia lo que se puede afirmar: en chat no hay borrador
    /// visible, y el casado es exacto por `sid`.
    mode: String,
    /// session_id del log — solo en modo chat, donde el protocolo lo da.
    /// Con esto el casado deja de ser heurística.
    sid: String,
    /// De qué máquina viene: vacío = esta. Lo pone quien lee, igual que el
    /// origen de los hits del coach (invariante del hub).
    origin: String,
}

/// Lee la carpeta del relevo y devuelve las sesiones VIVAS. Nunca falla:
/// sin carpeta, sin relevo instalado o con un archivo a medio escribir,
/// devuelve lo que pueda (una lista vacía es una respuesta válida).
/// Un estado de relevo (el JSON que escribe el propio relevo) convertido en
/// `Relay`. UNA sola versión para los TRES buzones —local, SSH y WSL—: si
/// mañana el relevo publica un campo nuevo, aparece en los tres a la vez o en
/// ninguno. Devuelve None si la sesión ya no cuenta como viva.
fn relay_from_json(v: &serde_json::Value, origin: &str, now: i64) -> Option<Relay> {
    let ts = v["ts"].as_i64().unwrap_or(0);
    if now - ts > RELAY_FRESH || !v["alive"].as_bool().unwrap_or(false) {
        return None;
    }
    let cwd = v["cwd"].as_str().unwrap_or("").to_string();
    Some(Relay {
        v: v["v"].as_u64().unwrap_or(1) as u32,
        pid: v["pid"].as_u64().unwrap_or(0) as u32,
        project: path_base(&cwd),
        cwd,
        started: v["started"].as_i64().unwrap_or(0),
        ts,
        ready: v["ready"].as_bool().unwrap_or(false),
        why: v["why"].as_str().unwrap_or("").to_string(),
        typed: v["typed"].as_bool().unwrap_or(false),
        idle_in: v["idle_in"].as_u64().unwrap_or(0),
        idle_out: v["idle_out"].as_u64().unwrap_or(0),
        user_cmd: v["user_cmd"].as_str().unwrap_or("").to_string(),
        user_cmd_ts: v["user_cmd_ts"].as_i64().unwrap_or(0),
        mode: v["mode"].as_str().unwrap_or("terminal").to_string(),
        sid: v["sid"].as_str().unwrap_or("").to_string(),
        origin: origin.to_string(),
    })
}

/// Los relevos de un buzón que se ve como carpeta (el de esta máquina o el de
/// una distro de WSL). `sweep` solo lo pone el local: la basura de una distro
/// la limpia su propio relevo al morir, y borrar a través de \\wsl.localhost
/// es justo lo que este proyecto evita.
fn scan_relay_dir(dir: &PathBuf, origin: &str, sweep: bool) -> Vec<Relay> {
    let mut out: Vec<Relay> = Vec::new();
    let now = Utc::now().timestamp();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&p) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        match relay_from_json(&v, origin, now) {
            Some(r) => out.push(r),
            None => {
                if sweep && now - v["ts"].as_i64().unwrap_or(0) > RELAY_STALE {
                    let _ = fs::remove_file(&p);
                }
            }
        }
    }
    out
}

fn scan_relays() -> Vec<Relay> {
    let mut out = scan_relay_dir(&relay_dir(), "", true);
    out.sort_by_key(|r| r.started);
    out
}

/// Última carpeta de una ruta, con las barras normalizadas (Windows mezcla
/// las dos). Mismo criterio que `cwd_name`, pero sobre una ruta suelta.
fn path_base(p: &str) -> String {
    p.replace('\\', "/")
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Un comando por SSH, con la MISMA tubería que el resto del hub. Devuelve
/// None en silencio si el servidor no responde: una fuente remota caída no
/// puede entorpecer al panel (misma regla que `fetch_remote`).
fn ssh_out(host: &str, command: &str, secs: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o"])
        .arg(format!("ConnectTimeout={secs}"))
        .arg(host)
        .arg(command);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// Etapa 4b: los relevos que viven en OTRA máquina. Se leen por SSH con un
/// `cat` de la carpeta remota — el mismo patrón que el exportador, sin
/// inventar protocolo. El `origin` (nombre que el usuario le dio al servidor)
/// viaja en el Relay igual que en los hits del coach: lo etiqueta quien lee.
///
/// Coste: una conexión SSH por servidor. Por eso NO va en el sondeo de 5 s de
/// la pestaña, sino en el compás del coach (3 min) — y el panel se queda con
/// lo último bueno mientras tanto.
fn scan_relays_remote() -> Vec<Relay> {
    let mut out = Vec::new();
    for r in load_remotes() {
        // una sola llamada por servidor: todos los .json de golpe, separados
        // por una marca que no puede aparecer dentro de un JSON de una línea
        let Some(raw) = ssh_out(
            &r.host,
            "for f in ~/.michiclaude/relevo/*.json; do [ -f \"$f\" ] && cat \"$f\" && echo; done",
            "5",
        ) else {
            continue;
        };
        let now = Utc::now().timestamp();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
                continue;
            };
            if let Some(rel) = relay_from_json(&v, &r.name, now) {
                out.push(rel);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// WSL: la tercera máquina (etapa 4d)
//
// Un usuario de Windows con Claude Code dentro de WSL no es un caso raro, y
// hasta aquí no lo veía nadie: el shim del PATH no alcanza a WSL (resuelve
// Windows, no la distro) y el alias de ~/.bashrc solo llegaba por SSH.
//
// La buena noticia es que WSL no necesita protocolo nuevo. Es Linux, así que
// valen los MISMOS guiones que en un servidor (TERM_ALIAS_PY para el alias,
// CHAT_WRAP_PY para el chat — Remote-WSL instala su .vscode-server en el home
// de la distro, exactamente como un servidor). Solo cambia el transporte:
// donde había `ssh host`, hay `wsl.exe -d <distro>`. Y el buzón del relevo se
// ve como CARPETA por \\wsl.localhost, así que leer estados e inyectar
// órdenes es fs a secas — sin SSH, sin daemon, sin puertos.
//
// Qué NO se hace y por qué: no se despiertan distros que no usan Claude (la
// lista sale de `wsl_claude_dirs`, que ya filtra por ~/.claude), y no se
// borran archivos ajenos a través de \\wsl.localhost (lento y falible; la
// basura la limpia el relevo de la distro al morir).
// ---------------------------------------------------------------------------

/// Las distros donde vive un Claude Code, sin repetir. Derivarlo de
/// `wsl_claude_dirs` en vez de listar WSL a secas evita ofrecer distros que
/// el usuario no usa para esto (y evita arrancarlas).
#[cfg(windows)]
fn wsl_distros() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for (d, _) in wsl_claude_dirs() {
        if !v.contains(&d) {
            v.push(d);
        }
    }
    v
}

#[cfg(not(windows))]
fn wsl_distros() -> Vec<String> {
    Vec::new()
}

/// El buzón de relevo de cada home con Claude dentro de WSL: el hermano de
/// `~/.claude` que ya sabemos encontrar.
#[cfg(windows)]
fn wsl_relay_dirs() -> Vec<(String, PathBuf)> {
    wsl_claude_dirs()
        .into_iter()
        .filter_map(|(d, c)| {
            c.parent()
                .map(|h| (d, h.join(".michiclaude").join("relevo")))
        })
        .collect()
}

#[cfg(not(windows))]
fn wsl_relay_dirs() -> Vec<(String, PathBuf)> {
    Vec::new()
}

/// Cómo se llama una distro cuando viaja como `origin` de un Relay. El prefijo
/// es el mismo que ya usan los proyectos de WSL en las estadísticas, así que
/// el usuario lee lo mismo en los dos sitios.
fn wsl_origin(distro: &str) -> String {
    format!("wsl-{distro}")
}

/// Los relevos que viven dentro de WSL. Puro sistema de archivos: ni SSH ni
/// arrancar la distro para preguntar.
fn scan_relays_wsl() -> Vec<Relay> {
    let mut out = Vec::new();
    for (distro, dir) in wsl_relay_dirs() {
        out.extend(scan_relay_dir(&dir, &wsl_origin(&distro), false));
    }
    out
}

/// El buzón donde vive ESE pid, para inyectarle. Se busca por el estado que
/// el propio relevo publica: si el archivo no está, la sesión no existe y no
/// se escribe nada a ciegas.
fn wsl_relay_dir(origin: &str, pid: u32) -> Option<PathBuf> {
    wsl_relay_dirs()
        .into_iter()
        .filter(|(d, _)| wsl_origin(d) == origin)
        .map(|(_, dir)| dir)
        .find(|dir| dir.join(format!("{pid}.json")).is_file())
}

#[tauri::command]
async fn get_relays(remote: bool) -> Result<Vec<Relay>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut v = scan_relays();
        if remote {
            v.extend(scan_relays_remote());
            // WSL viaja con las remotas y no con las locales a propósito:
            // \\wsl.localhost puede tardar, y el sondeo de 5 s de la pestaña
            // no puede quedarse esperando a una distro dormida.
            v.extend(scan_relays_wsl());
        }
        Ok(v)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// La MISMA lista blanca que el relevo. Se comprueba en los dos lados a
/// propósito: aquí para no escribir nunca una orden que no se pueda cumplir,
/// y allí porque es el límite duro — el relevo no se fía de quien le escriba.
const RELAY_ALLOWED: [&str; 2] = ["/compact", "/clear"];

/// Escritura atómica en el canal del relevo. El temporal AÑADE `.tmp` al
/// nombre ENTERO: con `with_extension`, `<pid>.cmd` y `<pid>.json`
/// compartirían `<pid>.tmp` y se pisarían (misma regla que en el relevo).
fn relay_write_cmd(path: &PathBuf, data: &str) -> bool {
    let Some(name) = path.file_name().and_then(|x| x.to_str()) else {
        return false;
    };
    let tmp = path.with_file_name(format!("{name}.tmp"));
    fs::write(&tmp, data).is_ok() && fs::rename(&tmp, path).is_ok()
}

/// Pide al relevo que teclee uno de los dos comandos y ESPERA su veredicto.
///
/// El panel pide; quien decide es el relevo: `attend()` vuelve a comprobar
/// R1-R3 (texto sin enviar, Claude generando, calma de teclado) en el instante
/// de escribir. Que el countdown de la UI haya terminado NO es un permiso —
/// entre que el usuario ve el aviso y el relevo actúa puede haber empezado a
/// teclear, y ese caso tiene que perder.
///
/// Devuelve el código `ERR_RELAY_*` que dé el relevo, sin traducir
/// (invariante #10). `ERR_RELAY_NOACK` es nuestro: la orden se escribió pero
/// nadie contestó — un relevo que murió justo en medio.
/// Etapa 4c: la misma orden, pero a una máquina remota. Se escribe el `.cmd`
/// por SSH (tmp+rename EN EL SERVIDOR, para que el relevo no lea un archivo a
/// medias) y se espera el acuse releyendo su estado.
///
/// El comando se pasa por STDIN, nunca interpolado en la línea de shell: el
/// texto sale de una lista blanca de dos elementos, pero un día alguien
/// ampliará esa lista y no quiero que ese día una comilla se convierta en
/// ejecución remota. Se cierra la puerta antes de que exista.
/// Devuelve la RUTA de la copia del /clear con red (vacía si no hubo), tal
/// como la publicó el acuse del relevo remoto — el registro guarda solo su
/// nombre (path_base) y el visor la trae por SSH cuando haga falta.
fn relay_inject_remote(
    host: &str,
    pid: u32,
    text: &str,
    id: &str,
    export: bool,
) -> Result<String, String> {
    use std::io::Write;
    let body =
        serde_json::json!({"id": id, "op": "inject", "text": text, "export": export}).to_string();
    let script = format!(
        "d=$HOME/.michiclaude/relevo; mkdir -p $d; cat > $d/{pid}.cmd.tmp && \
         mv $d/{pid}.cmd.tmp $d/{pid}.cmd"
    );
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(host)
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|_| "ERR_RELAY_WRITE".to_string())?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(body.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| "ERR_RELAY_WRITE".to_string())?;
    if !out.status.success() {
        return Err("ERR_RELAY_WRITE".to_string());
    }
    // el acuse: el relevo remoto lo publica en su estado, igual que el local.
    // Con red, la secuencia espera a la copia: más margen antes de rendirse.
    let tries = if export { 60 } else { 16 };
    for _ in 0..tries {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let Some(raw) = ssh_out(host, &format!("cat ~/.michiclaude/relevo/{pid}.json"), "5")
        else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(raw.trim()) else {
            continue;
        };
        if v["last"]["id"].as_str() != Some(id) {
            continue;
        }
        return if v["last"]["ok"].as_bool().unwrap_or(false) {
            Ok(v["last"]["export"].as_str().unwrap_or("").to_string())
        } else {
            Err(v["last"]["err"].as_str().unwrap_or("ERR_RELAY_NOACK").to_string())
        };
    }
    Err("ERR_RELAY_NOACK".to_string())
}

#[tauri::command]
async fn relay_inject(
    pid: u32,
    text: String,
    auto: bool,
    origin: Option<String>,
    export: Option<bool>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !RELAY_ALLOWED.contains(&text.as_str()) {
            return Err("ERR_RELAY_BADCMD".to_string());
        }
        // La red de seguridad (/export verificado antes) solo acompaña a
        // /clear, y solo si el panel la pidió — con un relevo viejo (v1) el
        // panel NO la pide: la marca se ignoraría y borraría sin copia.
        let export = export.unwrap_or(false) && text == "/clear";
        // ¿en otra máquina? El origen es el nombre que el usuario le dio al
        // servidor; se traduce a host aquí y no se acepta uno desconocido.
        // Un origen que no case con ningún servidor NO es un error todavía:
        // puede ser una distro de WSL (abajo).
        let origin = origin.unwrap_or_default();
        if !origin.is_empty() {
            if let Some(r) = load_remotes().into_iter().find(|r| r.name == origin) {
                let id = format!("app-{}", Utc::now().timestamp_millis());
                let res = relay_inject_remote(&r.host, pid, &text, &id, export);
                // la copia remota SÍ se apunta (2026-08-13): con el nombre y
                // el origen, el visor puede traerla por SSH (read_handoff)
                let copia = res.as_deref().map(path_base).unwrap_or_default();
                log_action_file(
                    "relay",
                    auto,
                    res.is_ok(),
                    text.clone(),
                    origin.clone(),
                    copia,
                    origin,
                );
                return res.map(|_| ());
            }
        }
        // WSL: el buzón se ve como carpeta, así que es el MISMO camino que el
        // local. Los servidores mandan primero (arriba): si alguien llamó a su
        // servidor igual que una distro, gana lo que configuró a mano.
        let dir = if origin.is_empty() {
            relay_dir()
        } else {
            match wsl_relay_dir(&origin, pid) {
                Some(d) => d,
                None => return Err("ERR_RELAY_GONE".to_string()),
            }
        };
        // Nombre del proyecto para el registro, del estado del propio relevo.
        let proj = fs::read_to_string(dir.join(format!("{pid}.json")))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| path_base(v["cwd"].as_str().unwrap_or("")))
            .unwrap_or_default();
        // Lo aplicado queda en el registro de acciones igual que los zombies y
        // el archivado: si Michi teclea en tu terminal, tiene que quedar
        // escrito. Crudo — lo traduce el panel (invariante #10).
        let id = format!("app-{}", Utc::now().timestamp_millis());
        match relay_inject_fs(&dir, pid, &text, &id, export) {
            Ok(a) => {
                log_action_file(
                    "relay",
                    auto,
                    a.ok,
                    text.clone(),
                    proj.clone(),
                    a.copia,
                    origin.clone(),
                );
                if a.ok {
                    Ok(())
                } else {
                    Err(a.err)
                }
            }
            Err(e) => {
                log_action("relay", auto, false, text.clone(), proj.clone());
                Err(e)
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Lo que contestó el relevo a una orden. `Err` de `relay_inject_fs` es "no
/// contestó nadie"; esto es "contestó, y dijo esto" — la diferencia importa
/// para el registro: un rechazo del candado SÍ se anota con su motivo.
struct RelayAck {
    ok: bool,
    err: String,
    copia: String,
}

/// Escribe la orden en un buzón que se ve como carpeta (el de esta máquina o
/// el de una distro de WSL) y espera el acuse. UNA implementación para los
/// dos: el canal por archivos se diseñó así justamente para que la distancia
/// no cambiara el código.
fn relay_inject_fs(
    dir: &PathBuf,
    pid: u32,
    text: &str,
    id: &str,
    export: bool,
) -> Result<RelayAck, String> {
    let wrote = relay_write_cmd(
        &dir.join(format!("{pid}.cmd")),
        &serde_json::json!({"id": id, "op": "inject", "text": text, "export": export})
            .to_string(),
    );
    if !wrote {
        return Err("ERR_RELAY_WRITE".to_string());
    }
    // El relevo mira su buzón cada 250 ms y publica el acuse en el estado.
    // 8 s de margen: si en ese tiempo no contestó, no está vivo. Con red,
    // la secuencia además espera a la copia: hasta ~15 s más.
    let state = dir.join(format!("{pid}.json"));
    let tries = if export { 150 } else { 40 };
    for _ in 0..tries {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let Ok(raw) = fs::read_to_string(&state) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if v["last"]["id"].as_str() != Some(id) {
            continue;
        }
        return Ok(RelayAck {
            ok: v["last"]["ok"].as_bool().unwrap_or(false),
            err: v["last"]["err"]
                .as_str()
                .unwrap_or("ERR_RELAY_NOACK")
                .to_string(),
            // La copia del /clear con red, para el botón "abrir la copia".
            // Se guarda SOLO el nombre (ver RemAction.file); la carpeta la
            // vuelve a poner el backend al abrir.
            copia: v["last"]["export"]
                .as_str()
                .map(path_base)
                .unwrap_or_default(),
        });
    }
    Err("ERR_RELAY_NOACK".to_string())
}

// ---------- atajo: que `claude` pase por el relevo (etapa 3c) ----------
// El problema que resuelve: si hay que acordarse de escribir `michi claude`,
// nadie se acuerda. Se trabaja media hora y después se descubre que la sesión
// no tenía relevo y Michi no podía hacer nada.
//
// POR QUÉ UN SHIM EN EL PATH y no un alias por shell: las terminales y los
// editores (Windows Terminal, VS Code, Cursor, Warp, Alacritty…) no
// interpretan `claude` — ejecutan un SHELL, y el shell resuelve el comando.
// Configurar shells serían cuatro mecanismos distintos (PowerShell 7, 5.1,
// cmd, Git Bash) y aun así se quedarían fuera los que salgan mañana. Con un
// `claude.cmd` propio primero en el PATH resuelve WINDOWS, así que vale para
// todos a la vez. Lo que NO alcanza: WSL y SSH (cruzan la frontera, son la
// etapa 4) y cualquier integración que llame al binario por ruta absoluta.

fn shim_dir() -> PathBuf {
    app_data_dir().join("bin")
}
fn shim_path() -> PathBuf {
    shim_dir().join("claude.cmd")
}
/// Copia del PATH de usuario ANTES de tocarlo. Modificar el PATH es lo más
/// invasivo que hace la app: si algo sale mal, aquí está el original.
fn path_backup_path() -> PathBuf {
    app_data_dir().join("path_backup.txt")
}

/// Dónde está `michi.exe`. Se busca en este orden: junto al ejecutable de la
/// app y en su carpeta `resources` (los dos sitios donde puede dejarlo el
/// instalador según cómo resuelva Tauri los recursos — se prueban AMBOS
/// porque equivocarse deja el atajo del PATH muerto en silencio), en el
/// `target` del crate del relevo (desarrollo) y por último en el PATH.
fn michi_exe() -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            // dev: target\debug\michiclaude.exe → ..\..\..\relevo\target\release
            let dev = d
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|root| {
                    root.join("relevo")
                        .join("target")
                        .join("release")
                        .join("michi.exe")
                });
            // EN DESARROLLO manda el binario que compila el crate, NO la copia
            // que `tauri dev` deja junto al ejecutable: esa se rehace en cada
            // arranque de la app, así que el ajuste del chat podía quedar
            // apuntando a una versión vieja — nos costó tres rondas
            // persiguiendo un fantasma (2026-08-10). En una instalación real
            // no existe tal copia y el orden de siempre es el bueno.
            #[cfg(debug_assertions)]
            if let Some(p) = dev.clone() {
                cands.push(p);
            }
            cands.push(d.join("michi.exe"));
            cands.push(d.join("resources").join("michi.exe"));
            cands.push(d.join("relevo").join("michi.exe"));
            if let Some(p) = dev {
                cands.push(p);
            }
        }
    }
    if let Some(p) = cands.into_iter().find(|p| p.is_file()) {
        return Some(p);
    }
    which_exe("michi")
}

/// Primera coincidencia de `where.exe <nombre>` que NO esté en nuestra carpeta
/// de shim (si no, el atajo se encontraría a sí mismo y se llamaría en bucle).
fn which_exe(name: &str) -> Option<PathBuf> {
    let mut cmd = std::process::Command::new("where.exe");
    cmd.arg(name);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    let txt = String::from_utf8_lossy(&out.stdout);
    let ours = shim_dir();
    txt.lines()
        .map(|l| PathBuf::from(l.trim()))
        .find(|p| p.is_file() && p.parent() != Some(ours.as_path()))
}

/// El atajo. Sin bloques `( )` a propósito: dentro de un bloque, `%errorlevel%`
/// se expande al PARSEAR y devuelve el valor viejo. Y dos salidas de seguridad
/// —`MICHI_RELEVO` ya puesto (estamos dentro de un relevo, no re-envolver) y
/// relevo ausente— para que este atajo NUNCA te deje sin Claude Code.
///
/// SOLO ASCII: un .cmd no declara codificación y cmd.exe lo lee con la página
/// de códigos que toque, así que una tilde o una raya se ven como `â€”` (visto
/// en la primera prueba). En un comentario es cosmético, pero un archivo de
/// órdenes con bytes que se reinterpretan es una bomba de relojería.
fn shim_body(michi: &str, real: &str) -> String {
    format!(
        "@echo off\r\n\
         rem  MichiClaude - atajo del relevo. Lo crea y lo borra el interruptor\r\n\
         rem  de Ajustes; no editar a mano.\r\n\
         if defined MICHI_RELEVO goto real\r\n\
         if not exist \"{michi}\" goto real\r\n\
         \"{michi}\" claude %*\r\n\
         exit /b %errorlevel%\r\n\
         :real\r\n\
         if not exist \"{real}\" goto missing\r\n\
         \"{real}\" %*\r\n\
         exit /b %errorlevel%\r\n\
         :missing\r\n\
         echo MichiClaude: no encuentro Claude Code. Apaga el atajo en Ajustes.1>&2\r\n\
         exit /b 9009\r\n"
    )
}

/// Lee/escribe el PATH de USUARIO por PowerShell. `setx` NO sirve: trunca a
/// 1024 caracteres y se puede cargar el PATH entero.
fn user_path_get() -> Option<String> {
    let mut cmd = std::process::Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Environment]::GetEnvironmentVariable('Path','User')",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn user_path_set(value: &str) -> Result<(), String> {
    // comilla simple duplicada = escape en PowerShell
    let script = format!(
        "[Environment]::SetEnvironmentVariable('Path','{}','User')",
        value.replace('\'', "''")
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err("ERR_ALIAS_PATH".into())
    }
}

fn path_has(path: &str, dir: &str) -> bool {
    path.split(';')
        .any(|p| p.trim().trim_end_matches('\\').eq_ignore_ascii_case(dir.trim_end_matches('\\')))
}

#[tauri::command]
async fn relay_alias_status() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let dir = shim_dir();
        let ds = dir.to_string_lossy().to_string();
        let in_path = user_path_get().map(|p| path_has(&p, &ds)).unwrap_or(false);
        Ok(serde_json::json!({
            "on": shim_path().is_file() && in_path,
            // sin el binario del relevo el atajo no se puede ofrecer: el panel
            // enseña el porqué en vez de un interruptor que no haría nada
            "michi": michi_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            "dir": ds,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_relay_alias(on: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = shim_dir();
        let ds = dir.to_string_lossy().to_string();
        let mut path = user_path_get().ok_or("ERR_ALIAS_PATH")?;
        if on {
            let michi = michi_exe().ok_or("ERR_ALIAS_NOMICHI")?;
            // El Claude Code de verdad se resuelve AHORA, antes de que nuestra
            // carpeta entre al PATH: así el atajo sabe a quién delegar.
            let real = which_exe("claude").ok_or("ERR_ALIAS_NOCLAUDE")?;
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            fs::write(
                shim_path(),
                shim_body(&michi.to_string_lossy(), &real.to_string_lossy()),
            )
            .map_err(|e| e.to_string())?;
            if !path_has(&path, &ds) {
                let _ = fs::write(path_backup_path(), &path);
                // DELANTE: el PATH efectivo es máquina + usuario, y el claude
                // de npm vive en el tramo de usuario. Detrás no lo taparía.
                path = format!("{ds};{path}");
                user_path_set(&path)?;
            }
        } else {
            let _ = fs::remove_file(shim_path());
            if path_has(&path, &ds) {
                // se quita EXACTAMENTE la nuestra; lo demás se respeta tal cual
                let keep: Vec<&str> = path
                    .split(';')
                    .filter(|p| {
                        !p.trim()
                            .trim_end_matches('\\')
                            .eq_ignore_ascii_case(ds.trim_end_matches('\\'))
                    })
                    .collect();
                user_path_set(&keep.join(";"))?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// El chat de VS Code en los SERVIDORES, sin editar JSON a mano: el panel
// enciende/apaga `claudeCode.claudeProcessWrapper` en los ajustes de máquina
// del vscode-server remoto. Es la pieza que faltaba para que el relevo del
// chat no dependa de que el usuario sepa que existe michi-wrap.sh.
// ---------------------------------------------------------------------------

/// Guion que corre EN el servidor (por STDIN, jamás interpolado — misma
/// puerta cerrada que relay_inject_remote). Habla en veredictos de UNA
/// palabra que el panel traduce (invariante #10).
///
/// Reglas que no se negocian:
/// - Un wrapper AJENO no se pisa jamás (OTHER): quitarle a alguien su
///   wrapper es romperle su flujo sin avisar.
/// - Ajustes que no se pueden entender no se tocan (MANUAL): editar a
///   ciegas un archivo de configuración de otro programa no es una opción.
/// - Encender sin el lanzador subido está prohibido (NOWRAP): un ajuste que
///   apunta a un archivo inexistente deja el chat MUERTO — exactamente lo
///   que el fail-open promete que nunca pasará.
/// - Antes de tocar un archivo que no escribimos nosotros, copia
///   `.michi-backup` (una sola vez).
/// VS Code acepta JSONC; los comentarios de línea entera se quitan antes de
/// parsear (los de dentro de una cadena, como "https://…", no casan con el
/// patrón porque exigen el // a inicio de línea).
const CHAT_WRAP_PY: &str = r#"
import json, os, re, sys

op = sys.argv[1] if len(sys.argv) > 1 else "status"
# misma regla que en TERM_ALIAS_PY: lo que no reconozco no se interpreta
if op not in ("status", "on", "off"):
    print("BADOP"); sys.exit(0)
WRAP = os.path.expanduser("~/.michiclaude/michi-wrap.sh")
KEY = "claudeCode.claudeProcessWrapper"
roots = [os.path.expanduser("~/" + d) for d in
         (".vscode-server", ".vscode-server-insiders", ".cursor-server")]
roots = [r for r in roots if os.path.isdir(r)]
if not roots:
    print("NOVSCODE"); sys.exit(0)


def read(p):
    """(obj, crudo, legible). obj None si el archivo no se entiende."""
    if not os.path.isfile(p):
        return {}, "", True
    raw = open(p, encoding="utf-8", errors="replace").read()
    try:
        return json.loads(raw), raw, True
    except Exception:
        pass
    txt = re.sub(r"^\s*//.*$", "", raw, flags=re.M)
    try:
        return json.loads(txt), raw, True
    except Exception:
        return None, raw, False


if op == "status":
    has_on = has_other = has_manual = False
    for r in roots:
        obj, raw, ok = read(os.path.join(r, "data", "Machine", "settings.json"))
        if not ok:
            # ilegible pero con nuestra ruta dentro = un archivo nuestro de
            # una version vieja; ilegible sin ella = de otro, ni tocarlo
            if WRAP in raw: has_on = True
            else: has_manual = True
            continue
        v = obj.get(KEY, "")
        if v == WRAP: has_on = True
        elif v: has_other = True
    print("OTHER" if has_other else "MANUAL" if has_manual
          else "ON" if has_on else "OFF")
    sys.exit(0)

if op == "on" and not os.path.isfile(WRAP):
    print("NOWRAP"); sys.exit(0)
if op == "on":
    os.chmod(WRAP, 0o755)

out = "OK"
for r in roots:
    p = os.path.join(r, "data", "Machine", "settings.json")
    obj, raw, ok = read(p)
    if not ok:
        if op == "off" and WRAP in raw:
            os.remove(p)
        elif op == "on":
            out = "MANUAL"
        continue
    if op == "on":
        if obj.get(KEY) == WRAP:
            continue
        if obj.get(KEY):
            out = "OTHER"
            continue
        if raw and not os.path.isfile(p + ".michi-backup"):
            open(p + ".michi-backup", "w", encoding="utf-8").write(raw)
        obj[KEY] = WRAP
    else:
        if obj.get(KEY) != WRAP:
            continue
        del obj[KEY]
        if not obj:
            os.remove(p)
            continue
    os.makedirs(os.path.dirname(p), exist_ok=True)
    tmp = p + ".michi.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=2, ensure_ascii=False)
        f.write("\n")
    os.replace(tmp, p)
print(out)
"#;

#[derive(Serialize)]
struct ChatRelayRow {
    /// Nombre de la máquina. VACÍO = esta misma (el panel lo traduce, igual
    /// que el `origin` de los hits del coach — invariante #10).
    name: String,
    state: String,
}

// ---------------------------------------------------------------------------
// El chat de VS Code de ESTA máquina (etapa 4e)
//
// El shim del PATH no llega aquí: la extensión no lanza `claude` por el PATH,
// lo lanza por una ruta. Su enganche oficial es un ajuste
// (`claudeCode.claudeProcessWrapper`) que apunta a un ejecutable, y ahí va
// michi.exe — que reconoce solo la llamada de la extensión por el protocolo,
// así que no hace falta ningún lanzador intermedio.
//
// LO QUE NO SE HACE, y es deliberado: NO se re-serializa el settings.json.
// Ese archivo es del usuario, admite comentarios y suele estar peinado a
// mano; volcarlo con serde se llevaría por delante comentarios y formato.
// Se lee con serde (tras quitar comentarios) para SABER qué hay, y se
// escribe tocando el TEXTO: una línea que ponemos y una línea que quitamos.
// Antes de guardar se comprueba que lo escrito sigue siendo JSON válido; si
// no lo fuera, no se toca el archivo y se contesta MANUAL.
// ---------------------------------------------------------------------------

const WRAP_KEY: &str = "claudeCode.claudeProcessWrapper";

/// Los `settings.json` de usuario de los editores que llevan la extensión.
fn vscode_user_settings() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(app) = std::env::var("APPDATA") else {
        return out;
    };
    for d in ["Code", "Code - Insiders", "Cursor"] {
        let p = PathBuf::from(&app).join(d).join("User");
        if p.is_dir() {
            out.push(p.join("settings.json"));
        }
    }
    out
}

/// El settings.json admite comentarios; serde no. Se limpian SOLO para leer.
/// Si aun así no se entiende (una coma colgante, por ejemplo), devuelve None
/// y quien llama NO toca el archivo: es del usuario.
fn jsonc_value(raw: &str) -> Option<serde_json::Value> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        return Some(v);
    }
    let limpio: Vec<&str> = raw
        .lines()
        .map(|l| if l.trim_start().starts_with("//") { "" } else { l })
        .collect();
    serde_json::from_str(&limpio.join("\n")).ok()
}

/// ¿Dos rutas de Windows son la misma? Mayúsculas y barras dan igual.
fn same_path(a: &str, b: &str) -> bool {
    !a.is_empty()
        && a.replace('/', "\\").to_lowercase() == b.replace('/', "\\").to_lowercase()
}

/// ¿Ese wrapper es NUESTRO, aunque apunte a otra copia? michi.exe vive en
/// varios sitios legítimos (junto a la app instalada, en `resources`, en el
/// target del crate durante el desarrollo) y una actualización puede mover
/// cuál se usa. Sin esto, el interruptor ve su propia ruta vieja como "de
/// otro", se niega a tocarla —que es la regla correcta con un wrapper
/// ajeno— y se queda encallado (visto 2026-08-10). Se reconoce por el
/// nombre del archivo: ningún otro programa envuelve Claude Code con algo
/// llamado michi.exe.
fn wrapper_nuestro(v: &str) -> bool {
    PathBuf::from(v.replace('/', "\\"))
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("michi.exe"))
        .unwrap_or(false)
}

/// Nuestra línea, SIEMPRE la primera del objeto, con su coma y SOLA en su
/// renglón: así quitarla después es borrar exactamente esa línea sin dejar el
/// JSON cojo ni llevarse nada del usuario. Lo de "sola" no es estética —
/// con un settings.json escrito en UNA línea, nuestra clave compartiría
/// renglón con sus ajustes y al apagar el interruptor se los llevaría por
/// delante (cazado en el banco antes de que tocara un archivo de verdad).
fn wrap_key_insert(raw: &str, michi: &str) -> Option<String> {
    let val = serde_json::to_string(michi).ok()?;
    let base = if raw.trim().is_empty() { "{}" } else { raw };
    let i = base.find('{')?;
    let resto = &base[i + 1..];
    let linea = if resto.trim_start().starts_with('}') {
        format!("\n  \"{WRAP_KEY}\": {val}\n")
    } else if resto.starts_with('\n') || resto.starts_with("\r\n") {
        format!("\n  \"{WRAP_KEY}\": {val},")
    } else {
        format!("\n  \"{WRAP_KEY}\": {val},\n")
    };
    Some(format!("{}{}{}", &base[..=i], linea, resto))
}

fn wrap_key_remove(raw: &str) -> String {
    let clave = format!("\"{WRAP_KEY}\"");
    raw.lines()
        .filter(|l| !l.trim_start().starts_with(&clave))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Estado del wrapper local: ON / OFF / OTHER (hay uno ajeno, no se pisa) /
/// MANUAL (el archivo no se entiende) / NOVSCODE (no hay editor instalado).
fn local_chat_status() -> String {
    let files = vscode_user_settings();
    if files.is_empty() {
        return "NOVSCODE".to_string();
    }
    let michi = michi_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let (mut on, mut other, mut manual) = (false, false, false);
    for f in files {
        let Ok(raw) = fs::read_to_string(&f) else {
            continue; // sin archivo aún: es un OFF, no un problema
        };
        match jsonc_value(&raw) {
            None => {
                // ilegible pero con nuestra ruta dentro = nuestro, de una
                // versión vieja; ilegible sin ella = de otro, ni tocarlo
                if !michi.is_empty() && raw.contains(&michi) {
                    on = true;
                } else {
                    manual = true;
                }
            }
            Some(v) => {
                let cur = v[WRAP_KEY].as_str().unwrap_or("");
                if cur.is_empty() {
                } else if wrapper_nuestro(cur) {
                    on = true;
                } else {
                    other = true;
                }
            }
        }
    }
    if other {
        "OTHER"
    } else if manual {
        "MANUAL"
    } else if on {
        "ON"
    } else {
        "OFF"
    }
    .to_string()
}

fn local_chat_set(on: bool) -> String {
    let files = vscode_user_settings();
    if files.is_empty() {
        return "NOVSCODE".to_string();
    }
    // Encender apuntando a un ejecutable que no está mataría el chat: sin
    // michi.exe no se enciende nada (la pareja del NOWRAP de los servidores).
    let michi = match michi_exe() {
        Some(p) => p.to_string_lossy().to_string(),
        None => return if on { "NOMICHI".to_string() } else { "OK".to_string() },
    };
    let mut out = "OK";
    for f in files {
        let raw = fs::read_to_string(&f).unwrap_or_default();
        let Some(v) = jsonc_value(if raw.trim().is_empty() { "{}" } else { &raw }) else {
            if on {
                out = "MANUAL";
            }
            continue;
        };
        let cur = v[WRAP_KEY].as_str().unwrap_or("").to_string();
        let nuevo = if on {
            if same_path(&cur, &michi) {
                continue; // ya estaba, y apuntando aquí
            }
            if !cur.is_empty() && !wrapper_nuestro(&cur) {
                out = "OTHER"; // wrapper de otro: no se pisa
                continue;
            }
            // nuestro pero de otra copia (una actualización movió cuál se
            // usa): se quita la línea vieja y se pone la de ahora
            let base = if cur.is_empty() { raw.clone() } else { wrap_key_remove(&raw) };
            match wrap_key_insert(&base, &michi) {
                Some(n) => n,
                None => {
                    out = "MANUAL";
                    continue;
                }
            }
        } else {
            if !wrapper_nuestro(&cur) {
                continue; // no es nuestro: no se quita
            }
            wrap_key_remove(&raw)
        };
        // Verificar ANTES de guardar: si lo que hemos armado no es JSON,
        // el archivo se queda como estaba.
        if jsonc_value(&nuevo).is_none() {
            out = "MANUAL";
            continue;
        }
        if !raw.is_empty() {
            let bak = PathBuf::from(format!("{}.michi-backup", f.to_string_lossy()));
            if !bak.is_file() {
                let _ = fs::write(&bak, &raw);
            }
        }
        if let Some(d) = f.parent() {
            let _ = fs::create_dir_all(d);
        }
        let tmp = PathBuf::from(format!("{}.michi.tmp", f.to_string_lossy()));
        if fs::write(&tmp, &nuevo).is_ok() {
            let _ = fs::rename(&tmp, &f);
        } else {
            out = "MANUAL";
        }
    }
    out.to_string()
}

/// Ejecuta un guion en el servidor con el MISMO python del exportador (ya
/// verificado en el alta) y devuelve su veredicto de una palabra. El guion
/// viaja por STDIN, jamás interpolado en la línea de shell. Lo comparten el
/// wrapper del chat y el alias de ~/.bashrc.
fn remote_verdict_py(host: &str, py: &str, script: &str, op: &str) -> Result<String, String> {
    use std::io::Write;
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"])
        .arg(host)
        .arg(format!("{py} - {op}"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|_| "FAIL".to_string())?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(script.replace("\r\n", "\n").as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| "FAIL".to_string())?;
    if !out.status.success() {
        return Err("FAIL".to_string());
    }
    let word = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if word.is_empty() {
        return Err("FAIL".to_string());
    }
    Ok(word)
}

fn chat_wrap_remote(host: &str, py: &str, op: &str) -> Result<String, String> {
    remote_verdict_py(host, py, CHAT_WRAP_PY, op)
}

/// La pareja WSL de `remote_verdict_py`: el MISMO guion, por STDIN, y el mismo
/// veredicto de una palabra. Cambia el transporte y nada más.
///
/// El `command -v python3` va DENTRO de la misma llamada: arrancar una distro
/// cuesta, y preguntar dos veces por lo mismo la despertaría dos veces. Sin
/// python3 la respuesta es NOPYTHON — un hecho, no un fallo genérico.
///
/// OJO, LO QUE NOS MORDIÓ (2026-08-10): `wsl.exe` NO entrega los argumentos
/// posicionales a `sh -c` como hace `ssh` — `$1` llega VACÍO. Aquí eso dejaba
/// la operación en blanco y el guion la tomaba por "apagar", contestando OK
/// sin hacer nada: el interruptor decía ✓ y no había tocado la distro. Por eso
/// la operación va DENTRO del guion; y como eso es interpolar, se comprueba
/// antes contra la lista cerrada — hoy los tres valores son constantes
/// nuestras, y mañana también tienen que serlo.
#[cfg(windows)]
fn wsl_verdict_py(distro: &str, script: &str, op: &str) -> Result<String, String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    if !["status", "on", "off"].contains(&op) {
        return Err("FAIL".to_string());
    }
    let sh = format!(
        "command -v python3 >/dev/null 2>&1 || {{ echo NOPYTHON; exit 0; }}; python3 - {op}"
    );
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.args(["-d", distro, "--", "sh", "-c", &sh])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .creation_flags(0x0800_0000);
    let mut child = cmd.spawn().map_err(|_| "FAIL".to_string())?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(script.replace("\r\n", "\n").as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| "FAIL".to_string())?;
    if !out.status.success() {
        return Err("FAIL".to_string());
    }
    let word = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if word.is_empty() {
        return Err("FAIL".to_string());
    }
    Ok(word)
}

#[cfg(not(windows))]
fn wsl_verdict_py(_distro: &str, _script: &str, _op: &str) -> Result<String, String> {
    Err("FAIL".to_string())
}

/// Deja un guion en `~/.michiclaude/` de la distro. El nombre va dentro del
/// comando —`wsl.exe` no entrega `$1`, ver `wsl_verdict_py`— y por eso se
/// comprueba antes contra la lista de los DOS archivos que este programa
/// sube. Y los saltos de línea van a LF: dentro de WSL esto es Linux, y un
/// intérprete llamado "python3\r" da un error que no dice nada.
#[cfg(windows)]
fn wsl_upload_script(distro: &str, name: &str, body: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    if ![RELEVO_NAME, WRAP_NAME].contains(&name) {
        return Err("FAIL".to_string());
    }
    let sh = format!(
        "mkdir -p \"$HOME/.michiclaude\" && cat > \"$HOME/.michiclaude/{name}\" \
         && chmod +x \"$HOME/.michiclaude/{name}\""
    );
    let mut cmd = std::process::Command::new("wsl.exe");
    cmd.args(["-d", distro, "--", "sh", "-c", &sh])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .creation_flags(0x0800_0000);
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(body.replace("\r\n", "\n").as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err("FAIL".to_string())
    }
}

#[cfg(not(windows))]
fn wsl_upload_script(_distro: &str, _name: &str, _body: &str) -> Result<(), String> {
    Err("FAIL".to_string())
}

/// Cómo se llama una distro en la lista de interruptores. Con el prefijo
/// delante no se confunde con un servidor SSH aunque compartan nombre.
fn wsl_label(distro: &str) -> String {
    format!("WSL: {distro}")
}

const RELEVO_NAME: &str = "michi-relevo.py";
const WRAP_NAME: &str = "michi-wrap.sh";

/// El python del exportador es el primer token de su comando; si el usuario
/// escribió el suyo a mano y no empieza por un python, se cae a `python3`.
fn remote_python(r: &RemoteSource) -> String {
    let tok = r.command.split_whitespace().next().unwrap_or("");
    if tok.contains("python") {
        tok.to_string()
    } else {
        "python3".to_string()
    }
}

#[tauri::command]
async fn chat_relay_status() -> Result<Vec<ChatRelayRow>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        // esta máquina primero: es la que el usuario tiene delante. Sin VS
        // Code instalado no hay fila — un interruptor que no puede hacer
        // nada no se enseña (invariante #8).
        let mut out = Vec::new();
        let local = local_chat_status();
        if local != "NOVSCODE" {
            out.push(ChatRelayRow { name: String::new(), state: local });
        }
        for r in load_remotes() {
            let state = chat_wrap_remote(&r.host, &remote_python(&r), "status")
                .unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: r.name.clone(), state });
        }
        // WSL detrás de los servidores: mismo guion, mismos veredictos, y el
        // panel no distingue el transporte (el bloque se enseña si HAY filas)
        for d in wsl_distros() {
            let state = wsl_verdict_py(&d, CHAT_WRAP_PY, "status").unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: wsl_label(&d), state });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_chat_relay(on: bool) -> Result<Vec<ChatRelayRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let op = if on { "on" } else { "off" };
        let mut out = Vec::new();
        let local = local_chat_set(on);
        if local != "NOVSCODE" {
            out.push(ChatRelayRow { name: String::new(), state: local });
        }
        for r in load_remotes() {
            if on {
                // el lanzador y el relevo se refrescan ANTES de encender: un
                // ajuste que apunte a un archivo inexistente mata el chat
                let _ = upload_script(&r.host, REMOTE_RELEVO_PATH, REMOTE_RELEVO);
                let _ = upload_script(&r.host, REMOTE_WRAP_PATH, REMOTE_WRAP);
            }
            let state =
                chat_wrap_remote(&r.host, &remote_python(&r), op).unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: r.name.clone(), state });
        }
        for d in wsl_distros() {
            if on {
                // el lanzador y el relevo, frescos ANTES de encender: la misma
                // regla que en SSH (un ajuste que apunte a un archivo que no
                // está mata el chat de esa distro)
                let _ = wsl_upload_script(&d, RELEVO_NAME, REMOTE_RELEVO);
                let _ = wsl_upload_script(&d, WRAP_NAME, REMOTE_WRAP);
            }
            let state = wsl_verdict_py(&d, CHAT_WRAP_PY, op).unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: wsl_label(&d), state });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// El alias de ~/.bashrc en los servidores (etapa 4, el fleco de las
// terminales): que teclear `claude` en una sesión SSH pase por
// michi-relevo.py sin cambiar el hábito. Es la pareja Linux del shim del
// PATH, con las diferencias que impone el terreno: en Linux el enganche es
// una FUNCIÓN de bash en ~/.bashrc (bloque con MARCAS que se reemplaza
// entero al actualizar), no un .cmd en el PATH. Sin bucle posible: las
// funciones de bash NO viajan a subprocesos, así que cuando el relevo lanza
// `claude` por PATH encuentra el binario real.
// ---------------------------------------------------------------------------

/// Guion que corre EN el servidor (por STDIN, como CHAT_WRAP_PY). Mismas
/// reglas de respeto: backup `.michi-backup` una sola vez antes del primer
/// toque, marcas desbalanceadas = MANUAL (alguien editó a mano, no se toca),
/// encender sin el relevo subido = NORELAY. La función es fail-open por
/// construcción: sin TTY, sin script o con MICHI_RELEVO puesto cae al
/// claude real — lo peor permitido es quedarse sin relevo, jamás sin Claude.
// OJO con el delimitador: el guion contiene `"# >>>` (comilla+almohadilla,
// las marcas del bloque) y eso CIERRA un r#"…"# normal — de ahí el ##.
const TERM_ALIAS_PY: &str = r##"
import os, sys

op = sys.argv[1] if len(sys.argv) > 1 else "status"
# Una operacion que no reconozco NO puede caer en la rama de apagar: eso hacia
# que un argumento perdido contestara OK sin tocar nada, y el interruptor
# ensenaba un ✓ que no habia ocurrido (WSL, 2026-08-10). Callar es peor que
# fallar: aqui se falla a la cara.
if op not in ("status", "on", "off"):
    print("BADOP"); sys.exit(0)
RC = os.path.expanduser("~/.bashrc")
RELAY = os.path.expanduser("~/.michiclaude/michi-relevo.py")
A = "# >>> michiclaude relevo >>>"
B = "# <<< michiclaude relevo <<<"
BLOCK = A + """
# Gestionado por MichiClaude: se reemplaza ENTERO al actualizar. No editar.
# Solo terminales interactivas; en scripts o sin relevo, el claude real.
claude() {
  if [ -z "$MICHI_RELEVO" ] && [ -t 0 ] && [ -t 1 ] && [ -f "$HOME/.michiclaude/michi-relevo.py" ] && command -v python3 >/dev/null 2>&1; then
    python3 "$HOME/.michiclaude/michi-relevo.py" claude "$@"
  else
    command claude "$@"
  fi
}
""" + B + "\n"

raw = ""
if os.path.isfile(RC):
    raw = open(RC, encoding="utf-8", errors="replace").read()
a, b = raw.find(A), raw.find(B)
# una marca sin la otra, o el cierre antes que la apertura: lo editaron a
# mano y una cirugia a ciegas podria llevarse texto ajeno
if (a < 0) != (b < 0) or (0 <= b < a):
    print("MANUAL"); sys.exit(0)

if op == "status":
    print("ON" if a >= 0 else "OFF"); sys.exit(0)

if op == "on":
    if not os.path.isfile(RELAY):
        print("NORELAY"); sys.exit(0)
    if a >= 0:
        end = b + len(B)
        if raw[end:end + 1] == "\n":
            end += 1
        new = raw[:a] + BLOCK + raw[end:]
    else:
        sep = "" if not raw else ("\n" if raw.endswith("\n") else "\n\n")
        new = raw + sep + BLOCK
else:
    if a < 0:
        print("OK"); sys.exit(0)
    end = b + len(B)
    if raw[end:end + 1] == "\n":
        end += 1
    head = raw[:a]
    # el renglon en blanco que abrimos al insertar se cierra al quitar
    if head.endswith("\n\n"):
        head = head[:-1]
    new = head + raw[end:]

if new == raw:
    print("OK"); sys.exit(0)
if raw and not os.path.isfile(RC + ".michi-backup"):
    open(RC + ".michi-backup", "w", encoding="utf-8").write(raw)
tmp = RC + ".michi.tmp"
with open(tmp, "w", encoding="utf-8") as f:
    f.write(new)
if os.path.isfile(RC):
    os.chmod(tmp, os.stat(RC).st_mode & 0o7777)
os.replace(tmp, RC)
print("OK")
"##;

#[tauri::command]
async fn term_relay_status() -> Result<Vec<ChatRelayRow>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out = Vec::new();
        for r in load_remotes() {
            let state = remote_verdict_py(&r.host, &remote_python(&r), TERM_ALIAS_PY, "status")
                .unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: r.name.clone(), state });
        }
        for d in wsl_distros() {
            let state = wsl_verdict_py(&d, TERM_ALIAS_PY, "status").unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: wsl_label(&d), state });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_term_relay(on: bool) -> Result<Vec<ChatRelayRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let op = if on { "on" } else { "off" };
        let mut out = Vec::new();
        for r in load_remotes() {
            if on {
                // el relevo se refresca ANTES de encender: con el script
                // ausente la función caería al claude real (fail-open) pero
                // el interruptor diría "encendido" — mentira silenciosa
                let _ = upload_script(&r.host, REMOTE_RELEVO_PATH, REMOTE_RELEVO);
            }
            let state = remote_verdict_py(&r.host, &remote_python(&r), TERM_ALIAS_PY, op)
                .unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: r.name.clone(), state });
        }
        for d in wsl_distros() {
            if on {
                let _ = wsl_upload_script(&d, RELEVO_NAME, REMOTE_RELEVO);
            }
            let state = wsl_verdict_py(&d, TERM_ALIAS_PY, op).unwrap_or_else(|e| e);
            out.push(ChatRelayRow { name: wsl_label(&d), state });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Un proceso MCP huérfano detectado. `start` (epoch de arranque) es la
/// mitad del anti-reciclaje de PID: para cerrar hay que devolverlo y
/// que siga coincidiendo.
#[derive(Serialize, Clone)]
struct Zombie {
    pid: u32,
    /// ejecutable (node.exe, python.exe…)
    name: String,
    /// nombre del servidor MCP configurado que casó
    server: String,
    /// epoch de arranque del proceso
    start: i64,
    /// edad en minutos (para enseñar evidencia)
    mins: i64,
    /// WorkingSetSize en bytes (para enseñar cuánta RAM retiene)
    mem: u64,
}

/// Firmas de los MCP stdio configurados: (nombre, token distintivo en
/// minúsculas). El token es el argumento más largo del comando (el nombre
/// del paquete o la ruta del script) — lo bastante específico para no
/// casar con procesos ajenos; tokens cortos (<5 chars) se descartan.
/// Misma fuente que mcp_unused: ~/.claude.json global + por proyecto.
fn mcp_stdio_signatures() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return out;
    };
    let Ok(raw) = fs::read_to_string(home.join(".claude.json")) else {
        return out;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return out;
    };
    let mut add = |m: &serde_json::Map<String, serde_json::Value>| {
        for (name, def) in m {
            // solo stdio: los http/sse no son procesos hijos
            let Some(cmd) = def["command"].as_str() else {
                continue;
            };
            let mut token = String::new();
            if let Some(args) = def["args"].as_array() {
                for a in args.iter().filter_map(|x| x.as_str()) {
                    // banderas tipo "-y" o "--stdio" no identifican nada
                    if !a.starts_with('-') && a.len() > token.len() {
                        token = a.to_string();
                    }
                }
            }
            if token.len() < 5 {
                // sin argumento distintivo, el propio comando (ruta de un
                // script propio, p. ej.); "npx"/"node" solos no sirven
                if cmd.len() >= 5 && !["npx", "node", "python", "uvx", "uv", "cmd"].contains(&cmd) {
                    token = cmd.to_string();
                } else {
                    continue;
                }
            }
            // Separadores NORMALIZADOS a "/": el config trae el paquete con
            // barra (@modelcontextprotocol/server-x) pero la línea de comando
            // del proceso ya resuelto lleva barra invertida
            // (…\node_modules\@modelcontextprotocol\server-x\dist\index.js).
            // Sin esto ningún MCP lanzado con npx casaría jamás.
            let token = token.to_lowercase().replace('\\', "/");
            if !out.iter().any(|(_, t)| *t == token) {
                out.push((name.clone(), token));
            }
        }
    };
    if let Some(m) = v["mcpServers"].as_object() {
        add(m);
    }
    if let Some(projs) = v["projects"].as_object() {
        for p in projs.values() {
            if let Some(m) = p["mcpServers"].as_object() {
                add(m);
            }
        }
    }
    out
}

/// Foto de procesos vía PowerShell/CIM (pid, ppid, nombre, cmdline,
/// arranque epoch, RAM). Sin dependencias nuevas (invariante #4): en un
/// Windows 11 PowerShell siempre está. ~1 s de CPU, por eso el sondeo
/// del panel es de baja cadencia y todo corre en spawn_blocking (10ter).
#[cfg(windows)]
fn ps_processes() -> Vec<(u32, u32, String, String, i64, u64)> {
    use std::os::windows::process::CommandExt;
    // OutputEncoding a UTF-8: PowerShell 5.1 redirigido emite OEM por
    // defecto y una ruta con acentos rompería el parseo del JSON.
    let script = "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;\
        $e=[datetime]::new(1970,1,1,0,0,0,[datetimekind]::Utc);\
        Get-CimInstance Win32_Process | ForEach-Object { [pscustomobject]@{ \
        p=$_.ProcessId; pp=$_.ParentProcessId; n=$_.Name; c=$_.CommandLine; \
        s=if($_.CreationDate){[int64]($_.CreationDate.ToUniversalTime()-$e).TotalSeconds}else{0}; \
        m=[int64]$_.WorkingSetSize } } | ConvertTo-Json -Compress";
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let Ok(out) = cmd.output() else {
        return Vec::new();
    };
    let txt = String::from_utf8_lossy(&out.stdout);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else {
        return Vec::new();
    };
    // ConvertTo-Json devuelve OBJETO (no lista) si solo hay un elemento
    let items: Vec<&serde_json::Value> = match v.as_array() {
        Some(a) => a.iter().collect(),
        None => vec![&v],
    };
    items
        .into_iter()
        .filter_map(|x| {
            Some((
                x["p"].as_u64()? as u32,
                x["pp"].as_u64().unwrap_or(0) as u32,
                x["n"].as_str().unwrap_or("").to_string(),
                x["c"].as_str().unwrap_or("").to_string(),
                x["s"].as_i64().unwrap_or(0),
                x["m"].as_u64().unwrap_or(0),
            ))
        })
        .collect()
}

/// Zombie = proceso que casa con la firma de un MCP configurado Y cuyo
/// padre ya no existe (o el PID del padre fue reciclado por un proceso
/// MÁS NUEVO que el hijo — un padre no puede nacer después que su hijo).
/// Un MCP con su sesión de Claude Code viva tiene al padre presente y
/// más viejo, así que jamás se marca.
#[cfg(windows)]
fn scan_zombies_impl() -> Vec<Zombie> {
    let sigs = mcp_stdio_signatures();
    if sigs.is_empty() {
        return Vec::new();
    }
    let procs = ps_processes();
    if procs.is_empty() {
        return Vec::new();
    }
    let starts: HashMap<u32, i64> = procs.iter().map(|p| (p.0, p.4)).collect();
    let me = std::process::id();
    let now = Utc::now().timestamp();
    let mut out = Vec::new();
    for (pid, ppid, name, cmdline, start, mem) in &procs {
        if *pid == me || *pid <= 4 || *start == 0 || cmdline.is_empty() {
            continue;
        }
        let low = cmdline.to_lowercase().replace('\\', "/");
        let Some((server, _)) = sigs.iter().find(|(_, tok)| low.contains(tok)) else {
            continue;
        };
        let orphan = match starts.get(ppid) {
            None => true,                  // el padre murió
            Some(ps) => *ps > *start + 2,  // PID de padre reciclado
        };
        if !orphan {
            continue;
        }
        out.push(Zombie {
            pid: *pid,
            name: name.clone(),
            server: server.clone(),
            start: *start,
            mins: (now - start).max(0) / 60,
            mem: *mem,
        });
    }
    // los que más RAM retienen primero; tope 20 (más es una anomalía)
    out.sort_by(|a, b| b.mem.cmp(&a.mem));
    out.truncate(20);
    out
}

/// Sin Win32 no hay procesos que mirar (espejo VPS / futuro port):
/// misma pareja de versiones que wsl_claude_dirs.
#[cfg(not(windows))]
fn scan_zombies_impl() -> Vec<Zombie> {
    Vec::new()
}

#[tauri::command]
async fn scan_zombies() -> Result<Vec<Zombie>, String> {
    tauri::async_runtime::spawn_blocking(|| Ok(scan_zombies_impl()))
        .await
        .map_err(|e| e.to_string())?
}

/// Cierra UN zombie con re-verificación anti-reciclaje: justo antes del
/// kill se consulta ese PID de nuevo y debe seguir con el MISMO ejecutable
/// y la MISMA hora de arranque (±2 s). Si ya no está → "gone" (se cerró
/// solo, no es error); si cambió → ERR_ZOMBIE_CHANGED y no se toca.
#[cfg(windows)]
fn kill_zombie_impl(pid: u32, name: &str, start: i64) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    // SALTOS DE LÍNEA REALES, no una sola línea: PowerShell no acepta el
    // `}` de un bloque seguido de otra sentencia sin separador, así que el
    // script entero moría en el parser, stdout salía vacío y todo cierre
    // acababa en ERR_ZOMBIE_KILL (2026-08-07, cazado validando en vivo:
    // Stop-Process a mano SÍ funcionaba). El escaneo no lo sufría porque
    // es una tubería de una sola sentencia.
    // El veredicto se decide RE-CONSULTANDO el PID, no con $?: con
    // -ErrorAction SilentlyContinue esa variable no distingue "no pude"
    // de "ya no estaba".
    let script = format!(
        "$e=[datetime]::new(1970,1,1,0,0,0,[datetimekind]::Utc)\n\
         $p=Get-CimInstance Win32_Process -Filter 'ProcessId={pid}'\n\
         if(-not $p){{ 'gone'; exit }}\n\
         $s=if($p.CreationDate){{[int64]($p.CreationDate.ToUniversalTime()-$e).TotalSeconds}}else{{0}}\n\
         if($p.Name -ne '{name}' -or [math]::Abs($s-{start}) -gt 2){{ 'changed'; exit }}\n\
         Stop-Process -Id {pid} -Force -ErrorAction SilentlyContinue\n\
         Start-Sleep -Milliseconds 300\n\
         if(Get-Process -Id {pid} -ErrorAction SilentlyContinue){{ 'fail' }} else {{ 'ok' }}",
        pid = pid,
        name = name.replace('\'', ""),
        start = start
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = cmd.output().map_err(|e| e.to_string())?;
    let txt = String::from_utf8_lossy(&out.stdout);
    let verdict = txt.trim();
    match verdict {
        "ok" => Ok("ok".into()),
        "gone" => Ok("gone".into()),
        "changed" => Err("ERR_ZOMBIE_CHANGED".into()),
        // Sin veredicto reconocible la UI solo puede decir "no se pudo"
        // (invariante #10: Rust no redacta textos), así que la foto cruda
        // va a un archivo, como quota_debug/coach_debug. Sin esto, el bug
        // del parser de arriba fue invisible hasta probarlo a mano.
        _ => {
            let err = String::from_utf8_lossy(&out.stderr);
            let _ = fs::write(
                app_data_dir().join("rem_debug.json"),
                serde_json::json!({
                    "ts": Utc::now().timestamp(),
                    "pid": pid,
                    "name": name,
                    "start": start,
                    "stdout": verdict,
                    "stderr": err.trim(),
                })
                .to_string(),
            );
            Err("ERR_ZOMBIE_KILL".into())
        }
    }
}

#[cfg(not(windows))]
fn kill_zombie_impl(_pid: u32, _name: &str, _start: i64) -> Result<String, String> {
    Err("ERR_WIN_ONLY".into())
}

#[tauri::command]
async fn kill_zombie(
    pid: u32,
    name: String,
    start: i64,
    server: String,
    auto: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let r = kill_zombie_impl(pid, &name, start);
        // "gone" no se registra: no lo cerró Michi. "changed" tampoco tocó
        // nada — solo los intentos reales de kill van a la bitácora.
        match &r {
            Ok(v) if v == "ok" => log_action("zombie", auto, true, server, name),
            Err(e) if e == "ERR_ZOMBIE_KILL" => log_action("zombie", auto, false, server, name),
            _ => {}
        }
        r
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Archivos .jsonl con más de 365 días (mtime) de ESTA máquina: el ~/.claude
/// propio Y las distros WSL (2026-08-15: hasta entonces WSL quedaba fuera y
/// una distro acumulaba sin freno; mover a través de \\wsl.localhost es
/// lento pero funciona, y si un archivo falla no detiene a los demás). Las
/// fuentes SSH ni se consideran: en el VPS solo se INFORMA (ver du). 365 y
/// no menos: el analizador, el Reporte y las marcas de arreglo viven de ese
/// historial (cleanupPeriodDays=365).
const ARCHIVE_MIN_DAYS: i64 = 365;

/// Raíces archivables: (raíz de projects, subcarpeta de destino en el
/// archivo). "" = este PC; "wsl-<distro>" = esa distro. Cada raíz va a SU
/// subcarpeta para que dos distros con el mismo nombre de proyecto no se
/// pisen.
fn archive_roots() -> Vec<(PathBuf, String)> {
    let mut out = vec![(claude_dir().join("projects"), String::new())];
    for (distro, d) in wsl_claude_dirs() {
        out.push((d.join("projects"), format!("wsl-{distro}")));
    }
    out
}

/// (archivo, bytes, raíz, subcarpeta de destino)
fn archivable_files() -> Vec<(PathBuf, u64, PathBuf, String)> {
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(ARCHIVE_MIN_DAYS as u64 * 86_400);
    let mut out = Vec::new();
    for (root, sub) in archive_roots() {
        let Ok(pdirs) = fs::read_dir(&root) else { continue };
        for pd in pdirs.flatten() {
            if !pd.path().is_dir() {
                continue;
            }
            for f in project_jsonls(&pd.path()) {
                let Ok(md) = fs::metadata(&f) else { continue };
                let Ok(mt) = md.modified() else { continue };
                if mt < cutoff {
                    out.push((f, md.len(), root.clone(), sub.clone()));
                }
            }
        }
    }
    out
}

#[derive(Serialize, Default)]
struct ArchScan {
    files: u64,
    bytes: u64,
}

#[tauri::command]
async fn scan_archivable() -> Result<ArchScan, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let list = archivable_files();
        Ok(ArchScan {
            files: list.len() as u64,
            bytes: list.iter().map(|(_, b, _, _)| b).sum(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Default)]
struct ArchResult {
    files: u64,
    bytes: u64,
    failed: u64,
    dest: String,
}

/// Mueve los .jsonl ≥365d a %APPDATA%\<app>\archive\<proyecto>\…
/// conservando la estructura (subagents incluido). ARCHIVAR, no borrar:
/// se puede volver a mirar o restaurar a mano. rename primero y si el
/// volumen no lo permite, copiar+borrar. Un archivo que falla no detiene
/// a los demás.
#[tauri::command]
async fn archive_old(auto: bool) -> Result<ArchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dest_root = app_data_dir().join("archive");
        let mut r = ArchResult {
            dest: dest_root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        for (f, bytes, root, sub) in archivable_files() {
            let Ok(rel) = f.strip_prefix(&root) else {
                r.failed += 1;
                continue;
            };
            let dest = if sub.is_empty() { dest_root.join(rel) } else { dest_root.join(&sub).join(rel) };
            let ok = dest
                .parent()
                .map(|p| fs::create_dir_all(p).is_ok())
                .unwrap_or(false)
                && (fs::rename(&f, &dest).is_ok()
                    || (fs::copy(&f, &dest).is_ok() && fs::remove_file(&f).is_ok()));
            if ok {
                // fecha de ARCHIVADO para el doble reloj de la purga: el
                // rename conserva el mtime del contenido, así que va aparte
                let _ = fs::write(
                    dest.with_extension("jsonl.arch"),
                    Utc::now().timestamp().to_string(),
                );
                r.files += 1;
                r.bytes += bytes;
            } else {
                r.failed += 1;
            }
        }
        if r.files > 0 || r.failed > 0 {
            log_action(
                "archive",
                auto,
                r.failed == 0,
                r.files.to_string(),
                format!("{:.1}", r.bytes as f64 / 1_048_576.0),
            );
        }
        Ok(r)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// PURGA DEL ARCHIVO (2026-08-15). El archivador MUEVE los .jsonl ≥365d a
// %APPDATA%\<app>\archive — los saca del camino, pero el disco no baja
// nunca. Esto es el último escalón: borrar de verdad lo que ya está en el
// archivo, con las reglas de seguridad acordadas con Oscar:
//   1. ORDEN SAGRADO consolidar → verificar → borrar: solo se purga lo que
//      NADIE lee — la ventana máxima analizable son 90 días y el cuadernito
//      (daily_history.json) ya tiene los días de esa época. Un archivo del
//      archivo no aporta a ninguna métrica.
//   2. SUELO ABSOLUTO no configurable: PURGE_FLOOR_DAYS en el archivo. Aunque
//      el JSON de config diga "1 día", no baja de ahí ("si el usuario la
//      riega").
//   3. DOBLE RELOJ: edad total (mtime del contenido) ≥ lo elegido Y llevar
//      ≥ PURGE_MIN_ARCHIVED_DAYS ya archivado (mtime de la copia — el
//      rename lo conserva, así que la fecha de archivado se guarda aparte
//      al mover; para archivos anteriores a esta pieza se toma la del
//      directorio de destino). Un archivo recién movido no se purga aunque
//      tenga 3 años.
//   4. ALLOWLIST FÍSICA: el purgador SOLO puede tocar dentro de
//      app_data_dir()/archive, canonicalizado. Estructuralmente no puede
//      entrar a ~/.claude/projects: un bug aquí jamás alcanza un log vivo.
//   5. SIMULACRO SIEMPRE: scan_purgeable devuelve qué/cuánto/de qué fechas
//      antes de que exista un botón de borrar. La confirmación fuerte
//      (escribir una palabra) vive en el panel.
//   6. TOPE POR PASADA: PURGE_MAX_FILES / PURGE_MAX_BYTES; si se toca, se
//      dice (capped) y sigue mañana.
//   7. Solo .jsonl: nunca otro archivo, nunca carpetas ajenas.
// Toda purga se anota en el registro de acciones. No cruza con el detector
// de integridad (ese solo mira los últimos ~32 días; esto, ≥365).
// ---------------------------------------------------------------------------

/// Suelo del doble reloj y de la edad. 180 días = "6 meses en el archivo".
const PURGE_FLOOR_DAYS: i64 = 180;
const PURGE_MIN_ARCHIVED_DAYS: i64 = 30;
const PURGE_MAX_FILES: usize = 500;
const PURGE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB por pasada

#[derive(Serialize, Deserialize, Default, Clone)]
struct PurgeCfg {
    /// días en el ARCHIVO antes de poder borrarse. 0 = nunca (por defecto).
    #[serde(default)]
    after_days: i64,
    /// automático (una pasada al día). Nace apagado.
    #[serde(default)]
    auto: bool,
}

fn purge_cfg_path() -> PathBuf {
    app_data_dir().join("purge_config.json")
}
fn load_purge_cfg() -> PurgeCfg {
    fs::read_to_string(purge_cfg_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
fn get_purge_config() -> PurgeCfg {
    load_purge_cfg()
}

#[tauri::command]
fn save_purge_config(cfg: PurgeCfg) -> Result<(), String> {
    let _ = fs::create_dir_all(app_data_dir());
    let s = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    fs::write(purge_cfg_path(), s).map_err(|e| e.to_string())
}

/// Días efectivos: lo elegido, pero NUNCA por debajo del suelo (regla 2).
/// 0 = purga apagada.
fn purge_effective_days(cfg: &PurgeCfg) -> i64 {
    if cfg.after_days <= 0 {
        0
    } else {
        cfg.after_days.max(PURGE_FLOOR_DAYS)
    }
}

/// Fecha en que un archivo entró al ARCHIVO. `rename` conserva el mtime del
/// contenido, así que se apunta aparte en un sidecar `.arch` al lado (una
/// línea: epoch). Sin sidecar (archivado antes de esta pieza) vale el mtime
/// de la carpeta que lo contiene, que sí cambió al crearse/moverse — y si ni
/// eso, se considera recién archivado (dirección segura: NO purgar).
fn archived_at(f: &std::path::Path) -> Option<i64> {
    let side = f.with_extension("jsonl.arch");
    if let Ok(s) = fs::read_to_string(&side) {
        if let Ok(t) = s.trim().parse::<i64>() {
            return Some(t);
        }
    }
    f.parent()
        .and_then(|p| fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// Raíz canónica del archivo. Todo lo que se purgue DEBE vivir debajo.
fn archive_root_canon() -> Option<PathBuf> {
    fs::canonicalize(app_data_dir().join("archive")).ok()
}

/// (archivo, bytes, mtime del contenido, archivado_en)
fn purgeable_files(days: i64) -> Vec<(PathBuf, u64, i64, i64)> {
    let mut out = Vec::new();
    if days <= 0 {
        return out;
    }
    let Some(root) = archive_root_canon() else { return out };
    let now = Utc::now().timestamp();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue; // regla 7
            }
            // regla 4: canonicalizado y debajo de la raíz, o no existe para
            // nosotros (un symlink que apunte fuera cae aquí)
            let Ok(canon) = fs::canonicalize(&p) else { continue };
            if !canon.starts_with(&root) {
                continue;
            }
            let Ok(md) = fs::metadata(&canon) else { continue };
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(now);
            let Some(arch) = archived_at(&canon) else { continue };
            // regla 3, doble reloj
            let old_enough = now - mtime >= days * 86_400;
            let settled = now - arch >= PURGE_MIN_ARCHIVED_DAYS * 86_400;
            if old_enough && settled {
                out.push((canon, md.len(), mtime, arch));
            }
        }
    }
    out.sort_by_key(|x| x.2); // lo más viejo primero
    out
}

#[derive(Serialize, Default)]
struct PurgeScan {
    files: u64,
    bytes: u64,
    /// mtime más viejo y más nuevo entre los candidatos (epoch)
    oldest: i64,
    newest: i64,
    /// días efectivos (con el suelo aplicado); 0 = apagado
    days: i64,
    floor: i64,
    dest: String,
    /// total del ARCHIVO, purgable o no — para que el usuario sepa cuánto
    /// pesa lo que guarda MichiClaude
    total_files: u64,
    total_bytes: u64,
}

/// El SIMULACRO (regla 5): qué se borraría, cuánto y de qué fechas. Nunca
/// toca nada.
#[tauri::command]
async fn scan_purgeable() -> Result<PurgeScan, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let cfg = load_purge_cfg();
        let days = purge_effective_days(&cfg);
        let list = purgeable_files(days);
        // total del archivo (todo .jsonl debajo de la raíz)
        let (mut tf, mut tb) = (0u64, 0u64);
        if let Some(root) = archive_root_canon() {
            let mut stack = vec![root];
            while let Some(dir) = stack.pop() {
                let Ok(rd) = fs::read_dir(&dir) else { continue };
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                        tf += 1;
                        tb += fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
        Ok(PurgeScan {
            files: list.len() as u64,
            bytes: list.iter().map(|x| x.1).sum(),
            oldest: list.first().map(|x| x.2).unwrap_or(0),
            newest: list.last().map(|x| x.2).unwrap_or(0),
            days,
            floor: PURGE_FLOOR_DAYS,
            dest: app_data_dir().join("archive").to_string_lossy().into_owned(),
            total_files: tf,
            total_bytes: tb,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Default)]
struct PurgeResult {
    files: u64,
    bytes: u64,
    failed: u64,
    /// true si se tocó el tope por pasada (regla 6): quedan más para mañana
    capped: bool,
}

/// BORRA de verdad. Solo lo que purgeable_files devuelve (todas las reglas
/// ya aplicadas), con tope por pasada. Anota en el registro de acciones.
#[tauri::command]
async fn purge_archive(auto: bool) -> Result<PurgeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = load_purge_cfg();
        let days = purge_effective_days(&cfg);
        let mut r = PurgeResult::default();
        if days <= 0 {
            return Ok(r); // apagada: no hay nada que hacer, jamás
        }
        let Some(root) = archive_root_canon() else { return Ok(r) };
        for (f, bytes, _, _) in purgeable_files(days) {
            if r.files as usize >= PURGE_MAX_FILES || r.bytes + bytes > PURGE_MAX_BYTES {
                r.capped = true;
                break;
            }
            // regla 4, otra vez, justo antes de borrar
            if !f.starts_with(&root) || f.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                r.failed += 1;
                continue;
            }
            if fs::remove_file(&f).is_ok() {
                let _ = fs::remove_file(f.with_extension("jsonl.arch"));
                r.files += 1;
                r.bytes += bytes;
            } else {
                r.failed += 1;
            }
        }
        if r.files > 0 || r.failed > 0 {
            log_action(
                "purge",
                auto,
                r.failed == 0,
                r.files.to_string(),
                format!("{:.1}", r.bytes as f64 / 1_048_576.0),
            );
        }
        Ok(r)
    })
    .await
    .map_err(|e| e.to_string())?
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            get_quota,
            get_local_stats,
            get_coach,
            ai_get_config,
            ai_set_config,
            ai_intent,
            ai_setup,
            ai_setup_status,
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
            open_handoff,
            read_handoff,
            read_cleared,
            is_dev,
            app_version,
            get_pill_layer,
            set_pill_layer,
            get_prices_status,
            set_prices_auto,
            refresh_prices_now,
            hover_card,
            set_notif_visible,
            set_tray_menu,
            get_findings,
            log_quota,
            get_quota_history,
            get_integrity,
            get_daily_history,
            pill_moved,
            get_ntfy,
            save_ntfy,
            ntfy_push,
            ntfy_qr,
            ntfy_regen,
            scan_zombies,
            kill_zombie,
            scan_archivable,
            archive_old,
            get_purge_config,
            save_purge_config,
            scan_purgeable,
            purge_archive,
            get_remote_du,
            get_action_log,
            get_relays,
            relay_inject,
            relay_alias_status,
            set_relay_alias,
            chat_relay_status,
            set_chat_relay,
            term_relay_status,
            set_term_relay
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
                // ...y ANTES de colocar nada, la corrección de una vez por la
                // franja de pensamiento del gatito (necesita la ventana ya
                // creada para saber la escala de la pantalla).
                migrate_cat_geometry(app.handle());
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
/// Los 48 px que el gatito creció hacia arriba en la PRIMERA versión de la
/// bombilla (2026-08-11) y que devolvió el mismo día: con la bombilla pequeña
/// y sin globo de pensamiento todo cabe en la ventana de siempre, y esa franja
/// era zona muerta que se tragaba clics. Solo queda aquí para DESHACER el
/// desplazamiento en las configuraciones que alcanzaron a guardarlo
/// (ver migrate_cat_geometry); no la use nadie más.
const CAT_GEOM_V1_TOP: f64 = 48.0;

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
    // versión de la GEOMETRÍA del widget. Sube cuando cambia el tamaño de una
    // ventana del widget y hay que corregir la posición guardada una sola vez
    // (ver migrate_cat_geometry). 0 = config anterior a la franja de
    // pensamiento del gatito.
    #[serde(default)]
    geom: u8,
}

/// Corrige la posición guardada cuando CAMBIA el tamaño de la ventana del
/// gatito. Hace falta porque esa posición es la esquina SUPERIOR izquierda: si
/// la ventana crece o mengua por arriba y nadie toca la `y`, el gato sube o
/// baja solo — y quien lo tenga posado sobre la barra de tareas (la posición
/// por defecto) se lo encuentra medio tapado. Se conserva el borde INFERIOR,
/// que es justo lo que ya hace `set_pill_style` al alternar pastilla ↔ gatito.
///
/// Versiones: 1 = el gatito creció 48 px para la primera bombilla (con globo de
/// pensamiento); 2 = esa franja se devolvió el mismo día. Así que a quien se
/// quedó en la 1 hay que SUMARLE lo que entonces se le restó.
///
/// En píxeles FÍSICOS: la posición lo es y la franja no (de ahí el factor de
/// escala; con pantalla al 150% son 72 px, no 48). Con la pastilla puesta no
/// hay nada que corregir —su tamaño no cambió— y si el usuario se pasa luego
/// al gatito, `set_pill_style` mide las ventanas vivas y ya lo resuelve; por
/// eso ahí solo se marca la versión.
const CAT_GEOM_V: u8 = 2;
fn migrate_cat_geometry(app: &tauri::AppHandle) {
    use tauri::Manager;
    let mut cfg = load_pill_config();
    if cfg.geom >= CAT_GEOM_V {
        return;
    }
    if cfg.style == "cat" {
        // sin ventana no hay escala que consultar: mejor no marcar la versión
        // y reintentar en el próximo arranque que dar la posición por corregida
        let Some(cat) = app.get_webview_window("cat") else { return };
        if let (Some(y), 1) = (cfg.y, cfg.geom) {
            let s = cat.scale_factor().unwrap_or(1.0);
            cfg.y = Some((y + (CAT_GEOM_V1_TOP * s).round() as i32).max(0));
        }
    }
    cfg.geom = CAT_GEOM_V;
    save_pill_config(&cfg);
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

/// Versión de la app, para "Acerca de" y para el reporte de problemas.
/// Sale del Cargo.toml en compilación: escribirla a mano en el frontend
/// sería una segunda verdad que se queda vieja sola.
#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
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

