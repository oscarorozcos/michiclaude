# ETAPA 0 DEL RUTEO INTELIGENTE - hook de juguete (Windows nativo).
# Ver scripts/ruteo-etapa0/README.md y docs/ruteo-inteligente.md sec.11.
#
# Que hace: se engancha a PreToolUse sobre la herramienta de subagentes
# (Task/Agent) y REESCRIBE su input para imponer el modelo `haiku`.
# Solo actua si el input trae la marca RUTEO-TEST, asi que jamas estorba
# trabajo real. Todo lo que ve y lo que responde queda en el log.
#
# Reglas que respeta (docs/ruteo-inteligente.md sec.10.1 y sec.5 "Principios"):
# - Devuelve el objeto de input COMPLETO, no el campo suelto.
# - Fallo silencioso: ante cualquier error sale con 0 y sin salida.

$ErrorActionPreference = 'Stop'

$MARCA       = 'RUTEO-TEST'        # sin ella, el hook no toca nada
$MARCA_ALLOW = 'RUTEO-TEST-ALLOW'  # variante B: anade permissionDecision
$MODELO      = 'haiku'

$log = Join-Path $env:USERPROFILE '.michiclaude\ruteo-etapa0.log'

function Apunta($titulo, $texto) {
    try {
        $dir = Split-Path $log -Parent
        if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
        $marca = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
        Add-Content -Path $log -Value "[$marca] ${titulo}: $texto" -Encoding UTF8
    } catch { }
}

try {
    $crudo = [Console]::In.ReadToEnd()
    Apunta 'ENTRA' $crudo

    try { $evento = $crudo | ConvertFrom-Json }
    catch { Apunta 'SALE' 'no es JSON - no hago nada'; exit 0 }

    $entrada = $evento.tool_input
    if ($null -eq $entrada) { Apunta 'SALE' 'sin tool_input - no hago nada'; exit 0 }

    $texto = $entrada | ConvertTo-Json -Depth 20 -Compress
    if ($texto -notmatch [regex]::Escape($MARCA)) {
        Apunta 'SALE' "sin la marca $MARCA - no hago nada"; exit 0
    }

    # copia profunda del objeto COMPLETO, con el modelo impuesto encima
    $nuevo = $texto | ConvertFrom-Json
    $antes = '(no venia)'
    if ($nuevo.PSObject.Properties.Name -contains 'model') {
        $antes = $nuevo.model
        $nuevo.model = $MODELO
    } else {
        $nuevo | Add-Member -NotePropertyName 'model' -NotePropertyValue $MODELO
    }

    $interior = [ordered]@{
        hookEventName = 'PreToolUse'
        updatedInput  = $nuevo
    }
    if ($texto -match [regex]::Escape($MARCA_ALLOW)) {
        $interior['permissionDecision']       = 'allow'
        $interior['permissionDecisionReason'] = 'ruteo etapa 0 (variante B)'
    }

    $respuesta = @{ hookSpecificOutput = $interior } | ConvertTo-Json -Depth 20 -Compress
    Apunta 'MODELO' "antes=$antes despues=$MODELO"
    Apunta 'SALE' $respuesta
    [Console]::Out.Write($respuesta)
} catch {
    Apunta 'ERROR' $_.Exception.Message   # fallo silencioso, pase lo que pase
}
exit 0
