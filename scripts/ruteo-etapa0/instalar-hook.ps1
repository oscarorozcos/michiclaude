# ETAPA 0 — instalar el hook de prueba en los ajustes LOCALES del proyecto.
#
# Qué toca: SOLO <repo>\.claude\settings.local.json (archivo local, que
# git ignora). Hace copia de respaldo antes de escribir y SE SUMA a los
# hooks que ya tengas — no pisa nada.
#
# Uso:  powershell -ExecutionPolicy Bypass -File .\scripts\ruteo-etapa0\instalar-hook.ps1

$ErrorActionPreference = 'Stop'

$repo   = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$ruta   = Join-Path $repo '.claude\settings.local.json'
$hookPs = Join-Path $PSScriptRoot 'hook-model-test.ps1'

if (-not (Test-Path $hookPs)) { Write-Host "No encuentro $hookPs" -ForegroundColor Red; exit 1 }

# 1) leer lo que ya hay (o empezar de cero)
if (Test-Path $ruta) {
    $respaldo = "$ruta.bak-" + (Get-Date -Format 'yyyyMMdd-HHmmss')
    Copy-Item $ruta $respaldo
    Write-Host "Respaldo: $respaldo" -ForegroundColor DarkGray
    $ajustes = Get-Content $ruta -Raw | ConvertFrom-Json
} else {
    $dir = Split-Path $ruta -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $ajustes = [PSCustomObject]@{}
}

# 2) el objeto hooks que ya tuvieras (o uno nuevo)
$hooks = [PSCustomObject]@{}
if (($ajustes.PSObject.Properties.Name -contains 'hooks') -and ($null -ne $ajustes.hooks)) {
    $hooks = $ajustes.hooks
}

# 3) la lista de PreToolUse que ya tuvieras
$pre = @()
if (($hooks.PSObject.Properties.Name -contains 'PreToolUse') -and ($null -ne $hooks.PreToolUse)) {
    $pre = @($hooks.PreToolUse)
}

# 4) informar de lo que ya había (transparencia: que se vea qué se respeta)
if ($pre.Count -gt 0) {
    Write-Host "Ya tenias $($pre.Count) entrada(s) de PreToolUse. Se respetan:" -ForegroundColor DarkGray
    foreach ($e in $pre) {
        foreach ($h in @($e.hooks)) {
            Write-Host "   [$($e.matcher)] $($h.command)" -ForegroundColor DarkGray
        }
    }
}

# 5) ¿ya está el nuestro? entonces no duplicar
foreach ($e in $pre) {
    foreach ($h in @($e.hooks)) {
        if ($h.command -like '*hook-model-test.ps1*') {
            Write-Host ''
            Write-Host 'El hook de la etapa 0 ya estaba instalado. No cambio nada.' -ForegroundColor Yellow
            exit 0
        }
    }
}

# 6) anadir el nuestro al final
$comando = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$hookPs`""
$entrada = [PSCustomObject]@{
    matcher = 'Task|Agent'
    hooks   = @([PSCustomObject]@{ type = 'command'; command = $comando; timeout = 10 })
}
$pre = @($pre) + $entrada

$hooks   | Add-Member -NotePropertyName 'PreToolUse' -NotePropertyValue @($pre) -Force
$ajustes | Add-Member -NotePropertyName 'hooks'      -NotePropertyValue $hooks  -Force

# 7) escribir SIN BOM (un BOM al principio rompe la lectura del JSON)
$texto = $ajustes | ConvertTo-Json -Depth 32
[System.IO.File]::WriteAllText($ruta, $texto, (New-Object System.Text.UTF8Encoding($false)))

Write-Host ''
Write-Host 'Hook de la etapa 0 anadido en:' -ForegroundColor Green
Write-Host "  $ruta"
Write-Host "  comando: $comando"
Write-Host ''
Write-Host 'AHORA REINICIA CLAUDE CODE.' -ForegroundColor Yellow
Write-Host 'Los hooks se leen al arrancar; uno anadido a media sesion no corre.'
