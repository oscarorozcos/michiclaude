# ETAPA 0 — quitar el hook de prueba y dejar todo como estaba.
#
# Qué toca: SOLO <repo>\.claude\settings.local.json. Borra la clave
# "hooks" (la que puso instalar-hook.ps1) y respeta el resto del archivo.
# Deja tambien un respaldo por si acaso.
#
# Uso:  powershell -ExecutionPolicy Bypass -File .\scripts\ruteo-etapa0\quitar-hook.ps1

$ErrorActionPreference = 'Stop'

$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$ruta = Join-Path $repo '.claude\settings.local.json'

if (-not (Test-Path $ruta)) { Write-Host "No existe $ruta — nada que quitar."; exit 0 }

$respaldo = "$ruta.bak-" + (Get-Date -Format 'yyyyMMdd-HHmmss')
Copy-Item $ruta $respaldo
Write-Host "Respaldo: $respaldo" -ForegroundColor DarkGray

$ajustes = Get-Content $ruta -Raw | ConvertFrom-Json
if ($ajustes.PSObject.Properties.Name -notcontains 'hooks') {
    Write-Host 'No habia clave "hooks". Todo igual.'; exit 0
}

$ajustes.PSObject.Properties.Remove('hooks')
$texto = $ajustes | ConvertTo-Json -Depth 32
[System.IO.File]::WriteAllText($ruta, $texto, (New-Object System.Text.UTF8Encoding($false)))

Write-Host ''
Write-Host 'Hook quitado.' -ForegroundColor Green
Write-Host 'Reinicia Claude Code para que deje de cargarlo.' -ForegroundColor Yellow
Write-Host ''
Write-Host 'Si quieres borrar tambien el rastro:'
Write-Host '  Remove-Item $env:USERPROFILE\.michiclaude\ruteo-etapa0.log'
