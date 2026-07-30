# Consejos — el coach estático de MichiClaude

> Diseño acordado entre Oscar y Claude el 2026-07-30, tras descartar el
> modelo local (ver docs/analizador-fugas.md §5) y refinarlo en debate.
> **Leer completo antes de escribir código de esta sección.** El estado de
> implementación vive en CLAUDE.md; aquí vive el porqué de cada decisión.

## 1. Qué es (y qué no es)

Una sección del panel que ayuda al usuario **mientras trabaja**, con tres
piezas y NINGUNA genera texto:

1. **Motor de reglas** — cuenta y compara umbrales sobre el log de la
   sesión activa. Cero IA, constantes con nombre, mismo patrón que los
   detectores de Hallazgos.
2. **Chuleta curada** — fichas de texto escritas a mano, en los 8 idiomas
   vía `t()`, que viajan en la app y se actualizan con el auto-updater.
   La frescura viene de las releases, no de ningún modelo.
3. **Plantilla de resumen de sesión** — rellena huecos con el `ai-title`
   del JSONL + contadores medidos. Sin prosa generada.

Lo que NO es: un chat, un modelo local, nada que gaste tokens. El
modelo-lector (leer la chuleta y redactar encima) queda como fase 2
OPCIONAL y como módulo APARTE, con compuertas medibles anotadas fuera del
repo (`~/.michiclaude/notas-coach-local.md`): demanda real en issues,
meseta de la FAQ, banco de preguntas con ≥90% correcto-anclado y cero
consejos peligrosos. Datos de hardware que enmarcan: el requisito oficial
de Claude Code es 4 GB sin GPU — un modelo de 2 GB excluiría justo al
segmento que Anthropic decidió no excluir, por eso jamás va en el
instalador.

## 2. La línea entre Hallazgos y Consejos

**El corte es el TIEMPO, no el tema.**

- **Hallazgos = autopsia.** Mira hacia atrás, con costo MEDIDO: "esto ya
  te costó $63".
- **Consejos = prevención.** Mira la sesión en curso, donde el gasto
  todavía es evitable: "vas al 68%, aún estás a tiempo".

La regla para el usuario: *"¿cuánto me costó?" → Hallazgos; "¿qué hago
ahora?" → Consejos.*

Que el mismo saber (p. ej. la regla de /clear vs /compact) aparezca en los
dos NO es duplicación: es la MISMA ficha de la chuleta citada desde dos
momentos. Una sola base de conocimiento, dos puertas. Los textos de fix de
las tarjetas de Hallazgos referencian fichas del mismo catálogo — nunca se
escribe el mismo consejo dos veces en dos archivos.

Escalada natural (no eco): si el usuario ignora el consejo a media sesión,
el hallazgo del día siguiente le enseña la factura medida de haberlo
ignorado.

## 3. Cómo ve el log sin romper la promesa

MichiClaude está AFUERA mirando archivos ("si muere, Claude Code sigue
igual") y eso no se toca. Tres niveles de frescura:

1. **Pasada diaria** (ya existe) — alimenta Hallazgos.
2. **Sondeo de la cola del log de la sesión activa en el ciclo normal del
   panel** (~1–3 min de retraso) — ESTE es el nivel del coach. El log en
   disco se actualiza turno a turno; leer su cola en cada refresco detecta
   "grep ×3" o "contexto alto" al minuto siguiente, que para un coach es
   igual de útil que al segundo.
3. **Hooks opt-in** (futuro, quizá nunca) — exactitud al instante a cambio
   de instalar algo dentro de Claude Code. Solo si el nivel 2 se queda
   corto y siempre como elección explícita del usuario.

## 4. Las reglas del motor

- Cada regla: condiciones medibles + umbrales en constantes con nombre
  (`GREP_MIN`, `CTX_HIGH`…), catálogo CORTO a propósito — un falso
  positivo cuesta la credibilidad del coach entero (misma filosofía que
  MECH_RE).
- **Anti-spam obligatorio**: cada regla dispara máximo UNA vez por sesión,
  más un tope global diario. Un coach que repite es un coach al que
  silencias para siempre.
- **Reglas POR SESIÓN, no globales**: VS Code y la terminal abiertos a la
  vez son dos sesiones con contadores separados.
- Ejemplos del catálogo inicial: comando repetido ≥3 veces (ficha de
  herramienta mejor), contexto alto + cambio de tema (ficha /clear vs
  /compact), inactividad > TTL del caché con contexto grande (ficha del
  caché vencido — la versión preventiva del detector cachebreak).

## 5. La superficie (UX)

- Las fichas se ACUMULAN en la sección Consejos del panel, con contador en
  la pestaña — mismo patrón que Hallazgos. Nada de un globo por ficha:
  tres fichas en una mañana como globos serían spam; como feed, perfectas.
- **Solo escala a globo lo que duele en dólares**, por la vía del "globo
  del día" (paso 2 del analizador) y su umbral configurable.
- Al disparar una regla se muestran SOLO sus 3–4 sub-fichas ligadas (árbol
  contextual); el índice completo de la chuleta vive en la sección, con
  filtro de texto del lado del cliente (~40 fichas, trivial).
- Interruptor en Preferencias, como todo lo demás.

## 6. El molde de la ficha

Se REUSA el molde visual de la tarjeta de Hallazgos (texto curado con
variables + Ignorar + i18n), extraído a clases compartidas, con una
variante `tip`:

- SIN costo y SIN borde de severidad cuando no hay dólares que enseñar
  (una ficha con "$0.00" devalúa el sistema — regla ya aprendida en
  Hallazgos).
- "Ignorar" se convierte en "No mostrar más" y ES el anti-spam manual
  persistente.
- Lo único nuevo: el cajón desplegable de sub-fichas.
- Variables SIEMPRE rellenadas con números medidos ("Repetiste `grep`
  **3** veces") vía las funciones de `I18N` — texto curado fijo, datos
  vivos dentro. Reformular con un modelo distorsiona; sustituir variables
  tipadas no.

## 7. Variantes por plataforma (decidido UNA vez, vale para todas)

Una ficha = un concepto; la plataforma es detalle de RENDER. Nunca fichas
separadas por SO (triplican el catálogo y se desincronizan entre idiomas).

- La ficha lleva campos `cmd: {win, mac, linux, generic}` y el render
  elige.
- **La plataforma correcta es la de la SESIÓN, no la de la app**: sesión
  local → SO de la app; WSL/remota → linux; en duda → `generic`.
- Este selector es el cimiento del pendiente "fix personalizado por
  entrypoint" de Hallazgos: mismo mecanismo, decidido aquí.

## 8. Resumen de sesión por plantilla

«{título}» — {min} min · {n} comandos · {archivos} editados · {extras}

- El título sale del `ai-title` del JSONL — campo INTERNO, no documentado
  (observado en la v2.1.204). Regla de la casa: **solo para display,
  nunca para lógica**, con cascada de respaldo ai-title → nombre del
  proyecto → primer comando. Si Anthropic lo quita, se pierde el título
  bonito y nada más.
- Todo lo demás son contadores propios, medidos.

## 9. Preguntas sin ficha (la cola larga, sin telemetría)

- Cuando el usuario busca algo que la chuleta no cubre, la app lo guarda
  LOCALMENTE (`faqMisses`) — nosotros no vemos nada, no hay telemetría.
- La sección muestra "N preguntas sin respuesta este mes" y un botón que
  abre un issue de GitHub PRE-LLENADO que el usuario revisa y envía él
  mismo. Cero datos silenciosos; las 20–30 preguntas de la compuerta del
  modelo-lector se juntan solas y a la vista.
- La ficha "pregúntale a Claude" debe además enseñar a preguntar barato:
  en una sesión NUEVA la duda cuesta centavos; dentro de la sesión grande
  arrastra todo el contexto.

## 10. Orden de implementación

1. Paso 2 del analizador (el globo del día, umbral en $ configurable) —
   Consejos lo necesita como salida para lo que duele en dinero.
2. La sección Consejos: molde compartido + catálogo inicial de fichas
   (empezando por el caso /clear vs /compact que originó todo).
3. El motor de reglas de sesión activa (nivel 2 de sondeo).
4. Resumen por plantilla al detectar sesión terminada.
5. faqMisses + issue pre-llenado.
