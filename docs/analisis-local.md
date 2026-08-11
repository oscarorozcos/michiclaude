# Análisis local (IA) — la insignia inteligente de /clear vs /compact

> Diseño vivo. LEERLO antes de tocar `ai_intent`, la evidencia del hit
> `press` o la insignia de la tarjeta de intención. La investigación de
> modelos y los benchmarks que sostienen cada decisión están en
> `modelos-locales-cpu.md` (mismo directorio). Aprobado por Oscar el
> 2026-08-11: empezar por ESTE caso, probar y pulir antes de abrir otros.

## La idea en una frase

Cuando la tarjeta de intención sale sin insignia (veredicto `unsure`), un
modelo local chico lee el TÍTULO de la sesión y los ÚLTIMOS mensajes del
usuario y sugiere `/clear` o `/compact` — una insignia más en la tarjeta,
distinta de la determinista, y nada más que eso.

## Reglas duras (no negociables)

1. **El modelo jamás sustituye una compuerta.** El auto-/clear sigue
   exigiendo veredicto `Boundary` DETERMINISTA (TodoWrite cerrado o commit
   limpio). Un "tema_nuevo" del modelo NO dispara nada: pinta una insignia
   en la tarjeta MANUAL y ahí se acaba su poder.
2. **Solo entra en `unsure`.** Si `intentVerdict()` ya decidió con hechos
   (todos abiertos, lista al 100%, commit limpio, cont≥40), el modelo ni se
   invoca. Los hechos le ganan a las inferencias, siempre.
3. **Sesgo asimétrico cocido en dos capas**: en el prompt ("en la duda,
   compact o unsure — nunca clear") y en el código (la insignia de `/clear`
   solo se pinta con razón `tema_nuevo`; todo lo demás cae a `/compact` o a
   nada). Un /clear mal sugerido cuesta una conversación; un /compact de
   más cuesta tokens.
4. **Fail-quiet total.** Sin modelo, apagado, sin evidencia (exportador
   viejo), timeout o salida ilegible → la tarjeta queda EXACTAMENTE como
   hoy. El análisis solo puede sumar; ningún camino puede restarle nada a
   lo que ya funciona.
5. **Bajo demanda, nunca residente.** `llama-server` arranca, contesta y SE
   MATA (guard con Drop: también si el comando falla a medias). La app pesa
   276 MB y eso es parte del producto. Una invocación por sesión como
   máximo (`aiSeen` en la tarjeta).
6. **Privacidad**: el prompt lleva título y fragmentos de mensajes — TODO
   local (`127.0.0.1`); jamás a ntfy, jamás al hub, jamás a un dominio.
   La evidencia cruda (`msgs`) NO se guarda en la tarjeta persistida:
   se usa al analizar y se suelta; en localStorage solo queda el veredicto
   `{rec, reason}`.
7. **Salida = enum cerrado por gramática GBNF** (lección central de la
   investigación: el modelo obedece la forma; la precisión depende de qué
   tan cerrada esté la tarea). El frontend traduce códigos con `t()`, como
   todo (invariante #10). Nada de texto libre del modelo en la UI.
8. **La insignia dice su origen.** "Recomendado" (determinista, un hecho) y
   "Análisis local" (inferencia) se ven DISTINTAS. La confianza del
   producto vale más que aparentar seguridad.

## Por qué el modelo NO puede "leer la conversación"

La sesión típica que dispara la tarjeta lleva 130-160k tokens y el prefill
medido en CPU es ~53 tok/s: leerla entera tomaría ~40 minutos, y el
hallazgo MRCR de la investigación dice que un modelo chico tampoco
recuperaría bien de un contexto así. La pregunta clave de la tarjeta —"¿lo
que sigue necesita lo ya hablado?"— se responde comparando LO RECIENTE
contra EL TEMA: por eso la evidencia es chica a propósito (~500-800 tokens
→ 10-20 s).

## La evidencia (ampliación aditiva del hit `press`, invariante #1)

El hit `press` gana dos cargas, replicadas en Rust y `meter-export.py`:

- `title` — campo que YA existía en CoachHit (lo usaba solo `sum`): el
  ai-title que Claude Code escribe en su propio log. Es el ancla del tema.
- `msgs` — NUEVO (`#[serde(default)]` / exportador viejo no lo manda y el
  panel simplemente no analiza): los últimos 3 mensajes HUMANOS del
  usuario, cada uno truncado a 300 caracteres. El filtro de "humano" es
  `user_turn_text()` — la MISMA lógica que `is_user_turn()` de los turnos
  útiles (se refactorizó para devolver el texto; el bool la envuelve:
  una sola implementación, cero divergencia).

La evidencia viaja por el mismo SSH que todo lo demás, así que las
sesiones del VPS se analizan igual: el modelo corre SIEMPRE en la máquina
del panel; el motor remoto solo aporta hechos (allá no hay llama.cpp ni
puede haberlo — el exportador es stdlib puro).

## La escalera (qué corre y cuándo)

```
1. intentVerdict()  determinista   —  decide casi siempre  (hoy, sin cambios)
2. [etapa 2] embeddings ~120 MB    —  similitud tema↔reciente, milisegundos
3. Qwen3.5-2B con gramática        —  solo la zona gris,   10-20 s
4. nada decidió                    —  tarjeta genérica de hoy
```

**La v1 implementa 1→3→4** (sin embeddings): Oscar ya tiene llama-server y
el GGUF instalados — cero descargas para empezar a probar LO QUE DECIDE si
esto sirve: la calidad del veredicto. Los embeddings son un atajo de
velocidad y entran en la etapa 2, cuando el veredicto haya demostrado
valer; añadirlos no cambia ninguna interfaz (misma salida, mismo sitio).

## Piezas

**`ai_config.json`** (junto a los demás json de la app):
`{enabled, server, model, port}` — `server` vacío = `llama-server` del
PATH; `port` default 8791, solo `127.0.0.1`. Comandos `ai_get_config` /
`ai_set_config` (el panel no toca el disco).

**`ai_intent` (Rust, async — invariante 10ter):**
1. Config y validaciones → `ERR_AI_OFF` / `ERR_AI_MODEL`.
2. Arranca `llama-server -m <gguf> --port <p> -c 2048 -t 4 -ngl 0
   --no-mmap --reasoning-budget 0 --temp 0` con CREATE_NO_WINDOW y guard
   kill-on-drop → `ERR_AI_START`.
3. Sondea `/health` hasta 45 s (la carga fría del GGUF tarda) →
   `ERR_AI_TIMEOUT`.
4. POST `/v1/chat/completions` con el prompt y `grammar` GBNF; 60 s →
   `ERR_AI_TIMEOUT`.
5. Parsea el enum; cualquier otra cosa → `ERR_AI_BADOUT`. Mata el server.

Prompt (inglés — el 2B instruye mejor en inglés; la evidencia va verbatim
en su idioma) con las reglas del sesgo. Gramática:

```
root ::= "{\"rec\":\"" rec "\",\"reason\":\"" reason "\"}"
rec ::= "clear" | "compact" | "unsure"
reason ::= "tema_nuevo" | "tema_cruzado" | "tarea_viva" | "cierre" | "na"
```

**Panel:** en `coachPoll`, al sintetizar la tarjeta de intención: si el
veredicto es `unsure`, hay `msgs` y el análisis está encendido →
`ai_intent` en segundo plano; al volver, `{rec, reason}` se guarda EN la
tarjeta (`c.ai`) y se repinta. La insignia "Análisis local" sale solo si
el veredicto determinista sigue `unsure` al pintar (si entre tanto llegó
un hecho, el hecho manda). Una sola invocación por sesión aunque falle
(el fallo también se anota — reintentar cada sondeo sería arrancar un
server de 1.3 GB en bucle).

**Ajustes:** interruptor "Análisis local (IA)" (nace APAGADO) + botón
**Descargar** (ver abajo) + rutas de llama-server y del .gguf como ajuste
avanzado + botón **Probar** con evidencia de ejemplo, que enseña veredicto
o error traducido. La prueba es la misma tubería real (`ai_intent`), no un
camino aparte.

## Descarga guiada (v1.1 — mismo día, pedida por Oscar)

Un usuario nuevo no tiene llama.cpp ni el modelo, y pedirle rutas era
pedirle que se complicara. Al encender el interruptor, si FALTA algo
(`ai_setup_status` cuenta también las rutas manuales: quien ya los tiene
no ve el botón), aparece **Descargar** con el tamaño real y una nota que
dice de dónde viene. `ai_setup`:

1. llama.cpp (~17 MB): zip del release de GitHub → SHA-256 verificada →
   `Expand-Archive` (PowerShell del sistema; misma decisión que la etapa 2
   de remediación: nada de deps nuevas) → se busca `llama-server.exe`
   donde haya caído (el zip cambia de forma entre builds).
2. El modelo (~1.3 GB): GGUF directo de Hugging Face → a `.part` →
   SHA-256 verificada → rename. Sin resume: media descarga corrupta se
   borra y se rehace (la verificación es por huella completa).
3. Rellena `ai_config.json` (respetando rutas manuales que ya funcionen)
   y enciende el análisis.

REGLAS: URLs y huellas son CUATRO CONSTANTES en el binario (la regla del
updater: jamás salen de algo descargado; actualizar juntas). Progreso por
eventos `ai:dl` al panel. Idempotente: reintentar tras un fallo baja solo
lo que falte. Es la ÚNICA conexión de la app que no va a api.anthropic.com
— GitHub y Hugging Face, una vez, opt-in y anunciada en la propia UI.

## Errores (códigos, invariante #10)

`ERR_AI_OFF` apagado · `ERR_AI_MODEL` falta la ruta del modelo ·
`ERR_AI_START` no arrancó llama-server · `ERR_AI_TIMEOUT` no respondió a
tiempo · `ERR_AI_BADOUT` salida ilegible. Todos → fail-quiet en la
tarjeta; solo el botón Probar los enseña.

## Cómo se prueba (v1)

1. Ajustes → encender, poner la ruta del .gguf → **Probar**: debe salir un
   veredicto en segundos (arranque frío ~10-20 s la primera vez).
2. Simulador 💡 (dev): fuerza la bombilla, no la tarjeta — para la tarjeta
   real hace falta una sesión ≥80% con veredicto unsure, que es la prueba
   en vivo de verdad.
3. Lo que decide si esto se queda: ¿la insignia acierta en tus sesiones
   reales? Anotar aciertos/fallos unos días antes de construir la etapa 2.

## Etapa 2 (después de validar la v1)

- ESPEJO en GitHub Releases del repo (cuando sea público, idea de Oscar
  2026-08-11): release `modelos-v1` con el GGUF, el zip de llama.cpp y el
  futuro modelo de embeddings — Apache 2.0 y MIT permiten redistribuir con
  su licencia adjunta, y los assets aguantan hasta 2 GB. En el código, URL
  de RESPALDO por constante (HF primero, espejo si falla); la MISMA SHA
  valida ambos: la autoridad es la huella, no el servidor. Hoy el riesgo
  es solo de alta (usuarios nuevos): los existentes corren sin internet.
- Embeddings (multilingual-e5-small GGUF, ~120 MB) como peldaño previo:
  similitud coseno título+viejo ↔ reciente; <0.45 = tema_nuevo, >0.65 =
  tema_cruzado, y el 2B queda solo para la banda media. (La descarga de
  modelos se adelantó a la v1.1.)
- Decidir si el veredicto del modelo puede DEGRADAR una recomendación
  determinista (freno, nunca acelerador): un "tarea_viva" del modelo sobre
  un Boundary… hoy NO — primero medir cuánto acierta.

## Lo descartado (para no rediscutir)

- **Crate `ort`/ONNX embebido**: 3.4x más lento en CPU y ~15 deps de
  Python para el tooling; llama-server es un binario y ya está medido.
- **Modelo residente**: 1.5-2 GB de RAM permanentes matan el pitch del
  widget ligero.
- **Que el modelo redacte texto de UI**: 8 idiomas vía `t()` y el español
  libre del 2B trae errores visibles. Enums, siempre.
- **Modelo en el VPS**: el exportador es stdlib puro y así se queda
  (invariante #1). Los hechos viajan, el modelo no.
