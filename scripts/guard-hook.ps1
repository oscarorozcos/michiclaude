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
# La ESCALERA de familias, de barata a cara (replica de guard-hook.py).
# A la ultima (fable) SOLO con `top` en la nota (interruptor opt-in).
$LADDER    = @('haiku','sonnet','opus','fable')
$CHEAP     = @('haiku','sonnet')
$TOP_PESO  = 3
$relayDir  = Join-Path $env:USERPROFILE '.michiclaude\relevo'

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

function TopDe($st) {
    # El alias del top SOLO si el interruptor esta encendido Y es un peldano
    # por encima de opus; si no, $null (todo como antes).
    if (-not ($st.PSObject.Properties.Name -contains 'top')) { return $null }
    $t = $st.top
    if (-not ($t -is [string]) -or -not ($t -match '^[A-Za-z]+$')) { return $null }
    $t = $t.ToLower()
    $k = [Array]::IndexOf($LADDER, $t)
    if ($k -gt [Array]::IndexOf($LADDER, 'opus')) { return $t }
    return $null
}

function Escalables($top) {
    # Sin top, haiku/sonnet; con top, todo lo que quede por debajo de el.
    if (-not $top) { return $CHEAP }
    $k = [Array]::IndexOf($LADDER, $top)
    return @($LADDER[0..($k - 1)])
}

function Umbral($tr) {
    # haiku 1, sonnet 2, opus 3 (solo con top)
    if ($tr -eq 'haiku') { return 1 }
    if ($tr -eq 'sonnet') { return 2 }
    return $TOP_PESO
}

function Destino($tr, $peso, $top) {
    $i = [Array]::IndexOf($LADDER, $tr)
    if ($i -lt 0) { return $null }
    $hi = $LADDER.Length - 2
    if ($top -and $peso -ge $TOP_PESO) { $j = [Array]::IndexOf($LADDER, $top) }
    elseif ($peso -lt 2) { $j = [Math]::Min($hi, $i + 1) }
    else { $j = [Math]::Min($hi, [Math]::Max($i + 1, [Array]::IndexOf($LADDER, 'opus'))) }
    if ($j -gt $i) { return $LADDER[$j] }
    return $null
}

function RelevoDe($sid, $cwd) {
    # El relevo de ESTA sesion: por sid exacto; si no, por cwd SOLO si es
    # unico. Fail-closed: en la duda, ninguno.
    if (-not (Test-Path $relayDir)) { return $null }
    $porCwd = @()
    $c2 = ('' + $cwd).Replace('\', '/').TrimEnd('/')
    foreach ($f in Get-ChildItem -Path $relayDir -Filter '*.json' -ErrorAction SilentlyContinue) {
        try { $st = Get-Content -Path $f.FullName -Raw -Encoding UTF8 | ConvertFrom-Json } catch { continue }
        if (-not (Tiene $st 'alive') -or -not [bool]$st.alive) { continue }
        if ($sid -and (Tiene $st 'sid') -and $st.sid -eq $sid) { return $st }
        if ($c2 -and (Tiene $st 'cwd') -and (('' + $st.cwd).Replace('\', '/').TrimEnd('/') -eq $c2)) { $porCwd += $st }
    }
    if ($porCwd.Count -eq 1) { return $porCwd[0] }
    return $null
}

function Escalar($sid, $cwd, $alias, $then) {
    # Deja la orden /model al relevo y SALE sin esperar el acuse (el relevo
    # solo queda libre tras el result del bloqueo, que espera a este hook).
    # $then = el prompt a REENVIAR tras el /model (5c); el texto no se
    # anota en ningun sitio.
    $st = RelevoDe $sid $cwd
    if ($null -eq $st -or -not (Tiene $st 'pid')) { return ,@($false, 'NORELAY', $false) }
    $pid2 = $st.pid
    $rid = 'esc-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $orden = [ordered]@{ id = $rid; op = 'inject'; text = ('/model ' + $alias); export = $false }
    $reenvio = [bool]$then
    if ($reenvio) { $orden['then'] = $then }
    $body = $orden | ConvertTo-Json -Compress -Depth 3
    $path = Join-Path $relayDir ($pid2.ToString() + '.cmd')
    try {
        $tmp = $path + '.tmp'
        [System.IO.File]::WriteAllText($tmp, $body, (New-Object System.Text.UTF8Encoding($false)))
        Move-Item -Path $tmp -Destination $path -Force
    } catch { return ,@($false, 'WRITE', $false) }
    return ,@($true, $rid, $reenvio)
}

function CodigoPelado($prompt) {
    # codigo SIN fences: el chat de VS Code se come los ``` (mordio 2026-08-17)
    $heads = 0; $prevOpen = $false
    foreach ($ln in ($prompt -split "`r?`n")) {
        if ($ln.Trim() -eq '') { continue }
        $t = $ln.Trim()
        if ($ln -match '^\s*(def |fn |function |class |import |from \S+ import|const |let |var |public |private |#include|SELECT |async def |return |if \(|for \(|while \()' -or $t -eq '{' -or $t -eq '}' -or $t -eq '};' -or $t -eq ');') { $heads++ }
        elseif ($prevOpen -and ($ln.StartsWith('    ') -or $ln.StartsWith("`t"))) { $heads++ }
        $r = $ln.TrimEnd()
        $prevOpen = ($r.EndsWith(':') -or $r.EndsWith('{') -or $r.EndsWith('(') -or $r.EndsWith('=>'))
        if ($heads -ge 2) { return $true }
    }
    return $false
}

function Senales($prompt) {
    $sig = @(); $peso = 0
    if (([regex]::Matches($prompt, '```').Count -ge 2) -or (CodigoPelado $prompt)) { $sig += 'code'; $peso += 2 }
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
    $top = TopDe $st
    if ($guardOn -and $tr -and ((Escalables $top) -contains $tr)) {
        $r = Senales $prompt; $sig = $r[0]; $peso = $r[1]
        # haiku 1, sonnet 2, opus 3 (solo con top); sin peldano, sin freno
        $dest = $null
        if ($peso -ge (Umbral $tr)) { $dest = Destino $tr $peso $top }
        if ($dest) {
            $sha = [System.Security.Cryptography.SHA1]::Create()
            $h = ([BitConverter]::ToString($sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($prompt.Trim()))) -replace '-','').ToLower().Substring(0,16)
            $insiste = $false; $auto = $false
            try {
                $l = Get-Content -Path $last -Raw -Encoding UTF8 | ConvertFrom-Json
                if ((Tiene $l 'h') -and $l.h -eq $h -and (Tiene $l 'tier') -and $l.tier -eq $tr -and (Tiene $l 'ts') -and ($ts - [double]$l.ts) -lt $INSIST_S) { $insiste = $true }
                if ((Tiene $l 'auto') -and [bool]$l.auto) { $auto = $true }
                # el peldano al que se subio: con el top ya no es siempre opus
                if ((Tiene $l 'to') -and ($l.to -is [string])) { $dest = '' + $l.to }
            } catch { }
            if ($insiste) {
                # el mismo prompt vuelve: o insististe tu, o lo reenvio el relevo (5c)
                $evn = 'insist'; if ($auto) { $evn = 'resent' }
                $f = Fila $evn; $f['model'] = $tr; $f['to'] = $dest; $f['sig'] = $sig; Apunta $f
            } else {
                $escOk = $false; $escErr = ''; $reenvio = $false
                $escOn = (Tiene $st 'esc') -and [bool]$st.esc
                $rsOn = (Tiene $st 'rs') -and [bool]$st.rs
                if ($escOn) {
                    # ESCALAR SOLO: el relevo de esta sesion teclea /model; el
                    # usuario solo reenvia - o con rs (5c) lo reenvia el relevo.
                    $thenTxt = $null; if ($rsOn) { $thenTxt = $prompt }
                    $r2 = Escalar $sid $cwd $dest $thenTxt; $escOk = $r2[0]; $escErr = $r2[1]; $reenvio = $r2[2]
                    $f = Fila 'escalate'; $f['model'] = $tr; $f['to'] = $dest; $f['ok'] = $escOk; $f['err'] = $escErr; $f['resend'] = $reenvio; $f['sig'] = $sig; Apunta $f
                } else {
                    $f = Fila 'block'; $f['model'] = $tr; $f['sig'] = $sig; $f['to'] = $dest; Apunta $f
                }
                # la memoria de insistencia se escribe DESPUES de escalar: asi
                # sabe si el proximo reenvio sera del relevo (auto) o tuyo
                try {
                    if (-not (Test-Path $michi)) { New-Item -ItemType Directory -Path $michi -Force | Out-Null }
                    Set-Content -Path $last -Value (@{ h = $h; tier = $tr; ts = $ts; to = $dest; auto = ($escOk -and $reenvio) } | ConvertTo-Json -Compress) -Encoding UTF8
                } catch { }
                if ($escOk -and $reenvio) {
                    $razon = "MichiClaude: this looked complex for $tr, so I'm switching this session to $dest and resending it for you. Nothing to do. / Esto se veia complejo para ${tr}: estoy subiendo la sesion a $dest y lo reenvio yo. No tienes que hacer nada."
                } elseif ($escOk) {
                    $razon = "MichiClaude: this looked complex for $tr, so I'm switching this session to $dest. Give it ~10 s and resend (Up + Enter). / Esto se veia complejo para ${tr}: estoy subiendo la sesion a $dest. Dale ~10 s y reenvialo (flecha arriba + Enter)."
                } else {
                    $razon = "MichiClaude: this looks complex and you're on $tr. Run /model $dest and resend, or prefix ~ to send as is. / Esto se ve complejo y vas en ${tr}: /model $dest y reenvia, o antepon ~ para mandarlo tal cual."
                }
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
