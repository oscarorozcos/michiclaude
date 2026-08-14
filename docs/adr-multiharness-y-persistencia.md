# ADR externo: multi-harness y persistencia defensiva — con veredicto

> Documento traído por Oscar el 2026-08-14 (propuesta de una sesión de
> estrategia externa, fechada 2026-08-13). Llegó con la codificación
> dañada y se restauró al guardarlo. PRIMERO va el veredicto cruzado
> contra lo que la app ES (mismo patrón que presion-y-rendimiento.md);
> el ADR original, íntegro, después. NADA de esto está decidido ni en
> obra: es análisis.

---

## VEREDICTO MICHICLAUDE (2026-08-14, análisis de Claude con contexto completo)

### Lo primero: este ADR choca con DOS decisiones vigentes de Oscar

CLAUDE.md §Estado: **"NO: rastrear otras herramientas, BD de historial,
modo equipo"**. La Parte 1 es exactamente "rastrear otras herramientas" y
la Parte 2 es exactamente "BD de historial". Oscar puede revertir sus
propios NOs, pero hay que decidirlo con nombre y apellido, no dejar que
un documento externo los revierta en silencio. La amenaza de los
limpiadores de JSONL sí es información NUEVA que justifica reabrir el
segundo NO **en parte** — no entero.

### Señal de cautela: el ADR describe una app que no existe

Menciona "el servidor axum del panel móvil", "modelo open-core", un
"dashboard multi-agente" y llama al producto "Michi Fugas". Nada de eso
existe. Quien lo escribió no conocía la arquitectura real: ignora por
completo el **exportador remoto** (meter-export.py, solo stdlib), el
**hub**, el **relevo** y el invariante #1 (cada métrica = TRES piezas en
sincronía). Ese hueco no es cosmético: cualquier "store" tendría que
existir también en el lado servidor, y el ADR ni se lo pregunta.

### Parte 1 (multi-harness): NO — y no solo por el NO vigente

1. **El foso del producto es Claude-específico.** Cuota real por OAuth,
   coach en vivo, relevo/automáticos, ruteo, gatito con manómetro: ~80%
   del valor no existe para Codex/OpenCode. Lo único portable es el
   medidor — y §9 del propio análisis de mercado (analizador-fugas.md)
   dice que esa capa está SATURADA y gratis: ccusage ya cubre Codex,
   OpenCode, Gemini CLI y Copilot CLI. Multi-harness nos mete al
   segmento regalado con nuestra feature más débil.
2. **El problema que dice resolver no lo tenemos.** No hay
   `if harness == "claude"` regado: TODO el código es Claude-específico
   a propósito. Y WSL ya está unificado sin trait (`wsl_claude_dirs()`,
   sufijo `wsl-<distro>`) — el caso que el ADR usa de motivación ya está
   resuelto en concreto.
3. **El costo real es ×3.** Invariante #1: el trait habría que
   replicarlo en el exportador Python (stdlib, sin SQLite) y mantener la
   sincronía por harness. El ADR estima el costo de UNA pieza.

Veredicto: el NO de Oscar se queda. Revisitar solo si cambia la
estrategia de producto (y entonces con este ADR como punto de partida —
el trait está bien pensado para ese futuro).

### Parte 2 (persistencia defensiva): el PROBLEMA es real; la solución, sobredimensionada

El diagnóstico es correcto y va directo a nuestra yugular: **la
confianza**. Si un limpiador estilo conversation-reclaim recorta los
JSONL, hoy MichiClaude enseñaría "bajó 40% el consumo" sin saber que fue
un borrado — exactamente la mentira que el invariante #8 prohíbe. Eso
hay que atajarlo. Pero la solución propuesta (SQLite + WAL + backups +
store de eventos + migraciones) ignora tres cosas de esta casa:

- **Invariante #4** (deps mínimas): rusqlite + WAL + backups + versionado
  de esquema es una superficie operativa enorme para un widget de bandeja
  que ya pelea por sus 276 MB.
- **Recalcular desde el crudo es una FEATURE, no una debilidad.** Hoy
  mismo (2026-08-14) el fix de uturns corrigió 30 días de historia
  retroactivamente porque los logs crudos seguían ahí (caché v3). Con
  rollups congelados, ese bug habría quedado FOSILIZADO en la historia
  (o pediría reconstruir rollups… desde logs que quizá ya no existen).
  Un store congelado protege contra borrados Y congela errores; el ADR
  no considera ese trade-off ni propone versionado de rollups.
- **Ya existen respuestas parciales de primera mano** que el ADR no vio:
  `cleanupPeriodDays: 365` (la retención de Claude Code ya está domada),
  el **archivador propio** (JSONL ≥365d se copian a
  `%APPDATA%\<app>\archive` — territorio de la app, fuera del alcance de
  limpiadores), `quota_history.json` (precedente de "historia chiquita
  en JSON local"), `scan_cache.json` (que YA guarda tamaño+mtime por
  archivo: la mitad del detector de truncado está construida) y las
  marcas de arreglo (`fndHist`/`fndMarks`).

### Lo que SÍ tomaría del ADR (versión ligera, alineada con la casa)

En orden de valor, las cuatro piezas baratas que capturan ~90% del
beneficio sin SQLite:

1. **Detección de recorte externo** (la joya del ADR, §2.5 casos C/D/E).
   `scan_cache.json` ya registra tamaño+mtime por archivo: detectar
   `tamaño < cacheado` (encogió), huella de inicio distinta (reescrito) o
   archivo desaparecido es casi gratis. Al detectarlo: anotarlo (bitácora
   de integridad chiquita, JSON), avisar UNA vez ("un limpiador recortó
   tus logs el día X"), y…
2. **"No concluyente" en las comparaciones** (§2.6, la parte más nuestra
   del ADR): si el Reporte compara dos periodos y uno tiene un recorte
   detectado dentro, el delta sale marcado "no comparable — hubo un
   recorte de logs el día X", nunca como mejora. Es invariante #8 puro.
3. **Persistir la serie diaria** en el patrón `quota_history.json`: un
   `daily_history.json` local (día × proyecto × modelo × origen,
   append-only, poda configurable, unos KB). Con eso las gráficas y el
   antes/después sobreviven a cualquier recorte del crudo, sin BD. OJO
   invariante #1: definir qué pasa con los orígenes remotos (el snapshot
   diario se toma tras la fusión, etiquetado por origen — el exportador
   NO necesita store propio).
4. **Congelar el "antes" de las marcas de arreglo** (§2.9 aplicado a
   nuestra feature real): cuando `fndMarks` clava una marca, guardar en
   la marca el snapshot de métricas del periodo previo (tokens/turno,
   costo). La comparación futura es `congelado vs. actual` y no depende
   de que los logs viejos sigan vivos.

Y un quinto gratis: **el archivador existente ya es la respuesta al
detalle crudo** — si algún día se quiere "consolidar antes de limpiar"
(§2.8), la pieza es extender el archivador, no crear un store paralelo.

### Lo que NO tomaría

- SQLite/WAL/backups/migraciones (invariante #4; dos fuentes de verdad).
- Store de eventos detallado (el crudo + archivador ya lo son).
- Detección específica de conversation-reclaim por manifest (sobra: la
  detección pasiva cubre a cualquier limpiador, presente o futuro).
- `event_uid` por hash de contenido, coverage por bucket, baselines
  genéricas: sobreingeniería para el tamaño real del problema aquí.

### Si Oscar da luz verde a la versión ligera

Orden sugerido: (1) detección de recorte + aviso + "no concluyente" —
es la defensa de la confianza y la más barata; (2) `daily_history.json`;
(3) snapshot congelado en las marcas. Cada una es un incremento chico,
con su regresión, sin tocar el resto. La Parte 1 se queda como documento
de referencia por si el NO estratégico algún día cambia.

---
---

# [ORIGINAL] MichiClaude — ADR: Arquitectura multi-harness y persistencia defensiva

**Estado:** Propuesta
**Fecha:** 2026-08-13
**Ámbito:** núcleo de ingesta (Rust), modelo de datos, UI de integridad
**Motivación externa:** la aparición de limpiadores de disco para agentes de IA
(p. ej. `conversation-reclaim`) demuestra que los JSONL de Claude Code **no son
un almacén estable**: son archivos temporales que terceros —y el propio
usuario— van a recortar, rotar o borrar.

---

## Resumen ejecutivo

Dos decisiones que se refuerzan entre sí:

1. **Multi-harness.** MichiClaude deja de estar acoplado a Claude Code.
   Se introduce un trait `Harness` con un registro descubrible; toda la app
   consume un **modelo canónico de eventos**, no JSONL crudo.

2. **Persistencia defensiva.** MichiClaude deja de tratar los JSONL como su
   base de datos. Pasa a mantener un **store propio, fuera del territorio de
   los agentes**, con ingesta idempotente, rollups permanentes, ventanas de
   cobertura explícitas y detección de intervención externa.

La segunda es la urgente. La primera es la que la vuelve barata de implementar
(el modelo canónico es el mismo prerrequisito para ambas).

---

# Parte 1 — Arquitectura multi-harness

## 1.1 Problema

Hoy la lectura de JSONL de Claude Code está entretejida con la lógica de
métricas, detección de fugas y UI. Agregar Codex, OpenCode o Antigravity por la
vía rápida significa `if harness == "claude"` regado por todo el código, y cada
formato nuevo rompe supuestos del anterior (Claude reporta cache creation/read,
OpenCode guarda en SQLite, Antigravity usa `step_type` numérico).

Además hay un caso que ya te pega hoy y que **no es un harness nuevo**: WSL2.
Es Claude Code con otras rutas. Si no existe la abstracción, WSL termina como
un parche condicional.

## 1.2 Decisión

Un trait `Harness` + registro. Cada integración es **un módulo, un archivo,
una responsabilidad**: sabe dónde vive su data, cómo enumerarla, cómo
convertirla al canónico y qué es capaz de reportar.

```rust
// src-tauri/src/harness/mod.rs

/// Identificador estable. Se persiste en la BD: NO cambiar una vez publicado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HarnessId(pub &'static str); // "claude-code", "codex", "opencode"

#[derive(Debug, Clone)]
pub struct HarnessMeta {
    pub id: HarnessId,
    pub display_name: &'static str,
    pub icon: IconRef,          // SVG inline; fallback neutro si no hay marca
    pub maturity: Maturity,     // Stable | Beta | Experimental
    pub docs_url: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Capabilities {
    pub token_counts: bool,        // ¿reporta tokens por mensaje?
    pub cache_breakdown: bool,     // ¿distingue cache_read / cache_creation?
    pub model_identity: bool,      // ¿sabemos qué modelo respondió?
    pub cost_attribution: bool,    // ¿podemos calcular costo confiable?
    pub compaction_marker: bool,   // ¿hay marcador de compactación detectable?
    pub subagents: bool,           // ¿existen sidechains/subagentes?
    pub live_tail: bool,           // ¿se puede seguir en tiempo real?
}

pub trait Harness: Send + Sync {
    fn meta(&self) -> &HarnessMeta;
    fn capabilities(&self) -> Capabilities;

    /// Raíces permitidas. Allowlist estricta: el motor JAMÁS lee fuera de aquí.
    fn allowed_roots(&self) -> Vec<PathBuf>;

    /// ¿Está instalado en esta máquina? Barato, sin I/O pesado.
    fn detect(&self) -> DetectionResult;

    /// Enumera las fuentes (archivos, tablas) candidatas a ingesta.
    fn enumerate_sources(&self) -> Result<Vec<SourceRef>, HarnessError>;

    /// Lee una fuente desde un offset y emite eventos canónicos.
    /// DEBE ser incremental y no bloquear si el agente tiene el archivo abierto.
    fn read_incremental(
        &self,
        source: &SourceRef,
        cursor: &SourceCursor,
    ) -> Result<IngestBatch, HarnessError>;
}
```

### Modelo canónico

Es el contrato. Todo lo de arriba (dashboard, Michi Fugas, panel móvil,
notificaciones ntfy) trabaja **solo** contra esto:

```rust
pub struct UsageEvent {
    pub harness: HarnessId,
    pub event_uid: String,      // UUID del mensaje / request_id. Clave de dedup.
    pub session_uid: String,
    pub project_key: Option<String>,  // ruta del proyecto, normalizada
    pub ts: DateTime<Utc>,
    pub role: Role,             // User | Assistant | System | Tool
    pub model: Option<String>,
    pub tokens: TokenBreakdown, // input, output, cache_read, cache_creation
    pub is_sidechain: bool,
    pub is_compaction_marker: bool,
}
```

**Regla dura:** `event_uid` debe ser estable entre lecturas. Si un harness no
expone un identificador propio, se deriva de forma determinista
(`blake3(source_id + byte_offset + ts + len)`) — nunca de un contador de
posición, porque un archivo recortado desplaza todas las posiciones.

### Degradación honesta por capacidades

Si `cache_breakdown == false`, la UI **no muestra un 0**: muestra "no
disponible en este agente". Un cero inventado es peor que un hueco declarado.
Este principio se repite en la Parte 2 y es el hilo conductor del diseño.

### Registro

```rust
pub fn registry() -> &'static [&'static dyn Harness] {
    &[&ClaudeCode, &ClaudeCodeWsl, &Codex /* , ... */]
}
```

Registro explícito, no macro-magia (`inventory`/`linkme`). Con 3–8 harnesses el
costo de mantener una lista es cero y el beneficio de que sea grepeable es alto.

## 1.3 WSL2 como variante, no como harness

`ClaudeCodeWsl` implementa el mismo trait pero:
- `allowed_roots()` resuelve `\\wsl$\<distro>\home\<user>\.claude\projects`
  enumerando distros vía `wsl.exe -l -q`.
- `HarnessId` es **el mismo** (`"claude-code"`), con un discriminante de
  `origin` en `SourceRef`. Así el usuario ve un solo Claude Code en la UI,
  con sus sesiones de Windows y de WSL unificadas, que es lo que espera.
- `live_tail` puede ser `false`: el sistema de archivos de WSL a través del
  plan 9 no siempre entrega eventos de `notify` confiables — fallback a polling
  con intervalo mayor.

## 1.4 Seguridad y límites

- **Solo lectura.** El motor de ingesta abre archivos en modo lectura. Punto.
  Ninguna operación de escritura o borrado vive en el mismo módulo.
- **Allowlist de rutas.** Cualquier `SourceRef` se valida contra
  `allowed_roots()` con canonicalización previa (defensa contra symlinks y
  `..`). Un harness comunitario mal escrito no debe poder hacer que Michi lea
  `~/.ssh`.
- **Nunca bloquear al agente.** Si el archivo está en uso, se salta y se
  reintenta. En Windows, apertura con `FILE_SHARE_READ | FILE_SHARE_WRITE`.
- **Sin contenido, solo métricas.** Ver §2.7.

## 1.5 Plan de adopción

| Fase | Alcance | Criterio de salida |
|---|---|---|
| 0 | Refactor: Claude Code como primer implementador del trait. Cero features nuevas. | Suite de tests pasa idéntica; el diff no toca UI. |
| 1 | `ClaudeCodeWsl` como variante. | Sesiones de WSL aparecen unificadas. |
| 2 | Store canónico (Parte 2) montado sobre el trait. | Rollups sobreviven a un borrado del JSONL. |
| 3 | Segundo harness real (Codex es el más parecido: JSONL + marcador estructurado). | Dashboard multi-agente sin `if` por harness. |
| 4 | Guía de contribución + badge `Experimental`. | Un tercero abre PR sin tocar el core. |

No prometer paridad de features entre harnesses. `Maturity` + `Capabilities`
son la manera honesta de decir "esto todavía no".

---

# Parte 2 — Persistencia defensiva del historial de uso

## 2.1 El problema, concreto

MichiClaude mide, verifica y compara. Las tres cosas dependen de tener
historia. Hoy esa historia vive en archivos que **no son de MichiClaude**:

- `conversation-reclaim` recorta todo lo anterior al último marcador de
  compactación. Una conversación de 776 MB queda en 2 MB. La documentación del
  propio proyecto ya advierte que herramientas de reportes históricos
  (menciona `ccusage`) quedan con totales bajos o incompletos.
- El usuario borra `~/.claude/projects/<proyecto>` cuando archiva un cliente.
- Claude Code puede rotar, migrar o cambiar de formato en cualquier release.
- Instalación nueva / máquina nueva / reinstalación → cero historia.

**El daño real no es perder bytes: es que Michi mienta.** Si una gráfica
muestra "bajó 40% el consumo" cuando en realidad se borraron los datos viejos,
Michi Fugas queda inservible y el usuario pierde la confianza que es todo el
producto.

## 2.2 Principio rector

> Los JSONL son un **flujo de eventos efímero**, no una base de datos.
> MichiClaude deriva de ese flujo su propio registro, y ese registro es el
> único origen de verdad para medir, verificar y comparar.

## 2.3 Dónde vive el store (crítico)

```
Windows: %APPDATA%\MichiClaude\
macOS:   ~/Library/Application Support/MichiClaude/
Linux:   ~/.local/share/michiclaude/
```

**Fuera de `~/.claude`, fuera de `~/.codex`, fuera de cualquier ruta de
agente.** Si el store vive dentro del territorio de un agente, el próximo
limpiador —o el propio agente— se lo lleva de corbata. Esto no es paranoia:
es exactamente el modo de falla que estamos evitando.

Archivos:
```
MichiClaude/
├── usage.db                 # SQLite, WAL
├── usage.db-wal
├── backups/
│   ├── usage-2026-08-01.db.zst
│   └── usage-2026-07-01.db.zst   # rotación mensual, 6 meses
└── exports/
```

## 2.4 Esquema

```sql
-- Detalle. Retención configurable (default 90 días).
CREATE TABLE events (
  harness      TEXT NOT NULL,
  event_uid    TEXT NOT NULL,
  session_uid  TEXT NOT NULL,
  project_key  TEXT,
  ts           INTEGER NOT NULL,      -- epoch ms UTC
  role         TEXT NOT NULL,
  model        TEXT,
  tok_in       INTEGER NOT NULL DEFAULT 0,
  tok_out      INTEGER NOT NULL DEFAULT 0,
  tok_cache_r  INTEGER NOT NULL DEFAULT 0,
  tok_cache_w  INTEGER NOT NULL DEFAULT 0,
  is_sidechain INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (harness, event_uid)     -- dedup: reingesta = INSERT OR IGNORE
) WITHOUT ROWID;

-- Agregados. Retención INFINITA. Son diminutos.
CREATE TABLE rollup_hourly (
  harness TEXT, project_key TEXT, model TEXT, bucket INTEGER,
  tok_in INTEGER, tok_out INTEGER, tok_cache_r INTEGER, tok_cache_w INTEGER,
  events INTEGER, sessions INTEGER,
  PRIMARY KEY (harness, project_key, model, bucket)
) WITHOUT ROWID;

CREATE TABLE rollup_daily (/* idem, bucket = día UTC + día local */);

-- Cursores de ingesta. El corazón de la detección de truncado.
CREATE TABLE source_cursor (
  harness      TEXT NOT NULL,
  source_id    TEXT NOT NULL,     -- ruta canónica (+ distro si WSL)
  file_key     TEXT,              -- inode (unix) / FileId (win). Detecta reemplazo.
  last_size    INTEGER NOT NULL,
  last_mtime   INTEGER NOT NULL,
  last_offset  INTEGER NOT NULL,
  prefix_hash  TEXT NOT NULL,     -- blake3 de los primeros 64 KiB
  updated_at   INTEGER NOT NULL,
  PRIMARY KEY (harness, source_id)
) WITHOUT ROWID;

-- Ventanas de observación. Sin esto, Michi no puede distinguir
-- "trabajaste 0" de "no había datos".
CREATE TABLE coverage (
  harness TEXT, source_id TEXT,
  from_ts INTEGER, to_ts INTEGER,
  kind    TEXT,   -- observed | gap_app_off | gap_truncated | gap_deleted | gap_unknown
  PRIMARY KEY (harness, source_id, from_ts)
);

-- Bitácora de integridad. Alimenta el banner de la UI.
CREATE TABLE integrity_log (
  id INTEGER PRIMARY KEY,
  ts INTEGER NOT NULL,
  kind TEXT NOT NULL,       -- truncation | file_replaced | source_vanished | external_tool
  harness TEXT, source_id TEXT,
  detail TEXT,              -- JSON: bytes perdidos, herramienta detectada, etc.
  acknowledged INTEGER NOT NULL DEFAULT 0
);

-- Líneas base inmutables para Michi Fugas.
CREATE TABLE baselines (
  id TEXT PRIMARY KEY,
  created_at INTEGER NOT NULL,
  label TEXT,
  scope TEXT NOT NULL,      -- JSON: harness, project, rango
  metrics TEXT NOT NULL,    -- JSON: snapshot congelado de los números
  source_coverage TEXT NOT NULL  -- JSON: cobertura al momento de medir
);
```

## 2.5 Ingesta idempotente y detección de truncado

Este es el algoritmo que resuelve el caso `conversation-reclaim`:

```
para cada source:
    leer cursor previo (file_key, last_size, last_offset, prefix_hash)
    stat actual → size, mtime, file_key_actual
    hash_actual = blake3(primeros 64 KiB)

    CASO A — file_key cambió o no había cursor:
        → archivo nuevo o reemplazado. Ingerir desde 0.
        Si había cursor previo: registrar integrity_log(file_replaced)
          y coverage(gap_truncated) desde el último evento conocido.

    CASO B — size >= last_offset  Y  hash_actual == prefix_hash:
        → append normal. Leer desde last_offset. Camino feliz.

    CASO C — size < last_offset:
        → EL ARCHIVO SE ENCOGIÓ. Recorte externo.
        NO reingerir desde 0 como si fuera nuevo (duplicaría con distinto
        offset si el uid dependiera de posición — por eso el uid es de contenido).
        Reingerir desde 0 con INSERT OR IGNORE: lo que ya teníamos se conserva,
        lo que sobrevive se reconoce, nada se duplica.
        Registrar integrity_log(truncation, bytes_perdidos = last_offset - size).

    CASO D — hash_actual != prefix_hash con size >= last_offset:
        → reescritura desde el inicio (exactamente lo que hace un recorte
        atómico: temp file + rename, conservando el nombre).
        Mismo tratamiento que CASO C.

    CASO E — source desapareció:
        → integrity_log(source_vanished). Los eventos ya ingeridos SE QUEDAN.
        coverage: cerrar ventana observed.
```

**La propiedad clave:** como `event_uid` se deriva del contenido y no de la
posición, y como los rollups ya están calculados, **un recorte externo no
destruye nada que Michi ya haya visto**. Michi se entera, lo anota, y sigue.

### Catch-up al arrancar

MichiClaude no vive 24/7 (PC apagada). Al arrancar: escaneo completo de
cursores antes de habilitar el watcher, y `coverage(gap_app_off)` para el
periodo entre `updated_at` más reciente y el arranque. Ese hueco es honesto y
distinto de un hueco por borrado.

## 2.6 Cobertura: no inventar ceros

Toda consulta de series temporales devuelve, junto a los valores, el estado de
cobertura del bucket. La UI lo pinta distinto:

- `observed` → barra normal.
- `gap_app_off` → banda gris tenue, tooltip "Michi no estaba corriendo".
- `gap_truncated` / `gap_deleted` → banda rayada, tooltip con la fecha y la
  causa detectada.

Y en cualquier comparación "antes vs. después" (el corazón de Michi Fugas),
si alguno de los dos periodos tiene cobertura incompleta, **el resultado sale
marcado como no concluyente**, no como una mejora. Un limpiador de disco puede
hacer que tu consumo "baje" 90% sin que hayas cambiado nada; Michi no puede
caer en eso.

## 2.7 Qué se guarda y qué no

El store guarda **métricas, no conversaciones**: ids, timestamps, conteos de
tokens, modelo, ruta de proyecto. Sin prompts, sin respuestas, sin nombres de
archivo del código del usuario.

Razones: (a) es el argumento de confianza del modelo open-core — el `.db` es
auditable y aburrido; (b) el tamaño se mantiene en decenas de MB por años;
(c) si algún día hay respaldo en la nube o panel móvil expuesto, la superficie
de riesgo es mínima. Si más adelante hiciera falta un preview de sesión,
que sea **opt-in explícito** y en tabla separada con su propia retención.

## 2.8 Convivencia con limpiadores externos

Postura: **no pelear con ellos, integrarse.**

1. **Detección pasiva.** El algoritmo de §2.5 detecta cualquier recorte, venga
   de donde venga. Es la defensa que no depende de conocer al tercero.
2. **Detección específica (opcional).** Si existe
   `~/.conversation-reclaim/manifest-*.jsonl`, leerlo para atribuir el evento
   con nombre y fecha exacta: *"Conversation Reclaim recortó 774 MB el 12 ago;
   tus métricas anteriores a esa fecha vienen de los rollups de Michi."*
   Esto convierte un bug reportado en un momento de confianza.
3. **Recomendación proactiva.** Si Michi detecta un limpiador instalado, sugiere
   una vez: *"Antes de limpiar, deja que Michi consolide. Toma 5 segundos."*
   Un solo aviso, no un nag.
4. **Orden correcto si Michi algún día limpia.** Consolidar → verificar
   rollups → limpiar. Nunca al revés.

## 2.9 Baselines inmutables

Para "verificar que la fuga se arregló", Michi **congela** el snapshot de
métricas al momento de crear la línea base (`baselines.metrics`), junto con su
cobertura. No se recalcula bajo demanda desde los JSONL, porque esos JSONL
pueden no existir mañana. Una comparación siempre es
`baseline_congelado vs. ventana_actual`, con ambas coberturas visibles.

## 2.10 Operación

- **WAL activado.** Un solo escritor: el proceso del tray. El servidor axum del
  panel móvil abre la BD **read-only** (`mode=ro`).
- **Respaldo automático.** Copia comprimida mensual a `backups/`, rotación a 6.
  Costo: irrelevante. Beneficio: sobrevive corrupción y borrado accidental.
- **Export.** JSON/CSV de rollups desde la UI. Es también la ruta de migración
  a máquina nueva y la respuesta a "¿y si abandono MichiClaude?".
- **Retención.** Detalle: 90 días por default, configurable, con purga que
  **primero** consolida a rollup. Rollups: nunca se purgan.
- **Integridad.** `PRAGMA integrity_check` al arrancar tras un cierre sucio;
  si falla, restaurar del backup más reciente y avisar.

---

## Consecuencias

**A favor**
- Michi mide bien aunque el resto del mundo borre archivos.
- Los rollups eternos permiten comparaciones a 1 año con un `.db` de pocos MB.
- El trait `Harness` abre Codex/OpenCode sin reescribir el core.
- La honestidad de cobertura es un diferenciador defendible: es exactamente lo
  que un "plomero" debe hacer — no decir que la fuga se arregló si no puede
  probarlo.

**En contra**
- Trabajo de refactor antes de features visibles (Fases 0–2).
- Un `.db` propio es superficie nueva: corrupción, migraciones de esquema,
  soporte. Mitigado con backups + versionado de esquema desde el día 1.
- Riesgo de que la UI se vuelva ruidosa con avisos de integridad. Regla: un
  banner agregado y descartable, nunca un modal.

## Pendientes de decidir

- ¿`event_uid` derivado incluye el modelo? (afecta dedup si Claude reescribe
  metadata en un mismo mensaje).
- Versionado de esquema: `user_version` + migraciones idempotentes.
- ¿La cobertura se calcula por `source_id` o por sesión? Por source es más
  barato; por sesión es más preciso en la UI de proyecto.
- Zona horaria de los rollups diarios: guardar ambos buckets (UTC y local) para
  no recalcular al cambiar de huso.
