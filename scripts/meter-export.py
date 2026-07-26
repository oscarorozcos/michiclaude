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


def price_for(model):
    """(input, output, cache_write, cache_read) USD por MTok.

    La tarifa depende de la VERSIÓN, no solo de la familia: Opus bajó de
    $15/$75 a $5/$25 a partir de la 4.5 (3, 4.0 y 4.1 siguen en la vieja).
    Escritura de caché = 1.25x input, lectura = 0.1x input.
    MANTENER EN SINCRONÍA con price_for() de src-tauri/src/lib.rs.
    """
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


def main():
    days = 7
    args = sys.argv[1:]
    if "--days" in args:
        try:
            days = max(1, min(90, int(args[args.index("--days") + 1])))
        except (IndexError, ValueError):
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

    if projects_dir.is_dir():
        for proj in sorted(projects_dir.iterdir()):
            if not proj.is_dir():
                continue
            raw = proj.name
            for f in sorted(proj.glob("*.jsonl")):
                files_scanned += 1
                try:
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
                    if raw not in display:
                        cwd = v.get("cwd") or ""
                        base = cwd.replace("\\", "/").rstrip("/").rsplit("/", 1)[-1]
                        if base:
                            display[raw] = base
                    msg = v.get("message") or {}
                    usage = msg.get("usage")
                    if not isinstance(usage, dict):
                        continue
                    key = "%s:%s" % (msg.get("id") or "", v.get("requestId") or "")
                    if key != ":":
                        if key in seen:
                            deduped += 1
                            continue
                        seen.add(key)
                    model = msg.get("model") or "unknown"
                    if model == "<synthetic>":
                        continue
                    inp = usage.get("input_tokens") or 0
                    out = usage.get("output_tokens") or 0
                    cw = usage.get("cache_creation_input_tokens") or 0
                    cr = usage.get("cache_read_input_tokens") or 0
                    pi, po, pcw, pcr = price_for(model)
                    cost = (inp * pi + out * po + cw * pcw + cr * pcr) / 1e6
                    ts = parse_ts(v.get("timestamp"))
                    if ts is not None and ts >= window_ago:
                        cost_window += cost
                        tokens_window += inp + out + cw  # cache_read excluido
                        e = per_project.setdefault(raw, [0.0, 0, {}])
                        e[0] += cost
                        e[1] += inp + out + cw
                        e[2][model] = e[2].get(model, 0.0) + cost
                        m = models.setdefault(model, {
                            "input": 0, "output": 0, "cache_write": 0,
                            "cache_read": 0, "cost": 0.0})
                        m["input"] += inp
                        m["output"] += out
                        m["cache_write"] += cw
                        m["cache_read"] += cr
                        m["cost"] += cost
                    if ts is not None and ts >= day_ago:
                        cost_today += cost
                    if ts is not None and ts >= month_ago:
                        d = ts.strftime("%Y-%m-%d")
                        daily[d] = daily.get(d, 0.0) + cost

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
