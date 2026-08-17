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

- **Modelo: `embeddinggemma-300M-Q8_0.gguf`** (~319 MB, GGUF OFICIAL de
  ggml-org — 500k+ descargas). NO es el e5-small del plan original, y el
  cambio se ganó en el banco: las conversiones comunitarias de e5 salieron
  ROTAS por partida doble — sin `token_type_count` NI CARGAN en llama.cpp
  moderno ("bert model needs to define token type count", cazado en el
  emb_server.log de Oscar), y la única que cargaba (keisuke) tenía el
  tokenizer dañado: "receta de carbonara" ↔ "CSS del widget" daba 0.93,
  MÁS que una subtarea del mismo proyecto (0.90) — sin separación no hay
  umbral posible (matriz pooling×prefijo completa en el banco, todas
  solapadas). Subido al release-estante `modelos-v1` como asset nuevo; el
  e5 roto se RETIRÓ del estante (ningún release de la app lo referenció
  jamás — no es "reemplazar un binario publicado"). Constantes: NUEVE,
  juntas.
- **Dónde corre:** `ai_emb_verdict()` DENTRO de `ai_intent_impl`, ANTES de
  arrancar el 2B — mismo llama-server con `--embeddings -c 1024` (SIN
  pisar el pooling: el GGUF oficial trae el suyo), puerto del 2B +1, guard
  kill-on-drop. SIN prefijos, a propósito: en el banco gemma separó MEJOR
  sin ellos (tema nuevo 0.15-0.36, continuación ~0.53, mismo tema entre
  idiomas 0.84) y esa distribución CALZA con los umbrales del diseño; el
  "task: sentence similarity | query:" de su ficha comprimía todo hacia
  la banda media. Medido con los flags exactos: 3 pares en 0.3 s ya
  cargado.
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

## Etapa 3 — TEMAS en los hallazgos `inflate` — HECHA (2026-08-17)

> El diseño de abajo se implementó ENTERO el 2026-08-17 con dos cambios
> ganados al construir y probar, que son ahora REGLAS VIGENTES:
>
> 1. **La evidencia se recoge en la pasada que ya existe**, no reabriendo
>    los .jsonl: `SessFindings.umsgs` se llena junto al detector de pegado
>    (mismo `user_turn_text`, dedup PROPIO — el `seen` compartido es de
>    otros detectores y colarse ahí los apagaría). Medido antes de
>    decidirlo (VPS, 9 días): 551 mensajes humanos = 0.17 MB. Así los TRES
>    modos comparten UN camino: local y WSL llenan `umsgs`/`crs` en el
>    escaneo, el exportador los manda por SSH con las MISMAS constantes
>    (`topic_sample` es réplica exacta) y el modelo corre SIEMPRE en la
>    máquina del panel. Ventaja extra: una sesión reanudada (que copia
>    líneas a otro archivo) se resuelve sola con el dedup por uuid.
> 2. **El ahorro usa el MÁXIMO CORRIDO del `cache_read` en las fronteras**,
>    no el valor del instante. Lo cazó la prueba con datos reales: en una
>    sesión con rupturas de caché, AÑADIR una frontera BAJABA el ahorro
>    ($38.06 → $14.23), que es imposible. Una ruptura hace caer el
>    `cache_read` aunque la conversación siga entera; lo que suelta un
>    `/clear` es todo lo anterior y eso no encoge porque el proveedor
>    reescriba su caché. Se conserva el tope por turno (`min` con su
>    propio `cache_read`): no se puede "ahorrar" más de lo que ese turno
>    leyó. Verificado con 1200 combinaciones de cortes sobre las sesiones
>    reales del VPS: monotonía y tope, 0 fallos.
>
> 3. **La capa va en una SEGUNDA pasada de fondo**, no dentro del escaneo
>    que pinta las tarjetas: arrancar el server de embeddings cuesta ~20 s
>    de carga fría y meterlo en el camino haría ESPERAR a la pestaña por
>    un extra. `loadFindings` pide los hallazgos sin temas, pinta, y
>    después `fndTopicsLater()` repite la consulta con `topics:true` y
>    repinta si trajo tramos. Con el análisis apagado —el caso normal— esa
>    segunda consulta NI SE HACE (se mira `ai_get_config` antes). Se
>    descarta el resultado si mientras tanto cambió el periodo o entró un
>    escaneo más nuevo (`fndCacheAt`). Regla de la etapa: la capa solo
>    puede SUMAR, y tardar más es restar.
>
> **Interruptor: el del análisis local, sin casilla nueva** (como decía el
> diseño). Sin ese opt-in no se embebe nada; el texto de la casilla lo dice
> ahora en los 8 idiomas. Y `topics:true` lo pide SOLO la pasada COMPLETA
> de Hallazgos: la ligera de 1 día y el Reporte no arrancan ningún modelo.
>
> **Cómo se probó** (el VPS no tiene toolchain de Rust NI el GGUF): banco
> del algoritmo portado línea por línea a Python con vectores de similitud
> conocida — 22/22 (tres temas nítidos, tema único, outlier que no corta,
> mensajes cortos que no votan, tramo corto que se funde, primer mensaje
> corto, muestreo, bordes); ahorro con fixture a mano y con datos reales;
> render del panel 22/22 incluido el ESCAPADO de las etiquetas (son texto
> del usuario). Y REGRESIÓN del exportador con ventana congelada
> (`--end`): hallazgos y `waste` IDÉNTICOS byte a byte sin las claves
> nuevas — el hallazgo determinista no se movió. FALTA: `cargo check` en
> el Windows de Oscar y la primera tarjeta con temas REALES (ahí vive el
> modelo).

### Diseño (2026-08-16), tal como se aprobó

### El problema que Oscar señaló

La tarjeta "Una conversación siguió creciendo durante 71 turnos · $2.70"
CUENTA (turnos, tokens releídos, `cr_cost`) y luego SUPONE: la ficha dice
"un /clear al cambiar de tema…" sin saber si hubo cambio de tema. Una
sesión larguísima de UN solo tema saca la misma tarjeta y ahí el consejo
correcto sería `/compact`. Oscar (2026-08-16): "me gustaría que fuera más
inteligente aparte de contar o suponer". Esta etapa hace que la tarjeta
DEMUESTRE los temas y CALCULE lo que cada frontera habría ahorrado.

Cómo se vería (mismo hallazgo, ampliado):

> Esta sesión tuvo 3 temas — bug del login (turnos 1-22) · README
> (23-48) · docker-compose (49-71). Un `/clear` en el turno 23 y otro en
> el 49 habrían ahorrado ~$1.90 de los $2.70.
> — o bien — Un solo tema, muy largo: aquí conviene `/compact`, no `/clear`.

### Reglas duras de la etapa (heredan las de arriba y las del analizador)

1. **El hallazgo determinista NO cambia.** `inflate` sigue naciendo con
   los mismos umbrales, `cr_cost` y clave (`fndKey` = kind|session|origin):
   la capa semántica es ADITIVA sobre la tarjeta ya nacida. Sin GGUF, sin
   opt-in, o con cualquier fallo → la tarjeta de hoy, tal cual (fail-quiet,
   regla #4). Nunca un hallazgo nace ni muere por el modelo.
2. **Solo embeddings, nunca el 2B.** Encaja con analizador-fugas.md §5
   ("determinista, nunca un modelo local") porque un embedding NO genera:
   mismo modelo + mismo texto = mismo vector, el coseno es aritmética, se
   testea con fixture (texto → similitud esperada ±0.01) y ENSEÑA su
   trabajo (la similitud viaja al debug). Lo que §5 prohíbe —adivinar y
   presentarlo como juicio— queda fuera: la banda media dice "no sé".
3. **El ahorro no se supone, se calcula:** para cada frontera en el turno
   f, ahorro = Σ (turnos > f) de `cache_read` de los tokens que pertenecen
   a los tramos ANTERIORES a f, al precio de lectura de caché del modelo
   de cada turno (mismo `price_for` que `cr_cost`). Aproximación honesta:
   el contexto de un turno t se reparte por tramos según su tamaño en
   `first_cr`→`last_cr`; el doc lo dirá con "~" (`estimated: true` en el
   campo nuevo, NO en el hallazgo).
4. **Privacidad igual que el press:** la evidencia son mensajes HUMANOS
   (`user_turn_text`, el ÚNICO filtro; réplica exacta en el exportador)
   recortados a 300 chars; NO se persisten (ni en `findings` guardados ni
   en `fndHist`), solo los tramos resultantes `{from, to, label}` y el
   ahorro. Los textos NUNCA salen de la máquina que corre el modelo ni van
   al hub/ntfy. La `label` de cada tramo es el PRIMER mensaje humano del
   tramo recortado a ~40 chars (no lo redacta el modelo — regla #6 y
   analizador §5: el modelo no escribe UI).
5. **Sin red y bajo demanda:** mismo llama-server de embeddings
   (`--embeddings -c 1024`, kill-on-drop) que `ai_emb_verdict`; se arranca
   UNA vez por pasada, embebe todo lo pendiente y se mata. Solo corre en
   la pasada COMPLETA de Hallazgos (abrir la pestaña / refresco >5 min),
   NUNCA en `fndPass()` ligero ni en el ciclo de cuota. Tope: los
   `inflate` que salgan (≤12) × sus turnos humanos (tope 200 msgs/sesión;
   más allá se muestrea uniforme y se dice "muestreado").
6. **Caché por sesión:** `inflate_topics.json` en app_data, clave =
   `origin|session_id_completo|turnos` → `{tramos, ahorro, sim_min}`. Los
   logs viejos no cambian: se embebe una vez. Si la sesión sigue viva
   (turnos crecen), se recalcula solo esa.

### El algoritmo (determinista, constantes a propósito)

- Entrada: lista ordenada de mensajes humanos `m[0..n]` de la sesión (ya
  filtrados por `is_user_turn`), con el número de turno de cada uno.
- Embeber cada `m[i]` SIN prefijos (calibrado en la etapa 2).
- Recorrer con un CENTRO del tramo actual = media de los vectores del
  tramo (renormalizada). Para cada `m[i]`: `sim = cos(m[i], centro)`.
- **Frontera candidata** si `sim < TOPIC_NEW` (arranca en `EMB_NEW` =
  0.45, constante propia porque el caso es distinto: mensaje suelto vs
  centro de tramo). Se CONFIRMA solo si los `TOPIC_HOLD` = 2 mensajes
  siguientes también quedan por debajo contra el centro viejo Y por
  encima entre sí (>`EMB_CROSS`) — un "corre las pruebas otra vez" es
  lejano de todo y no debe cortar. Mensajes de ≤3 palabras no votan
  (ni cortan ni confirman): se adjuntan al tramo vigente.
- Tramo mínimo `TOPIC_MIN_TURNS` = 4 mensajes humanos; si un corte deja
  un tramo menor, se funde con el vecino más parecido.
- Salida: `topics: [{from, to, label}]` (turnos humanos), `saved` ($, con
  el reparto de la regla 3), `sim_min` (la similitud más baja vista, para
  el debug) y `sampled: bool`.
- Veredicto de la ficha (JS, `t()`): 1 tramo → variante "un solo tema →
  /compact"; ≥2 → variante "N temas → /clear en los turnos X, Y; ahorro
  ~$Z". Sin capa (None) → ficha de hoy sin cambios.

### Piezas y enganches (las tres, invariante #1)

- **Rust (local + WSL):** `Finding` gana `topics: Option<TopicSplit>`
  (`#[serde(default)]`). `get_findings` — tras `scan_local_findings` y
  el tope de 12 — llama a `topics_for_inflates(&mut findings)` en el mismo
  `spawn_blocking`: para cada `inflate` con `origin` vacío o `wsl-*` abre
  su jsonl (guarda `session` completo internamente; el `sid8` público no
  alcanza — resolver por `scan_cache`/ruta, no por prefijo), saca los
  mensajes humanos con `user_turn_text`, consulta la caché, embebe lo que
  falte con `ai_emb_sim`-refactorizado a `ai_emb_vecs(texts) ->
  Vec<Vec<f32>>` (hoy solo devuelve un coseno; el servidor y el guard se
  comparten). Rastro: `topics_debug.txt` (sesión, n msgs, fronteras,
  sim por mensaje — SIN los textos).
- **Exportador (VPS por SSH):** el modelo NO va al VPS (§Lo descartado;
  el exportador sigue stdlib). Bajo `--findings` cada `inflate` gana
  `umsgs: [{turn, text≤300}]` (mismo `user_turn_text` que ya usa el coach
  para `msgs`; tope 200 con muestreo declarado `sampled`). Es la ÚNICA
  novedad remota: el TEXTO viaja por SSH al Windows, el Windows embebe.
  Es tu propio servidor por tu propia llave — misma naturaleza que `msgs`
  del press que ya viaja desde 2026-08-05. Rust, al fusionar, corre el
  mismo `topics_for_inflates` sobre esos `umsgs` (origen = nombre del
  server) y los DESCARTA antes de persistir (regla 4). Exportador viejo
  → sin `umsgs` → tarjeta de hoy (degradación sola, como `--coach`).
- **Panel:** la tarjeta `inflate` pinta debajo de la fila de costo una
  línea de tramos (chips "1-22 · bug del login" …) y el ahorro; textos
  nuevos `fnd_inflate_one`/`fnd_inflate_multi` ×8 idiomas. `fndKey` NO
  cambia (visto/ignorado se conservan). Interruptor: el MISMO opt-in del
  análisis local (`ai_config`), sin casilla nueva; con el análisis
  apagado no se embebe nada.
- **Hub:** `topics` NO viaja en las fotos (`hosts/*.json`): quien lee no
  tiene los textos y no debe fingir. Se recalcula donde hay modelo.

### Orden y validación

1. Rust local (Windows/WSL) + panel, sin tocar el exportador. Medir con
   las sesiones reales de la semana de Oscar: ¿las fronteras coinciden con
   lo que él recuerda? Anotar aciertos/fallos en la bitácora ANTES de
   mover umbrales (`TOPIC_NEW`/`TOPIC_HOLD`/`TOPIC_MIN_TURNS`).
2. Exportador `umsgs` + fusión — invariante #1: réplica de
   `user_turn_text` ya existe; regresión byte a byte de `--findings` sin
   el campo nuevo salvo `umsgs`.
3. Fixture de test: un jsonl sintético con 3 temas evidentes y otro de un
   tema largo → tramos esperados; y el caso "corre las pruebas" que NO
   corta.
- Va DETRÁS del cierre de las pruebas en vivo del auto-/compact y
  auto-/clear (misma zona de código: `ai_emb_*`), como el ruteo.

### Lo que NO es (para no confundir con lo de arriba)

- No toca el automático ni `intentVerdict`: es análisis A POSTERIORI de
  sesiones cerradas o largas, no del hit `press`.
- No clasifica "qué era cada tema" (debug/feature/docs): eso es
  analizador §5 "clasificar sesiones" con heurísticas de herramientas, y
  si un día se hace, va por lógica, no por embedding.

### Etapa 3-bis (algún día, sin fecha)

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
