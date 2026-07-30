#!/usr/bin/env python3
"""Exportador remoto para MichiClaude.

Agrega los logs locales de Claude Code (~/.claude/projects/**/*.jsonl) con la
MISMA lógica que collect_local_stats en Rust (deduplicación por
message.id+requestId, exclusión de <synthetic>, cache_read fuera de los tokens
de trabajo, precios equivalente-API) y emite el JSON con la forma exacta de
LocalStats: projects (con by_model), models, cost_today, cost_week (= ventana),
tokens_week, daily (serie de 30 días).

Uso:  meter-export.py [--days N] [--exclude-host ID]
      --days N          ventana del gasto por proyecto (def. 7)
      --exclude-host ID no devolver el resumen de esa máquina (modo hub)

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
    prefijo de proveedor, sin variante [1m] y sin la fecha del snapshot."""
    s = model.lower().rsplit("/", 1)[-1].split("[", 1)[0].strip()
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
CACHE_VERSION = 1
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


def parse_file(path, keep_after):
    """Parsea un .jsonl a entradas compactas, deduplicando dentro del archivo.

    Devuelve (display, [[ts|None, model, inp, out, cw, cr, key], ...], dups).
    Solo se conservan las entradas dentro de `keep_after` (o sin timestamp,
    que no suman en ninguna ventana pero sí ocupan su turno en la dedup).
    `dups` son los duplicados internos, para que el contador de diagnóstico
    siga dando lo mismo que el escaneo completo. OJO: los duplicados TAMBIÉN
    cruzan archivos (365 medidos en los logs reales), así que la dedup global
    al fusionar es imprescindible, no un lujo.
    """
    display = None
    entries = []
    seen = set()
    dups = 0
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return (None, [], 0)
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
    return (display, entries, dups)


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
CACHEBREAK_MIN_PREV = 20_000    # prefijo cacheado mínimo para evaluar un turno
CACHEBREAK_MIN_TOKENS = 300_000  # reescritos por sesión para avisar (~$2-4)
SUB_MIN_TOKENS = 50_000  # tokens de trabajo de subagentes para avisar
HOOKNOISE_MIN_FIRES = 15    # disparos de un hook en la ventana para avisar
HOOKNOISE_MIN_TOKENS = 10_000  # ~tokens inyectados (chars/4) para avisar
MAX_FINDINGS = 12       # las tarjetas no son un log: lo más caro primero


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


def scan_findings(projects_dir, window_ago, days):
    """Corre los detectores sobre la ventana pedida y devuelve la lista de
    hallazgos, la más cara primero. Relee los .jsonl (sin caché): solo corre
    bajo --findings y tarda ~1 s por cada 50 MB de logs."""
    sessions = {}   # sid -> estado por sesión
    pend_reads = {}  # tool_use_id -> (sid, ruta)  para casar con su resultado
    mech = [0, 0, 0.0]           # [peticiones, tokens, costo]
    sub = [0, 0, 0.0]            # subagentes: [turnos, tokens, costo]
    mcp_used = set()
    skills_used = set()          # invocadas en la ventana (logs)
    seen = set()                 # misma dedup que la agregación
    skip_before = (window_ago - timedelta(days=2)).timestamp()

    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            for f in sorted(proj.glob("*.jsonl")):
                try:
                    if f.stat().st_mtime < skip_before:
                        continue
                    lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
                except OSError:
                    continue
                for line in lines:
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
                        "models": {}, "proj": proj.name,
                        "cb": [], "compacts": [], "hooks": {}})
                    # una compactación reescribe el contexto A PROPÓSITO:
                    # se marca para no contarla como ruptura de caché
                    if v.get("isCompactSummary") or v.get("subtype") == "compact_boundary":
                        cts = parse_ts(v.get("timestamp"))
                        if cts:
                            S["compacts"].append(cts)
                    # /comandos del usuario: quedan como <command-name> en el
                    # mensaje (estas líneas no traen usage, va antes del filtro)
                    if "<command-name>" in line:
                        cts = parse_ts(v.get("timestamp"))
                        if cts and cts >= window_ago:
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
                            if cts and cts >= window_ago and u and u not in seen:
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
                    if ts is None or ts < window_ago:
                        continue
                    inp = usage.get("input_tokens") or 0
                    out_t = usage.get("output_tokens") or 0
                    cw = usage.get("cache_creation_input_tokens") or 0
                    cr = usage.get("cache_read_input_tokens") or 0
                    pi, po, pcw, pcr = price_for(model)
                    S["turns"] += 1
                    S["models"][model] = S["models"].get(model, 0) + 1
                    if S["first_cr"] is None:
                        S["first_cr"] = cr
                    S["last_cr"] = cr
                    S["cr_cost"] += cr * pcr / 1e6   # MEDIDO: releer el contexto
                    # hilo principal en orden para el detector de rupturas;
                    # los subagentes llevan SU contexto y mezclarlos
                    # fabricaría rupturas que no existieron
                    if not v.get("isSidechain"):
                        S["cb"].append((ts, model, cr, cw))
                    else:
                        # subagentes: costo MEDIDO de su propio usage — ya
                        # está dentro del total, pero ahí es invisible
                        sub[0] += 1
                        sub[1] += inp + out_t + cw
                        sub[2] += (inp * pi + out_t * po
                                   + cw * pcw + cr * pcr) / 1e6
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
                            p = (b.get("input") or {}).get("file_path")
                            if p:
                                S["reads"][p] = S["reads"].get(p, 0) + 1
                                pend_reads[b.get("id")] = (sid, p)
                    if uses and all(b.get("name") == "Bash" and MECH_RE.match(
                            str((b.get("input") or {}).get("command") or ""))
                            for b in uses):
                        mech[0] += 1
                        mech[1] += inp + out_t + cw
                        mech[2] += (inp * pi + out_t * po + cw * pcw + cr * pcr) / 1e6

    findings = []
    hooks_g = {}   # hookName -> [disparos, chars, costo] sumado entre sesiones
    for sid, S in sessions.items():
        if not S["models"]:
            continue
        top_model = max(S["models"], key=S["models"].get)
        pi = price_for(top_model)[0]
        # los disparos se acumulan por hook GLOBAL, pero el costo se valora
        # con el modelo dominante de la sesión donde ocurrieron
        for hname, hk in S["hooks"].items():
            g = hooks_g.setdefault(hname, [0, 0, 0.0])
            g[0] += hk[0]
            g[1] += hk[1]
            g[2] += hk[1] / 4 * pi / 1e6
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
                "kind": "reread", "file": path, "project": S["proj"],
                "count": n, "tokens": stacked,
                "cost": stacked * pi / 1e6, "estimated": True,
                "session": sid[:8]})
        growth = ((S["last_cr"] or 0) - (S["first_cr"] or 0))
        if growth >= INFLATE_MIN_GROWTH and S["turns"] >= INFLATE_MIN_TURNS:
            findings.append({
                "kind": "inflate", "project": S["proj"], "session": sid[:8],
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
                "kind": "cachebreak", "project": S["proj"],
                "session": sid[:8], "count": breaks, "tokens": rew_tok,
                "cost": rew_cost, "estimated": False})
    if mech[0] >= MECH_MIN:
        findings.append({"kind": "mech", "count": mech[0],
                         "tokens": mech[1], "cost": mech[2],
                         "estimated": False})
    # subagentes: una tarjeta con el costo agregado de la ventana. No juzga
    # si valieron la pena — solo hace VISIBLE un gasto que hoy se mezcla
    # con el total de la conversación principal.
    if sub[1] >= SUB_MIN_TOKENS:
        findings.append({"kind": "subagents", "count": sub[0],
                         "tokens": sub[1], "cost": sub[2],
                         "estimated": False})
    # hooks ruidosos: la salida de un hook entra al contexto en CADA disparo
    # (tamaño × turnos). Tokens ~ chars/4 (heurística → "~") y costo PISO a
    # precio de input — la realidad es mayor porque además se relee en los
    # turnos posteriores. No juzga si el hook sirve: mide lo que cuesta
    # cargarlo, igual que skills_unused y mcp_unused.
    for hname in sorted(hooks_g):
        nf, nch, hcost = hooks_g[hname]
        tok = nch // 4
        if nf < HOOKNOISE_MIN_FIRES or tok < HOOKNOISE_MIN_TOKENS:
            continue
        findings.append({"kind": "hooks_noise", "file": hname, "count": nf,
                         "tokens": tok, "cost": hcost, "estimated": True})
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
    findings.sort(key=lambda x: -x["cost"])
    return findings[:MAX_FINDINGS]


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


def main():
    days = 7
    args = sys.argv[1:]
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
    now = datetime.now(timezone.utc)
    window_ago = now - timedelta(days=days)
    day_ago = now - timedelta(hours=24)
    month_ago = now - timedelta(days=30)

    seen = set()
    display = {}
    per_project = {}   # raw -> [cost, tokens, {model: cost}]
    rows = {}          # (fecha, proyecto, modelo) -> [cost, tokens]
    models = {}
    daily = {}
    cost_today = cost_window = 0.0
    tokens_window = 0
    files_scanned = 0
    deduped = 0

    # La ventana más amplia de esta ejecución: la elegida o los 30 días de la
    # tendencia, lo que sea mayor. Nada anterior a eso entra en ningún cálculo.
    span = max(days, 30) + SKIP_MARGIN_DAYS
    keep_after = now - timedelta(days=span)
    skip_before = keep_after.timestamp()
    cache_in = load_cache(keep_after.timestamp())
    cache_out = {}

    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            raw = proj.name
            for f in sorted(proj.glob("*.jsonl")):
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
                if (isinstance(hit, dict) and hit.get("len") == st.st_size
                        and hit.get("mtime") == int(st.st_mtime)):
                    disp = hit.get("display")
                    entries = hit.get("entries") or []
                    fdups = hit.get("dups") or 0
                else:
                    disp, entries, fdups = parse_file(f, keep_after)
                cache_out[fk] = {"len": st.st_size, "mtime": int(st.st_mtime),
                                 "display": disp, "entries": entries,
                                 "dups": fdups}
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
                    if ts >= window_ago:
                        cost_window += cost
                        tokens_window += inp + out + cw  # cache_read excluido
                        e = per_project.setdefault(raw, [0.0, 0, {}])
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
                        daily[d] = daily.get(d, 0.0) + cost

    save_cache(cache_out, keep_after.timestamp())  # solo lo visto: se autopurga

    projects = [
        {"name": display.get(raw) or raw.rsplit("-", 1)[-1] or raw,
         "cost": cost, "tokens": tokens, "by_model": by_model}
        for raw, (cost, tokens, by_model) in per_project.items()
    ]
    projects.sort(key=lambda p: -p["cost"])

    print(json.dumps({
        "projects": projects,
        "models": models,
        "cost_today": cost_today,
        "cost_week": cost_window,
        "tokens_week": tokens_window,
        "files_scanned": files_scanned,
        "entries_deduped": deduped,
        "daily": [{"date": d, "cost": c} for d, c in sorted(daily.items())],
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
        "findings": scan_findings(projects_dir, window_ago, days)
        if want_findings else [],
    }))


if __name__ == "__main__":
    main()
