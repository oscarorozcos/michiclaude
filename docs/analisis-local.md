# Análisis local (IA) — la insignia inteligente de /clear vs /compact

> Diseño vivo. LEERLO antes de tocar `ai_intent`, la evidencia del hit
> `press` o la insignia de la tarjeta de intención. La investigación de
> modelos y los benchmarks que sostienen cada decisión están en
> `~/.michiclaude/modelos-locales-cpu.md` (FUERA del repo, como las notas
> de negocio: trae contexto de otro proyecto de Oscar). Aprobado el
> 2026-08-11: empezar por ESTE caso, probar y pulir antes de abrir otros.

## La idea en una frase

Cuando la tarjeta de intención sale sin insignia (veredicto `unsure`), un
modelo local chico lee el TÍTULO de la sesión y los ÚLTIMOS mensajes del
usuario y sugiere `/clear` o `/compact` — una insignia más en la tarjeta,
distinta de la determinista, y nada más que eso.

## Reglas duras (no negociables)

1. **El modelo jamás sustituye una compuerta** (REVISADA 2026-08-12, ver
   §"El automático por inferencia"). En la v1 el modelo solo pintaba una
   insignia en la tarjeta MANUAL. Ahora puede además disparar el
   auto-/clear, pero NO sustituyendo la compuerta `Boundary`: es un camino
   PARALELO con interruptor propio (`relayClearAi`, nace APAGADO) que
   mantiene TODO lo demás — red del /export verificada, 3 manuales, relevo
   v≥2, widget a la vista, una vez por sesión, cuenta atrás cancelable — y
   encima añade dos exigencias suyas. Lo que NO cambia: **un hecho nunca se
   sobreescribe con una opinión**.
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

**La v1 implementó 1→3→4**; desde 2026-08-13 la escalera está COMPLETA
(1→2→3→4): el peldaño 2 entró tras validar la v1 en vivo (5/5 aciertos) —
ver §"Etapa 2 — HECHA". Añadirlo no cambió ninguna interfaz: misma
salida, mismo sitio, y sin el GGUF de embeddings todo se comporta
exactamente como la v1.

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
4. POST `/v1/chat/completions` con el prompt y `response_format`; 60 s →
   `ERR_AI_TIMEOUT`.
5. Parsea el enum; cualquier otra cosa → `ERR_AI_BADOUT`. Mata el server.

Prompt (inglés — el 2B instruye mejor en inglés; la evidencia va verbatim
en su idioma) con las reglas del sesgo. La forma se FUERZA con esquema:

```json
"response_format": {"type": "json_object", "schema": {
  "rec":    {"enum": ["clear", "compact", "unsure"]},
  "reason": {"enum": ["tema_nuevo","tema_cruzado","tarea_viva","cierre","na"]}}}
```

**TRAMPA (costó la primera prueba real, 2026-08-12):** `grammar` (GBNF)
solo lo acepta el endpoint NATIVO `/completion`. En el de chat se IGNORA
en silencio — el modelo contesta en prosa y el parseo muere con
`ERR_AI_BADOUT`. La vía correcta ahí es `response_format` con esquema, que
llama-server convierte él mismo a gramática: los `enum` se cumplen al
MUESTREAR, no al validar. El parseo además es tolerante (mira
`reasoning_content` si `content` viene vacío y recorta al primer `{...}`).

**SEGUNDA TRAMPA, del mismo día:** Qwen3.5 **razona por defecto**, y el
`--reasoning-budget 0` de la línea de comandos es solo un default que la
plantilla de chat pisa. Resultado: el modelo gastó su presupuesto de
tokens en "Thinking Process:", dejó `content` VACÍO y devolvió
`finish_reason: length`. Se apaga en la PETICIÓN con
`"chat_template_kwargs": {"enable_thinking": false}`, y el prompt termina
en `/no_think` como cinturón (las dos vías están en
la investigación §3 — estaban escritas ANTES de que costaran una
ronda). Ojo también: la gramática del `response_format` solo restringe el
canal `content`; lo que el modelo escriba razonando NO pasa por ella.

**Rastro `ai_debug.txt`** (carpeta de datos de la app, se SOBRESCRIBE): la
petición y la respuesta cruda del último intento. Misma familia que
quota_debug.json y wrap_debug.txt — un fallo que solo dice "no se pudo
leer" obliga a adivinar. Contiene la evidencia del prompt: es local, como
todo lo demás de esta función.

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

## Cuándo aparece la tarjeta (lo que se pregunta cuando "no llega")

El detonante NO es cambiar de tema: es **presión ≥80% del techo de ESA
sesión** (`INTENT_PCT`), con la sesión VIVA — el hit `press` exige tocada
hace <10 min (`PRESS_QUIET_MAX`), así que dejarla quieta no la saca; al
revés, la apaga. Una sesión nueva de prueba anda por el 1-2% y con techo
de 1M harían falta ~800k tokens. Tampoco es el tope diario: la tarjeta de
intención está EXENTA. Y sale una por sesión (`tipSeen`): despachada con
✕ o "Ahora no", no vuelve. El termómetro a la vista es la bombilla del
gatito — la tarjeta cae entre "se enreda" (60%) y "muerta" (85%).

## Cómo se prueba (v1)

1. Ajustes → encender, poner la ruta del .gguf → **Probar**: debe salir un
   veredicto en segundos (arranque frío ~10-20 s la primera vez).
2. Simulador 🎯 "Simular intención" (dev): crea la tarjeta con veredicto
   `unsure` y corre el ai_intent DE VERDAD sobre la evidencia de tu sesión
   activa (la más fresca con `msgs`; si no hay, una de ejemplo). Es la
   única forma de ver la insignia sin esperar días a que una sesión llegue
   al 80%. Vive en memoria (como el de hallazgos): el veredicto se cuelga
   del hit (`_ai`) porque en modo simulación las tarjetas se rehacen en
   cada render. El 💡 es otra cosa: fuerza la bombilla, no la tarjeta.
   REGLA del simulador: las señales deterministas van NEUTRAS (topen,
   ttotal, cont, gclean en cero) — copiar el `cont` real de una sesión de
   trabajo da "alive" y suprime la insignia del modelo, que es justo lo
   que se quiere ver (pasó en la primera prueba). La evidencia sí es real.
3. Lo que decide si esto se queda: ¿la insignia acierta en tus sesiones
   reales? Anotar aciertos/fallos unos días antes de construir la etapa 2.

## El automático por inferencia (2026-08-12, a petición de Oscar)

Oscar lo pidió explícitamente para probarlo unos días: que el `/clear` se
aplique solo cuando lo recomiende el modelo, con las reglas y la red ya
existentes. Cruza la regla #1 de la v1, así que se implementó como CAMINO
PARALELO y no como sustitución: hay DOS razones válidas para el
auto-/clear y cada una tiene su interruptor.

| | Razón (a) — HECHO | Razón (b) — INFERENCIA |
|---|---|---|
| Qué la dispara | `Boundary`: lista de tareas al 100% o commit limpio | `unsure` + el modelo dice `clear` por `tema_nuevo` |
| Interruptor | `relayClear` | `relayClearAi` (cuelga del anterior) |
| Cuenta atrás | 15 s | **30 s** |

Todo lo demás se exige IGUAL en las dos: interruptor maestro del
automático, sus 3 manuales de `/clear` ganadas a mano, relevo v≥2, widget A
LA VISTA, una vez por sesión (sellada antes de empezar), cualquier toque la
para, R1-R4 se revuelven al escribir, y la **red del `/export` verificado
en disco o no hay `/clear`** (`ERR_RELAY_EXPORT`, fail-closed). La red es
lo que hace esto defendible: un `/clear` por inferencia equivocada cuesta
una copia que sigue en disco, no la conversación.

**Las dos exigencias extra del camino (b):**

1. **`topen === 0`** — con tareas abiertas NO se aplica, nunca. El
   veredicto `unsure` ya lo implica (con tareas abiertas sería `alive`),
   pero se comprueba otra vez a propósito: defensa en profundidad, para que
   el día que alguien toque `intentVerdict` esta puerta siga cerrada.
2. **`reason === "tema_nuevo"`** — el sesgo asimétrico de la regla #3
   llevado al automático: `tema_cruzado`, `tarea_viva` y `cierre` NO
   disparan `/clear`; caen al `/compact` de siempre, que no borra.

**La cuenta atrás es el doble (30 s).** Sigue la escalera que ya usaba el
proyecto: 5 s cuando lo pides tú, 15 cuando lo decide un hecho medido, 30
cuando lo decide una inferencia. Cuanto más blanda la razón, más tiempo
para pararla.

**El automático ESPERA el veredicto** (`aiPending`). Sin esto el camino
nuevo no serviría de nada: al llegar al 80% con veredicto `unsure`, el
automático de siempre aplicaría `/compact` en el primer sondeo, antes de
que el modelo alcance a hablar. Ahora, si el camino (b) está armado y el
análisis está en marcha, el sondeo se abstiene y espera al siguiente. Está
ACOTADO: `AI_WAIT_MIN` (10 min) desde que nació la tarjeta, y un fallo del
análisis marca `aiErr` para dejar de esperar de inmediato. La presión de
contexto solo sube, así que esperar nunca empeora nada.

**La cuenta atrás DICE QUÉ va a aplicar** (2026-08-12, encontrado al
escribirle los ejemplos a Oscar): antes solo se veía el segundero, así que
la cuenta de un `/compact` y la de un `/clear` eran idénticas en pantalla —
y una resume mientras la otra BORRA. Ahora el chip lleva el comando y el
color: ÁMBAR `/compact 15`, ROJO `/clear 30`. En el gatito no caben las dos
cosas, así que mientras la cuenta corre el "Sesión X%" se aparta y la
cápsula queda dedicada a lo único que importa esos segundos. El texto
completo (`rly_auto_msg`) ya viajaba en el evento desde la etapa 3c-2 y
nadie lo pintaba. Regla hermana de la del veredicto ✓/✕: **una cuenta atrás
que no dice qué va a hacer deja al usuario adivinando igual que una que
acaba en silencio.**

**Cómo se audita la prueba de unos días.** El rastro del flujo (📜 en dev)
distingue quién lo decidió: `relevo auto: aplicado /clear por IA
(tema_nuevo)` frente a `… por hecho`. Es EL dato de la prueba: si aparece
un `por IA` donde no debía, ahí está la copia del `/export` en
`<datos>/handoff/` (90 días) y el botón "abrir la copia" en el registro de
acciones.

**Si esto sale mal**, el orden de retirada es: apagar `relayClearAi` (el
resto del automático sigue como estaba) → si el problema es el veredicto,
afinar el prompt → si es sistemático, volver a la v1 (solo insignia). El
camino (a) nunca depende del modelo.

## Espejo de modelos — HECHO (2026-08-12)

Idea de Oscar (2026-08-11), ejecutada el día que el repo se hizo público.
Release `modelos-v1` en el propio repo con el GGUF y el zip de llama.cpp
(Apache 2.0 y MIT permiten redistribuir con su licencia adjunta; los
assets aguantan hasta 2 GB). Reglas ganadas al hacerlo:

- El release va como **PRERELEASE**: `releases/latest` (el endpoint del
  updater) ignora prereleases — sin esa marca, `modelos-v1` taparía a la
  versión real de la app. Y el tag NO empieza con `v`, así el workflow de
  release no se dispara.
- En el código: `AI_LS_URL_MIRROR` / `AI_MODEL_URL_MIRROR` y `ai_fetch()`
  (fuente original primero, espejo después). La caída al espejo salta
  tanto por fallo de RED como por fallo de HUELLA: que la fuente original
  responda con OTRO archivo (lo reemplazaron río arriba) es exactamente el
  caso que el espejo cubre. La MISMA SHA valida ambas fuentes: la
  autoridad es la huella, no el servidor.
- Al cambiar de build o de modelo: actualizar las SEIS constantes juntas
  y subir las copias nuevas a un release `modelos-v2` (no reutilizar el
  viejo: un binario ya publicado no se reemplaza, misma regla que el
  updater).
- Verificado en vivo: subido con `gh release upload`, descargado de
  vuelta SIN autenticación y huella idéntica; `latest.json` siguió
  anunciando la versión vigente.

El riesgo que cubre es solo de ALTA (usuarios nuevos descargando): los
existentes corren sin internet.

## Etapa 2 — HECHA (2026-08-13; adelantada por Oscar con la v1 en 5/5)

El peldaño de embeddings quedó construido el día que el automático por
inferencia se validó de punta a punta (terminal y chat, mismo día):

- **Modelo:** `multilingual-e5-small-q8_0.gguf` (~126 MB, de
  cstr/multilingual-e5-small-GGUF). Subido al MISMO release-estante
  `modelos-v1` como asset NUEVO (aditivo: nada publicado se reemplazó, la
  regla del modelos-v2 es para REEMPLAZOS) y verificado igual que los
  otros: descarga anónima de vuelta, huella idéntica. Las constantes de la
  descarga guiada ahora son NUEVE (`AI_EMB_URL/_MIRROR/_SHA`) y siguen
  actualizándose juntas.
- **Dónde corre:** `ai_emb_verdict()` DENTRO de `ai_intent_impl`, ANTES de
  arrancar el 2B — mismo llama-server con `--embeddings --pooling mean
  -c 512`, puerto del 2B +1, guard kill-on-drop. e5 exige el prefijo
  `query: ` en ambos lados (tarea simétrica) — sin él la similitud se
  degrada en silencio.
- **La comparación del diseño:** TEMA = título + mensajes viejos; RECIENTE
  = el último mensaje. Coseno: `<0.45` → clear·tema_nuevo, `>0.65` →
  compact·tema_cruzado, banda media → el 2B como en la v1. Los umbrales
  son constantes (`EMB_NEW`/`EMB_CROSS`) A PROPÓSITO: se afinan con la
  muestra de uso natural, no en caliente.
- **Fail-quiet en cadena (regla #4 intacta):** sin GGUF, server que no
  arranca, salida rara → `None` y decide el 2B. El peldaño solo puede
  ACELERAR (10-26 s → 1-3 s), jamás quitar lo que ya funciona.
- **Auditoría:** `AiVerdict` gana `via` ("emb"/"llm") y `sim` (aditivos,
  serde default). El flowLog los enseña ("ai: veredicto clear ·
  tema_nuevo (embeddings 0.38)") y `ai_debug.txt` guarda tema/reciente y
  la similitud — no los vectores. La tarjeta persistida sigue guardando
  SOLO `{rec, reason}` (privacidad, regla #6).
- **Descarga:** `ai_setup` baja el tercer archivo solo si falta
  (idempotente); con la v1 ya instalada el botón dice "Descargar el
  modelo rápido (~126 MB)" (`ai_dl_emb`). Rutas manuales: `ai_config.emb`
  (vacía = la descargada).
- PENDIENTE de la etapa 2: verla decidir en vivo (primer `via:emb` real)
  y revisar umbrales tras unos días de muestra.

## Etapa 3 (algún día, sin fecha)

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
