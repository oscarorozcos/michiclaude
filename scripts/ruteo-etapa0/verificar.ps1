# ETAPA 0 - el veredicto: con que modelo CORRIO de verdad el subagente?
# Solo LEE archivos. No cambia nada.
#
# Mira el transcript mas reciente de subagente
# (~\.claude\projects\<proyecto>\<sesion>\subagents\agent-*.jsonl) y saca
# el campo `model` de cada respuesta del asistente. Ese es el hecho: lo
# que el hook pidio esta en el log; lo que paso, aqui.

$ErrorActionPreference = 'Stop'

$raiz = Join-Path $env:USERPROFILE '.claude\projects'
if (-not (Test-Path $raiz)) { Write-Host "No encuentro $raiz"; exit 1 }

$transcripts = Get-ChildItem -Path $raiz -Recurse -Filter 'agent-*.jsonl' -ErrorAction SilentlyContinue |
               Sort-Object LastWriteTime -Descending

if (-not $transcripts) { Write-Host 'No hay transcripts de subagente todavia.'; exit 0 }

foreach ($t in ($transcripts | Select-Object -First 3)) {
    Write-Host ''
    Write-Host "=== $($t.Name)   ($($t.LastWriteTime))" -ForegroundColor Cyan

    $meta = $t.FullName -replace '\.jsonl$', '.meta.json'
    if (Test-Path $meta) {
        $m = Get-Content $meta -Raw | ConvertFrom-Json
        Write-Host "    tipo de agente: $($m.agentType)"
    }

    $modelos = Select-String -Path $t.FullName -Pattern '"model":"([^"]+)"' -AllMatches |
               ForEach-Object { $_.Matches } |
               ForEach-Object { $_.Groups[1].Value } |
               Group-Object | Sort-Object Count -Descending

    if (-not $modelos) { Write-Host '    (sin campo model)'; continue }
    foreach ($mo in $modelos) { Write-Host "    modelo: $($mo.Name)   ($($mo.Count) mensajes)" }
}

Write-Host ''
Write-Host 'Log del hook:' -ForegroundColor Yellow
$log = Join-Path $env:USERPROFILE '.michiclaude\ruteo-etapa0.log'
if (Test-Path $log) { Get-Content $log -Tail 20 } else { Write-Host "  (no existe $log - el hook nunca corrio)" }
