# MichiClaude - Hook B del ruteo inteligente (Windows nativo).
# Diseno: docs/ruteo-inteligente.md sec.5 (pieza 2) y sec.11 (etapa 2).
# Es la REPLICA EXACTA de router-hook.py - los dos en sincronia, como el
# exportador. ASCII PURO a proposito: PowerShell 5.1 lee sin BOM como ANSI
# y una tilde rompe el script entero (mordida de la etapa 0, 2026-08-14).
#
# PreToolUse sobre Task|Agent: decide el modelo del subagente leyendo la
# nota de %USERPROFILE%\.michiclaude\router_state.json.
#   - exploracion/busqueda -> haiku (siempre)
#   - analisis profundo    -> sonnet SOLO si la cuota aprieta (>=70%)
#   - lo demas (implementacion) -> sonnet
# Estado ausente o >10 min de viejo = salir sin tocar nada (fail-quiet).
# `model` explicito en el input = se respeta. Se devuelve el objeto
# COMPLETO. Cada decision queda en ruteo_log.jsonl (JSON plano).

$ErrorActionPreference = 'Stop'

$STALE_S     = 600
$PRESSURE    = 70
$MODEL_LIGHT = 'haiku'
$MODEL_MID   = 'sonnet'
$LIGHT_WORDS = @('explore','search','scout','locate','lookup','grep')
$THINK_WORDS = @('plan','review','audit','analy','research','judge','verify','architect','security')

$michi = Join-Path $env:USERPROFILE '.michiclaude'
$state = Join-Path $michi 'router_state.json'
$log   = Join-Path $michi 'ruteo_log.jsonl'
$LOG_MAX = 512KB

function Apunta($fila) {
    # Deja la decision en el cuaderno; si no se puede, el hook no falla.
    try {
        if (-not (Test-Path $michi)) { New-Item -ItemType Directory -Path $michi -Force | Out-Null }
        if ((Test-Path $log) -and ((Get-Item $log).Length -gt $LOG_MAX)) {
            Move-Item -Path $log -Destination ($log + '.1') -Force
        }
        Add-Content -Path $log -Value ($fila | ConvertTo-Json -Compress -Depth 5) -Encoding UTF8
    } catch { }
}

function EstadoFresco {
    # La nota del refri, solo si es de hace <10 min; si no, $null.
    try {
        $st = Get-Content -Path $state -Raw -Encoding UTF8 | ConvertFrom-Json
        $ahora = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        if ($null -eq $st.ts -or ($ahora - [double]$st.ts) -gt $STALE_S) { return $null }
        return $st
    } catch { return $null }
}

function Clase($tipo) {
    $t = ('' + $tipo).ToLower()
    foreach ($w in $LIGHT_WORDS) { if ($t.Contains($w)) { return 'light' } }
    foreach ($w in $THINK_WORDS) { if ($t.Contains($w)) { return 'think' } }
    return 'work'
}

function Presion($st) {
    # El peor de los dos numeros que haya; sin ninguno, no hay presion.
    $vals = @()
    if ($st.PSObject.Properties.Name -contains 'week_pct' -and $null -ne $st.week_pct) { $vals += [double]$st.week_pct }
    if ($st.PSObject.Properties.Name -contains 'session_pct' -and $null -ne $st.session_pct) { $vals += [double]$st.session_pct }
    if ($vals.Count -eq 0) { return $false }
    return (($vals | Measure-Object -Maximum).Maximum -ge $PRESSURE)
}

try {
    $crudo = [Console]::In.ReadToEnd()
    try { $evento = $crudo | ConvertFrom-Json } catch { exit 0 }
    $entrada = $evento.tool_input
    if ($null -eq $entrada) { exit 0 }

    $st = EstadoFresco
    if ($null -eq $st) { exit 0 }   # MichiClaude no esta: como si el hook no existiera

    $tipo = ''
    if ($entrada.PSObject.Properties.Name -contains 'subagent_type') { $tipo = '' + $entrada.subagent_type }
    $antes = $null
    if ($entrada.PSObject.Properties.Name -contains 'model') { $antes = $entrada.model }
    $sid = ''; if ($evento.PSObject.Properties.Name -contains 'session_id') { $sid = '' + $evento.session_id }
    $cwd = ''; if ($evento.PSObject.Properties.Name -contains 'cwd') { $cwd = '' + $evento.cwd }
    $ts = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()

    # la fila del cuaderno se arma explicita cada vez: sumar diccionarios
    # ordered es terreno resbaloso en PowerShell 5.1
    function Fila($ev, $why, $after) {
        $f = [ordered]@{ ts = $ts; type = $tipo; before = $antes; sid = $sid; cwd = $cwd; ev = $ev; why = $why }
        if ($null -ne $after) { $f['after'] = $after }
        return $f
    }

    # El padre ya eligio modelo: se respeta y se anota.
    if ($antes) { Apunta (Fila 'skip' 'explicit' $null); exit 0 }
    # Escotilla manual: un prompt que empiece por ~ no se toca.
    $prompt = ''; if ($entrada.PSObject.Properties.Name -contains 'prompt') { $prompt = '' + $entrada.prompt }
    if ($prompt.StartsWith('~')) { Apunta (Fila 'skip' 'bypass' $null); exit 0 }

    $c = Clase $tipo
    if ($c -eq 'light') {
        $modelo = $MODEL_LIGHT; $why = 'light'
    } elseif ($c -eq 'think') {
        if (Presion $st) { $modelo = $MODEL_MID; $why = 'think-pressure' }
        else {
            # cuota holgada: el analisis se queda con el modelo del padre
            Apunta (Fila 'skip' 'think-comfort' $null); exit 0
        }
    } else {
        $modelo = $MODEL_MID; $why = 'work'
    }

    # copia profunda del objeto COMPLETO, con el modelo impuesto encima
    $nuevo = ($entrada | ConvertTo-Json -Depth 20 -Compress) | ConvertFrom-Json
    if ($nuevo.PSObject.Properties.Name -contains 'model') { $nuevo.model = $modelo }
    else { $nuevo | Add-Member -NotePropertyName 'model' -NotePropertyValue $modelo }

    Apunta (Fila 'route' $why $modelo)

    $respuesta = @{ hookSpecificOutput = [ordered]@{
        hookEventName = 'PreToolUse'
        updatedInput  = $nuevo
    } } | ConvertTo-Json -Depth 20 -Compress
    [Console]::Out.Write($respuesta)
} catch { }
exit 0
