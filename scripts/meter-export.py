#!/usr/bin/env python3
"""Exportador remoto para MichiClaude.

Agrega los logs locales de Claude Code (~/.claude/projects/**/*.jsonl) con la
MISMA lógica que collect_local_stats en Rust (deduplicación por
message.id+requestId, exclusión de <synthetic>, cache_read fuera de los tokens
de trabajo, precios equivalente-API) y emite el JSON con la forma exacta de
LocalStats: projects (con by_model), models, cost_today, cost_week (= ventana),
tokens_week, daily (serie de 30 días).

Uso:  meter-export.py [--days N]      (N = ventana del gasto por proyecto; def. 7)

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
    }))


if __name__ == "__main__":
    main()
