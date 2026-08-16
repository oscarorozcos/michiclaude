# Hooks de Claude Code, ahorro de tokens y oportunidades para MichiClaude

**Fecha:** 15 de agosto de 2026
**Origen:** Análisis de drstone → mecánica de hooks → alternativas del ecosistema → estrategia MichiClaude

> **NOTA DE ENCAJE CON EL REPO (2026-08-16):** documento de análisis
> EXTERNO, escrito sin contexto del código real. Donde dice "SQLite",
> "daemon" o "watcher" léase: caches JSON en AppData + ciclo del panel
> (invariante #4 prohíbe SQLite; no hay daemon aparte — ver
> `adr-multiharness-y-persistencia.md`). El "flyout" ya no existe: el
> panel es ventana persistente (Oscar 2026-08-14). Y el pendiente §3.1
> ("¿la inyección del hook aparece en los JSONL?") YA ESTÁ RESUELTO por
> el propio proyecto: el detector `hooks_noise` cuenta disparos vía
> attachments `hook_success` — sí aparecen. Cuando este doc y CLAUDE.md
> discrepen, manda CLAUDE.md.

---

## 1. Análisis de drstone (Reikor-Arg/drstone)

### Qué es realmente

Un `echo` de una línea colgado de un hook `UserPromptSubmit`. Todo el plugin es:

```
DRSTONE: keep answers short. NEVER: filler, pleasantries, narrating tool calls, unrequested extras. Code and errors verbatim.
```

Sin skill, sin runtime, sin Node. Equivale a ~15 líneas de JSON en `settings.json`.

### Dónde le gana a caveman

El mecanismo: `UserPromptSubmit` inyecta el stdout del hook pegado al mensaje del usuario, **cada turno, automático**. Ataca un problema real: la instrucción en `CLAUDE.md` se queda hasta arriba del contexto y a las dos horas de sesión el modelo la ignora. Caveman hay que invocarlo manualmente; esto no.

### Dónde vende humo

- El "80–99% de ahorro" no tiene benchmark; es un número con una anécdota atrás.
- Repo con 1 estrella y 11 commits — sin validación de terceros.
- No hace falta instalar nada: se copia el bloque JSON manual. Correr `irm ... | iex` de un repo desconocido que toca `~/.claude/settings.json` es riesgo gratis, sobre todo con hooks y config custom ya presentes.

### Riesgo real

Bajo, pero: ~22 tokens de input por mensaje para siempre, y en tareas donde conviene que el modelo se explaye (arquitectura, debugging raro) corta de más → el usuario pide "explícame más" → turno extra → más tokens netos.

### Config manual (opción segura, sin instalador)

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo DRSTONE: keep answers short. NEVER: filler, pleasantries, narrating tool calls, unrequested extras. Code and errors verbatim.",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

---

## 2. Simulación día a día (números estimados, NO medidos)

**Turno ejemplo:** "cambia el puerto del axum server de 3000 a 8080"

- **Sin hook:** ~340 tokens de output (saludo, narración de tool calls, cambio, tres sugerencias no pedidas, pregunta final).
- **Con hook:** ~45 tokens (tool calls + `src/server.rs:42 → "127.0.0.1:3000" → "127.0.0.1:8080"`).
- Ahorro en ese turno: 295 output − 22 input = **87%** → el turno escogido a modo del que sale el número bonito del README.

### Sesión real (mezcla de tipos de turno)

| Tipo de turno | % | Sin hook (tok) | Con hook (tok) | Ahorro |
|---|---|---|---|---|
| Trivial / conversacional | 40% | 340 | 55 | 285 |
| Con código generado | 40% | 600 | 480 | 120 |
| Explicación / arquitectura | 20% | 900 | 850 | 50 |

- **Ponderado:** 560 → 386 tok/turno = **~31% de ahorro real**, no 87%.
- **Sesión de 50 turnos:** 28,000 → 19,300 = ~8,700 tokens de output ahorrados, ya restados los ~1,100 de input inyectado.
- El código no se comprime — por eso el bloque de en medio apenas se mueve. Y en arquitectura a veces sale caro (turno extra por corte).
- **Conclusión honesta:** ~30% es bueno para 15 líneas de JSON, pero es 30%, no 99%. Ese número exacto — el del usuario, con sus sesiones — MichiClaude puede calcularlo de verdad.

---

## 3. Mecánica del hook: dónde se ejecuta, dónde se mete, qué pasa con compact/clear

### Ciclo del turno

```
Das Enter → corre el hook (echo, LOCAL en tu PC) → stdout se inyecta
como system reminder junto a tu prompt → Claude lo lee y responde
```

- **Ejecución:** en tu máquina, como proceso de shell, al dar Enter, antes de que Claude vea nada. No sube a ningún lado.
- **Inyección:** para `UserPromptSubmit` (y `SessionStart`, `UserPromptExpansion`), Claude Code toma el stdout en texto plano y lo agrega como contexto visible para Claude, inyectado como system reminder que empieza con el nombre del hook. Queda pegado al mensaje, no arriba de la sesión.
- **Persistencia:** cada inyección se acumula en el historial de la sesión. Turno 50 = 50 copias (~1,100 tokens de input). Van en el prefijo cacheado (baratas), pero ocupan contexto.
- **`/compact`:** el resumen se come las inyecciones, no las conserva. Da igual: el hook re-dispara en el siguiente mensaje. Ese es el punto de diseño — sobrevive al compact solo.
- **`/clear`:** borra todo; el hook sigue disparando igual. El hook no guarda estado — es un echo sin memoria.

### Hallazgos relevantes de docs/issues

1. **Contradicción con el README de drstone:** los docs oficiales dicen que ninguno de los dos canales de inyección (stdout plano / `additionalContext` JSON) produce entrada visible en el transcript; para confirmar entrega hay que revisar el debug log. El README dice "si ves `DRSTONE:` arriba de la primera respuesta, funciona". **Antes de armar el experimento en MichiClaude: verificar si la línea aparece en los JSONL o no** — de eso depende poder detectar automáticamente que el hook está activo. *(Resuelto en el repo: sí aparece — ver nota de encaje arriba.)*
2. **Bug abierto:** hooks instalados *como plugin* no se capturan ni se pasan al agente (issue de plugin hook output). Otro bug: JSON `hookSpecificOutput` marca error en el primer mensaje de sesión nueva; texto plano funciona. **Conclusión: usar la opción A (bloque en `settings.json`, texto plano), no la instalación como plugin.**
3. Exit codes: `exit 0` = éxito (stdout inyectado en los eventos que lo permiten); `exit 2` = bloquea el prompt (stderr al usuario); `exit 1` u otros = error no bloqueante, sigue la ejecución.

---

## 4. ¿MichiClaude puede hacer lo de drstone igual o mejor?

**Sí, y en un punto claramente mejor — pero no en todos.**

- **Igual sin esfuerzo:** escribir 15 líneas en settings.json no es diferenciador.
- **Mejor y exclusivo: cerrar el ciclo.** drstone inyecta a ciegas y vende un 99% inventado. MichiClaude lee los JSONL → puede decir "instalaste esto el martes, tu output por turno bajó de 560 a 390". Medir el efecto es terreno exclusivo de MichiClaude y le falta a todo el ecosistema.
- **Genuinamente superior: condicional.** drstone inyecta siempre lo mismo. MichiClaude sabe en qué punto de la ventana de 5h va el usuario → puede inyectar solo cuando queda poco presupuesto y callarse cuando sobra.

### El costo de la versión avanzada

- El hook deja de ser echo de milisegundos → proceso por mensaje → lag perceptible si tarda, turno roto si truena. La mayor virtud de drstone (cero runtime) se pierde.
- **Escribir en `settings.json` contradice la arquitectura de MichiClaude** (persistencia defensiva *fuera* del territorio de archivos del agente). Editar settings.json = dejar de ser observador = cambio de categoría de producto.

### Orden recomendado de implementación

1. **Medir** (encaja con "encuentra fugas sin gastar un token", riesgo cero, nadie lo hace). Ya.
2. **Recomendar y generar el snippet** para que el usuario pegue. Cero escritura, cero riesgo. Convierte a drstone en feature de MichiClaude en vez de competidor.
3. **Instalar/administrar el hook** — solo con el unlock progresivo de confianza probado, backup y rollback, detrás de opt-in explícito.
4. **Hook dinámico con binario** — el que más superficie de falla agrega por el menor beneficio marginal. Al final.

Regla: **no competir con drstone haciendo un drstone.** Niveles 1 y 2 ganan sin arriesgar nada.

---

## 5. Escenarios 1 y 2 en concreto: un día miércoles

### 9:40 — Detección (escenario 1, corriendo de fondo)

18 turnos en la sesión. El watcher de JSONL calcula la mediana de output por turno: 540 tok. La compara contra la línea base propia de 4 semanas: 380. Estás 40% arriba. No es alerta de límite — es detección de *desperdicio*.

### 9:41 — Tarjeta en el tray (escenario 2)

> **Output alto en esta sesión**
> Mediana: 540 tok/turno · tu base: 380
> 9 de 18 turnos fueron respuestas cortas que salieron largas.
> `[Ver el detalle]` `[Copiar fix]` `[Ignorar 7 días]`

- "Ver el detalle": lista los 9 turnos ofensores con prompt truncado y conteo.
- "Copiar fix": pone en el portapapeles el bloque de settings.json + la ruta del archivo. **MichiClaude no escribe; el usuario pega.**

### 9:45 — El usuario pega y reinicia

MichiClaude tiene un **watcher read-only** sobre `~/.claude/settings.json`. Ve el cambio de mtime, parsea, encuentra el `UserPromptSubmit` nuevo, marca el timestamp. No tocó nada y ya sabe exactamente cuándo empezó el "después". Sesiones subsecuentes quedan etiquetadas `hook: true`.

### Viernes — Reporte que cierra el ciclo

> **Efecto del hook de brevedad**
> 3 días antes · 61 turnos · mediana 512 tok
> 2 días después · 44 turnos · mediana 341 tok
> **Δ −33%** · ~7,500 tokens de output ahorrados
> Costo: 968 tokens de input inyectado

### Datos a guardar (casi todo ya existe)

| Campo | De dónde sale |
|---|---|
| `output_tokens` por turno | ya se parsea del JSONL |
| `bucket` (corto / con código / explicación) | heurística: ¿bloques de código? ¿cuántos? |
| `hook_active` | watcher read-only de settings.json |
| baseline rodante (mediana 28 días) | agregado persistido (JSON, no SQLite — ver nota) |

### El detalle que muerde

La comparación antes/después está contaminada por el tipo de trabajo (lunes arquitectura vs jueves fixes ≠ efecto del hook). Por eso:

- El `bucket` **no es opcional** — comparar dentro de la misma categoría.
- **Mediana, no promedio** (un turno de 3,000 tok destroza el promedio).
- Con menos de ~40 turnos por lado: mostrar como "tendencia preliminar", no publicar el número.

El rigor es el diferenciador. Número honesto con margen = algo que nadie más tiene. Otro "99%" = ser drstone con mejor UI.

---

## 6. Arquitectura "más automática sin romper al usuario": el hook tonto

### La idea: escribir una sola vez, y nunca más

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "michiclaude-hook",
        "timeout": 2
      }]
    }]
  }
}
```

El archivo no se vuelve a tocar jamás. Toda la inteligencia (cuándo inyectar, qué texto, agresividad, si el usuario lo apagó) vive en el estado propio de MichiClaude, fuera del territorio del agente. Una escritura en la vida del producto.

### Evitar el lag: el hook es tonto

```
proceso residente (la app)     →  escribe verdict.txt cada X segundos
michiclaude-hook (cada Enter)  →  lee verdict.txt, lo imprime o no, exit 0
```

La app ya monitorea JSONL, presupuesto y medianas. Deja el veredicto escrito. El binario abre un archivo de ~100 bytes, hace print, muere. Sub-10ms, sin cómputo, sin red, sin base de datos en el camino caliente. Y `verdict.txt` puede estar **vacío cuando sobra presupuesto** — la superioridad conceptual sobre drstone.

### Reglas para no romper nada

- **Fail-open siempre:** archivo inexistente/corrupto/app muerta → `exit 0` sin imprimir. Nunca `exit 2` (bloquearía el prompt). `timeout: 2` como red del propio Claude Code.
- **Escritura inicial bien hecha:** backup con timestamp → parsear JSON existente (no sobrescribir) → merge si ya hay `UserPromptSubmit` de otro plugin → escribir a temporal + rename atómico → re-parsear para verificar → si falla algo, restaurar `.bak` y avisar.
- **Bucle exclusivo de MichiClaude:** los errores de hook aparecen en el transcript → MichiClaude los lee en los JSONL → puede detectar que *su propio hook* falla y auto-desactivarse, revirtiendo settings.json al backup. Ningún plugin puede hacer eso. Encaja con el flujo de auto-remediación.
- **Desinstalación en un clic**, visible en el panel.

### El costo que no desaparece

Aunque sea una escritura, se cruza la línea: el pitch pasa de "solo lee, no toca nada" a "solo lee... excepto una línea, con permiso, reversible". Defendible, pero ya no la misma frase.

### Solución: dos modos, default pasivo

- **Observador (default):** solo mide, muestra el snippet, el usuario pega. Cero escrituras.
- **Activo (opt-in explícito):** pantalla que explica exactamente qué línea agrega y dónde está el backup; MichiClaude instala y administra.

Comercial: modo activo = candidato natural a **Pro**. Gratuito dice qué está mal; de pago lo arregla solo. Encaja con el unlock progresivo de confianza ya diseñado.

---

## 7. Panorama del ecosistema (investigación GitHub)

### Familia A — Brevedad (la de drstone)

| Plugin | Mecanismo | Claim de ahorro | Lo notable |
|---|---|---|---|
| **drstone** (Reikor-Arg) | UserPromptSubmit echo | 80–99% (sin benchmark) | Simplicidad absoluta; cero runtime |
| **concise** (o4f6bgpac3) | Skill/plugin | ~60–70% | **Mejor que drstone:** inglés natural, multi-harness (Codex, Cursor, Cline, Windsurf, Copilot), y "pide más detalle cuando quieras y el modo conciso se reanuda solo" — resuelve el problema del turno de arquitectura |
| **token-saver** (shirish-singh) | UserPromptSubmit toggleable | Estimación modelada, etiquetada | **El más honesto y el más parecido a la filosofía MichiClaude:** ON/OFF por usuario, cero overhead en OFF, reporta "ahorrado" como estimación con supuesto visible, separada del uso medido. Skill aparte confirm-first para comprimir prompts, nunca automático |
| **claude-token-efficient** (drona23) | Solo CLAUDE.md con perfiles | 60% (contexto) | Sin hook. Insight clave: "las reglas genéricas ayudan, pero las ganancias reales vienen de atacar fallas específicas que ya observaste" — argumento directo pro-MichiClaude: primero medir, luego recetar |
| **cc-token-saver** | Modo cost-conscious | 30–60% | Combina concisión + routing + patrones de workflow |

### Familia B — Sobrevivir a compact/clear

| Plugin | Eventos | Cómo preserva | Costo tokens | Notas |
|---|---|---|---|---|
| **context-handoff** (who96) | PreCompact, **SessionEnd(clear)**, SessionStart(compact\|clear) | Extracción mecánica: últimos 15 mensajes usuario (dedup 85%), 10 snippets asistente (filtrados/truncados), rutas de archivos de tool inputs. Restaura como `additionalContext` | **Cero** (sin LLM) | **El único que cubre `/clear`.** Fallback `latest-handoff.md` con guardas: mismo cwd + ventana de edad (default 900s). Limitación: hooks no pueden reescribir slash commands — reemplazo automático de /compact requiere supervisor externo (Python). 1 estrella, 6 commits, artesanal (rutas del autor hardcodeadas en README) |
| **compact-plus** (u-ichi) | PreCompact | Respaldo del transcript + state file de 10 secciones generado por LLM; acepta instrucciones naturales en `/compact` como guía prioritaria | **Alto** (llamada LLM por compact) | Backends primario/fallback configurables por env vars; presupuesto de hook de 180s |
| **compact-ops** (kenimo49, derivado de compact-plus) | PreCompact, SessionStart(resume), UserPromptSubmit | Igual que compact-plus + al resumir inyecta state propio o el más reciente del proyecto (72h) + warning de uso con recitación de 3 líneas una sola vez | Alto | **Todos los hooks fail-open** — si un hook falla, ni compactación ni prompt se detienen. El mejor ejemplo de ingeniería del grupo. Limitación conocida: no puede pre-avisar si el auto-compact llega en un solo turno gigante |
| **magic-compact** (aerovato) | Comando manual `/magic-compact` | "Lossless": preserva el esqueleto, reemplaza turnos viejos del asistente con resúmenes de alta fidelidad, mensajes del usuario verbatim, poda tool I/O voluminoso pero recuperable vía `read_omitted_content`. Recompactable | Medio | El más ambicioso conceptualmente. Compactación bajo demanda, no en el loop agéntico. OpenCode con soporte de primera clase |
| **precompact-hook** (mvara-ai) | PreCompact | Manda los últimos 50 mensajes del transcript a una instancia fresca de Claude (`claude -p`) que genera un "recovery brief" interpretativo — inyectado post-compact | Alto | Insight: el subagente con contexto vacío dedica atención completa a interpretar. Captura "qué significó", no solo "qué pasó" |
| **morph plugin** (morphllm) | PreCompact + SessionStart | Compacta con su API externa; inyecta prompt pidiendo al summarizer nativo que casi no resuma | Requiere API key externa | Admite que el prompt injection no es fiable; no se puede desactivar el compaction nativo |

**Contexto:** hay múltiples feature requests abiertos en anthropics/claude-code pidiendo mejores hooks Pre/PostCompact — el nicho existe porque la herramienta nativa se queda corta.

### Familia C — Model routing (¡compite con el roadmap de MichiClaude!)

| Plugin | Mecanismo | Lo notable |
|---|---|---|
| **claude-model-router-hook** (tzachbon) | SessionStart + UserPromptSubmit + PreToolUse | El más serio: clasificación effort-first, heurísticas primero + fallback opcional a haiku headless para prompts ambiguos, **boundary damping** (prompt cerca del límite advierte en vez de cambiar) = la histéresis de MichiClaude con otro nombre. En autoswitch escribe el modelo recomendado a settings.json para la *siguiente* sesión; nunca cambia mid-flight. Enforcement de routing en sub-agentes al spawn |
| **smart-model-router** (maiha28781-cloud) | UserPromptSubmit clasificador | Sin API calls, cero latencia. **Ya trae analytics:** total clasificados, % bloqueados por modelo equivocado, breakdown de recomendaciones, top flujos de mismatch, NDJSON estructurado |
| **claude-router** (bmersereau) | UserPromptSubmit + subagentes | Híbrido reglas+LLM; redujo su propio overhead de 11.9k a 3.4k tokens. Estima 50–70% |
| **gearbox** (Adityaraj0421) | SessionStart policy | T0 haiku exploración/mecánico, T1 sonnet implementación, T2 opus diseño/debug duro |
| **fable-baton** (realgarit) | Orquestador | Fable 5 como orquestador token-frugal con subagentes tiered Opus/Sonnet/Haiku. Dato técnico útil: los hooks NO reciben el modelo de sesión al startup — lo lee del transcript (sesión nueva sin transcript = fallback de auto-aplicación) |
| **TokenWise** (CodeShuX) | Router | Se describe como "measurement-driven" — el más cercano en lenguaje al posicionamiento MichiClaude |

**Implicación:** el routing ya no es territorio virgen. La ventaja de MichiClaude ya no es *hacerlo*, es hacerlo **informado por datos históricos reales del propio usuario** (histéresis calibrada con su uso, no keywords genéricos).

### Familia D — Medición (los vecinos filosóficos de MichiClaude)

| Proyecto | Qué hace |
|---|---|
| **karanb192/claude-code-hooks** | **El que hay que estudiar con lupa.** Marketplace de hooks de safety/cost/observability. Leaderboard de costo de contexto por archivo (atribuye tokens de cada tool result a los archivos que cargó), "flight recorder" personal (tasa de fallo, churn de edits, tokens/tarea por versión de modelo), scorecard de cumplimiento de CLAUDE.md (qué reglas ignora Claude crónicamente), registro de dead-ends (enfoques intentados y revertidos con costo estimado, advierte antes de reintentar). Recorders async en background, ~cero latencia. Autor: Head of AI en ArmorCode, base de charla en OWASP GenAI Summit. **Diferencia clave: él vive dentro de Claude Code; MichiClaude vive fuera, con UI real** |
| **Claude-Token-Saver** (awesomo913) | Otro ángulo: ahorro de INPUT (88%) pre-cargando snippets dirigidos en vez de dejar que Claude lea archivos completos. App Windows con exe |
| Otros del topic token-saving | Lazy-loading de tools MCP (85%+ en schemas), delegación a sub-agentes de otros modelos, restauración de sesión leyendo transcripts sin LLM ("cheaper than /compact"), contadores live en statusline, dashboards HTML de costo con timeline de ventana de 5h |

---

## 8. Comparativa de decisión: compact/clear

| Criterio | context-handoff | compact-plus / compact-ops | magic-compact |
|---|---|---|---|
| Cubre `/clear` | **Sí, único** | No | No |
| Costo en tokens | Cero (mecánico) | Alto (LLM por compact) | Medio (resume turnos viejos) |
| Calidad del rescate | Literal, sin interpretación | Alta (10 secciones estructuradas) | La más alta (esqueleto + podado recuperable) |
| Robustez | Artesanal (1★, 6 commits) | compact-ops: todo fail-open | Requiere compactar manual con su comando |
| Madurez | v1.0.0 inicial | Familia iterando con derivados | El más pulido conceptualmente |

**Veredictos:**
- **Para la molestia de `/clear`:** context-handoff, no hay alternativa. Pero NO con su instalador: copiar la idea de sus 3 hooks a mano en settings.json y ajustar env vars (`HANDOFF_MAX_USER_MESSAGES`, `HANDOFF_MAX_ASSISTANT_CHARS`, `HANDOFF_DEDUP_THRESHOLD`, `HANDOFF_LATEST_MAX_AGE_SEC`). Son ~200 líneas auditables en 20 minutos.
- **Para compact en general:** compact-ops si prioriza robustez; magic-compact si el dolor es la calidad del resumen.
- **Advertencia:** compact-plus/ops gastan tokens de la suscripción (LLM) para ahorrar contexto — es un trade, no ahorro neto.

---

## 9. ¿MichiClaude puede hacer lo de compact/clear? Sí, con ventaja estructural

Todos esos plugins comparten el mismo problema: solo capturan en el instante del hook, con lo que alcancen a leer, corriendo como script suelto.

**MichiClaude ya tiene lo que ellos construyen a mano:** lee los JSONL continuamente. El handoff que context-handoff genera con prisas en el SessionEnd puede estar **ya calculado y actualizado todo el tiempo**: últimos archivos tocados, últimos prompts, comandos corridos, proyecto activo.

### Arquitectura propuesta (reusa el patrón del hook tonto)

1. **SessionEnd / PreCompact hook** — avisa a la app "sesión X murió por clear/compact" (telemetría hacia MichiClaude; no inyecta nada al modelo; cero riesgo — encaja en modo observador).
2. **SessionStart hook** — binario tonto que lee un `handoff.md` que la app mantiene fresco, lo imprime, exit 0. Fail-open.
3. La app decide **qué** va en el handoff con calma, entre turnos, no en los 5 segundos de un hook.

### Lo que ninguno de ellos puede hacer

**Mostrar el handoff al usuario en el panel ANTES de necesitarlo** ("esto es lo que Claude recordará si haces /clear ahora"), con opción de editarlo. Ellos son cajas negras; MichiClaude tiene UI.

### Encuadre de producto

- Requiere modo activo (hooks en settings.json) — opt-in, una sola escritura — **candidato natural a Pro**.
- "Tu sesión sobrevive al /clear" es más vendible que "respuestas más cortas": es dolor inmediato y sentido.
- **Acción práctica ya:** instalar la idea de context-handoff a mano esta semana para quitar la molestia; usarlo como prototipo vivo — lo que guste y lo que falle es el spec de la versión MichiClaude.

---

## 10. Síntesis estratégica para MichiClaude

1. **No construir el hook de brevedad como diferenciador** — concise y token-saver ya lo hacen mejor que drstone. La jugada es **medir**: hay 5+ plugins prometiendo 60–88% y nadie con número verificado. "Probamos los 5 con datos reales de JSONL" es artículo/feature que posiciona arriba de todos. Refuerza el pitch "encuentra fugas sin gastar un token".
2. **El patrón fail-open + hook tonto + estado afuera está validado en el ecosistema** (compact-ops lo confirma como práctica).
3. **PreCompact/SessionEnd como telemetría hacia MichiClaude** = timestamp exacto de compact/clear sin inferirlo del JSONL. Cero riesgo, modo observador.
4. **Apurar el routing o reposicionarlo:** la ventaja ya no es hacerlo, es calibrarlo con datos históricos reales del usuario.
5. **Roadmap de escalada:** medir (ya) → recomendar snippet (ya) → instalar/administrar con opt-in y rollback (Pro) → hook dinámico con binario (último).
6. **Pendiente de verificar antes del experimento:** si la inyección del hook aparece o no en los JSONL (docs vs README de drstone se contradicen) — determina la detección automática de `hook_active`. *(Resuelto: ver nota de encaje.)*
7. **Estudiar karanb192/claude-code-hooks a fondo** — el vecino filosófico más cercano (medición async, atribución de costo por archivo, flight recorder), con la diferencia de que MichiClaude vive fuera del harness y con UI.

---

## Referencias

- drstone: https://github.com/Reikor-Arg/drstone
- concise: https://github.com/o4f6bgpac3/concise
- token-saver: https://www.claudepluginhub.com/plugins/shirish-singh-token-saver-token-saver
- claude-token-efficient: https://github.com/drona23/claude-token-efficient
- context-handoff: https://github.com/who96/claude-code-context-handoff
- compact-plus: https://github.com/u-ichi/compact-plus
- compact-ops: https://github.com/kenimo49/compact-ops
- magic-compact: https://github.com/aerovato/magic-compact
- precompact-hook: https://github.com/mvara-ai/precompact-hook
- morph plugin: https://github.com/morphllm/morph-claude-code-plugin
- claude-model-router-hook: https://github.com/tzachbon/claude-model-router-hook
- smart-model-router: https://github.com/maiha28781-cloud/claude-smart-model-router
- claude-router: https://github.com/bmersereau/claude-router
- fable-baton: https://github.com/realgarit/fable-baton
- karanb192 hooks: https://github.com/karanb192/claude-code-hooks
- Claude-Token-Saver: https://github.com/awesomo913/Claude-Token-Saver
- Hooks reference (docs oficiales): https://code.claude.com/docs/en/hooks
- Issues relevantes: anthropics/claude-code #13912, #17550, #15923, #17237, #43946, #67898
- Topic token-saving: https://github.com/topics/token-saving
- awesome-claude-code-toolkit: https://github.com/rohitg00/awesome-claude-code-toolkit
