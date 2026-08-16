#!/usr/bin/env python3
"""Exportador remoto para MichiClaude.

Agrega los logs locales de Claude Code (~/.claude/projects/**/*.jsonl) con la
MISMA lógica que collect_local_stats en Rust (deduplicación por
message.id+requestId, exclusión de <synthetic>, cache_read fuera de los tokens
de trabajo, precios equivalente-API) y emite el JSON con la forma exacta de
LocalStats: projects (con by_model), models, cost_today, cost_week (= ventana),
tokens_week, daily (serie de 30 días).

Uso:  meter-export.py [--days N] [--end EPOCH] [--exclude-host ID]
      --days N          ventana del gasto por proyecto (def. 7)
      --end EPOCH       final de la ventana (def. ahora). Con él la ventana
                        se desplaza al pasado y cubre [end - days, end]:
                        así se pide un rango de fechas concreto.
      --exclude-host ID no devolver el resumen de esa máquina (modo hub)
      --coach           SOLO consejos de sesiones vivas (atajo del sondeo
                        del panel; réplica del coach de lib.rs)

El meter en Windows lo invoca por SSH. Solo stdlib; sin dependencias.
"""
import json
import os
import re
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path


# Precios frescos que manda MichiClaude por stdin (--prices-stdin): una sola
# fuente de verdad. Si no llegan, se usa la tabla embebida de abajo.
PRICES = {}


def price_key(model):
    """Clave normalizada, igual que price_key() en Rust: minúsculas, sin
    prefijo de proveedor, sin variante [1m], con la versión en GUIONES y sin la
    fecha del snapshot.

    Lo del punto: OpenRouter escribe "claude-opus-4.8" donde LiteLLM,
    models.dev y los logs usan "claude-opus-4-8". Sin unificarlo, la tercera
    fuente de la cascada casaba 6 modelos de 14 y ocho vigentes se quedaban sin
    precio y sin techo en silencio. Solo entre dígitos, para no tocar
    "anthropic.claude-opus-5"."""
    s = model.lower().rsplit("/", 1)[-1].split("[", 1)[0].strip()
    s = re.sub(r"(?<=\d)\.(?=\d)", "-", s)
    head, sep, tail = s.rpartition("-")
    if sep and len(tail) == 8 and tail.isdigit():
        s = head
    return s


def price_for(model):
    """(input, output, cache_write, cache_read) USD por MTok.

    Primero los precios descargados que llegan por stdin; si no hay, la tabla
    embebida. La tarifa depende de la VERSIÓN, no solo de la familia: Opus bajó
    de $15/$75 a $5/$25 a partir de la 4.5 (3, 4.0 y 4.1 siguen en la vieja).
    Escritura de caché = 1.25x input, lectura = 0.1x input.
    MANTENER EN SINCRONÍA con price_for() de src-tauri/src/lib.rs.
    """
    p = PRICES.get(price_key(model))
    if p:
        return (p["input"], p["output"], p["cache_write"], p["cache_read"])
    m = model.lower()
    # versión del id, ignorando la fecha del snapshot (8 dígitos)
    nums = [int(t) for t in re.findall(r"\d+", m) if len(t) != 8]
    major = nums[0] if nums else 0
    minor = nums[1] if len(nums) > 1 else 0

    if "fable" in m or "mythos" in m:
        inp, out = 10.0, 50.0
    elif "opus" in m:
        if major > 4 or (major == 4 and minor >= 5):
            inp, out = 5.0, 25.0
        else:
            inp, out = 15.0, 75.0  # Opus 3 / 4.0 / 4.1
    elif "haiku" in m:
        inp, out = 1.0, 5.0
    else:
        inp, out = 3.0, 15.0  # sonnet y desconocidos
    return (inp, out, inp * 1.25, inp * 0.1)


# Techo de contexto: 200k era el de TODOS los modelos hasta Opus/Sonnet 4.6,
# que saltaron a 1M. Es el denominador del manómetro de presión.
CTX_FALLBACK = 200_000
CTX_1M = 1_000_000


def ctx_for(model):
    """Techo de contexto del modelo en tokens. Primero la tabla descargada que
    llega por stdin (trae `ctx` junto a los precios), luego el respaldo por
    VERSIÓN — nunca por lista de modelos.

    En la duda devuelve 200k A PROPÓSITO: quedarse corto hace que el manómetro
    avise antes de tiempo; pasarse haría que no avisara nunca.
    MANTENER EN SINCRONÍA con ctx_for() de src-tauri/src/lib.rs."""
    m = (model or "").lower()
    # la variante de contexto largo va en el id y price_key() la recorta
    if "[1m]" in m:
        return CTX_1M
    p = PRICES.get(price_key(m))
    if p and (p.get("ctx") or 0) > 0:
        return int(p["ctx"])
    nums = [int(t) for t in re.findall(r"\d+", m) if len(t) != 8]
    major = nums[0] if nums else 0
    minor = nums[1] if len(nums) > 1 else 0
    if "fable" in m or "mythos" in m:
        return CTX_1M
    if "haiku" in m:
        return CTX_FALLBACK
    if ("opus" in m or "sonnet" in m) and (major > 4 or (major == 4 and minor >= 6)):
        return CTX_1M
    return CTX_FALLBACK


# Escalones de ventana que existen de verdad. NO es una lista de modelos
# (invariante #6): son magnitudes, y solo se usan cuando lo MEDIDO contradice
# a la tabla.
CTX_LADDER = (200_000, 1_000_000, 2_000_000, 5_000_000)


def ctx_full(model, seen):
    """Techo corregido con la evidencia de esta máquina. Si una sesión ya
    superó lo que dice la tabla, la tabla está mal y mandan los tokens
    medidos; se sube al primer escalón por encima de lo visto (devolver lo
    visto a secas dejaría el manómetro clavado en 100%).
    MANTENER EN SINCRONÍA con ctx_full() de src-tauri/src/lib.rs."""
    base = ctx_for(model)
    if seen <= base:
        return base
    for s in CTX_LADDER:
        if s > seen:
            return s
    return seen


def parse_ts(s):
    if not isinstance(s, str):
        return None
    try:
        ts = datetime.fromisoformat(s.replace("Z", "+00:00"))
    except ValueError:
        return None
    if ts.tzinfo is None:
        ts = ts.replace(tzinfo=timezone.utc)
    return ts


# ---------- caché de escaneo (lectura incremental) ----------
# El escaneo completo relee TODOS los .jsonl en cada ciclo, y el historial solo
# crece. Dos ideas, ambas sin cambiar un solo número:
#   1) Un archivo cuya última escritura sea anterior a la ventana más amplia
#      (30 días de la tendencia) no puede contener nada dentro de ninguna
#      ventana: se salta entero.
#   2) De los recientes se cachea el resultado del PARSEO (no los totales)
#      indexado por tamaño+mtime. Si el archivo no cambió, se reutiliza.
# Se cachean tokens y timestamp, nunca el coste: así un cambio de precios se
# aplica a todo el historial al instante. El caché es reconstruible: si se
# borra o no se entiende, se recalcula desde los logs.
# v2 (2026-08-06): el caché guarda también los turnos de usuario (uturns);
# un caché v1 devolvería 0 en silencio para archivos sin cambios.
CACHE_VERSION = 3   # v3: user_turn_text excluye isCompactSummary (2026-08-14)
# Margen sobre la ventana más amplia que pida esta ejecución (la tendencia son
# 30 días, pero --days admite hasta 90): 2 días de colchón por relojes
# desajustados y zonas horarias.
SKIP_MARGIN_DAYS = 2


def cache_path():
    base = os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache")
    return Path(base) / "michiclaude" / "scan_cache.json"


def load_cache(need_from):
    """Caché válido solo si retiene al menos hasta `need_from`: si esta
    ejecución pide más historial del que se guardó (p. ej. --days 90 tras un
    ciclo de 7), se descarta y se reconstruye en vez de devolver de menos."""
    try:
        c = json.loads(cache_path().read_text(encoding="utf-8"))
        if (isinstance(c, dict) and c.get("version") == CACHE_VERSION
                and (c.get("retained_from") or 1e18) <= need_from):
            return c.get("files") or {}
    except Exception:
        pass
    return {}


def save_cache(files, retained_from):
    try:
        p = cache_path()
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps({"version": CACHE_VERSION,
                                 "retained_from": retained_from,
                                 "files": files}), encoding="utf-8")
    except Exception:
        pass  # el caché es un lujo: si no se puede escribir, no pasa nada


def is_user_turn(v, msg):
    """¿Turno ÚTIL del usuario? Envuelve a user_turn_text: UNA sola
    implementación del filtro (réplica exacta del Rust, invariante #1)."""
    return user_turn_text(v, msg) is not None


def user_turn_text(v, msg):
    """El TEXTO de un turno humano real (None = turno maquinal): fuera los
    meta, los de subagente, los resultados de herramienta (llegan con rol
    user pero los escribe la máquina) y los envoltorios de comandos
    locales. Refactor 2026-08-11: la evidencia del análisis local
    (docs/analisis-local.md) necesita los mensajes, no solo contarlos —
    réplica EXACTA de user_turn_text en lib.rs (invariante #1)."""
    if v.get("type") != "user":
        return None
    if v.get("isSidechain") or v.get("isMeta"):
        return None
    # los resúmenes de continuación de una compactación viajan con rol user
    # pero los escribe la máquina ("This session is being continued…"):
    # contaban como turno útil e inflaban el denominador del rendimiento
    # (cazado 2026-08-14 diseñando el detector de pegado masivo). Cambio de
    # semántica => CACHE_VERSION 3 en los dos lados.
    if v.get("isCompactSummary"):
        return None
    if v.get("toolUseResult") is not None:
        return None
    if (msg.get("role") or "") != "user":
        return None
    c = msg.get("content")
    if isinstance(c, str):
        txt = c
    elif isinstance(c, list):
        for b in c:
            if isinstance(b, dict) and b.get("type") == "tool_result":
                return None
        txt = " ".join(b.get("text") or "" for b in c
                       if isinstance(b, dict) and b.get("type") == "text")
    else:
        return None
    t = txt.strip()
    # envoltorios que Claude Code o el IDE inyectan con rol user sin marcar
    # isMeta (el <ide_... se vio en logs reales del VPS, 2026-08-06). Lista
    # explícita y conservadora — réplica exacta del Rust.
    if (not t or t.startswith("<command-") or t.startswith("<local-command")
            or t.startswith("<ide_") or t.startswith("<system-reminder")
            or t.startswith("[Request interrupted")):
        return None
    return t


def parse_file(path, keep_after):
    """Parsea un .jsonl a entradas compactas, deduplicando dentro del archivo.

    Devuelve (display, [[ts|None, model, inp, out, cw, cr, key], ...],
    [[ts, uuid], ...], dups). Solo se conservan las entradas dentro de
    `keep_after` (o sin timestamp, que no suman en ninguna ventana pero sí
    ocupan su turno en la dedup). `dups` son los duplicados internos, para
    que el contador de diagnóstico siga dando lo mismo que el escaneo
    completo. OJO: los duplicados TAMBIÉN cruzan archivos (365 medidos en
    los logs reales), así que la dedup global al fusionar es imprescindible,
    no un lujo.
    """
    display = None
    entries = []
    uturns = []
    seen = set()
    dups = 0
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return (None, [], [], 0)
    for line in lines:
        try:
            v = json.loads(line)
        except ValueError:
            continue
        if not isinstance(v, dict):
            continue
        if display is None:
            cwd = v.get("cwd") or ""
            base = cwd.replace("\\", "/").rstrip("/").rsplit("/", 1)[-1]
            if base:
                display = base
        msg = v.get("message") or {}
        # turnos de usuario: no traen usage, se recogen aparte ANTES del filtro
        if is_user_turn(v, msg):
            ts = parse_ts(v.get("timestamp"))
            if ts is not None and ts >= keep_after:
                uturns.append([ts.timestamp(), v.get("uuid") or ""])
            continue
        usage = msg.get("usage")
        if not isinstance(usage, dict):
            continue
        key = "%s:%s" % (msg.get("id") or "", v.get("requestId") or "")
        if key != ":":
            if key in seen:
                dups += 1
                continue
            seen.add(key)
        model = msg.get("model") or "unknown"
        if model == "<synthetic>":
            continue
        ts = parse_ts(v.get("timestamp"))
        if ts is not None and ts < keep_after:
            continue
        entries.append([
            ts.timestamp() if ts is not None else None, model,
            usage.get("input_tokens") or 0, usage.get("output_tokens") or 0,
            usage.get("cache_creation_input_tokens") or 0,
            usage.get("cache_read_input_tokens") or 0, key,
        ])
    return (display, entries, uturns, dups)


def is_estimated(model):
    """Ni está en los precios descargados ni es una familia conocida: su coste
    sale de la tarifa por defecto y la UI debe marcarlo como estimación."""
    if price_key(model) in PRICES:
        return False
    m = model.lower()
    return not any(f in m for f in ("fable", "mythos", "opus", "haiku", "sonnet"))


# ---------------------------------------------------------------------------
# Analizador de fugas (--findings). Diseño completo en docs/analizador-fugas.md
# — leerlo antes de tocar esto. Las reglas que no se negocian:
#   · Solo hallazgos ESTRUCTURALES y medibles. Nada que exija adivinar qué tan
#     difícil era una tarea ("esto no merecía Opus" está PROHIBIDO).
#   · Costos MEDIDOS de los propios logs siempre que se pueda; donde entre una
#     heurística (chars/4 ≈ tokens) el hallazgo va con estimated:true y la UI
#     lo enseña con "~".
#   · Pasada APARTE que solo corre bajo el flag (patrón want_rows): necesita
#     detalle por línea (sesión, herramientas, contenidos devueltos) que el
#     caché de escaneo no guarda, y el ciclo normal del panel no debe pagarla.
# ---------------------------------------------------------------------------

# Comandos deterministas: turnos donde Claude no piensa, solo ejecuta. La
# lista es CORTA a propósito — un falso positivo aquí ("marcó como mecánico
# algo que sí pensaba") cuesta la credibilidad del detector entero.
MECH_RE = re.compile(
    r"^\s*(?:cd\s+\S+\s*(?:&&|;)\s*)?"
    r"(?:git\b|pytest\b|cargo\s+(?:check|fmt|clippy)\b|"
    r"npm\s+(?:test|ci|install)\b)")

REREAD_MIN = 3          # lecturas del mismo archivo en una sesión para avisar
REREAD_MIN_TOKENS = 2000  # por debajo es ruido: una tarjeta de $0.00 devalúa
                          # a las demás (y el usuario deja de mirarlas)
INFLATE_MIN_GROWTH = 50_000   # tokens de contexto acumulados
INFLATE_MIN_TURNS = 10
MECH_MIN = 5            # peticiones mecánicas en la ventana para avisar
ACOMPACT_MIN = 3        # auto-compacts por proyecto en la ventana para avisar
# Pegado masivo: umbral por MENSAJE calibrado con logs reales (2026-08-14:
# mediana tecleada 290 chars, p90 1.7k — 5k es ~17× la mediana, un mensaje
# así casi seguro lleva un bloque pegado). PISO honesto: a veces pegar ES
# lo correcto (un error de consola no tiene ruta) — por eso además del
# conteo se exige volumen, y el fix no regaña.
PASTE_MIN_CHARS = 5000    # chars en UN mensaje humano para contarlo
PASTE_MIN_COUNT = 3       # pegotes por proyecto en la ventana para avisar
PASTE_MIN_TOKENS = 10_000  # ~tokens pegados (chars/4) para avisar
CACHEBREAK_MIN_PREV = 20_000    # prefijo cacheado mínimo para evaluar un turno
CACHEBREAK_MIN_TOKENS = 300_000  # reescritos por sesión para avisar (~$2-4)
SUB_MIN_TOKENS = 50_000  # tokens de trabajo de subagentes para avisar
HOOKNOISE_MIN_FIRES = 15    # disparos de un hook en la ventana para avisar
HOOKNOISE_MIN_TOKENS = 10_000  # ~tokens inyectados (chars/4) para avisar
CLAUDEMD_MIN_LINES = 5      # líneas sin respaldo en un CLAUDE.md para avisar
CLAUDEMD_MAX_TOKENS = 400   # tope de identificadores a buscar por archivo
CLAUDEMD_LOAD_LIMIT = 40_000  # chars que Claude Code CARGA de un CLAUDE.md;
#                               lo que sobre no se lee nunca (detector 10)
MAX_FINDINGS = 12       # las tarjetas no son un log: lo más caro primero
# % de desperdicio estructural (presion-y-rendimiento.md §fórmula): la CLASE
# de detectores que entra al numerador — una línea de factura por detector,
# para que la suma sea disjunta por construcción. claudemdsize NUNCA (fuga
# de instrucciones, no de dinero); los de costo 0 (mcp/skills) entran solos
# el día que se midan. Lo conductual (inflate/reread/mech/subagents) fuera.
WASTE_KINDS = {"claudemd", "hooks_noise", "cachebreak",
               "mcp_unused", "skills_unused"}
WASTE_MAX_ITEMS = 100   # tarjetas SIN recortar que viajan en waste.items


def skills_installed():
    """Skills propias del usuario (~/.claude/skills). Los plugins NO se
    cuentan: la carpeta de marketplaces es el catálogo ENTERO cacheado
    (docenas de skills que nadie instaló) y contarla fabricaría hallazgos
    falsos — la credibilidad del detector vale más que su alcance."""
    out = set()
    d = Path.home() / ".claude" / "skills"
    try:
        for p in d.iterdir():
            if (p / "SKILL.md").is_file():
                out.add(p.name.lower())
    except OSError:
        pass
    return out


def skills_used_at(window_ago):
    """Skills con uso registrado por el PROPIO Claude Code (skillUsage de
    ~/.claude.json) dentro de la ventana. Complementa a los logs: cubre las
    invocadas por la herramienta Skill aunque el log ya se haya borrado."""
    out = set()
    try:
        cfg = json.loads((Path.home() / ".claude.json").read_text(encoding="utf-8"))
        for name, u in (cfg.get("skillUsage") or {}).items():
            if isinstance(u, dict) and (u.get("lastUsedAt") or 0) / 1000 >= window_ago.timestamp():
                out.add(str(name).split(":")[-1].lower())
    except (OSError, ValueError):
        pass
    return out


SKILL_CMD_RE = re.compile(r"<command-name>/?([^<\s]+)</command-name>")


def _md_token_ok(tok):
    """Filtro de identificadores del CLAUDE.md. Descarta lo corto (falsos
    verdes por subcadena), los patrones con comodines, las URLs (son
    referencias, no afirmaciones de uso) y lo que no lleva letras."""
    if len(tok) < 4 or len(tok) > 80:
        return False
    if tok.startswith("http://") or tok.startswith("https://"):
        return False
    if any(c in "<>{}*$|`\"" or c.isspace() for c in tok):
        return False
    return any(c.isalpha() for c in tok)


def _md_line_tokens(line):
    """Identificadores verificables de una línea de CLAUDE.md: lo que va
    entre backticks (ahí el autor señala código) más palabras sueltas con
    pinta de ruta o de archivo.ext. Determinista y conservador: una línea
    sin nada verificable queda GRIS (sin opinión), nunca roja."""
    toks = []
    parts = line.split("`")
    for i, seg in enumerate(parts):
        if i % 2 == 1:               # dentro de backticks
            span = seg.strip()
            if span:
                toks.append(span.split()[0])
        else:                        # texto normal: rutas y archivo.ext
            for w in seg.split():
                w = w.lstrip("(\"'«“[").rstrip(".,;:!?)\"'»”]")
                dotted = any(w[j] == "." and w[j - 1].isalnum()
                             and w[j + 1].isalnum()
                             for j in range(1, len(w) - 1))
                if "/" in w or "\\" in w or dotted:
                    toks.append(w)
    out = []
    for tok in toks:
        tl = tok.lower()
        if _md_token_ok(tl) and tl not in out:
            out.append(tl)
    return out


def _claude_mds(projects_dir, skip_before):
    """CLAUDE.md global + el de cada proyecto con actividad en la ventana.
    El cwd real sale de las primeras líneas de sus .jsonl: el nombre de la
    carpeta de logs viene aplanado y no se puede revertir sin ambigüedad."""
    mds = []   # (ruta, proyecto_o_None, texto)
    vistos = set()   # por ruta real: un symlink de carpeta renombrada haría
    #                  analizar (y cobrar) dos veces el mismo archivo
    g = Path.home() / ".claude" / "CLAUDE.md"
    try:
        mds.append((str(g), None, g.read_text(encoding="utf-8",
                                              errors="replace")))
        vistos.add(os.path.realpath(g))
    except OSError:
        pass
    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            cwd = None
            for f in sorted(proj.glob("*.jsonl")):
                try:
                    if f.stat().st_mtime < skip_before:
                        continue
                    with open(f, encoding="utf-8", errors="replace") as fh:
                        for _ in range(20):
                            ln = fh.readline()
                            if not ln:
                                break
                            try:
                                c = json.loads(ln).get("cwd")
                            except ValueError:
                                continue
                            if c:
                                cwd = c
                                break
                except OSError:
                    continue
                if cwd:
                    break
            if not cwd:
                continue
            p = Path(cwd) / "CLAUDE.md"
            rp = os.path.realpath(p)
            if rp in vistos:
                continue
            try:
                mds.append((str(p), proj.name,
                            p.read_text(encoding="utf-8", errors="replace")))
                vistos.add(rp)
            except OSError:
                continue
    return mds


def mcp_servers_configured():
    """Servidores MCP dados de alta en ~/.claude.json (global y por proyecto)."""
    out = set()
    try:
        cfg = json.loads((Path.home() / ".claude.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return out
    if isinstance(cfg.get("mcpServers"), dict):
        out |= set(cfg["mcpServers"])
    for p in (cfg.get("projects") or {}).values():
        if isinstance(p, dict) and isinstance(p.get("mcpServers"), dict):
            out |= set(p["mcpServers"])
    return out


def project_jsonls(proj):
    """Los .jsonl de un proyecto: los planos MÁS los transcripts de
    subagentes. Claude Code moderno (visto en v2.1.221, 2026-08-04) ya no
    escribe los turnos del subagente en el archivo de la sesión — los pone
    en <sesión>/subagents/agent-*.jsonl. Sin entrar ahí, ni el costo por
    proyecto ni el detector de subagentes los ven. Mantener en sincronía
    con project_jsonls() de lib.rs."""
    return sorted(proj.glob("*.jsonl")) + sorted(proj.glob("*/subagents/*.jsonl"))


def find_cleared(projects_dir, sid, cwd, ts):
    """El .jsonl de la sesión que un /clear acaba de borrar de la vista.

    Réplica EXACTA de read_cleared() de lib.rs (invariante #1): con sid
    COMPLETO (>=36 chars, solo lo tiene el modo chat) casa por nombre de
    archivo; sin él, por cwd normalizado + cercanía al momento del /clear,
    excluyendo la sesión que NACIÓ con ese /clear (primer timestamp
    < ts-30). Devuelve el texto o None. Solo lectura.

    Existe para las sesiones de TERMINAL en un servidor SSH: ahí no hay sid
    y el Windows no puede mirar el disco del servidor. Los datos llegan por
    STDIN (nunca interpolados en el shell) igual que --prices-stdin."""
    CAP = 4_000_000

    def cwd_key(p):
        return str(p).replace("\\", "/").rstrip("/").lower()

    def head_of(fp):
        try:
            with open(fp, "rb") as fh:
                return fh.read(16384).decode("utf-8", "replace")
        except OSError:
            return ""

    def raw_str(head, key):
        pat = '"%s":"' % key
        i = head.find(pat)
        if i < 0:
            return None
        out, esc = [], False
        for c in head[i + len(pat):]:
            if esc:
                out.append(c); esc = False
            elif c == "\\":
                esc = True
            elif c == '"':
                return "".join(out)
            else:
                out.append(c)
        return None

    def read_capped(fp):
        try:
            s = open(fp, encoding="utf-8", errors="replace").read()
        except OSError:
            return None
        return s[:CAP] + "\n… [recortado]" if len(s) > CAP else s

    if sid and len(sid) >= 36:
        for proj in sorted(projects_dir.glob("*")):
            fp = proj / (sid + ".jsonl")
            if fp.is_file():
                return read_capped(fp)
        return None
    want = cwd_key(cwd)
    if not want or ts <= 0:
        return None
    best = None
    for proj in sorted(projects_dir.glob("*")):
        if not proj.is_dir():
            continue
        for fp in sorted(proj.glob("*.jsonl")):
            try:
                mtime = int(fp.stat().st_mtime)
            except OSError:
                continue
            if mtime > ts + 120 or ts - mtime > 6 * 3600:
                continue
            if best and best[0] >= mtime:
                continue
            head = head_of(fp)
            c = raw_str(head, "cwd")
            if c is None or cwd_key(c) != want:
                continue
            born = parse_ts(raw_str(head, "timestamp") or "")
            if born is None or int(born.timestamp()) >= ts - 30:
                continue        # nació con el /clear: es la sesión NUEVA
            best = (mtime, fp)
    return read_capped(best[1]) if best else None


def scan_findings(projects_dir, window_ago, days, end):
    """Corre los detectores sobre la ventana pedida y devuelve la lista de
    hallazgos, la más cara primero. Relee los .jsonl (sin caché): solo corre
    bajo --findings y tarda ~1 s por cada 50 MB de logs."""
    sessions = {}   # sid -> estado por sesión
    pend_reads = {}  # tool_use_id -> (sid, ruta)  para casar con su resultado
    mech = [0, 0, 0.0, 0]        # [peticiones, tokens, costo, última actividad]
    sub = [0, 0, 0.0, 0]         # subagentes: [turnos, tokens, costo, última actividad]
    total_cost = 0.0             # denominador del waste: TODO lo gastado en
    #                              la ventana, de esta MISMA pasada (mismo
    #                              escaneo = numerador y denominador nunca
    #                              se desfasan). Subagentes incluidos.
    mcp_used = set()
    skills_used = set()          # invocadas en la ventana (logs)
    seen = set()                 # misma dedup que la agregación
    skip_before = (window_ago - timedelta(days=2)).timestamp()

    # CLAUDE.md sin respaldo: identificadores por línea, a buscar en el
    # texto CRUDO de los logs de la ventana. Solo con 7+ días, como skills:
    # "no lo mencionaste HOY" no dice nada. El tope de búsqueda va por
    # archivo y en orden de lectura; las líneas cuyos identificadores no
    # entraron quedan grises (sin opinión), nunca rojas.
    md_meta = []      # (ruta, proyecto_o_None)
    md_lines = []     # (md_idx, line_no, chars, [tokens buscados])
    md_pending = set()
    md_found = set()
    md_big = []       # (ruta, proyecto_o_None, chars) — detector 10
    if days >= 7:
        for ruta, pj, texto in _claude_mds(projects_dir, skip_before):
            if len(texto) > CLAUDEMD_LOAD_LIMIT:
                md_big.append((ruta, pj, len(texto)))
            idx = len(md_meta)
            md_meta.append((ruta, pj))
            added = set()
            for ln_no, ln in enumerate(texto.splitlines(), 1):
                keep = []
                for tok in _md_line_tokens(ln):
                    if tok in md_pending or tok in added:
                        keep.append(tok)
                    elif len(added) < CLAUDEMD_MAX_TOKENS:
                        added.add(tok)
                        keep.append(tok)
                if keep:
                    md_lines.append((idx, ln_no, len(ln), keep))
            md_pending |= added

    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            for f in project_jsonls(proj):
                try:
                    if f.stat().st_mtime < skip_before:
                        continue
                    text = f.read_text(encoding="utf-8", errors="replace")
                except OSError:
                    continue
                # identificadores del CLAUDE.md contra el texto crudo, con
                # eliminación temprana: el encontrado deja de buscarse
                if md_pending:
                    low = text.lower()
                    for tok in list(md_pending):
                        if tok in low:
                            md_pending.discard(tok)
                            md_found.add(tok)
                for line in text.splitlines():
                    try:
                        v = json.loads(line)
                    except ValueError:
                        continue
                    if not isinstance(v, dict):
                        continue
                    sid = v.get("sessionId") or ""
                    msg = v.get("message") or {}
                    content = msg.get("content")
                    blocks = content if isinstance(content, list) else []
                    S = sessions.setdefault(sid, {
                        "first_cr": None, "last_cr": None, "turns": 0,
                        "cr_cost": 0.0, "reads": {}, "read_chars": {},
                        # "proj" = carpeta de logs, se usa para CASAR con
                        # el detector de CLAUDE.md (no tocar). "disp" = el
                        # nombre real del cwd, solo para enseñar: la carpeta
                        # codifica la ruta entera con guiones y al recortarla
                        # se parten los nombres compuestos.
                        "models": {}, "proj": proj.name, "disp": "", "ts": 0,
                        "cb": [], "compacts": [], "hooks": {},
                        "ac": [0, 0, 0],   # auto-compacts: [n, preTokens, ts]
                        "pb": [0, 0, 0]})  # pegotes: [n, chars, ts]
                    # una compactación reescribe el contexto A PROPÓSITO:
                    # se marca para no contarla como ruptura de caché
                    if not S["disp"]:
                        cw = v.get("cwd")
                        if isinstance(cw, str) and cw.strip():
                            base = cw.replace("\\", "/").rstrip("/").rsplit("/", 1)[-1]
                            if base:
                                S["disp"] = base
                    if v.get("isCompactSummary") or v.get("subtype") == "compact_boundary":
                        cts = parse_ts(v.get("timestamp"))
                        if cts:
                            S["compacts"].append(cts)
                        # detector 11 — frecuencia de auto-compacts: solo las
                        # AUTOMÁTICAS (una manual la hiciste tú; las del relevo
                        # entran como manual y quedan fuera solas). Dedup por
                        # uuid: las reanudaciones copian la línea al archivo
                        # nuevo. preTokens = contexto al compactar — la
                        # compactación NO trae usage, así que es PISO ("~").
                        if (v.get("type") == "system"
                                and v.get("subtype") == "compact_boundary"):
                            cm = v.get("compactMetadata") or {}
                            u = v.get("uuid") or ""
                            if (cm.get("trigger") != "manual" and cts
                                    and window_ago <= cts <= end
                                    and u and u not in seen):
                                seen.add(u)
                                S["ac"][0] += 1
                                S["ac"][1] += cm.get("preTokens") or 0
                                S["ac"][2] = max(S["ac"][2], int(cts.timestamp()))
                    # detector 12 — pegado masivo: un mensaje HUMANO
                    # anormalmente grande casi siempre lleva un bloque pegado.
                    # user_turn_text ya filtra lo maquinal (tool results,
                    # resúmenes de compactación, <ide_…, comandos). Dedup por
                    # uuid: las reanudaciones copian la línea al archivo nuevo.
                    ptxt = user_turn_text(v, msg)
                    if ptxt is not None and len(ptxt) >= PASTE_MIN_CHARS:
                        pcts = parse_ts(v.get("timestamp"))
                        u = v.get("uuid") or ""
                        if (pcts and window_ago <= pcts <= end
                                and u and u not in seen):
                            seen.add(u)
                            S["pb"][0] += 1
                            S["pb"][1] += len(ptxt)
                            S["pb"][2] = max(S["pb"][2], int(pcts.timestamp()))
                    # /comandos del usuario: quedan como <command-name> en el
                    # mensaje (estas líneas no traen usage, va antes del filtro)
                    if "<command-name>" in line:
                        cts = parse_ts(v.get("timestamp"))
                        if cts and window_ago <= cts <= end:
                            texts = ([content] if isinstance(content, str) else
                                     [b.get("text") or "" for b in blocks
                                      if isinstance(b, dict)])
                            for tx in texts:
                                for m in SKILL_CMD_RE.finditer(tx):
                                    skills_used.add(
                                        m.group(1).split(":")[-1].lower())
                    # salida de hooks: cada disparo queda como attachment
                    # hook_success y su `content` es EXACTAMENTE lo que entró
                    # al contexto en ese turno (verificado con un log real
                    # 2026-07-30). Dedup por uuid: las reanudaciones copian
                    # las líneas viejas al archivo nuevo.
                    if v.get("type") == "attachment":
                        a = v.get("attachment") or {}
                        if a.get("type") == "hook_success":
                            cts = parse_ts(v.get("timestamp"))
                            u = v.get("uuid") or ""
                            if cts and window_ago <= cts <= end and u and u not in seen:
                                seen.add(u)
                                hk = S["hooks"].setdefault(
                                    a.get("hookName") or "?", [0, 0])
                                hk[0] += 1
                                hk[1] += len(a.get("content") or "")
                        continue   # los attachments nunca traen usage
                    # resultados de lecturas: se MIDE lo que viajó de verdad
                    for b in blocks:
                        if not isinstance(b, dict):
                            continue
                        if b.get("type") == "tool_result":
                            k = pend_reads.pop(b.get("tool_use_id"), None)
                            if k:
                                c = b.get("content")
                                n = (len(c) if isinstance(c, str) else
                                     sum(len(x.get("text") or "") for x in c
                                         if isinstance(x, dict)) if isinstance(c, list) else 0)
                                st = sessions[k[0]]
                                st["read_chars"][k[1]] = st["read_chars"].get(k[1], 0) + n
                    usage = msg.get("usage")
                    if not isinstance(usage, dict):
                        continue
                    key = "%s:%s" % (msg.get("id") or "", v.get("requestId") or "")
                    if key != ":":
                        if key in seen:
                            continue
                        seen.add(key)
                    model = msg.get("model") or "unknown"
                    if model == "<synthetic>":
                        continue
                    ts = parse_ts(v.get("timestamp"))
                    if ts is None or ts < window_ago or ts > end:
                        continue
                    inp = usage.get("input_tokens") or 0
                    out_t = usage.get("output_tokens") or 0
                    cw = usage.get("cache_creation_input_tokens") or 0
                    cr = usage.get("cache_read_input_tokens") or 0
                    pi, po, pcw, pcr = price_for(model)
                    total_cost += (inp * pi + out_t * po
                                   + cw * pcw + cr * pcr) / 1e6
                    # los subagentes llevan SU contexto y SU sessionId es el
                    # de la sesión MADRE (verificado 2026-08-04 con un
                    # transcript real): si tocaran el estado de la sesión,
                    # su cache_read chico rompería first/last_cr (infladas)
                    # y fabricaría rupturas que no existieron. Solo suman a
                    # su propia tarjeta; sus tool_use de abajo sí cuentan
                    # (un MCP invocado por el subagente ES un MCP usado).
                    if not v.get("isSidechain"):
                        S["turns"] += 1
                        S["ts"] = max(S["ts"], int(ts.timestamp()))  # última actividad -> Finding.ts (epoch, como Rust)
                        S["models"][model] = S["models"].get(model, 0) + 1
                        if S["first_cr"] is None:
                            S["first_cr"] = cr
                        S["last_cr"] = cr
                        S["cr_cost"] += cr * pcr / 1e6   # MEDIDO: releer el contexto
                        # hilo principal en orden para el detector de rupturas
                        S["cb"].append((ts, model, cr, cw))
                    else:
                        # subagentes: costo MEDIDO de su propio usage — ya
                        # está dentro del total, pero ahí es invisible
                        sub[0] += 1
                        sub[1] += inp + out_t + cw
                        sub[2] += (inp * pi + out_t * po
                                   + cw * pcw + cr * pcr) / 1e6
                        sub[3] = max(sub[3], int(ts.timestamp()))
                    uses = [b for b in blocks
                            if isinstance(b, dict) and b.get("type") == "tool_use"]
                    for b in uses:
                        name = b.get("name") or ""
                        if name.startswith("mcp__"):
                            mcp_used.add(name.split("__")[1] if "__" in name[5:]
                                         else name[5:])
                        if name == "Skill":
                            sk = (b.get("input") or {}).get("skill") or ""
                            if sk:
                                skills_used.add(str(sk).split(":")[-1].lower())
                        if name == "Read":
                            p = read_key(b.get("input") or {})
                            if p:
                                S["reads"][p] = S["reads"].get(p, 0) + 1
                                pend_reads[b.get("id")] = (sid, p)
                    if uses and all(b.get("name") == "Bash" and MECH_RE.match(
                            str((b.get("input") or {}).get("command") or ""))
                            for b in uses):
                        mech[0] += 1
                        mech[1] += inp + out_t + cw
                        mech[2] += (inp * pi + out_t * po + cw * pcw + cr * pcr) / 1e6
                        mech[3] = max(mech[3], int(ts.timestamp()))

    findings = []
    hooks_g = {}   # hookName -> [disparos, chars, costo] sumado entre sesiones
    acomp_g = {}   # proyecto -> [n, preTokens, costo, ts] entre sesiones
    paste_g = {}   # proyecto -> [n, chars, costo, ts] entre sesiones
    for sid, S in sessions.items():
        if not S["models"]:
            continue
        top_model = max(S["models"], key=S["models"].get)
        pi = price_for(top_model)[0]
        # auto-compacts por PROYECTO ("7 veces esta semana en X" es el
        # hábito; por sesión sería confeti). Costo PISO: releer el contexto
        # que había (preTokens) una vez a precio de input del modelo
        # dominante — el costo real no es medible (sin usage).
        if S["ac"][0]:
            g = acomp_g.setdefault(S["disp"] or S["proj"], [0, 0, 0.0, 0])
            g[0] += S["ac"][0]
            g[1] += S["ac"][1]
            g[2] += S["ac"][1] * pi / 1e6
            g[3] = max(g[3], S["ac"][2])
        # pegotes por PROYECTO. Costo PISO: una ingesta de lo pegado
        # (chars/4, "~") al input del modelo dominante — la realidad es
        # mayor porque viaja en cada turno posterior.
        if S["pb"][0]:
            g = paste_g.setdefault(S["disp"] or S["proj"], [0, 0, 0.0, 0])
            g[0] += S["pb"][0]
            g[1] += S["pb"][1]
            g[2] += S["pb"][1] / 4 * pi / 1e6
            g[3] = max(g[3], S["pb"][2])
        # los disparos se acumulan por hook GLOBAL, pero el costo se valora
        # con el modelo dominante de la sesión donde ocurrieron
        for hname, hk in S["hooks"].items():
            g = hooks_g.setdefault(hname, [0, 0, 0.0, 0])
            g[0] += hk[0]
            g[1] += hk[1]
            g[2] += hk[1] / 4 * pi / 1e6
            g[3] = max(g[3], S["ts"])
        # archivos releídos: el contenido se APILA en la conversación, no se
        # reemplaza. Tokens ~ chars/4 de lo devuelto tras la primera lectura;
        # el costo es el PISO (una ingesta a precio de input) — la realidad es
        # mayor porque además se relee en cada turno posterior.
        for path, n in S["reads"].items():
            if n < REREAD_MIN:
                continue
            chars = S["read_chars"].get(path, 0)
            stacked = int(chars * (n - 1) / n / 4) if n else 0
            if stacked < REREAD_MIN_TOKENS:
                continue
            findings.append({
                "kind": "reread", "file": path, "ts": S["ts"], "project": S["disp"] or S["proj"],
                "count": n, "tokens": stacked,
                "cost": stacked * pi / 1e6, "estimated": True,
                "session": sid[:8]})
        growth = ((S["last_cr"] or 0) - (S["first_cr"] or 0))
        if growth >= INFLATE_MIN_GROWTH and S["turns"] >= INFLATE_MIN_TURNS:
            findings.append({
                "kind": "inflate", "ts": S["ts"], "project": S["disp"] or S["proj"], "session": sid[:8],
                "turns": S["turns"], "tokens": growth,
                "cost": S["cr_cost"], "estimated": False})
        # rupturas de caché: turnos donde el prefijo cacheado se PERDIÓ
        # (cache_read cae a menos de la mitad) y la conversación se
        # reescribió a precio de escritura (1.25x input) en vez de leerse
        # a 0.1x. Causas típicas: pausa mayor al TTL del caché o cambio de
        # modelo (cada modelo tiene el suyo). El costo es MEDIDO: tokens
        # que ya estaban escritos, cobrados otra vez a tarifa de escritura.
        cb = sorted(S["cb"], key=lambda x: x[0])
        breaks, rew_tok, rew_cost = 0, 0, 0.0
        for i in range(1, len(cb)):
            ts_i, m_i, cr_i, cw_i = cb[i]
            prev = cb[i - 1][2] + cb[i - 1][3]
            if prev < CACHEBREAK_MIN_PREV or cr_i * 2 >= prev:
                continue
            if any(abs((ts_i - c).total_seconds()) < 120
                   for c in S["compacts"]):
                continue
            rew = min(cw_i, prev)   # PISO: solo lo que ya estaba escrito
            breaks += 1
            rew_tok += rew
            rew_cost += rew * price_for(m_i)[2] / 1e6
        if rew_tok >= CACHEBREAK_MIN_TOKENS:
            findings.append({
                "kind": "cachebreak", "ts": S["ts"], "project": S["disp"] or S["proj"],
                "session": sid[:8], "count": breaks, "tokens": rew_tok,
                "cost": rew_cost, "estimated": False})
    if mech[0] >= MECH_MIN:
        findings.append({"kind": "mech", "ts": mech[3], "count": mech[0],
                         "tokens": mech[1], "cost": mech[2],
                         "estimated": False})
    # subagentes: una tarjeta con el costo agregado de la ventana. No juzga
    # si valieron la pena — solo hace VISIBLE un gasto que hoy se mezcla
    # con el total de la conversación principal.
    if sub[1] >= SUB_MIN_TOKENS:
        findings.append({"kind": "subagents", "ts": sub[3], "count": sub[0],
                         "tokens": sub[1], "cost": sub[2],
                         "estimated": False})
    # hooks ruidosos: la salida de un hook entra al contexto en CADA disparo
    # (tamaño × turnos). Tokens ~ chars/4 (heurística → "~") y costo PISO a
    # precio de input — la realidad es mayor porque además se relee en los
    # turnos posteriores. No juzga si el hook sirve: mide lo que cuesta
    # cargarlo, igual que skills_unused y mcp_unused.
    for hname in sorted(hooks_g):
        nf, nch, hcost, hts = hooks_g[hname]
        tok = nch // 4
        if nf < HOOKNOISE_MIN_FIRES or tok < HOOKNOISE_MIN_TOKENS:
            continue
        findings.append({"kind": "hooks_noise", "file": hname, "ts": hts,
                         "count": nf, "tokens": tok, "cost": hcost,
                         "estimated": True})
    # detector 11 — auto-compacts frecuentes: el salvavidas del ~94% no se
    # toca ni se sugiere apagar; la tarjeta señala el HÁBITO (llegar ahí
    # seguido) y su piso. El fix es entrar antes: /compact al 80%.
    for pj in sorted(acomp_g):
        n, pre, acost, ats = acomp_g[pj]
        if n < ACOMPACT_MIN:
            continue
        findings.append({"kind": "acompact", "project": pj, "ts": ats,
                         "count": n, "tokens": pre, "cost": acost,
                         "estimated": True})
    # detector 12 — pegado masivo: el fix no regaña (a veces pegar ES lo
    # correcto): si es un archivo del proyecto, mencionar la ruta sale
    # más barato que pegarlo.
    for pj in sorted(paste_g):
        n, nch, pcost, pts = paste_g[pj]
        tok = nch // 4
        if n < PASTE_MIN_COUNT or tok < PASTE_MIN_TOKENS:
            continue
        findings.append({"kind": "paste", "project": pj, "ts": pts,
                         "count": n, "tokens": tok, "cost": pcost,
                         "estimated": True})
    for server in sorted(mcp_servers_configured() - mcp_used):
        findings.append({"kind": "mcp_unused", "server": server,
                         "tokens": 0, "cost": 0.0, "estimated": False})
    # skills instaladas y sin usar en la ventana: UNA tarjeta agregada (una
    # por skill inundaría el reporte, y las tarjetas no son un log). Solo
    # con ventana de 7+ días: "no usaste tu skill HOY" no dice nada y
    # devalúa a las demás tarjetas.
    unused = sorted(skills_installed() - skills_used - skills_used_at(window_ago))
    if days < 7:
        unused = []
    if unused:
        shown = ", ".join(unused[:8]) + (" …" if len(unused) > 8 else "")
        findings.append({"kind": "skills_unused", "count": len(unused),
                         "file": shown, "tokens": 0, "cost": 0.0,
                         "estimated": False})
    # líneas de CLAUDE.md sin respaldo: NINGUNA de sus menciones aparece en
    # los logs de la ventana. Costo PISO, nunca "líneas × turnos" (la trampa
    # documentada: tras el primer turno está cacheado): esas líneas entran
    # al contexto UNA vez por sesión — tokens ~ chars/4 ("~") por sesión de
    # la ventana, al precio de input del modelo dominante de cada una.
    # Limitación asumida (dirección segura del error): si el CLAUDE.md se
    # leyó o editó en la ventana, sus líneas viajan en los logs y salen
    # verdes — el detector calla en vez de arriesgar un falso rojo.
    for idx, (ruta, pj) in enumerate(md_meta):
        reds = [(ln_no, ch) for (i2, ln_no, ch, toks) in md_lines
                if i2 == idx and all(t not in md_found for t in toks)]
        if len(reds) < CLAUDEMD_MIN_LINES:
            continue
        tok_est = sum(ch for _, ch in reds) // 4
        cost = 0.0
        for S in sessions.values():
            if not S["models"]:
                continue
            if pj is not None and S["proj"] != pj:
                continue
            pi2 = price_for(max(S["models"], key=S["models"].get))[0]
            cost += tok_est * pi2 / 1e6
        if cost <= 0:
            continue   # sin sesiones en la ventana, ese CLAUDE.md no viajó
        nums = ", ".join("L%d" % ln_no for ln_no, _ in reds[:6]) \
            + (" …" if len(reds) > 6 else "")
        findings.append({"kind": "claudemd", "count": len(reds),
                         "file": "%s · %s" % (ruta, nums),
                         "project": pj or "", "tokens": tok_est,
                         "cost": cost, "estimated": True})
    # detector 10 — CLAUDE.md más grande de lo que Claude Code CARGA (40k
    # chars): lo que sobra no se lee NUNCA, así que reglas al fondo del
    # archivo no llegan al modelo, en silencio. No es fuga de dinero sino
    # de INSTRUCCIONES: tarjeta de estado (costo 0 — el costo del tramo
    # cargado ya lo mide el detector de líneas); tokens ~ del tramo que
    # queda sin leer. Nos pasó en carne propia el 2026-08-04: 118.8k chars
    # y semanas sin ver el aviso amarillo de la terminal.
    for ruta, pj, sz in md_big:
        findings.append({"kind": "claudemdsize", "count": sz // 1000,
                         "file": ruta, "project": pj or "",
                         "tokens": (sz - CLAUDEMD_LOAD_LIMIT) // 4,
                         "cost": 0.0, "estimated": True})
    findings.sort(key=lambda x: -x["cost"])
    # % de desperdicio estructural: el numerador se arma ANTES del tope de
    # 12 (el tope ordena por costo y decapita justo a los estructurales, que
    # son los baratos). items lleva las tarjetas SIN recortar para que el
    # panel descuente las ignoradas (fndIgnore) con su misma clave; es un
    # PISO y el copy lo dice ("al menos") — invariante #8.
    struct_items = [f for f in findings
                    if f["kind"] in WASTE_KINDS][:WASTE_MAX_ITEMS]
    waste = {
        "struct_cost": sum(f["cost"] for f in struct_items),
        "struct_tokens": sum(f.get("tokens", 0) for f in struct_items),
        "total_cost": total_cost,
        "sessions": sum(1 for S in sessions.values() if S["models"]),
        "days": days,
        "end": int(end.timestamp()),
        "estimated": any(f["estimated"] and f["cost"] > 0
                         for f in struct_items),
        "items": struct_items,
    }
    return findings[:MAX_FINDINGS], waste




# ---------- coach de sesión activa (--coach) ----------
# Réplica del motor de coach_scan() en lib.rs (invariante #1: ambos lados en
# sincronía), para que las sesiones que corren EN este servidor también
# aconsejen — hasta 2026-08-05 el coach era solo local y trabajar por SSH
# dentro del VPS no avisaba de nada aunque ahí se gaste lo más.
# El estado incremental (offsets y banderines una-vez-por-sesión) vive en
# ~/.cache/michiclaude/coach_state.json: el exportador es un proceso nuevo
# en cada sondeo y sin él tendría que releer sesiones enteras cada 3 min.
# Reconstruible: si se borra o no se entiende, se rehace desde los logs (el
# frontend deduplica por rule|session, así que repetir un hecho no duplica
# tarjetas ni pushes). Los transcripts de subagentes NO entran: el coach
# queda plano a propósito, como en Rust.
COACH_ACTIVE_MIN = 30
# % del techo del modelo para sugerir /compact (antes 120k FIJO: con techo
# de 1M avisaba al 12%). Con modelo desconocido ctx_full cae a 200k → 120k.
COACH_CTX_PCT = 60
COACH_GAP_MIN = 6
COACH_GAP_CTX = 30_000
COACH_REREAD = 3
# Capturas de pantalla miradas en la sesión (Read sobre una IMAGEN): NO son
# relecturas — el archivo se regenera entre una y otra (revision.png ×19 en
# sparky-site, 2026-08-15, disparó "attach" y el consejo no aplicaba) — pero
# cada una entra al contexto y viaja en todos los turnos siguientes. Regla
# aparte "shots" con su propio consejo. Réplica del Rust.
COACH_SHOTS = 10
IMG_EXT = (".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp", ".svg")


def is_image_path(p):
    return str(p).lower().endswith(IMG_EXT)


def read_key(inp):
    """Clave de una lectura para contar RELECTURAS: archivo + rango. Leer
    trozos DISTINTOS de un archivo grande (lib.rs en 6 tandas de 1000
    líneas, 2026-08-15) no apila copias — es justo lo que la ficha
    recomienda — y contaba como 6 relecturas. Sin offset/limit la clave
    es la ruta a secas. Réplica del Rust (Hallazgos y coach)."""
    p = str(inp.get("file_path") or "")
    off, lim = inp.get("offset"), inp.get("limit")
    if off is None and lim is None:
        return p
    try:
        o = int(off or 0)
        end = str(o + int(lim)) if lim is not None else ""
    except (TypeError, ValueError):
        return p
    return "%s#L%d-%s" % (p, o, end)


COACH_SUM_QUIET = 10
COACH_SUM_MIN_TURNS = 5
COACH_DONE_QUIET = 5
COACH_DONE_TURNS = 5
COACH_ASK_QUIET = 3
PRESS_QUIET_MAX = 10  # manómetro: sesión "bajo los dedos" (réplica del Rust)


def coach_state_path():
    base = os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache")
    return Path(base) / "michiclaude" / "coach_state.json"


def _coach_default():
    return {"offset": 0, "last_ctx": 0, "first_turn": 0, "last_turn": 0,
            "turns": 0, "cmds": 0, "reads": {}, "edits": [], "tool_ids": [],
            # capturas (imágenes leídas) por ruta — aparte de reads
            "shots": {},
            "title": "", "proj": "", "cost": 0.0, "gaps": 0, "done": False,
            "notified": False, "asked": False, "pending_tool": False,
            # señales del clasificador de tarea viva (réplica del Rust,
            # docs/remediacion.md etapa 1b) — solo hechos, cero veredicto
            "todos_open": 0, "todos_total": 0, "trail": [],
            # últimos 3 mensajes HUMANOS truncados a 300 chars: la evidencia
            # del análisis local (docs/analisis-local.md), réplica del Rust
            "umsgs": [],
            "commit_clean": False,
            # cwd COMPLETO de la sesión (barras normalizadas): con qué se casa
            # una sesión con un relevo abierto en esa carpeta (etapa 3b)
            "scwd": "",
            # modelo del último turno: de él sale el techo de contexto
            "model": "",
            # contexto máximo visto: evidencia que corrige a la tabla
            "ctx_seen": 0,
            # último compact_boundary AUTOMÁTICO (los manuales los hiciste tú)
            "acomp_ts": 0, "acomp_pre": 0, "acomp_done": 0}


def coach_leaks(st):
    """Mini-auditoría del cierre — mismas tres reglas que coach_leaks() en
    Rust: releídos, contexto (ctx y cache EXCLUYENTES) y pausas con caché
    vencido."""
    out = []
    reread = [(f, n) for f, n in st["reads"].items() if n >= COACH_REREAD]
    if reread:
        f, n = max(reread, key=lambda x: x[1])
        base = f.replace("\\", "/").rsplit("/", 1)[-1]
        out.append({"kind": "reread", "file": base, "n": n})
    shots = sum(st.get("shots", {}).values())
    if shots >= COACH_SHOTS:
        out.append({"kind": "shots", "file": "", "n": shots})
    if st["last_ctx"] >= (ctx_full(st.get("model", ""), st.get("ctx_seen", 0))
                          * COACH_CTX_PCT // 100):
        out.append({"kind": "ctx", "file": "", "n": st["last_ctx"] // 1000})
    elif st["last_ctx"] >= COACH_GAP_CTX:
        out.append({"kind": "cache", "file": "", "n": st["last_ctx"] // 1000})
    if st["gaps"] > 0:
        out.append({"kind": "gap", "file": "", "n": st["gaps"]})
    return out


def coach_scan(projects_dir):
    hits = []
    dbg = []
    now = int(datetime.now(timezone.utc).timestamp())
    try:
        state = json.loads(coach_state_path().read_text(encoding="utf-8"))
        if not isinstance(state, dict):
            state = {}
    except Exception:
        state = {}
    alive = {}
    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            proj_name = proj.name
            # PLANO a propósito: los transcripts de subagentes no aconsejan
            for fp in sorted(proj.glob("*.jsonl")):
                try:
                    st_f = fp.stat()
                except OSError:
                    continue
                mtime = int(st_f.st_mtime)
                if now - mtime > COACH_ACTIVE_MIN * 60:
                    continue
                key = str(fp)
                st = state.get(key) or _coach_default()
                # completar campos si el estado viene de una versión vieja
                for k, v in _coach_default().items():
                    st.setdefault(k, v)
                # estado de antes de 2026-08-15: las imágenes contaban como
                # relecturas — se mudan a shots una sola vez
                for k in [k for k in st["reads"] if is_image_path(k)]:
                    st["shots"][k] = st["shots"].get(k, 0) + st["reads"].pop(k)
                size = st_f.st_size
                if size < st["offset"]:
                    st = _coach_default()   # truncado: de cero
                if size > st["offset"]:
                    try:
                        with open(fp, "rb") as fh:
                            fh.seek(st["offset"])
                            buf = fh.read(size - st["offset"])
                    except OSError:
                        continue
                    cut = buf.rfind(b"\n")
                    if cut < 0:
                        continue   # ni una línea completa nueva
                    st["offset"] += cut + 1
                    tool_ids = set(st["tool_ids"])
                    edits = set(st["edits"])
                    for line in buf[:cut + 1].decode("utf-8", "replace").splitlines():
                        try:
                            v = json.loads(line)
                        except ValueError:
                            continue
                        if not isinstance(v, dict):
                            continue
                        ts = parse_ts(v.get("timestamp"))
                        ts_s = int(ts.timestamp()) if ts else None
                        if not st["proj"] or not st.get("scwd"):
                            cwd = (v.get("cwd") or "").replace("\\", "/").rstrip("/")
                            base = cwd.rsplit("/", 1)[-1]
                            if base and not st["proj"]:
                                st["proj"] = base
                            if cwd and not st.get("scwd"):
                                st["scwd"] = cwd
                        if (v.get("type") == "system"
                                and v.get("subtype") == "compact_boundary"):
                            # solo las AUTOMÁTICAS: una manual la hiciste tú
                            cm = v.get("compactMetadata") or {}
                            if cm.get("trigger") != "manual":
                                st["acomp_ts"] = ts_s or 0
                                st["acomp_pre"] = cm.get("preTokens") or 0
                            # el contexto acaba de vaciarse, lo compactara quien
                            # lo compactara (fuera del if): 0 = "sin medida"
                            # hasta el próximo turno, y el hit press no sale.
                            # Réplica exacta de lib.rs (invariante #1).
                            st["last_ctx"] = 0
                        if v.get("type") == "ai-title":
                            t2 = (v.get("aiTitle") or "").strip()
                            if t2:
                                st["title"] = t2
                        msg = v.get("message") or {}
                        # evidencia del análisis local: mismos cortes que Rust
                        # (300 chars, últimos 3)
                        utxt = user_turn_text(v, msg)
                        if utxt is not None:
                            st["umsgs"] = (st.get("umsgs", [])
                                           + [utxt[:300]])[-3:]
                        usage = msg.get("usage")
                        side = bool(v.get("isSidechain"))
                        if isinstance(usage, dict) and not side:
                            # un turno nuevo del hilo principal limpia el
                            # pendiente (el blindaje del fantasma, 2026-08-03)
                            st["pending_tool"] = False
                            cr = usage.get("cache_read_input_tokens") or 0
                            cw = usage.get("cache_creation_input_tokens") or 0
                            ctx = cr + cw
                            if (ts_s and st["last_turn"] > 0
                                    and ts_s - st["last_turn"] >= COACH_GAP_MIN * 60
                                    and st["last_ctx"] >= COACH_GAP_CTX):
                                st["gaps"] += 1
                            if ctx > 0:
                                st["last_ctx"] = ctx
                                # el MÁXIMO visto es evidencia dura del techo
                                # real de esta máquina (ver ctx_full)
                                if ctx > st.get("ctx_seen", 0):
                                    st["ctx_seen"] = ctx
                            st["turns"] += 1
                            # el modelo del ÚLTIMO turno decide el techo del
                            # manómetro; se guarda el id crudo y no el techo ya
                            # resuelto, para que una tabla nueva lo corrija
                            st["model"] = msg.get("model") or "unknown"
                            pi, po, pcw, pcr = price_for(st["model"])
                            st["cost"] += ((usage.get("input_tokens") or 0) * pi
                                           + (usage.get("output_tokens") or 0) * po
                                           + cw * pcw + cr * pcr) / 1e6
                            if ts_s:
                                if st["first_turn"] == 0:
                                    st["first_turn"] = ts_s
                                st["last_turn"] = ts_s
                        blocks = msg.get("content")
                        if isinstance(blocks, list):
                            for b in blocks:
                                if not isinstance(b, dict):
                                    continue
                                bt = b.get("type")
                                if bt == "tool_use" and not side:
                                    st["pending_tool"] = True
                                elif bt == "tool_result" and not side:
                                    st["pending_tool"] = False
                                if bt != "tool_use":
                                    continue
                                bid = b.get("id") or ""
                                if not bid or bid in tool_ids:
                                    continue
                                tool_ids.add(bid)
                                name = b.get("name") or ""
                                inp = b.get("input") or {}
                                if name == "Read" and inp.get("file_path"):
                                    fpx = inp["file_path"]
                                    if is_image_path(fpx):
                                        st["shots"][fpx] = st["shots"].get(fpx, 0) + 1
                                    else:
                                        rk = read_key(inp)   # archivo + rango
                                        st["reads"][rk] = st["reads"].get(rk, 0) + 1
                                    st["trail"].append(fpx)
                                elif name == "Bash":
                                    st["cmds"] += 1
                                    # señal de cierre: commit deja la mesa
                                    # limpia hasta que algo se edite después
                                    if "git commit" in (inp.get("command") or ""):
                                        st["commit_clean"] = True
                                elif name in ("Edit", "Write", "NotebookEdit"):
                                    if inp.get("file_path"):
                                        edits.add(inp["file_path"])
                                        st["trail"].append(inp["file_path"])
                                    st["commit_clean"] = False
                                elif name == "TodoWrite":
                                    # la señal REINA: la lista de tareas
                                    td = inp.get("todos")
                                    if isinstance(td, list):
                                        st["todos_total"] = len(td)
                                        st["todos_open"] = sum(
                                            1 for x in td if isinstance(x, dict)
                                            and x.get("status") != "completed")
                                # rastro acotado a los últimos 20 archivos
                                if len(st["trail"]) > 20:
                                    st["trail"] = st["trail"][-20:]
                    st["tool_ids"] = sorted(tool_ids)
                    st["edits"] = sorted(edits)
                    st["asked"] = False   # el log creció: se rearma la espera
                sid = fp.stem[:8]
                proj_disp = st["proj"] or proj_name.split("-")[-1]

                def hit(rule, value, **extra):
                    h = {"rule": rule, "session": sid, "value": value,
                         "project": proj_disp}
                    h.update(extra)
                    hits.append(h)

                if st["last_ctx"] >= (ctx_full(st.get("model", ""),
                                               st.get("ctx_seen", 0))
                                      * COACH_CTX_PCT // 100):
                    # scwd aditivo (2026-08-16, réplica de Rust): con el cwd
                    # la ficha caliente casa un relevo y ofrece "Aplicar"
                    hit("compact", st["last_ctx"] // 1000,
                        scwd=st.get("scwd", ""))
                gap_min = (now - st["last_turn"]) // 60
                if (st["last_turn"] > 0 and gap_min >= COACH_GAP_MIN
                        and st["last_ctx"] >= COACH_GAP_CTX):
                    hit("cache", max(gap_min, 0), scwd=st.get("scwd", ""))
                reread = [(f, n) for f, n in st["reads"].items()
                          if n >= COACH_REREAD]
                if reread:
                    # el nombre del archivo viaja (aditivo): la ficha dice
                    # QUÉ releyó Claude — sin él el usuario creía que la
                    # regla hablaba de lo que ÉL pegaba (Oscar 2026-08-15)
                    f, n = max(reread, key=lambda x: x[1])
                    hit("attach", n,
                        file=f.replace("\\", "/").rsplit("/", 1)[-1])
                shots = st.get("shots", {})
                if sum(shots.values()) >= COACH_SHOTS:
                    f = max(shots, key=shots.get)
                    hit("shots", sum(shots.values()),
                        file=f.replace("\\", "/").rsplit("/", 1)[-1])
                quiet_min = max(0, now - mtime) // 60
                # manómetro (docs/remediacion.md etapa 1): presión de contexto
                # de la sesión activa — dato puro, sin compuertas (como en Rust)
                if (st["last_ctx"] > 0 and st["last_turn"] > 0
                        and quiet_min < PRESS_QUIET_MAX):
                    # continuidad de archivos: Jaccard % de los últimos 10
                    # contra los 10 previos del rastro (misma cuenta que Rust)
                    trail = st["trail"]
                    cont = 0
                    if len(trail) >= 12:
                        a, b = set(trail[-10:]), set(trail[-20:-10])
                        u = a | b
                        if u:
                            cont = len(a & b) * 100 // len(u)
                    hit("press", st["last_ctx"], quiet=quiet_min,
                        topen=st["todos_open"], ttotal=st["todos_total"],
                        cont=cont, gclean=st["commit_clean"],
                        full=ctx_full(st.get("model", ""), st.get("ctx_seen", 0)),
                        scwd=st.get("scwd", ""),
                        title=st.get("title", ""),
                        msgs=st.get("umsgs", []))
                # auto-compact reciente y sin avisar (30 min: no revivir
                # compactaciones viejas si el estado se reconstruye de cero)
                if (st["acomp_ts"] and st["acomp_ts"] != st["acomp_done"]
                        and now - st["acomp_ts"] < 30 * 60):
                    st["acomp_done"] = st["acomp_ts"]
                    hit("acomp", st["acomp_pre"] // 1000)
                if (st["pending_tool"] and not st["asked"]
                        and quiet_min >= COACH_ASK_QUIET and st["turns"] >= 1):
                    st["asked"] = True
                    hit("ask", quiet_min, turns=st["turns"])
                mins = max((st["last_turn"] - st["first_turn"]) // 60, 1)
                if (not st["notified"] and not st["pending_tool"]
                        and quiet_min >= COACH_DONE_QUIET
                        and st["turns"] >= COACH_DONE_TURNS):
                    st["notified"] = True
                    hit("done", mins, title=st["title"], cmds=st["cmds"],
                        edits=len(st["edits"]), turns=st["turns"],
                        cost=st["cost"], leaks=coach_leaks(st))
                if (not st["done"] and not st["pending_tool"]
                        and quiet_min >= COACH_SUM_QUIET
                        and st["turns"] >= COACH_SUM_MIN_TURNS):
                    st["done"] = True
                    hit("sum", mins, title=st["title"], cmds=st["cmds"],
                        edits=len(st["edits"]), turns=st["turns"],
                        cost=st["cost"], leaks=coach_leaks(st))
                alive[key] = st
                dbg.append({"sid": sid, "proj": proj_disp, "turns": st["turns"],
                            "quiet_min": quiet_min, "ctx": st["last_ctx"],
                            "pending": st["pending_tool"], "asked": st["asked"],
                            "notified": st["notified"], "sum_done": st["done"],
                            "gaps": st["gaps"],
                            "cost": round(st["cost"], 2)})
    # solo sesiones vivas: las dormidas salen del estado solas (acotado)
    try:
        p = coach_state_path()
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(alive), encoding="utf-8")
        (p.parent / "coach_debug.json").write_text(json.dumps(
            {"at": datetime.now(timezone.utc).isoformat(),
             "sessions": dbg,
             "hits": ["%s|%s" % (h["rule"], h["session"]) for h in hits]},
            indent=1), encoding="utf-8")
    except Exception:
        pass   # el estado es un lujo, como el caché de escaneo
    return hits


HOSTS_DIR = Path.home() / ".michiclaude" / "hosts"


def read_hosts(exclude_id, days):
    """Resúmenes que dejaron OTRAS máquinas en este servidor (modo hub).

    Se devuelven aparte, sin fusionar: quien pregunta les pone la etiqueta de
    su máquina. Se salta el del propio preguntante — si se le devolviera lo
    suyo, se contaría dos veces y los totales crecerían solos en cada ciclo.
    Un archivo ilegible se ignora: un resumen roto no puede tumbar la lectura
    de los demás.

    La VENTANA se elige aquí, en el servidor, porque quien lee no puede
    recortar un resumen ajeno: su desglose por proyecto ya viene sumado. Cada
    máquina sube una foto por ventana; si le piden una que no subió (o el
    resumen es de una versión vieja), se cae a `stats` y se marca con
    `window_exact: false` para que quien lo lea no lo dé por exacto.
    """
    out = []
    if not HOSTS_DIR.is_dir():
        return out
    for f in sorted(HOSTS_DIR.glob("*.json")):
        try:
            snap = json.loads(f.read_text(encoding="utf-8"))
        except Exception:
            continue
        if not isinstance(snap, dict) or not isinstance(snap.get("stats"), dict):
            continue
        if exclude_id and snap.get("id") == exclude_id:
            continue
        wins = snap.get("windows") or {}
        picked = wins.get(str(days))
        out.append({
            "id": snap.get("id", ""),
            "machine": snap.get("machine") or f.stem,
            "stats": picked if isinstance(picked, dict) else snap["stats"],
            "window_exact": isinstance(picked, dict),
            # cuándo se escribió: quien lee decide si está viejo. Aquí no se
            # borra ni se descarta nada por antigüedad — la app no puede
            # distinguir "se fue" de "está de vacaciones".
            "seen_at": datetime.fromtimestamp(f.stat().st_mtime, timezone.utc)
                       .isoformat(timespec="seconds"),
        })
    return out


def disk_usage_report():
    """`--du`: cuánto pesan los logs de ESTE servidor y cuánto es viejo.
    SOLO LECTURA — desde MichiClaude no se borra nada por SSH (decisión de
    Oscar 2026-08-15: la operación más peligrosa y la única sin simulacro
    decente); el panel enseña la ruta, el peso y un comando ACOTADO POR
    EDAD para que el usuario decida allá y no se lleve todo."""
    root = Path.home() / ".claude" / "projects"
    now = datetime.now(timezone.utc).timestamp()
    tot_n = tot_b = old_n = old_b = 0
    oldest = None
    if root.is_dir():
        for proj in root.iterdir():
            if not proj.is_dir():
                continue
            for f in project_jsonls(proj):
                try:
                    st = f.stat()
                except OSError:
                    continue
                tot_n += 1
                tot_b += st.st_size
                if now - st.st_mtime >= 365 * 86400:
                    old_n += 1
                    old_b += st.st_size
                if oldest is None or st.st_mtime < oldest:
                    oldest = st.st_mtime
    print(json.dumps({"path": str(root), "files": tot_n, "bytes": tot_b,
                      "old_files": old_n, "old_bytes": old_b,
                      "oldest": int(oldest) if oldest else 0}))


def main():
    days = 7
    args = sys.argv[1:]
    if "--du" in args:
        disk_usage_report()
        return
    if "--days" in args:
        try:
            days = max(1, min(90, int(args[args.index("--days") + 1])))
        except (IndexError, ValueError):
            pass
    # Precios frescos desde MichiClaude (una sola fuente de verdad). Si el JSON
    # no llega o viene roto, se sigue con la tabla embebida sin quejarse.
    # Identificador de quien pregunta, para no devolverle su propio resumen.
    exclude_id = ""
    if "--exclude-host" in args:
        try:
            exclude_id = args[args.index("--exclude-host") + 1]
        except IndexError:
            pass

    # Final de la ventana (epoch). Sin él, "ahora" — el comportamiento de
    # siempre. Con él, la ventana se desplaza al pasado y cubre
    # [end - days, end]: así un RANGO DE FECHAS se pide con los dos datos de
    # siempre (ancho + final). Mantener en sincronía con lib.rs (invariante
    # #1): allí es el parámetro `end_ts` de collect_own_stats.
    end_ts = None
    if "--end" in args:
        try:
            end_ts = int(args[args.index("--end") + 1])
        except (IndexError, ValueError):
            end_ts = None

    # Filas del reporte (fecha × proyecto × modelo). Solo cuando se piden:
    # el panel no las necesita y engordarían cada consulta.
    want_rows = "--rows" in args
    # Hallazgos del analizador de fugas: pasada aparte, solo bajo demanda.
    want_findings = "--findings" in args

    if "--prices-stdin" in args:
        try:
            raw = sys.stdin.read()
            if raw.strip():
                got = json.loads(raw)
                if isinstance(got, dict):
                    PRICES.update(got)
        except Exception:
            pass

    claude_dir = Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude")
    projects_dir = claude_dir / "projects"

    # --coach: ATAJO — solo los consejos de las sesiones vivas, sin la
    # agregación de gasto. Es lo que el panel pide cada 3 min; hacerle
    # calcular todo el gasto para tirar el 95% sería trabajo tirado.
    if "--coach" in args:
        print(json.dumps({"coach": coach_scan(projects_dir)}))
        return

    # --cleared-stdin: el .jsonl de una sesión que un /clear borró de la
    # vista, para el VISOR del panel. Las señas ({sid,cwd,ts}) llegan por
    # STDIN, jamás en la línea de comandos: son datos del usuario y ahí no
    # hay shell que los interprete. Sale texto crudo (o nada).
    if "--cleared-stdin" in args:
        try:
            got = json.loads(sys.stdin.read() or "{}")
            txt = find_cleared(projects_dir, str(got.get("sid") or ""),
                               str(got.get("cwd") or ""), int(got.get("ts") or 0))
        except Exception:
            txt = None
        if txt:
            sys.stdout.write(txt)
        return
    now = datetime.now(timezone.utc)
    end = (datetime.fromtimestamp(end_ts, timezone.utc)
           if end_ts is not None else now)
    window_ago = end - timedelta(days=days)
    # "hoy" y la serie de 30 días van con AHORA a propósito: no son la
    # ventana elegida y moverlos convertiría "Hoy" en "el último día del
    # rango", que no es lo que dice la etiqueta.
    day_ago = now - timedelta(hours=24)
    month_ago = now - timedelta(days=30)

    seen = set()
    useen = set()      # dedup global de turnos de usuario (uuid)
    display = {}
    per_project = {}   # raw -> [cost, tokens, {model: cost}, uturns]
    rows = {}          # (fecha, proyecto, modelo) -> [cost, tokens]
    models = {}
    daily = {}         # fecha -> [cost, tokens, uturns]
    cost_today = cost_window = 0.0
    tokens_window = 0
    uturns_window = 0
    files_scanned = 0
    deduped = 0

    # La ventana más amplia de esta ejecución: la elegida o los 30 días de la
    # tendencia, lo que sea mayor. Nada anterior a eso entra en ningún cálculo.
    keep_after = min(now - timedelta(days=30 + SKIP_MARGIN_DAYS),
                     end - timedelta(days=days + SKIP_MARGIN_DAYS))
    skip_before = keep_after.timestamp()
    cache_in = load_cache(keep_after.timestamp())
    cache_out = {}
    # INTEGRIDAD DE LAS FUENTES (pieza 1, réplica exacta de lib.rs —
    # invariante #1): un limpiador de disco o el propio usuario pueden
    # recortar o borrar estos .jsonl, y sin darse cuenta el panel diría
    # "bajó el consumo". El caché ya guarda tamaño+mtime, así que un archivo
    # que ENCOGIÓ se ve solo. Va en la salida; quien lee le pone el origen.
    # Solo se juzga si la raíz se pudo LEER (root_ok): un disco desmontado
    # no es un borrado.
    shrunk_n = shrunk_b = 0
    root_ok = projects_dir.is_dir()

    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            raw = proj.name
            for f in project_jsonls(proj):
                try:
                    st = f.stat()
                except OSError:
                    continue
                # (1) demasiado viejo para caber en ninguna ventana: ni se abre
                if st.st_mtime < skip_before:
                    continue
                files_scanned += 1
                fk = str(f)
                hit = cache_in.get(fk)
                # el archivo ENCOGIÓ = recorte externo. Solo se observa (el
                # parseo relee el archivo entero: no hay offset que corregir).
                if isinstance(hit, dict) and (hit.get("len") or 0) > st.st_size:
                    shrunk_n += 1
                    shrunk_b += (hit.get("len") or 0) - st.st_size
                if (isinstance(hit, dict) and hit.get("len") == st.st_size
                        and hit.get("mtime") == int(st.st_mtime)):
                    disp = hit.get("display")
                    entries = hit.get("entries") or []
                    futurns = hit.get("uturns") or []
                    fdups = hit.get("dups") or 0
                else:
                    disp, entries, futurns, fdups = parse_file(f, keep_after)
                cache_out[fk] = {"len": st.st_size, "mtime": int(st.st_mtime),
                                 "display": disp, "entries": entries,
                                 "uturns": futurns, "dups": fdups}
                deduped += fdups   # los internos; los cruzados se cuentan abajo
                if disp and raw not in display:
                    display[raw] = disp
                # (2) agregación: siempre con los precios y la ventana de AHORA
                for ts_s, model, inp, out, cw, cr, key in entries:
                    if key != ":":
                        if key in seen:
                            deduped += 1
                            continue
                        seen.add(key)
                    if ts_s is None:
                        continue
                    ts = datetime.fromtimestamp(ts_s, timezone.utc)
                    pi, po, pcw, pcr = price_for(model)
                    cost = (inp * pi + out * po + cw * pcw + cr * pcr) / 1e6
                    if ts >= window_ago and ts <= end:
                        cost_window += cost
                        tokens_window += inp + out + cw  # cache_read excluido
                        e = per_project.setdefault(raw, [0.0, 0, {}, 0])
                        e[0] += cost
                        e[1] += inp + out + cw
                        e[2][model] = e[2].get(model, 0.0) + cost
                        if want_rows:
                            k = (ts.strftime("%Y-%m-%d"), raw, model)
                            r = rows.setdefault(k, [0.0, 0])
                            r[0] += cost
                            r[1] += inp + out + cw
                        m = models.setdefault(model, {
                            "input": 0, "output": 0, "cache_write": 0,
                            "cache_read": 0, "cost": 0.0, "estimated": False})
                        m["input"] += inp
                        m["output"] += out
                        m["cache_write"] += cw
                        m["cache_read"] += cr
                        m["cost"] += cost
                        m["estimated"] = is_estimated(model)
                    if ts >= day_ago:
                        cost_today += cost
                    if ts >= month_ago:
                        d = ts.strftime("%Y-%m-%d")
                        cell = daily.setdefault(d, [0.0, 0, 0])
                        cell[0] += cost
                        cell[1] += inp + out + cw  # mismo criterio: sin cache_read
                # turnos útiles del usuario: dedup global por uuid (también
                # cruzan archivos) y las MISMAS ventanas que el resto
                for uts, uid in futurns:
                    if uid:
                        if uid in useen:
                            continue
                        useen.add(uid)
                    if uts is None:
                        continue
                    ts = datetime.fromtimestamp(uts, timezone.utc)
                    if ts >= window_ago and ts <= end:
                        uturns_window += 1
                        per_project.setdefault(raw, [0.0, 0, {}, 0])[3] += 1
                    if ts >= month_ago:
                        daily.setdefault(ts.strftime("%Y-%m-%d"), [0.0, 0, 0])[2] += 1

    # INTEGRIDAD, la mitad de los DESAPARECIDOS: estaba en el caché y ya no
    # está en disco. Un archivo que solo envejeció fuera de la ventana SIGUE
    # existiendo y no cuenta (misma regla que en Rust).
    gone_n = gone_b = 0
    if root_ok:
        for fk, hit in cache_in.items():
            if fk in cache_out or not isinstance(hit, dict):
                continue
            if os.path.exists(fk):
                continue
            gone_n += 1
            gone_b += hit.get("len") or 0
    integrity = []
    nowts = int(now.timestamp())
    if shrunk_n:
        integrity.append({"t": nowts, "kind": "truncated",
                          "n": shrunk_n, "b": shrunk_b, "o": ""})
    if gone_n:
        integrity.append({"t": nowts, "kind": "vanished",
                          "n": gone_n, "b": gone_b, "o": ""})

    save_cache(cache_out, keep_after.timestamp())  # solo lo visto: se autopurga

    projects = [
        {"name": display.get(raw) or raw.rsplit("-", 1)[-1] or raw,
         "cost": cost, "tokens": tokens, "by_model": by_model,
         "uturns": uturns}
        for raw, (cost, tokens, by_model, uturns) in per_project.items()
    ]
    projects.sort(key=lambda p: -p["cost"])

    # Analizador de fugas + % de desperdicio: UNA pasada para las dos cosas
    # (numerador y denominador del waste nacen del mismo escaneo). Sin
    # --findings, waste vacío = "sin datos" para quien lee (nunca 0%).
    fnd, waste = (scan_findings(projects_dir, window_ago, days, end)
                  if want_findings else ([], {}))

    print(json.dumps({
        "projects": projects,
        "models": models,
        "cost_today": cost_today,
        "cost_week": cost_window,
        "tokens_week": tokens_window,
        "uturns_week": uturns_window,
        "files_scanned": files_scanned,
        "entries_deduped": deduped,
        "daily": [{"date": d, "cost": c[0], "tokens": c[1], "uturns": c[2]}
                  for d, c in sorted(daily.items())],
        # Modo hub: lo que dejaron las OTRAS máquinas. Un MichiClaude viejo
        # ignora esta clave y sigue viendo solo los datos de este servidor.
        "hosts": read_hosts(exclude_id, days),
        # El nombre legible del proyecto se resuelve aquí: durante el recorrido
        # solo se tiene la carpeta cruda, y el bonito puede aparecer en
        # cualquier línea del .jsonl.
        "rows": [
            {"date": d, "project": display.get(raw) or raw.rsplit("-", 1)[-1] or raw,
             "model": model, "cost": c, "tokens": t}
            for (d, raw, model), (c, t) in sorted(rows.items())
        ],
        # Analizador de fugas: solo bajo --findings, para que ni el ciclo del
        # panel ni las fotos del hub paguen la pasada extra.
        "findings": fnd,
        "waste": waste,
        # Integridad de ESTE servidor (pieza 1). Va siempre (no depende de
        # --findings): es barata y quien lee la necesita para no cantar una
        # mejora que fue un borrado.
        "integrity": integrity,
    }))


if __name__ == "__main__":
    main()
