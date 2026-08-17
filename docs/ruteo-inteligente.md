# MichiClaude — Ruteo Inteligente de Modelos
## Investigación completa, análisis competitivo y propuesta de diseño

**Fecha:** 13 de agosto de 2026 (v2 — añade regla de sesión mixta, contabilidad de tokens y rol del modelo local)
**Tema:** Sugerir y facilitar el cambio de modelo (Haiku/Sonnet/Opus/Fable) en Claude Code según el comportamiento del usuario y su cuota real, integrado a MichiClaude.

> Nota del repo (2026-08-13): documento aportado por Oscar (investigación
> externa) y copiado a `docs/` con la codificación reparada. La auditoría
> contra lo ya construido en MichiClaude está en el §10, añadido al copiar.

---

## 1. La idea original

Que MichiClaude detecte qué tipo de trabajo está haciendo el usuario y sugiera (o aplique) el modelo adecuado:

- Pregunta simple ("¿para qué sirve `/compact`?") → no quemar Opus/Fable; bastaría un modelo menor.
- Tarea compleja ("hazme el UX de esto con demo interactiva") → usar un modelo superior.
- Basarse en el historial y patrones reales del usuario, no en adivinanzas.
- Idealmente en "tiempo real": analizar lo que se pide **antes** de ejecutarlo y tomar la mejor decisión por turno, tolerando un usuario impredecible que alterna entre consultas y código complejo.
- Tolerar el caso más común de todos: la **sesión mixta** — el usuario está ajustando y probando código, y entre esos ajustes pregunta cosas sencillas de otros temas, todo en la misma sesión.

---

## 2. Qué permite la plataforma (hechos verificados)

### 2.1 Lo que SÍ se puede con hooks de Claude Code

| Mecanismo | Hook | Qué logra |
|---|---|---|
| Leer el prompt antes de que el modelo lo procese | `UserPromptSubmit` | Clasificar, validar, agregar contexto |
| **Bloquear un prompt** con una razón visible | `UserPromptSubmit` (exit 2 o `decision: block`) | Detener el turno ANTES de gastar tokens y redirigir al usuario ("corre `/model opus` y reenvía") |
| **Inyectar contexto** que Claude ve junto al prompt | `UserPromptSubmit` (`additionalContext`) | Darle al modelo información de estado (cuota, modo ahorro) para que él mismo sugiera |
| Inyectar reglas al inicio de sesión | `SessionStart` | Reglas de selección de modelo para subagentes |
| **Forzar el modelo de cada subagente** al lanzarlo | `PreToolUse` sobre Agent/Task | Ruteo real, en vivo, por tarea — sin tocar la sesión principal |
| Cambiar el modelo de la **siguiente** sesión | Escribir `settings.json` | Cambio persistente (no aplica al turno en curso) |

Restricciones importantes:

- `UserPromptSubmit` **bloquea el procesamiento** mientras corre y tiene timeout corto (30 s por defecto; en la práctica se quiere < 2 s). La clasificación debe ser local e instantánea — nunca llamar a un modelo para decidir qué modelo usar.
- Exit code 2 = bloqueo; exit 1 **no bloquea nada** (error no bloqueante).

### 2.2 Lo que NO se puede

- **Cambiar el modelo del turno en curso.** No existe mecanismo de hook para eso. Hay una petición de feature abierta en el repo oficial de Claude Code (issue #31342) pidiendo exactamente esto, sin implementar.
- Escribir `settings.json` a media sesión NO cambia la sesión corriente — solo la siguiente. Cualquier herramienta que diga "Switched opus → haiku" en el momento está describiendo un efecto diferido.

### 2.3 Por qué Anthropic no lo ha hecho (análisis)

1. **Ya lo hizo a medias, con reglas simples y predecibles:** modo `opusplan` (Opus planea, Sonnet ejecuta) y fallback automático por cuota en planes Max. Lo que evita es la versión "decido por ti en cada prompt".
2. **Riesgo reputacional:** cambiar modelos en silencio alimentaría el eterno "nerfearon el modelo" — y esta vez con fundamento. Para el proveedor, la predictibilidad vale más que la optimización. Un tercero instalado voluntariamente puede asumir ese riesgo; el default de millones de usuarios no.
3. **Razón técnica — el caché de prompts es por modelo:** cambiar de modelo a media sesión invalida el caché del contexto; hay que reprocesarlo completo al precio del modelo nuevo. El cambio "para ahorrar" puede salir más caro y más lento. Por eso los cambios naturales son en fronteras: sesión nueva, subagente, fase.
4. **Dirección del roadmap:** thinking adaptativo y niveles de esfuerzo apuntan a que un solo modelo module su propio gasto. Construir un router elaborado entre tiers sería administrar un problema que planean eliminar.
5. **Incentivos:** en suscripción, cada token ahorrado es margen para Anthropic (la teoría cínica no cuadra); en API, bajar modelos sí baja ingresos. Tener ambos segmentos vuelve políticamente incómodo cualquier ruteo automático global.

**Implicación:** la ventana para terceros existe porque Anthropic no puede asumir el riesgo a su escala. Pero la ventana del *ruteo* puede cerrarse en cualquier release; la de la *medición* no.

---

## 3. Estado del arte en GitHub (lo investigado)

### 3.1 `tzachbon/claude-model-router-hook` (~41⭐, MIT, adaptado de `model-matchmaker`)

**Qué hace:** dos/tres scripts (bash + Python inline, luego paquete Python). `SessionStart` inyecta reglas de tier para subagentes; `UserPromptSubmit` clasifica el prompt con keywords y regex, y en modo `warn` sugiere / en modo `autoswitch` escribe `settings.json`. `PreToolUse` (versión nueva) refuerza el ruteo de subagentes. Prefijo `~` para bypass. Log de cada decisión.

**Pros (vale la pena copiar el patrón):**
- Prefijo `~` de bypass: un carácter, cero fricción. Excelente UX.
- `warn` por defecto, `autoswitch` opt-in: respeta la agencia del usuario.
- Log grepeable de decisiones (`~/.claude/hooks/model-router-hook.log`) — además, si el usuario lo tiene instalado, MichiClaude podría leerlo y cruzarlo con tokens reales para responder lo que el hook no puede: *¿sirvió?*
- Ruteo de subagentes: ataca una fuga real y silenciosa (subagentes heredando el modelo caro).
- Config por proyecto que gana sobre la global.
- Distribución vía plugin marketplace de Claude Code (`claude plugin install ...`): instalación en dos comandos. Canal a investigar para MichiClaude.
- Truco del "Setup Prompt": un texto que pegas en Claude Code y él mismo instala los hooks.

**Contras:**
- El "autoswitch" no cambia el turno actual (efecto diferido) pero el mensaje sugiere que sí — engañoso sin querer.
- **Solo inglés.** Keywords/regex (`analyze`, `git commit`, `implement`). Prompts en español no matchean nada, y falla en silencio.
- **Confunde longitud con dificultad** (>200 palabras = opus). Un prompt largo y detallado suele ser el fácil; "arregla el bug del login" (5 palabras) puede ser lo más difícil del repo.
- El caso de uso original de esta investigación ("¿para qué sirve /compact?") **no matchea ninguna regla** — no recomienda nada. Y "analyze this function" (trivial) → opus.
- Regex de sonnet tan amplios (`test`, `update`, `api`, `function`, `write`) que sonnet es el default de facto.
- **No sabe cuánta cuota queda.** Rutea a ciegas sobre el recurso que de verdad importa.
- **No mide resultados.** Loguea decisiones, nunca impacto.
- Escritura NO atómica de `settings.json` (`json.dump` directo): riesgo real de corromper la config del usuario.
- macOS/Linux únicamente. Windows no.
- La inyección de reglas en cada `SessionStart` cuesta ~350 tokens de contexto por sesión — una herramienta de ahorro que nunca midió su propio gasto.

### 3.2 `ypollak2/llm-router` (~33⭐ pero 627 commits, 119 releases, 1900+ tests)

**Qué hace:** router universal multi-proveedor (Claude Code, Codex, Gemini CLI). Clasifica cada prompt y lo manda al modelo más barato capaz — cadena "free-first": Ollama local → Gemini Flash → GPT/Claude. Políticas configurables (aggressive/balanced/conservative), circuit breakers, 60 herramientas MCP.

**Pros:**
- **Es cuota-consciente:** monitoreo de presión de presupuesto con auto-downgrade; herramientas que leen el uso de la suscripción de Claude (`llm_check_usage`, `llm_refresh_claude_usage`).
- Modo `zero_claude`: los prompts se completan por ejecución externa o **se bloquean antes de que Claude Code invoque su modelo** (usa el mismo mecanismo de bloqueo de prompt propuesto aquí), con prefijo `claude:` para forzar un turno nativo.
- **Mide, y es honesto midiendo:** registra cada tarea en SQLite local, calcula ahorros contra línea base, y documenta sus limitaciones (línea base = peor caso, tokens estimados con caracteres/4, el "87%" fue el pico de un usuario, no garantía).
- Clasificación en cascada: heurística gratis para ~70% de casos, modelo local/barato solo para ambiguos.
- Local-first, sin proxy hosteado, sin cuenta.

**Contras:**
- **Su respuesta a "poca cuota de Claude" es mandar tus prompts a OTROS proveedores** (Google, OpenRouter con 343 modelos, Ollama). Para el usuario que pagó Max porque quiere Claude: los prompts viajan a terceros y la calidad cambia de respuesta en respuesta. La medicina es peor que la enfermedad para ese perfil.
- Ahorro medido con tokens estimados (caracteres/4) contra línea base hipotética — no contra el historial real del usuario.
- 60 herramientas MCP registradas = mucho contexto consumido; irónico en una herramienta de ahorro (existe modo "slim" que lo reconoce).
- Solo Python/Linux/macOS en la práctica.
- Auto-promoción de una versión "enterprise" del mismo autor en el README.

### 3.3 `musistudio/claude-code-router` (CCR) — el proyecto grande del espacio

**Qué hace:** un **proxy/gateway local** (127.0.0.1:3456). Claude Code se conecta al proxy en vez de directo a Anthropic; el proxy intercepta cada petición y ahí decide proveedor y modelo, con condiciones, rewrites de request, reintentos y fallbacks ordenados. Multi-agente (Claude Code, Codex, otros CLIs), con app de escritorio para macOS/Windows/Linux.

**Pros:**
- **Resuelve el "tiempo real" de verdad:** al ver cada request antes de salir, puede reescribir el campo `model` de ESE turno. Cambio por turno, en vivo. Es la única arquitectura que rompe la restricción de los hooks.
- Maduro, multiplataforma (incluye Windows), con UI de administración, logs de proveedor/modelo/tokens/latencia.

**Contras (decisivos para MichiClaude):**
- **Todo el tráfico pasa por el proxy:** prompts, código, contexto completo. Rompe frontalmente la promesa de MichiClaude ("tu token nunca sale de tu equipo salvo hacia api.anthropic.com; la app solo LEE").
- **Con suscripción es zona gris de ToS:** CCR nació para BYOK/API keys multi-proveedor. Interceptar/reescribir el tráfico OAuth de una suscripción Pro/Max es territorio que Anthropic puede castigar.
- Es una app completa que administrar: otro proceso, otra config, otro punto de falla en medio del flujo de trabajo.

### 3.4 `OmniRoute` (diegosouzapw/OmniRoute)

Proxy multi-proveedor que **conecta el OAuth de Claude Code** a un pool con tracking de cuota de 5 h y semanal por modelo, más Codex/GitHub/otros. Técnicamente logra tiempo real + cuota-consciencia, pero manipula la cuenta/OAuth del usuario de formas claramente grises respecto a los términos de servicio. Descartado como referencia de diseño; útil solo como evidencia de que el mercado quiere cuota-consciencia.

### 3.5 `claude-model-router` (PyPI)

Ruteo cost-aware para la **API** (no suscripción): una llamada barata a Haiku clasifica la complejidad de cada prompt y ejecuta en el tier más barato capaz. Detalle interesante: su fallback heurístico **nunca** asigna el tier trivial — solo el clasificador puede, porque malrutear trabajo real a Haiku cuesta más en reintentos de lo que ahorra el tier. Ese principio (el error de bajar es más caro que el de subir) coincide con el diseño propuesto aquí. No aplica directo (gasta una llamada por prompt; es para API).

### 3.6 Resumen comparativo

| | Cambio por turno real | Sabe tu cuota Claude | Se queda en Claude | Windows | Mide resultados | Historial conductual | Multiidioma | Privacidad intacta |
|---|---|---|---|---|---|---|---|---|
| Hook tzachbon | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| llm-router | Parcial (bloqueo/desvío) | ⚠️ Parcial | ❌ (desvía) | ❌ | ✅ | ❌ | ❌ | ⚠️ (terceros) |
| CCR (proxy) | ✅ | ❌ | Opcional | ✅ | Parcial | ❌ | n/a | ❌ (proxy) |
| OmniRoute | ✅ | ✅ | ⚠️ gris ToS | ✅ | ❌ | ❌ | n/a | ❌ |
| **Hueco disponible** | Bloqueo+escalada | ✅ nativa | ✅ | ✅ | ✅ vs. historial real | ✅ | ✅ | ✅ |

**Conclusión del análisis:** el mercado ya validó que la cuota es el problema (tres proyectos independientes llegaron ahí), pero todos lo atacan con la palanca incómoda — desviar tráfico o interceptarlo. La palanca que nadie usa: observar, diagnosticar con historial propio, sugerir con datos, y medir el resultado. Es exactamente la posición natural de MichiClaude.

---

## 4. El problema del ping-pong (usuario impredecible)

Escenario: 5 turnos de consultas → el sistema sugiere bajar → el usuario acepta → turno 6 es código complejo → sugiere subir → turnos 7-8 vuelven a consultas → ¿sugiere bajar otra vez? Un sistema que reacciona a cada turno persigue el ruido y marea al usuario.

**Solución: histéresis + asimetría** (concepto de ingeniería de control — el termostato no prende a 25.1° y apaga a 24.9°; espera una banda para no ciclar).

La asimetría se deriva de que los dos errores no cuestan lo mismo:

| Error | Consecuencia | Costo |
|---|---|---|
| Bajar de modelo por error (tarea compleja en Haiku) | Código malo, retrabajos, reintentos | **Caro** |
| Subir de modelo por error (pregunta simple en Opus) | Unos tokens de más | Barato |

**Reglas concretas:**

1. **Subir: inmediato, con 1 señal fuerte.** Sin cooldown para escalar. Es el único caso que amerita interrumpir (bloqueo de prompt).
2. **Bajar: solo con patrón sostenido** — mínimo 8-10 turnos consecutivos "ligeros" (sin ediciones de archivo, salidas cortas) **Y** presión de cuota real. Si el bucket va al 40%, ni molestar.
3. **Cooldown:** tras cualquier cambio o escalada, silencio total N turnos / 30-60 min. Dos preguntas no revierten una escalada.
4. **Memoria de rechazo:** un "no" del usuario = no repetir la misma sugerencia en esa sesión. Tres "no" en un proyecto = ese proyecto queda en modo manual hasta que el usuario diga.
5. **Señales estructurales, no keywords:** ¿trae bloque de código? ¿rutas de archivo? ¿imperativo largo o pregunta corta? Funcionan igual en español, inglés o japonés, y corren en microsegundos en Rust (crítico por el timeout del hook).
6. **Los subagentes no tienen ping-pong posible:** cada uno nace con su modelo por tarea; la volatilidad del usuario no los afecta.
7. **Regla de sesión mixta:** una sesión con ediciones de archivo activas es "sesión de trabajo" completa; las preguntas intercaladas NO cuentan como señal de bajar (el contador de turnos ligeros se reinicia con cada turno de código). Las sugerencias de bajar solo aplican a (a) sesiones cuyo patrón dominante es consulta, o (b) rachas largas de consulta al final de una sesión — y siempre apuntando a la **siguiente** sesión, nunca a la actual.

### 4.1 El caso de la sesión mixta (por qué "no hacer nada" es lo correcto)

La sesión pura no existe: el flujo real es ajuste → prueba → "¿y este flag qué hace?" → otro ajuste → "¿cómo se llama ese patrón?" → commit. La decisión de diseño es que en ese escenario el sistema **no toque la sesión**, y no es una limitación sino la matemática:

- La pregunta simple intercalada cuesta ~500 tokens. Cambiar a Sonnet para contestarla y regresar a Opus costaría dos fricciones **más la invalidación del caché de prompts**: el contexto de la sesión de trabajo (miles de tokens cacheados a precio de centavos) se tira, y el regreso lo reprocesa completo al precio del modelo. **El "ahorro" de la pregunta sale más caro que la pregunta.**
- La pregunta intercalada en una sesión de trabajo **no es una fuga** — es ruido barato dentro de un flujo caro ya pagado y cacheado. La fuga real es la sesión entera de consultas, o el proyecto cuyo patrón dominante es preguntar. Por eso se miden patrones sostenidos, no turnos sueltos.

Qué hace cada pieza en una sesión mixta (ej.: turnos 1-4 código, 5 pregunta, 6-9 código, 10 pregunta, 11 código):

| Pieza | Comportamiento |
|---|---|
| Perfil conductual | La sesión tiene ediciones + tool use + salidas largas → "sesión de trabajo", aunque 20% de sus turnos sean preguntas. El contador de turnos ligeros llega a 1 y se reinicia en el turno 6. El gatito nunca sugiere bajar. |
| Contexto inyectado | Se adapta solo: quien clasifica el turno fino es Claude, que lee cada prompt de todos modos. En el turno de código no dice nada; en la pregunta suelta (si la cuota aprieta) puede cerrar con una línea tipo *"esto lo resuelves igual en claude.ai sin gastar tu bucket de Code"*, y al turno siguiente vuelve al código sin drama. |
| Bloqueo de escalada | No se dispara (solo protege hacia arriba; en sesión de trabajo normalmente ya estás arriba). |
| Subagentes | Se rutean igual — son por tarea, no por sesión; la mezcla no los toca. |

Caso intermedio legítimo: la sesión que empezó como código (3 turnos) y derivó en 15 turnos de consulta. Ahí la racha consecutiva sí se acumula y la sugerencia al turno ~12 procede: *"esta sesión empezó como código pero lleva 12 turnos de consulta — **la siguiente** podría arrancar en Sonnet."* La frontera de sesión es el único punto donde cambiar sale gratis (caché nuevo de todos modos), y ahí el mecanismo diferido de `settings.json` —que parecía limitación de la plataforma— resulta ser exactamente lo que se quiere.

**Filosofía que lo resume: el sistema no optimiza turnos, optimiza fronteras** — subagentes, sesiones nuevas, resets. Dentro de una sesión viva el único guardián es el bloqueo de escalada (existe para el único error que sí es caro); todo lo demás es el gatito tomando notas para hablar en el momento en que cambiarse no cueste nada.

---

## 5. La propuesta: "Modo MichiClaude" (hooks como manos, MichiClaude como cerebro)

### Principios no negociables

- **Nada se interpone en el tráfico.** Sin proxy, sin API keys de terceros, sin desviar prompts. El token sigue viajando solo a api.anthropic.com.
- **Nunca cambia nada en silencio.** Sugerencia con botón; el usuario decide. (El mismo riesgo reputacional que Anthropic evita, MichiClaude tampoco lo asume.)
- **El análisis histórico usa solo metadata** (turnos, herramientas usadas, tokens, ediciones de archivo) — no contenido de prompts. El hook que sí ve el prompt del momento es un componente separado, opt-in, visible y explicado — para no romper la promesa del README ("no lee tus mensajes ni tu código").
- **Fallo silencioso:** si el estado no existe o está viejo, los hooks no hacen nada. Jamás estorban el trabajo.
- **Todo se mide después.** La sugerencia sin comprobación es opinión; con comprobación es producto.

### Pieza 1 — El cerebro (MichiClaude; ~70% ya existe)

Sobre lo ya construido (JSONL parseados, gauges de cuota vía endpoint OAuth, costos por proyecto):

- **Perfil conductual por proyecto:** % de turnos sin edición de archivos, longitud típica de sesión, distribución por modelo, horarios de mayor quema. Inferencia por *forma*, no por contenido: una sesión de 2 turnos, sin tool use, con 400 tokens de salida es casi seguro una consulta.
- **Motor de histéresis** con las reglas de la sección 4.
- **Estado compartido:** archivo local (p. ej. `%APPDATA%\com.oscarorozco.michiclaude\router_state.json`) con lo que los hooks necesitan: `{opus_semanal: 0.88, reset_en_horas: 72, modo_proyecto: "ahorro", cooldown_hasta: <ts>}`. Actualizado en cada refresco de MichiClaude.

### Pieza 2 — Las manos (dos hooks generados e instalados por MichiClaude)

Botón "Activar ruteo inteligente" → MichiClaude escribe los hooks en `~/.claude/hooks/` (PowerShell para Windows nativo, bash para WSL/SSH — cobertura que ningún competidor tiene). Los hooks **no piensan**: leen `router_state.json` y actúan en microsegundos. Su fuente se publica en el repo con hash, siguiendo el mismo patrón de transparencia del lector SSH.

**Hook A — `UserPromptSubmit` (el guardián):**
1. Lee el estado; si falta o está viejo → exit 0 sin hacer nada.
2. **Escalada en tiempo real** (único caso que interrumpe): usuario en Haiku/Sonnet + señales estructurales pesadas (bloque de código, varias rutas de archivo, imperativo largo) → **bloqueo con exit 2**: *"Esto se ve complejo y estás en Sonnet (ahorro activo). Corre `/model opus` y reenvía, o antepón `~` para continuar así."* El prompt no se ejecutó en el modelo equivocado; se perdieron 3 segundos, no una respuesta mala.
3. **Contexto inyectado — la clasificación gratis y multiidioma:** en los demás casos, 2 líneas de `additionalContext` con el estado (*"Opus al 88%, reset en 3 días, modo ahorro; si la tarea es trivial sugiere bajar, si es arquitectura confirma el tier"*). El propio Claude — que ya iba a leer el prompt de todos modos, en cualquier idioma, con typos y sarcasmo — hace la clasificación fina y da la sugerencia dentro de su respuesta. Costo: ~60 tokens/turno, medible (y MichiClaude lo reporta, a diferencia de todos los demás).
4. Prefijo `~` = bypass total (patrón copiado de tzachbon).

**Hook B — `PreToolUse` sobre Agent/Task (el ahorrador silencioso):**
Cada subagente nace con el modelo correcto para su tarea: exploración/búsqueda → Haiku; implementación → Sonnet; análisis profundo → según cuota disponible. Ruteo verdaderamente en vivo, sin tocar la sesión principal, sin ping-pong posible. Es donde más se fuga dinero sin que nadie lo vea, y el componente más simple de construir.

### Pieza 3 — Cambios de sesión (siempre con consentimiento)

Cuando patrón sostenido + presión de cuota lo ameritan, **la sugerencia viene del gatito** (globo/notificación), no del hook:

> 🐱 *"Van 3 días para tu reset y queda 12% de Opus. En este proyecto 8 de cada 10 turnos son consultas sin editar código. ¿Lo paso a Sonnet?"*
> `[Sí, solo este proyecto]` · `[Sí, hasta el reset]` · `[No]`

Un clic → MichiClaude escribe `settings.json` de forma **atómica** (temp + rename + respaldo — corrigiendo el bug del hook de tzachbon) y comunica con honestidad: *"aplica desde tu próxima sesión"* (nunca fingir que cambió la actual). Al reset semanal, notificación vía ntfy (infraestructura ya construida): *"cuota restablecida — ¿volvemos a Opus por default?"*

### Pieza 4 — El cierre del ciclo (el diferenciador que nadie puede copiar)

Cada decisión de los hooks queda en log; los JSONL registran lo que realmente pasó después. MichiClaude cruza ambos:

> *"Ruteo activo 12 días: 340 turnos redirigidos, tu bucket de Opus rinde 41% más, 2 escaladas a tiempo, 0 sugerencias rechazadas."*

llm-router mide contra una línea base hipotética con tokens estimados (caracteres/4). MichiClaude mide contra **el historial real del usuario, con los números reales de su cuenta** (gauges del endpoint OAuth). Es la diferencia entre estimar y comprobar. Además: el pitch correcto para suscriptores no son dólares (son nocionales) sino **no quedarse sin cuota un jueves a las 4 pm** — y esa frase solo la puede demostrar quien tiene los gauges.

### 5.1 Contabilidad de tokens: ¿el sistema gasta o ahorra?

Auditoría del propio sistema (la que ningún competidor se hizo a sí mismo):

| Pieza | Gasta | Detalle |
|---|---|---|
| Cerebro MichiClaude (JSONL, perfiles, histéresis, `router_state.json`) | **0 tokens** | Lectura local de disco; el endpoint de cuota es HTTP, no consume tokens |
| Hook B (subagentes) | **0 tokens** | A diferencia de tzachbon (~350 tokens de reglas inyectadas por sesión pidiéndole al modelo que rutee), aquí `PreToolUse` **reescribe el parámetro `model` directamente** con script local. No le pide al modelo: le impone. Puro ahorro |
| Bloqueo de escalada ("detener el prompt para analizarlo") | **0 tokens** | El análisis ocurre EN la máquina, ANTES de que el prompt salga (señales estructurales en microsegundos). Si bloquea, el prompt nunca llegó a Anthropic. Es la pieza que más ahorra por evento: evita la respuesta completa del modelo equivocado + reintentos |
| Sugerencias del gatito, escritura de settings, ntfy | **0 tokens** | — |
| **Contexto inyectado (Hook A)** | **~60 tokens/turno entrada** + ~25 salida ocasionales | El ÚNICO costo recurrente del diseño. Contexto: un turno típico de Claude Code mueve miles/decenas de miles de tokens — esto es 0.1-1% del turno, y se carga al bucket del modelo activo (si ya bajaste a Sonnet, el impuesto lo paga el bucket barato) |

Detalles de diseño derivados:

- El estado inyectado debe ser **grueso** (redondear a "~90%", no "88.3%") para no cambiar el texto en cada turno sin necesidad.
- El contexto inyectado es **apagable por separado**: corriendo solo Hook B + bloqueo + gatito, el sistema es de costo literalmente cero. El contexto inyectado es el "lujo" que compra la clasificación fina multiidioma.
- Costo indirecto ajeno al sistema: escalar a media sesión (`/model opus` + reenviar) invalida el caché de prompts y el modelo nuevo reprocesa el contexto completo — lo cobra Anthropic, no el hook. Es la razón física por la que el diseño prefiere fronteras y solo interrumpe cuando NO escalar saldría más caro.
- Balance: con que el sistema redirija **una consulta al día** fuera de Opus, ya pagó su sobrecosto diario varias veces.
- Diferenciador de marca: MichiClaude reporta su propio consumo en el panel — *"el ruteo te costó 4,200 tokens este mes y te ahorró 310,000"*. El renglón de autocrítica en números es más creíble que cualquier promesa.

### 5.2 ¿Y un modelo local? Dónde ayuda y dónde no

Análisis basado en benchmarks propios en el hardware objetivo (CPU-only, 8 GB RAM): Qwen3.5-2B → 8.46 tok/s con reasoning desactivado (`llama-server --reasoning-budget 0`).

**Regla de diseño: camino caliente = tonto y rápido; camino frío = listo y lento.** El hook debe resolver en microsegundos; la inteligencia local va donde no hay reloj.

| Lugar | ¿Modelo local? | Cuál | Por qué |
|---|---|---|---|
| Contexto inyectado | ❌ | — | No hay nada que pensar: son 2 líneas armadas con formateo de strings desde `router_state.json` |
| Clasificación en vivo del prompt (reemplazar al clasificador) | ❌ LLM | — | A 8.46 tok/s, clasificar añade 1-2 s a CADA prompt (camino crítico del hook). Peor: exige el modelo **residente en RAM** (2-3 GB en la máquina de 8 GB) — mataría el argumento de huella de MichiClaude (276 MB). Y la clasificación vía contexto inyectado la hace **Claude mismo** —el mejor clasificador multiidioma que existe, que ya iba a leer el prompt— por ~60 tokens y cero latencia añadida. El 2B local pierde en calidad, velocidad y costo |
| Bloqueo de escalada v2 | ⚠️ Embeddings (no LLM) | Encoder multilingüe ONNX (~100-400 MB, tipo e5-small) | Clasifica contra centroides pre-calculados en <50 ms en CPU. Cabe en el presupuesto del hook, multilingüe, más fino que regex, sin LLM residente. Aun así: **v2** — las señales estructurales cubren ~90% de los casos de escalada, y cada pieza en el camino caliente es una pieza que puede fallar frente al usuario |
| **Análisis histórico en frío** | ✅✅ | Qwen3.5-2B con la config ya validada | **El lugar bueno.** Batch nocturno/en idle, sin timeout, sobre los JSONL que ya están en disco. Desbloquea el diagnóstico por categorías que la metadata sola no puede dar: *"el 62% de tu gasto de Opus fueron preguntas conceptuales; tus top 5 categorías de fuga: sintaxis de comandos, explicación de errores, dudas de git..."* A 8 tok/s en batch se procesan miles de turnos sin que nadie lo note |
| Reporte semanal de fugas por categoría | ✅ | El mismo, mismo batch | Es el output del análisis en frío. Feature Pro natural |

**Nota de privacidad (crítica):** el análisis en frío SÍ lee contenido de prompts, cosa que el diseño base evita a propósito. Se sostiene la promesa fuerte ("**nada sale de tu equipo**" — el modelo corre local) pero cambia la débil ("la app no lee tus mensajes"). Por eso va como componente **opt-in separado, apagado por defecto**, con explicación explícita: *"Análisis profundo local: un modelo en tu propia máquina lee tu historial para categorizar tus fugas. Nada sale de tu PC."* Coherente con el estilo de transparencia del README, y no es infraestructura nueva: es darle un segundo trabajo a la línea de modelos CPU-only ya en desarrollo para MichiClaude.

---

## 6. El ejemplo final, de punta a punta

**Miércoles.** El usuario abre Claude Code en `sparkyflow`. Modelo: Opus. Bucket semanal: 88%. Reset: sábado.

| Momento | Qué pasa | Qué pieza actúa |
|---|---|---|
| **Turnos 1–5** — consultas sueltas ("¿para qué sirve /compact?") | El hook inyecta el estado en cada turno. En el turno 3, el propio Claude cierra su respuesta: *"vas al 88% de Opus y esto es consulta simple — con `/model sonnet` te rinde más la semana."* Sin interrumpir. En paralelo el gatito se pone ámbar y ofrece botones. El usuario acepta "solo este proyecto". MichiClaude escribe la config (atómica) y avisa: *"aplica desde tu próxima sesión."* | Hook A (contexto) + Pieza 3 |
| **Turno 6** — sesión nueva en Sonnet; llega refactor complejo con bloque de código | Señales estructurales pesadas → **bloqueo antes de gastar un token**: *"Esto se ve complejo y estás en Sonnet. `/model opus` y reenvía, o `~` para seguir."* El usuario escala, reenvía, Opus resuelve. Durante el refactor, Claude lanza 3 subagentes: 2 de exploración salen en **Haiku**, 1 de análisis en Opus. | Hook A (bloqueo) + Hook B |
| **Turnos 7–8** — vuelven las consultas | **Silencio absoluto.** Cooldown activo tras la escalada; dos preguntas no son patrón. (Aquí el sistema ingenuo habría mareado; este no.) | Histéresis |
| **Turnos 9–19** — 10+ consultas seguidas, sin tocar archivos, cuota al 91% | Ahora sí es patrón sostenido + presión real → el gatito, **una sola vez**: *"10 turnos de consulta seguidos. ¿Regresamos a Sonnet?"* Un "no" = no repite en la sesión. Tres "no" en el proyecto = modo manual. | Histéresis + Pieza 3 |
| **Jueves — sesión mixta** (código, prueba, "¿qué hace este flag?", más código, otra pregunta, commit) | **El sistema no toca la sesión.** El contador de turnos ligeros se reinicia con cada turno de código; el gatito no sugiere nada (cambiar por una pregunta suelta invalidaría el caché y saldría más caro que la pregunta). A lo sumo, en la pregunta suelta y con la cuota apretada, Claude cierra con una línea: *"esto lo resuelves igual en claude.ai sin gastar tu bucket de Code."* Los subagentes se siguen ruteando normal. | Regla de sesión mixta (4.1) |
| **Viernes** — reporte | 🐱 *"Desde el miércoles: 61 turnos en Sonnet, 11 en Opus (2 escalados a tiempo), 6 subagentes en Haiku. Opus va al 93% en vez de reventar el jueves. **Llegas al sábado con cuota.**"* | Pieza 4 |
| **Sábado** — reset semanal | Notificación al celular vía ntfy: *"Cuota restablecida. ¿Volvemos a Opus por default?"* | Pieza 3 + ntfy existente |

**Contraste con el estado del arte en el mismo escenario:** el hook de tzachbon no habría hecho nada (prompts en español no matchean; no sabe la cuota); llm-router habría desviado los prompts a Gemini/OpenRouter; CCR lo habría resuelto en vivo pero pasando todo el tráfico por un proxy en zona gris de ToS.

---

## 7. Orden de construcción recomendado (valor/esfuerzo)

1. **Hook B (subagentes).** El más simple; cero ping-pong posible; ahorro inmediato y medible. MVP y demo para Reddit/Discord.
2. **Reporte de impacto** (aunque solo mida el hook B). La medición es el pitch — nadie más la tiene contra datos reales.
3. **Sugerencias del gatito con histéresis** (gauges + perfil conductual, ambos ya existentes en la app).
4. **Hook A completo** (bloqueo de escalada + contexto inyectado). El más delicado; al final, probado con el grupo beta.
5. **(v2, opcional) Análisis histórico en frío con modelo local** — diagnóstico de fugas por categoría, opt-in, feature Pro. Reutiliza la línea de modelos CPU-only ya en desarrollo.
6. **(v2, opcional) Embeddings ONNX en el bloqueo de escalada** — solo si las señales estructurales muestran huecos reales en el beta.

## 8. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Anthropic saca ruteo nativo o el thinking adaptativo lo vuelve irrelevante | El ruteo es **feature**, no producto. La medición histórica local es ortogonal: mientras mejor ruteen ellos, más interesante es medirlo. "Ellos arreglan la llave; MichiClaude es el medidor de agua." |
| Romper la promesa de privacidad del README | Análisis histórico = solo metadata. El hook que ve prompts = componente separado, opt-in, con fuente publicada y hash. Comunicación ruidosa de la separación. |
| Corromper `settings.json` del usuario | Escritura atómica (temp + rename) + respaldo previo. |
| Hook lento que congela cada prompt | Todo local, en Rust/PowerShell, leyendo un JSON pre-computado. Presupuesto < 100 ms. Fallo silencioso si el estado falta. |
| Ping-pong / fatiga de sugerencias | Histéresis, asimetría subir-rápido/bajar-lento, cooldowns, memoria de rechazo, modo manual por proyecto. |
| Fingir cambios "en tiempo real" que son diferidos | Honestidad literal en cada mensaje: "aplica desde tu próxima sesión". |
| El costo del propio sistema (contexto inyectado) | ~60 tokens/turno, medido y reportado por MichiClaude — a diferencia de todos los competidores, que nunca midieron su propio gasto. Apagable por separado: sin él, el sistema es de costo cero. |
| Molestar en sesiones mixtas (el caso más común) | Regla de sesión mixta: ediciones activas = sesión de trabajo; el contador de turnos ligeros se reinicia con cada turno de código; sugerencias solo hacia la siguiente sesión. |
| Modelo local en el camino caliente (latencia + RAM residente) | Prohibido por diseño: el LLM local solo trabaja en frío (batch nocturno). En vivo, como mucho embeddings ONNX (<50 ms) y solo en v2. |
| El análisis en frío lee prompts (tensión con la promesa "no leo tus mensajes") | Componente opt-in separado, apagado por defecto, todo local ("nada sale de tu PC"), explicado en la UI con el mismo estándar de transparencia del README. |

## 9. La frase que resume la posición

> **"MichiClaude no toca tu tráfico ni decide por ti: te muestra la fuga, te da el botón, y luego te demuestra con tus propios números que funcionó."**

Contra los cuatro proyectos investigados, esa frase es simultáneamente el diferenciador técnico (sin proxy, sin desvío), el diferenciador ético (consentimiento, honestidad sobre lo diferido) y el diferenciador de producto (medición contra historial real, no contra líneas base hipotéticas).

---

## 10. Auditoría contra lo ya construido (añadido al copiar a docs, 2026-08-13)

Sección del repo, no de la investigación original. LEERLA antes de
construir cualquier pieza: cruza el diseño con los invariantes de
CLAUDE.md y con lo que MichiClaude ya tiene.

### 10.1 Hechos de plataforma re-verificados contra la doc oficial

- **PreToolUse SÍ puede reescribir el input** de una herramienta
  (`hookSpecificOutput.updatedInput`, desde Claude Code v2.0.10). CAVEAT
  operativo: hay que devolver el objeto de input COMPLETO, no solo el
  campo `model` — reescribir campos sueltos da problemas. El Hook B es
  viable tal como está diseñado. **COMPROBADO EN VIVO el 2026-08-14**
  (etapa 0, ver §11): deja de ser lectura de documentación.
- **UserPromptSubmit SÍ bloquea** (exit 2 / `decision` con `reason`,
  cero tokens gastados) **y SÍ inyecta** `additionalContext`. Timeout
  por defecto 30 s, como dice el §2.1.
- **`settings.json` a media sesión: confirmado que solo aplica a la
  siguiente** — la honestidad de "aplica desde tu próxima sesión" no es
  estilo, es la mecánica real.
- **Alternativa sin hooks para subagentes** que el doc no menciona: el
  frontmatter `model:` de `.claude/agents/*.md` y la variable
  `CLAUDE_CODE_SUBAGENT_MODEL` fijan el modelo de subagentes de forma
  soportada. Son ESTÁTICAS (no saben de cuota); el Hook B sigue teniendo
  sentido porque decide con `router_state.json`, pero el MVP más barato
  de todos es sugerir esa config estática desde el gatito.

### 10.2 Lo que ya existe en MichiClaude (no construir dos veces)

- Cuota por buckets + resets: `get_quota`. `router_state.json` es un
  volcado del ciclo del panel — el ÚNICO que llama al endpoint (regla
  vigente); los hooks deben tratar >10 min de antigüedad como "estado
  viejo" → exit 0.
- Perfil conductual: `uturns`, `by_model`, la serie `daily`, y las
  señales del coach (`topen/ttotal`, `cont`, `trail`, tokens de contexto)
  ya dan "% de turnos sin edición" y forma de sesión. El motor nuevo se
  cuelga del escaneo del coach — NO abrir los JSONL por segunda vez.
- Histéresis/anti-spam: patrones ya inventados y validados (`tipSeen`,
  tope diario, `ackPending`, `fndIgnore`, "Ahora no"). Reutilizarlos.
- Medición: `quota_history.json` (90 días, fotos reales de cuota) es la
  línea base que ningún competidor tiene; el reporte de impacto vive en
  la pestaña Reporte (fase 3), no en UI nueva. Invariante #8: presentar
  el antes/después como correlación honesta, nunca "41% más" sin base.
- Transparencia de scripts: el patrón `include_str!` + re-subida de
  `meter-export.py` aplica idéntico a los hooks (fuente en el repo,
  embebida en el exe).
- Modelo local: `ai_setup`/`ai_intent`/llama-server bajo demanda ya
  existen; el "análisis en frío" (§5.2) es literalmente darles un
  segundo trabajo. BLOQUEADO hasta cerrar la validación de análisis
  local v1 (pendiente vigente en CLAUDE.md).

### 10.3 Conflictos con reglas vigentes (correcciones al diseño)

a. **`hooks_noise` nos marcaría a nosotros**: el detector de Hallazgos
   señala hooks con ≥15 disparos y ≥10k tokens — el Hook A (contexto
   inyectado) calificaría. Decisión: el ruteo se AUTO-REPORTA como fila
   propia de consumo (es el pitch de §5.1), nunca lista blanca
   silenciosa en el detector.
b. **LISTA BLANCA del relevo** (docs/remediacion.md): /compact y /clear,
   nada más. NADA de este diseño teclea `/model`; si algún día se
   quisiera automatizar, es cambio de regla dura con su propio diseño,
   no un detalle de implementación.
c. **Invariante #1**: si el perfil conductual entra a `get_local_stats`,
   hay réplica obligatoria en `meter-export.py`. Recomendación v1: motor
   y estado LOCAL-only (los hooks viven en la máquina donde corre Claude
   Code; el ruteo remoto no existe), documentado explícitamente para no
   morder después.
d. **"BD de historial" está en la lista NO** de CLAUDE.md: el log de
   decisiones va en JSON plano (patrón `quota_history.json`), no SQLite.
e. **Hoy la app solo LEE `~/.claude`**; instalar hooks es la primera
   ESCRITURA en territorio de Claude Code. Mismo estándar que el
   análisis local: opt-in apagado por defecto, anunciado en llano, y
   botón "quitar hooks" que deja todo como estaba.
f. **ntfy es de ida**: el push del sábado solo INFORMA ("cuota
   restablecida"); la pregunta "¿volvemos a Opus?" se contesta en la
   app. Cumple la regla de privacidad (porcentajes, horas y frases de
   diccionario; un nombre de modelo no es dato privado).
g. **Invariante #10**: todo texto del gatito/botones pasa por `t()` (8
   idiomas). El `additionalContext` al modelo puede ir en un solo idioma
   (lo lee Claude, no el usuario).

### 10.4 Orden recomendado, ajustado

El orden del §7 se sostiene, con dos cambios:

0. **(nuevo, antes de todo) Experimento de 10 minutos** en el Windows de
   Oscar: un PreToolUse de juguete que reescriba `model` de un subagente
   vía `updatedInput` (objeto completo) y verificar en el JSONL que el
   subagente corrió con el modelo impuesto. Es la apuesta técnica de la
   que cuelga el Hook B; se valida antes de escribir una línea de la
   pieza real.
1-4. Como el §7.
5-6. Sin cambios, pero el 5 espera además el veredicto de análisis
   local v1 (pendiente vigente).

**Cuándo**: no arrancar mientras sigan abiertas las pruebas en vivo del
auto-/clear y del análisis local v1 — comparten zona (coach/gatito) y
contaminarían la medición. Estratégicamente esto ES la apuesta "asesor"
(la #3 de diferenciadores): va después de pulir Windows, como estaba
previsto.

---

## 11. Plan de tareas final (2026-08-13)

Compuerta de arranque: cerrar las pruebas vivas (auto-/clear y análisis
local v1). Cada etapa se termina, se mide y se valida antes de la
siguiente; las etapas 1-3 son el MVP publicable.

**Etapa 0 — HECHA Y VALIDADA (2026-08-14, VPS por SSH).** Autopsia
completa en la bitácora; el experimento vive en `scripts/ruteo-etapa0/`.
A/B con Claude Code 2.1.231, padre en Sonnet, subagente
`general-purpose`: con el hook, el `agent-*.jsonl` dice
`claude-haiku-4-5-20251001`; sin él (control, 27 s después),
`claude-sonnet-5`. El plan B (frontmatter `model:` /
`CLAUDE_CODE_SUBAGENT_MODEL`) NO hizo falta y queda de respaldo.
Tres reglas duras que salieron de ahí, para el Hook B:

- El matcher DEBE ser `Task|Agent`: el nombre de la herramienta no es
  estable entre builds (aquí llegó como `Agent`). Si no dispara,
  sospechar del nombre antes que del script.
- El input NO trae `model`; `updatedInput` lo AÑADE. Y traía
  `run_in_background`: por eso se devuelve el objeto COMPLETO.
- `updatedInput` a secas basta — sin `permissionDecision: allow`.
- Regalo: el payload trae `cwd`, `session_id`, `permission_mode` y
  `effort:{level}`. El `cwd` resuelve el proyecto sin adivinar.

CERRADA la misma tarde con la corrida en Windows nativo (v2.1.232,
Sonnet 5): hook roto → `claude-sonnet-5`, hook arreglado →
`claude-haiku-4-5-20251001`, 5 min de diferencia en la misma sesión.
Los dos mundos cubiertos: Linux (VPS por SSH; WSL es el mismo caso —
mismo script, mismo `~/.claude`) y Windows nativo. Los tres hechos de
arriba se repitieron IDÉNTICOS en ambos.

**Etapa 1 — La nota del refri (`router_state.json`). HECHA (2026-08-17).**
El ciclo del panel (único llamador del endpoint) escribe el estado
grueso: bucket redondeado, horas al reset, modo del proyecto, cooldown.
Los hooks solo LEEN; >10 min de viejo = no hacer nada (fail-quiet).

Cómo quedó: `pushRouterState()` (frontend, junto a `logQuota`, mismo
guard `simRunning` — el simulador JAMÁS alimenta a los hooks) manda a
`save_router_state` (Rust, async+spawn_blocking por el SSH) el JSON
`{v, ts, session_pct, session_reset_h, week_pct, week_reset_h}` con los
% redondeados a 5 (la nota no debe cambiar en cada ciclo sin necesidad,
§5.1). Destinos: `~/.michiclaude/` local, cada home WSL con Claude (por
`\\wsl.localhost`, fs puro) y cada servidor SSH (`upload_state`, fallar
es gratis). Con el interruptor apagado (`ruteo.json` en APPDATA) NO se
escribe nada: sin hooks no hay lector, y a los 10 min cualquier hook
huérfano se apaga solo. El modo del proyecto y el cooldown quedaron
para la etapa 4 (son del gatito consejero): la nota es ADITIVA, añadir
campos no rompe a los hooks viejos.

**Etapa 2 — Hook B, el ahorrador silencioso (subagentes). HECHA Y
VALIDADA EN VIVO (2026-08-17, VPS; falta el lado Windows, abajo).**
REGLA DURA que salió de la etapa 0 (2026-08-14): el `.ps1` va en ASCII
PURO. Windows PowerShell 5.1 lee los archivos sin BOM como ANSI, y un
guion largo se vuelve `â€"` cuyo `"` cierra la cadena a media línea —
el script no compila y el hook muere (sin bloquear, eso sí). Como el
script viaja embebido con `include_str!`, esto NO lo avisa nadie en
tiempo de compilación: se comprueba con un grep de no-ASCII.
Script embebido (patrón `include_str!` de meter-export.py), botón
opt-in "Activar ruteo" + botón "Quitar hooks" que deja todo como
estaba; escritura atómica en settings del usuario. Log de decisiones en
JSON plano (patrón quota_history.json — NUNCA SQLite). Exploración →
Haiku; implementación → Sonnet; análisis → según cuota.

Cómo quedó (reglas VIGENTES del Hook B):

- `scripts/router-hook.py` (Linux/WSL/SSH) y `scripts/router-hook.ps1`
  (Windows nativo), RÉPLICAS EXACTAS — tocar uno = tocar el otro, como
  el exportador. Embebidos en el exe; editar el .py en el servidor no
  tiene efecto (se re-sube al encender).
- Clase por NOMBRE del `subagent_type` (subcadenas, nunca keywords del
  prompt — §4.5): LIGHT (`explore/search/scout/locate/lookup/grep`) →
  haiku SIEMPRE; THINK (`plan/review/audit/analy/research/judge/verify/
  architect/security`) → sonnet SOLO con presión (peor bucket ≥70,
  `PRESSURE`); resto (implementación) → sonnet. `model` explícito en el
  input SE RESPETA; prompt que empieza por `~` = escotilla; estado
  ausente/viejo = silencio total (ni una línea de log).
- El alta/baja la hace el guion `RUTEO_PY` (Rust, por STDIN como
  CHAT_WRAP_PY) en SSH/WSL y `ruteo_local()` en Windows — misma lógica
  los dos lados: respaldo `.michi-backup` una vez, merge atómico
  (tmp+rename), MANUAL si el settings no parsea, NOHOOK si el script no
  llegó, BADOP para op desconocida (mordida del wrapper 2026-08-10).
  Apagar quita SOLO nuestra entrada (huella `router-hook` en el
  command), poda ramas vacías y borra el script. `ruteo_log.jsonl`
  rota a `.1` al pasar de 512 KB.
- Interruptor en Ajustes (claves `rt_*`, 8 idiomas): una fila por
  máquina con veredicto traducido (patrón del wrapper del chat) y el
  recordatorio de que los hooks se fotografían al ARRANCAR Claude Code.

VALIDADO EN VIVO (VPS, sesión headless `claude -p`, 2026-08-17): con
nota fresca, padre en Sonnet + subagente Explore → el `agent-*.jsonl`
REAL dice `claude-haiku-4-5-20251001` y el log anota
`route/light/haiku`; SIN nota, el mismo subagente hereda
`claude-sonnet-5` y el log NI CRECE; `off` deja el settings.json
IDÉNTICO byte a byte (con un hook ajeno de por medio en el banco de
pruebas, intacto). Matriz sintética 16/16 del .py y del guion. FALTA:
`cargo check` en el Windows de Oscar (el VPS no tiene toolchain),
primera corrida real del `.ps1` y el interruptor de Ajustes en vivo.

**Etapa 3 — La medición (pestaña Reporte). HECHA (2026-08-17).**
Cruce del log de decisiones × JSONL reales × quota_history.json:
"N subagentes redirigidos, X tokens ahorrados, el ruteo costó Y".
Incluye el AUTOCONSUMO del propio sistema (invariante #8: correlación
honesta, jamás un % sin base). Con esto el Hook B ya es demo publicable.

Cómo quedó (reglas VIGENTES): `scan_ruteo` en Rust y en el exportador
(`--ruteo --days N [--end E]`, precios por stdin) — RÉPLICAS. Cada fila
`route` busca su `agent-*.jsonl` (mismo sid en cualquier proyecto, nacido
en [ts−5, ts+180] s, misma familia de modelo, sin reutilizar) y suma sus
tokens; ahorro = tokens × (tarifa del PADRE − tarifa del impuesto). El
padre lo trae la fila (`parent`, lo anota el Hook B desde el transcript)
y las filas viejas caen al transcript madre con tolerancia +5 s (el hook
anota segundos enteros y el transcript milisegundos — mordió). SIN
padre o SIN transcript casado NO se factura (se cuenta como "sin casar").
Contexto inyectado = autoconsumo estimado (60 tok/evento) con «~».
`get_ruteo_report(days,end)` = local + WSL (fs) + SSH (una fila por
origen; exportador viejo → sin fila). Tarjeta `repRuteo` en Reporte:
ahorrado, ruteados/casados, costó/habría costado, "SUBIERON" en contra
(padre Haiku + implementación → Sonnet es un upgrade y se dice), frenos
del guardián, autoconsumo. Verificado con datos reales del VPS (4/4
casados, un caso a mano al céntimo).

**Etapa 4 — El gatito consejero (cambios de sesión con histéresis).
HECHA (2026-08-17; falta la primera tarjeta en vivo).**
Perfil conductual colgado del escaneo del coach (NO segunda pasada de
JSONL): contador de turnos ligeros que se REINICIA con cada turno de
código (regla de sesión mixta). Sugerir bajar SOLO con 8-10 ligeros
consecutivos + cuota apretada; subir sin cooldown; memoria de rechazo
(1 no = silencio en la sesión; 3 no = modo manual del proyecto).
Botones → settings.json atómico → "aplica desde tu PRÓXIMA sesión"
(honestidad literal). Push ntfy al reset SOLO informa. Textos por t().

Cómo quedó (reglas VIGENTES): en el motor del coach (Rust y exportador,
réplicas) `light` = turnos HUMANOS consecutivos sin Edit/Write y con
salida <1500 tok (`COACH_LIGHT_OUT`); se juzga el turno ANTERIOR al
llegar el siguiente humano; racha a 0 con cualquier edición o salida
larga; hit `light` UNA vez por racha al llegar a 8 (`COACH_LIGHT_MIN`),
con `model` (campo aditivo de CoachHit), turns, scwd, title. El motor
NO mira la cuota: la compuerta vive en `coachPoll` (modelo opus/fable/
mythos Y peor gauge ≥70 `LIGHT_QUOTA_PCT` Y sin 3 «no» en el proyecto
en 30 días — `lightNo` en localStorage; sin lectura de cuota no se
aconseja). Tarjeta `light` en Consejos con tres botones; `set_default_
model(scope,model,cwd,origin)`: "project" → `.claude/settings.local.
json` del scwd, "user" → settings.json de esa máquina (local/WSL/SSH,
guion SETMODEL_PY con parámetros en base64, modelo de LISTA CERRADA,
respaldo + atómico). "Hasta el reset" guarda `lightRevert`; al cambio
de ventana semanal (`trackResets`) nace la tarjeta `lightrev` con
"Volver a X" / "Seguir en Sonnet". NADA de esto teclea `/model` (lista
blanca del relevo intacta): solo cambia el DEFAULT de sesiones nuevas,
y la tarjeta lo dice literal. Validado con sesión sintética y contra
las sesiones reales del VPS (una sesión de 378 turnos con ediciones da
light=0: no molesta en sesión mixta).

**Etapa 5 — Hook A, el guardián. HECHA Y VALIDADA EN VIVO (2026-08-17,
VPS; se adelantó a la 3-4 a petición de Oscar: es el error CARO).**
(a) Bloqueo de escalada: señales estructurales (bloque de código, rutas,
imperativo largo — nada de keywords), exit 2 con mensaje, prefijo `~` de
bypass. (b) Contexto inyectado (~60 tok/turno): apagable por separado y
AUTO-REPORTADO como consumo propio — hooks_noise no le hace lista blanca.

Cómo quedó (reglas VIGENTES): `scripts/guard-hook.py` / `.ps1`
(RÉPLICAS, embebidos) en `UserPromptSubmit`, instalados JUNTO al Hook B
por el mismo alta (RUTEO_PY / ruteo_local gestionan los dos; ON = ambos).
Se gobierna con las banderas `guard` y `ctx` que viajan DENTRO de la
nota (ruteo.json → save_router_state): apagarlas no toca settings.json.
Modelo de la sesión: cola de 64 KB del `transcript_path` (0.4 ms) →
respaldo `model` de settings.json → si no, NO se adivina (turno 1 de una
sesión nueva pasa siempre). Solo actúa en haiku/sonnet. Señales y pesos:
≥2 fences de código = 2; ≥2 rutas = 1; traza de error = 1; "largo" = ≥60
palabras O ≥300 caracteres sin «?» final = 1 (por caracteres porque
japonés/chino no separan palabras — mordió en pruebas). Umbral: haiku 1,
sonnet 2. Bloqueo = JSON `{decision:"block", reason}` bilingüe (el hook
no tiene el diccionario del panel). Insistencia: el MISMO prompt (sha1)
en <10 min con el mismo tier PASA (`guard_last.json`); `~` = escotilla;
comandos `/` ni se miran. Al log SOLO señales/conteos/plen — JAMÁS el
texto del prompt. `ctx` inyecta `additionalContext` (modelo + cuota
gruesa, en inglés: lo lee Claude) y anota `ev:ctx` para el autoconsumo.
VALIDADO EN VIVO (VPS, `claude -p --resume`): bloqueo real SIN gastar
(assistant turns siguió en 1), insistencia pasa, `~` pasa, Opus nunca
bloquea, ctx citado LITERAL por Claude con la cuota real. Matriz 24/24.

**El interruptor del modelo TOP (2026-08-17 (20), petición de Oscar).**
El ruteo nunca tocaba el modelo más caro (hoy fable): el guardián no
escalaba a él y el Hook B no lo daba a nadie. Oscar pidió un interruptor
PROPIO para ese modelo (y para el que venga más caro después): apagado =
todo como antes; encendido = «entra a la lógica igual que los demás».
Cómo quedó (reglas VIGENTES):

- `RuteoCfg.top` (`ruteo.json`, nace apagado, casilla en Ajustes bajo el
  ruteo, deshabilitada sin ruteo). El ALIAS del top NO se configura: es el
  ÚLTIMO de `RELAY_MODEL_ALIASES` (lista cerrada del relevo, ordenada de
  barato a caro; `top_alias()` en lib.rs) — la casilla lo pinta con
  `prettyModel` (`get_ruteo_cfg` devuelve `top_alias`). Cuando salga un
  modelo más caro: añadirlo AL FINAL de esa lista, de `LADDER` en los dos
  guard-hook y de las listas del relevo (michi-relevo.py, relevo/main.rs,
  MODEL_CHOICES) — y el interruptor lo sigue solo. NO se busca en tablas
  de precios (un alias sin versión no se sabe cobrar).
- Viaja en la nota como `top: "<alias>"` SOLO con el interruptor
  encendido; apagado NO se escribe el campo (los hooks lo tratan como
  inexistente — un hook viejo tampoco lo mira). Llega a local/WSL/SSH en
  el mismo empuje que las demás banderas.
- Hook B (réplicas .py/.ps1): subagente THINK, sin presión (<70) Y con
  cuota SOBRADA (`TOP_ROOM` = 50, peor de sesión/semana; SIN cifras no hay
  holgura) Y padre que no vaya ya en el top → nace en el top
  (`route/think-top`); entre 50 y 70 hereda como siempre; LIGHT y WORK
  ni se enteran. En el Reporte cuenta como «subieron» (es un upgrade y se
  dice; invariante #8).
- Guardián (réplicas): con `top` la escalera llega al último peldaño —
  peso ≥3 (`TOP_PESO`) = destino top desde cualquier tier de abajo — y
  opus pasa a ser ESCALABLE con umbral 3 (haiku 1 / sonnet 2 siguen
  igual). Sin `top`, opus nunca frena y a fable no se sube. Alias que no
  sea peldaño por ENCIMA de opus = como sin top (no se escala a ciegas).
  Sin peldaño al que subir no hay freno (antes caía a «opus» por
  defecto; el caso no se daba y ahora no puede darse).
- La lista blanca del relevo ya aceptaba `/model fable`: no cambia.
- Salieron de la prueba en vivo (VPS): (a) los hooks EMBEBIDOS solo se
  re-subían al ENCENDER el interruptor — tras actualizar la app las tres
  máquinas seguían con la versión vieja (el guardián decía «opus» con
  `top` en la nota). Ahora `ruteo_refresh_scripts()` los refresca AL
  ARRANCAR (solo con ruteo ON, solo los scripts, hilo aparte, como el
  exportador). (b) El latido «N → Haiku, M → Sonnet» era fijo: ahora el
  desglose es DINÁMICO por destino (invariante #6). (c) `insist`/`resent`
  llevan `to` (la memoria `guard_last.json` lo recuerda): con el top ya
  no es siempre opus.
- Verificado con matriz sintética 29/29 de los dos .py (HOME temporal;
  Hook B: sin top hereda / top+20 → fable / top+60 hereda / top+75 →
  sonnet / padre fable hereda / alias raro, booleano o mayúsculas / sin
  cifras NO sube; guardián: sin top opus nunca frena y sonnet peso 4 →
  opus jamás fable; con top opus peso 4 → fable, opus peso 2 pasa,
  sonnet peso 2 → opus, peso 3 → fable, fable nunca frena, top=opus o
  desconocido = como sin top, guard OFF nada, insistencia pasa). Los
  .ps1 revisados a ojo (sin pwsh en el VPS); `cargo check` y la casilla
  en vivo quedan para el Windows de Oscar.

**Etapa 6 — v2 opcionales.**
Análisis histórico en frío con el modelo local (espera el veredicto de
análisis local v1) y embeddings ONNX en el bloqueo solo si el beta
muestra huecos reales.
