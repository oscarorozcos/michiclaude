# MichiClaude - Hook A, el GUARDIAN de escalada (Windows nativo).
# Diseno: docs/ruteo-inteligente.md sec.5 (Hook A) y sec.11 (etapa 5).
# REPLICA EXACTA de guard-hook.py - los dos en sincronia. ASCII PURO a
# proposito (PowerShell 5.1 lee sin BOM como ANSI; una tilde rompe todo).
#
# UserPromptSubmit: si la sesion va en un modelo barato (haiku/sonnet) y el
# prompt trae senales ESTRUCTURALES pesadas (bloque de codigo, varias rutas,
# traza de error, imperativo largo), BLOQUEA antes de gastar un token y
# dice como seguir. Nota ausente, >10 min o `guard` apagado = no hace nada.
# El modelo sale del transcript (cola de 64 KB) o de settings.json.
# Mismo prompt en <10 min = pasa (insistencia). Prefijo ~ = escotilla.
# Al log SOLO senales y conteos, JAMAS el texto del prompt.

$ErrorActionPreference = 'Stop'

$STALE_S  = 600
$INSIST_S = 600
$TAIL     = 65536

$michi = Join-Path $env:USERPROFILE '.michiclaude'
$state = Join-Path $michi 'router_state.json'
$log   = Join-Path $michi 'ruteo_log.jsonl'
$last  = Join-Path $michi 'guard_last.json'
$LOG_MAX = 512KB

function Apunta($fila) {
    try {
        if (-not (Test-Path $michi)) { New-Item -ItemType Directory -Path $michi -Force | Out-Null }
        if ((Test-Path $log) -and ((Get-Item $log).Length -gt $LOG_MAX)) {
            Move-Item -Path $log -Destination ($log + '.1') -Force
        }
        Add-Content -Path $log -Value ($fila | ConvertTo-Json -Compress -Depth 5) -Encoding UTF8
    } catch { }
}

function EstadoFresco {
    try {
        $st = Get-Content -Path $state -Raw -Encoding UTF8 | ConvertFrom-Json
        $ahora = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        if ($null -eq $st.ts -or ($ahora - [double]$st.ts) -gt $STALE_S) { return $null }
        return $st
    } catch { return $null }
}

function Tiene($obj, $name) { return ($null -ne $obj) -and ($obj.PSObject.Properties.Name -contains $name) -and ($null -ne $obj.$name) }

function ModeloSesion($transcript, $cfgDir) {
    try {
        if ($transcript -and (Test-Path $transcript)) {
            $fs = [System.IO.File]::Open($transcript, 'Open', 'Read', 'ReadWrite')
            try {
                $len = $fs.Length
                $from = [Math]::Max(0, $len - $TAIL)
                $fs.Seek($from, 'Begin') | Out-Null
                $buf = New-Object byte[] ($len - $from)
                $fs.Read($buf, 0, $buf.Length) | Out-Null
            } finally { $fs.Close() }
            $tail = [System.Text.Encoding]::UTF8.GetString($buf)
            $ms = [regex]::Matches($tail, '"model"\s*:\s*"(claude-[^"]+)"')
            if ($ms.Count -gt 0) { return $ms[$ms.Count - 1].Groups[1].Value }
        }
    } catch { }
    try {
        $sj = Get-Content -Path (Join-Path $cfgDir 'settings.json') -Raw -Encoding UTF8 | ConvertFrom-Json
        if (Tiene $sj 'model') { return '' + $sj.model }
    } catch { }
    return $null
}

function Tier($model) {
    $m = ('' + $model).ToLower()
    foreach ($t in @('haiku','sonnet','opus','fable','mythos')) { if ($m.Contains($t)) { return $t } }
    return $null
}

function Senales($prompt) {
    $sig = @(); $peso = 0
    if ([regex]::Matches($prompt, '```').Count -ge 2) { $sig += 'code'; $peso += 2 }
    $paths = [regex]::Matches($prompt, '(?:[A-Za-z]:\\|\.{0,2}/|~/)?(?:[\w.\-]+[\\/]){1,}[\w.\-]+\.[A-Za-z0-9]{1,6}\b') | ForEach-Object { $_.Value } | Sort-Object -Unique
    if (@($paths).Count -ge 2) { $sig += 'paths'; $peso += 1 }
    if ($prompt -match '(Traceback \(most recent|\bat [\w$.<>]+ \([^)]+:\d+:\d+\)|panicked at|error\[E\d+\]|Exception in thread)') { $sig += 'trace'; $peso += 1 }
    $words = ($prompt -split '\s+' | Where-Object { $_ -ne '' }).Count
    $trim = $prompt.TrimEnd()
    if (($words -ge 60 -or $prompt.Length -ge 300) -and -not ($trim.EndsWith('?') -or $trim.EndsWith([string][char]0xFF1F))) { $sig += 'long'; $peso += 1 }
    return ,@($sig, $peso)
}

try {
    $crudo = [Console]::In.ReadToEnd()
    try { $ev = $crudo | ConvertFrom-Json } catch { exit 0 }
    if (-not (Tiene $ev 'prompt')) { exit 0 }
    $prompt = '' + $ev.prompt
    if ($prompt.Trim() -eq '') { exit 0 }
    $st = EstadoFresco
    if ($null -eq $st) { exit 0 }

    $sid = ''; if (Tiene $ev 'session_id') { $sid = '' + $ev.session_id }
    $cwd = ''; if (Tiene $ev 'cwd') { $cwd = '' + $ev.cwd }
    $ts = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    function Fila($evn) { return [ordered]@{ ts = $ts; sid = $sid; cwd = $cwd; plen = $prompt.Length; ev = $evn } }

    $lead = $prompt.TrimStart()
    if ($lead.StartsWith('/')) { exit 0 }
    if ($lead.StartsWith('~')) { Apunta (Fila 'bypass'); exit 0 }

    $cfgDir = $env:CLAUDE_CONFIG_DIR
    if (-not $cfgDir) { $cfgDir = Join-Path $env:USERPROFILE '.claude' }
    $tp = $null; if (Tiene $ev 'transcript_path') { $tp = '' + $ev.transcript_path }
    $model = ModeloSesion $tp $cfgDir
    $tr = Tier $model

    # --- (a) el guardian ---
    $guardOn = (Tiene $st 'guard') -and [bool]$st.guard
    if ($guardOn -and ($tr -eq 'haiku' -or $tr -eq 'sonnet')) {
        $r = Senales $prompt; $sig = $r[0]; $peso = $r[1]
        $umbral = 2; if ($tr -eq 'haiku') { $umbral = 1 }
        if ($peso -ge $umbral) {
            $sha = [System.Security.Cryptography.SHA1]::Create()
            $h = ([BitConverter]::ToString($sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($prompt.Trim()))) -replace '-','').ToLower().Substring(0,16)
            $insiste = $false
            try {
                $l = Get-Content -Path $last -Raw -Encoding UTF8 | ConvertFrom-Json
                if ((Tiene $l 'h') -and $l.h -eq $h -and (Tiene $l 'tier') -and $l.tier -eq $tr -and (Tiene $l 'ts') -and ($ts - [double]$l.ts) -lt $INSIST_S) { $insiste = $true }
            } catch { }
            if ($insiste) {
                $f = Fila 'insist'; $f['model'] = $tr; $f['sig'] = $sig; Apunta $f
            } else {
                try {
                    if (-not (Test-Path $michi)) { New-Item -ItemType Directory -Path $michi -Force | Out-Null }
                    Set-Content -Path $last -Value (@{ h = $h; tier = $tr; ts = $ts } | ConvertTo-Json -Compress) -Encoding UTF8
                } catch { }
                $f = Fila 'block'; $f['model'] = $tr; $f['sig'] = $sig; Apunta $f
                $razon = "MichiClaude: this looks complex and you're on $tr. Run /model opus and resend, or prefix ~ to send as is. / Esto se ve complejo y vas en ${tr}: /model opus y reenvia, o antepon ~ para mandarlo tal cual."
                [Console]::Out.Write((@{ decision = 'block'; reason = $razon } | ConvertTo-Json -Compress))
                exit 0
            }
        }
    }

    # --- (b) contexto inyectado (opcional, apagado por defecto) ---
    $ctxOn = (Tiene $st 'ctx') -and [bool]$st.ctx
    if ($ctxOn) {
        $partes = @()
        if (Tiene $st 'week_pct') { $wr = '?'; if (Tiene $st 'week_reset_h') { $wr = '' + $st.week_reset_h }; $partes += ('weekly quota ~{0}% used, resets in ~{1}h' -f [int]$st.week_pct, $wr) }
        if (Tiene $st 'session_pct') { $partes += ('5h session ~{0}%' -f [int]$st.session_pct) }
        if ($partes.Count -gt 0) {
            $mm = 'unknown'; if ($model) { $mm = $model }
            $ctx = "[MichiClaude] Session model: $mm. " + ($partes -join '; ') + ". If this request is trivial, briefly suggest a cheaper /model; if it is architecture-level, confirm the tier. Keep it to one line, only when relevant."
            $f = Fila 'ctx'; $f['model'] = $tr; Apunta $f
            [Console]::Out.Write((@{ hookSpecificOutput = [ordered]@{ hookEventName = 'UserPromptSubmit'; additionalContext = $ctx } } | ConvertTo-Json -Compress -Depth 5))
        }
    }
} catch { }
exit 0
