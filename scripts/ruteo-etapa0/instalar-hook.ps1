# ETAPA 0 — instalar el hook de prueba en los ajustes LOCALES del proyecto.
#
# Qué toca: SOLO <repo>\.claude\settings.local.json (archivo local, que
# git ignora). Hace copia de respaldo antes de escribir y se niega a
# seguir si ya tenías hooks configurados, para no pisártelos.
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

# 2) si ya tenías hooks, NO los piso: mejor parar y avisar
if ($ajustes.PSObject.Properties.Name -contains 'hooks') {
    Write-Host ''
    Write-Host 'ALTO: ya tienes una clave "hooks" en ese archivo.' -ForegroundColor Yellow
    Write-Host 'No la piso. Anade el hook a mano con el comando /hooks de Claude Code'
    Write-Host '(matcher Task|Agent) o quita esa clave antes de correr esto.'
    exit 1
}

# 3) el bloque del experimento
$comando = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$hookPs`""
$bloque  = @{
    PreToolUse = @(
        @{
            matcher = 'Task|Agent'
            hooks   = @(
                @{ type = 'command'; command = $comando; timeout = 10 }
            )
        }
    )
}
$ajustes | Add-Member -NotePropertyName 'hooks' -NotePropertyValue $bloque -Force

# 4) escribir SIN BOM (un BOM al principio rompe la lectura del JSON)
$texto = $ajustes | ConvertTo-Json -Depth 32
[System.IO.File]::WriteAllText($ruta, $texto, (New-Object System.Text.UTF8Encoding($false)))

Write-Host ''
Write-Host 'Hook instalado en:' -ForegroundColor Green
Write-Host "  $ruta"
Write-Host "  comando: $comando"
Write-Host ''
Write-Host 'AHORA REINICIA CLAUDE CODE.' -ForegroundColor Yellow
Write-Host 'Los hooks se leen al arrancar; uno anadido a media sesion no corre.'
