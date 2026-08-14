# Etapa 0 del ruteo inteligente — el experimento de 10 minutos

Plan completo: `docs/ruteo-inteligente.md` §11. Esta carpeta es SOLO el
experimento; no es código de la app y no toca nada de MichiClaude.

## Qué se está probando (y por qué importa)

De esta única pregunta cuelga el **Hook B**, el ahorrador silencioso de
subagentes — la pieza con mejor relación valor/esfuerzo de todo el plan:

> ¿Un hook `PreToolUse` puede REESCRIBIR el modelo con el que nace un
> subagente, devolviendo `hookSpecificOutput.updatedInput`?

La documentación dice que sí desde Claude Code v2.0.10 (§10.1 del
diseño). Esto lo COMPRUEBA en la máquina real, contra el transcript del
subagente — no contra lo que el subagente diga de sí mismo.

**Regla de honestidad:** un modelo preguntado "¿qué modelo eres?"
responde mal a menudo. La verdad está en el `.jsonl`, que lo escribe
Claude Code, no el modelo. Por eso el veredicto lo da `verificar.ps1`.

## Las piezas

| Archivo | Para qué |
|---|---|
| `hook-model-test.ps1` | El hook (Windows nativo, PowerShell) |
| `hook-model-test.py` | El mismo hook (Linux / WSL / macOS) |
| `instalar-hook.ps1` / `quitar-hook.ps1` | Ponerlo y quitarlo sin editar JSON a mano |
| `verificar.ps1` / `verificar.py` | El veredicto: con qué modelo corrió de verdad |

**Los `.ps1` van en ASCII PURO, sin tildes ni rayas largas.** Windows
PowerShell 5.1 lee los archivos sin BOM como ANSI: un `—` se convierte
en `â€"` y ese `"` de la basura CIERRA la cadena a media línea, así que
el script no compila. Mordió el 2026-08-14 con los cuatro scripts a la
vez. Vale igual para el Hook B de verdad cuando se embeba en el exe.

El hook **solo actúa si ve la marca `RUTEO-TEST`** en el input del
subagente. Sin la marca sale sin decir nada, así que puedes dejarlo
puesto mientras trabajas normal y no te va a estorbar. Ante cualquier
error también sale callado (principio de "fallo silencioso" del diseño).

Todo lo que el hook recibe y responde queda en
`%USERPROFILE%\.michiclaude\ruteo-etapa0.log` (en Linux,
`~/.michiclaude/ruteo-etapa0.log`).

## Los 10 minutos, paso a paso (Windows)

### 1. Traer los archivos

```powershell
cd C:\Users\oscar\Claude\MichiClaude
git pull
```

### 2. Registrar el hook

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\ruteo-etapa0\instalar-hook.ps1
```

Escribe en `.claude\settings.local.json` (local, git lo ignora): hace
respaldo antes de tocar, imprime los hooks que ya tenías y respeta, no
duplica si ya estaba, y añade el suyo al final.

**El menú `/hooks` de Claude Code es de SOLO LECTURA** (verificado en
v2.1.232: *"To add or modify hooks, edit settings.json directly"*).
Sirve para comprobar que quedó registrado, no para instalarlo.

<details>
<summary>Alternativa: a mano en <code>.claude\settings.local.json</code></summary>

Ese archivo YA EXISTE y tiene tus permisos dentro. Hay que FUSIONAR
esta clave, no reemplazar el archivo:

```json
"hooks": {
  "PreToolUse": [
    {
      "matcher": "Task|Agent",
      "hooks": [
        {
          "type": "command",
          "command": "powershell -NoProfile -ExecutionPolicy Bypass -File \"C:\\Users\\oscar\\Claude\\MichiClaude\\scripts\\ruteo-etapa0\\hook-model-test.ps1\"",
          "timeout": 10
        }
      ]
    }
  ]
}
```
</details>

### 3. Reiniciar Claude Code

**Imprescindible.** Claude Code fotografía los hooks al arrancar; un
hook añadido a media sesión no corre. Salir y volver a entrar.

Para confirmar que quedó registrado: `/hooks` debe listarlo.

### 4. Ponerse en un modelo que NO sea Haiku

```
/model sonnet
```

Si la sesión ya está en Haiku, el experimento no demuestra nada: el
subagente nacería en Haiku con hook o sin él. Con Sonnet, "haiku en el
transcript" solo puede venir del hook.

TRAMPA: `.claude\settings.json` puede tener el modelo CLAVADO
(*"pins Haiku 4.5 — that applies on restart"*). `/model` cambia la
sesión de YA, pero al reiniciar vuelve el clavado. Por eso el orden es:
hook instalado → reiniciar → `/model` → prueba. Reiniciar DESPUÉS del
`/model` deshace el cambio.

### 5. Lanzar el subagente de prueba

En esa misma sesión, pedir literalmente:

```
RUTEO-TEST: lanza un subagente con la herramienta Task, subagent_type
general-purpose, y que el prompt del subagente empiece exactamente con
"RUTEO-TEST:" y solo diga en una línea qué modelo cree ser. No uses
ninguna otra herramienta.
```

Si Claude reformula el prompt y se pierde la marca, el hook no actúa
(lo dirá el log). Insistir en que la marca vaya literal.

### 6. El veredicto

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\ruteo-etapa0\verificar.ps1
```

Lee el transcript de subagente más reciente y dice qué modelo corrió,
más las últimas líneas del log del hook. Solo lee archivos.

## Cómo se lee el resultado

| Qué ves | Qué significa | Qué sigue |
|---|---|---|
| El `agent-*.jsonl` dice `claude-haiku-*` y tu sesión es Opus | **ÉXITO.** La apuesta técnica se sostiene | Etapa 1: `router_state.json` |
| El log tiene `ENTRA`/`SALE` pero el jsonl sigue en Opus | El hook corre, pero `updatedInput` no manda sobre el modelo | Repetir el paso 4 con la marca `RUTEO-TEST-ALLOW` (variante B: añade `permissionDecision: allow`). Si tampoco → **plan B** |
| El log no existe o está vacío | El hook ni se disparó | Revisar matcher (`Task` vs `Agent`), la ruta del `.ps1`, y que SÍ reiniciaste |
| Claude Code se queja del JSON del hook | Salida mal formada | El log trae la línea `SALE:` exacta que se emitió |

**Plan B (documentado, no improvisado):** si `updatedInput` no impone el
modelo, el diseño cae a la configuración estática soportada —
frontmatter `model:` en `.claude/agents/*.md` y la variable
`CLAUDE_CODE_SUBAGENT_MODEL` — sugerida desde el gatito. Es más pobre
(no sabe de cuota), pero es soportada y no depende de esta ventana.
Ver §10.1 del diseño.

## Cómo quitarlo todo

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\ruteo-etapa0\quitar-hook.ps1
```

Quita SOLO la entrada del experimento; si tenías otros hooks, se
quedan. Después, reiniciar Claude Code. El log se borra aparte si
molesta: `Remove-Item $env:USERPROFILE\.michiclaude\ruteo-etapa0.log`

No queda nada más: el hook no escribe en ningún otro sitio.

## Resultado en el VPS (2026-08-14) — ÉXITO

Corrido por SSH en el VPS (Claude Code 2.1.231), sesión padre en Sonnet,
subagente `general-purpose`, A/B con 27 s de diferencia:

| Corrida | Modelo real en el `agent-*.jsonl` |
|---|---|
| Con marca (el hook actúa) | **`claude-haiku-4-5-20251001`** |
| Control sin marca (el hook calla) | `claude-sonnet-5` (hereda del padre) |

Tres cosas que el log enseñó, y que valen para el Hook B de verdad:

1. **El nombre de la herramienta no es estable**: aquí llegó como
   `Agent`, no `Task`. El matcher doble `Task|Agent` es obligatorio.
2. **El input NO trae `model`** (`antes=(no venía)`): `updatedInput`
   también AÑADE campos, no solo los reescribe. Y traía
   `run_in_background` — por eso se devuelve el objeto completo.
3. **La variante A basta**: sin `permissionDecision: allow`. El plan B
   no hizo falta.

Falta la corrida en **Windows nativo** (el `.ps1`). WSL es el mismo caso
que el VPS: Linux, mismo `hook-model-test.py`, misma mecánica.

## Estado de verificación de estos scripts

- `hook-model-test.py`: probado en el VPS con las 4 entradas que
  importan — sin marca (no hace nada), con marca (devuelve el input
  COMPLETO con `model: haiku`), variante ALLOW, y basura en la entrada
  (sale con 0 y sin salida). ✅
- `hook-model-test.ps1`: es la traducción literal del anterior; **en el
  VPS no hay PowerShell para probarlo**, así que su primera corrida real
  es la de Oscar. Si algo falla, el log dirá exactamente dónde.
