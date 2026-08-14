# Presión de contexto y métricas de rendimiento — análisis de viabilidad

> Estado: **FASES 1 Y 2 IMPLEMENTADAS Y FUNCIONANDO** (cerrado hasta ahí
> por Oscar el 2026-08-07; la cabecera decía "sin arrancar" hasta el
> 2026-08-09 — estaba desactualizada). El manómetro de presión (§tabla
> filas 9-10) también está vivo desde el 2026-08-07 como regla `press` +
> gauge en el widget. Análisis original del 2026-08-05 a partir de un
> documento de estrategia externo (sesión de producto de Oscar).
> LEER ESTE ARCHIVO COMPLETO antes de implementar nada de aquí.
> Lo que SIGUE pendiente de este doc, en una lista: ver §"Qué queda vivo
> de este doc" al final.

## De dónde viene

Oscar trajo un documento de una sesión de estrategia que analizaba el problema
del "context rot" (Claude pierde calidad conforme la ventana de contexto se
llena), el panorama competitivo de herramientas, y proponía niveles de
implementación para MichiClaude. Este archivo cruza ese documento contra lo que
la app YA tiene y deja el veredicto punto por punto.

**Veredicto general:** el "Nivel 1" del documento (donde él mismo dice que está
el 80% del valor) ya lo tenemos casi entero — 6 de 8 puntos completos, 2
parciales. Lo genuinamente nuevo y valioso es la idea de **medir desperdicio en
vez de consumo** (una razón, no un total — "kilómetros por litro") y el
**antes/después anclado a cada arreglo**. Un punto del documento choca de
frente con un invariante (telemetría) y queda descartado salvo decisión
explícita de Oscar.

## Tabla comparativa (2026-08-05)

Leyenda: ✅ ya lo hacemos · 🟡 parcial · ❌ no lo tenemos

| # | Punto del documento | Estado | Cómo lo cubrimos hoy | ¿Viable como mejora? |
|---|---|---|---|---|
| 1 | CLAUDE.md inflado | ✅ | Doble detector: `claudemd` (reglas que ninguna sesión menciona) + detector 10 `claudemdsize` (>40k chars, el límite REAL de carga) | Ya superamos lo que pide el doc |
| 2 | MCPs zombis | ✅ | Detector `mcp_unused` | Hecho. El propio doc avisa: Tool Search nativo ya mitigó este dolor — no apostarle más en el pitch |
| 3 | Archivo releído N veces | ✅ | Detector `reread` + regla `attach` del coach en vivo | Hecho por partida doble |
| 4 | Contexto inflándose "en escalera" | ✅ | Detector `inflate` (+50k y 10+ turnos) | Hecho |
| 5 | Priorización por dinero | ✅ | Tope 12 por costo, severidad roja ≥$10 / ámbar ≥$1 | Hecho — es nuestra filosofía |
| 6 | Proyección "topas el jueves" | ✅ | Marcador de ritmo + burn rate | Hecho |
| 7 | Memoria de decisiones ("no me lo vuelvas a marcar") | ✅ | Ignorar persistente (`fndIgnore`) | Hecho — el doc lo pinta como diferenciador de retención |
| 8 | Honestidad: "estimado", nunca afirmar ahorros | ✅ | Invariante #8, el "~" en costos, `spend_only_cc` | Ya es cultura del proyecto |
| 9 | Aviso "compacta ahora que aún sale bueno" | 🟡 | Coach: ctx ≥120k → ficha `compact` (multi-fuente) | Falta lo VISUAL: manómetro permanente, no solo la ficha |
| 10 | Gauge de presión de contexto en la pastilla | 🟡 | El dato ya existe (el coach calcula ctx de la sesión activa); la pastilla solo muestra cuota | Muy viable y barato: viajaría en `quota:update` como ya viaja el campo `coach` |
| 11 | Detección de auto-compacts frecuentes | 🟡 | `cachebreak` ya identifica compactaciones (para excluirlas); no contamos su frecuencia | Viable y barato: la señal ya está parseada |
| 12 | Sesiones sin `/clear` | 🟡 | `inflate` detecta el síntoma, no el hábito | Viable; medio pelo — se solapa con lo ya avisado |
| 13 | Turnos antes de topar el límite | 🟡 | Proyectamos hacia adelante (burn rate); no guardamos histórico "turnos por ventana de 5 h" | Viable; es la métrica "que la gente siente" |
| 14 | Generador de handoff (resumen de traspaso) | 🟡 | El recibo del coach ya junta min/comandos/archivos | Parcial: botón "copiar resumen de traspaso" en plantilla sí; "decisiones y enfoques fallidos" pediría IA que no tenemos ni queremos embeber |
| 15 | Auditoría CLAUDE.md línea a línea | 🟡 | `claudemd` señala identificadores jamás mencionados | La parte semántica (líneas redundantes) pide modelo → posponer |
| 16 | **Tokens por turno útil** | ❌ | — | **Sí, y barato**: los JSONL ya distinguen turnos de usuario; es una división más en la agregación existente |
| 17 | **Antes/después anclado a arreglos** | ❌ | — | **Sí, y es la joya del doc**: los hallazgos ya tienen ciclo de vida — detectar que uno desapareció, marcar la fecha, comparar rendimiento |
| 18 | % de desperdicio estructural | ❌ | — | FÓRMULA DEFINIDA el 2026-08-14 (§ propia al final); falta la obra |
| 19 | Score de salud único (42/100) | ❌ | — | Dudoso: un número único esconde más de lo que enseña; contradice "nunca inventar cifras". Ya ordenamos por costo real |
| 20 | Sesión contaminada (corrección tras corrección) | ❌ | — | El doc mismo la marca como la de mayor riesgo técnico (falsos positivos). ESPERAR — un falso "tu sesión está envenenada" quema la confianza |
| 21 | Modelo local opcional | ❌ | — | De acuerdo con el doc en NO hacerlo ahora; pelea con invariante #4 (app ligera) |
| 22 | Aprendizaje colectivo | ❌ | — | ⚠ **CHOCA con invariante #3 ("sin telemetría")**. Sería un cambio de postura de privacidad, no una feature. Solo con decisión explícita de Oscar y opt-in |

## Las 3 métricas de desperdicio (lo nuevo del doc)

El problema con comparar consumo: "gasté menos esta semana" no prueba nada —
las semanas no son comparables (más o menos trabajo). Se necesita una **razón**,
no un total: el equivalente a kilómetros por litro.

1. **Tokens por turno útil** — total de tokens ÷ mensajes escritos por el
   usuario. Si baja, mejoraste sin importar el volumen de la semana.
2. **% de desperdicio estructural** — de todo lo gastado, cuánto se fue en
   cosas que no eran trabajo (MCPs nunca invocados, CLAUDE.md recargándose,
   contexto arrastrado). Casi independiente del volumen. FÓRMULA SIN DEFINIR
   — es el trabajo de diseño previo obligatorio.
3. **Turnos antes de topar el límite** — la que la gente siente ("me aguantó
   toda la tarde" vs "me tronó a las 3"). Métrica de portada.

### El truco de credibilidad: anclar en el evento, no en el calendario

NO comparar semana vs semana. Comparar **antes y después de cada arreglo**:
cuando un hallazgo desaparece (el usuario lo arregló), guardar la fecha como
marca y mostrar *"quitaste el MCP de Playwright el 12 de agosto — antes
gastabas 8,400 tokens por turno, ahora 6,100"*. Le enseña al usuario que ÉL lo
arregló y la app solo se lo mostró → retención.

**Reglas de honestidad (alineadas con invariante #8):**
- Siempre "estimado, con N días de datos", nunca "te ahorraste $X" como
  afirmación.
- Mínimo de días de datos tras un arreglo antes de mostrar la comparación
  (definir el umbral en el diseño).
- En pantalla: UN número grande ("tu rendimiento mejoró 34%") y abajo,
  chiquito, qué arreglos lo lograron y cuándo. Nada de gráficas de consumo
  crudo en portada.

**Sinergia con APUESTA #2:** "tu rendimiento mejoró 34%" es exactamente el
número que pide la tarjeta compartible del gatito (marketing). Estas métricas
la alimentan directo.

## Orden sugerido cuando se arranque

1. **Métrica de rendimiento** (tokens/turno útil) + **antes/después por
   arreglo** — la propuesta de valor nueva; alimenta la tarjeta compartible.
2. **Manómetro de contexto en pastilla/gatito** — barato, el dato ya existe en
   el coach, hueco de mercado sin ocupar (nadie hace vigilancia pasiva desde
   la bandeja). La pastilla NO llama a nada: el dato viaja en `quota:update`.
3. **Contador de auto-compacts** como detector nuevo — señal ya parseada.
4. Definir la **fórmula de desperdicio estructural** (diseño, no código) antes
   de prometer esa métrica.

**Regalo de costo casi cero, aplicable desde ya:** el lenguaje del plomero
para el copy — "presión de contexto", "fuga", "purgar la línea", manómetro.
El plomero no dice "context rot"; dice "la presión está en zona roja". Solo
texto en el diccionario `I18N`, alinea toda la narrativa de la app.

## Qué pasa HOY al arreglar algo (el hueco que esto cierra)

Pregunta de Oscar (2026-08-06): "¿cómo veo si aplicar los consejos funcionó?"
Respuesta honesta: hoy la única recompensa es POR AUSENCIA — el hallazgo deja
de aparecer cuando el gasto viejo sale de la ventana de días (no al instante),
el coach deja de regañar, y el gasto baja (pero eso no prueba nada: quizá
trabajó menos). La app nunca dice "funcionó, mejoraste X". Ese silencio es
exactamente lo que el antes/después viene a llenar.

**Dato clave: la métrica de rendimiento es RETROACTIVA.** Tokens por turno
útil sale de los mismos JSONL ya parseados — el día que se implemente, el
primer reporte ya enseña la curva de los 30 días anteriores completos. Lo
único que necesita tiempo real son las MARCAS de arreglo (ver que un hallazgo
llevaba días saliendo y dejó de salir → clavar la fecha); arreglos previos a
la implementación se pueden anotar a mano.

## Reporte ejecutivo periódico (diseño de producto, 2026-08-06)

Petición de Oscar: reporte configurable (semanal/mensual) para un usuario que
está EMPEZANDO y no entiende mucho — lenguaje llano, sin jerga. Estructura
acordada (7 pisos, del más gordo al más fino):

1. **Portada — EL número:** rendimiento (tokens/turno útil) con veredicto en
   palabras: "esta semana tus tokens te rindieron 18% más que la anterior".
   Un número grande, una frase, una flecha. Nada más.
2. **Tu periodo en corto:** 3-4 frases llanas ("trabajaste 12 días, tu
   proyecto más caro fue X, cerraste 2 fugas, se abrió 1 nueva").
3. **La sensación, medida:** los usuarios dicen "esta semana me duró menos" /
   "se me duplicó" — es una sensación Y SE PUEDE MEDIR: (a) turnos antes de
   topar el límite por ventana de 5 h, (b) cuántas veces llegó al 100%
   (sesión y semanal), (c) a qué hora/día se le acabó el semanal. Comparado
   con el periodo anterior: "topaste el límite 2 veces (la semana pasada
   fueron 5)". REQUISITO TÉCNICO NUEVO: hoy NO se persiste histórico de
   cuota (se sondea cada 3 min y se tira) — hace falta un histórico local
   chiquito (JSON con % por ventana; privado, nunca sale de la máquina).
4. **Proyectos de mayor a menor:** costo + tokens por proyecto, y POR QUÉ
   quemó: sus hallazgos y avisos del coach son la explicación ("aquí se te
   fue el 40% en releer archivos"). Cada proyecto con su delta vs periodo
   anterior (mejoró/igual/empeoró).
5. **Marcas de arreglo (antes/después):** qué arregló, cuándo, y el efecto
   medido — con las reglas de honestidad (estimado, mínimo de días).
6. **Hallazgos y consejos del periodo:** cerrados / nuevos / ignorados, por
   proyecto.
7. **Qué mirar el periodo que entra:** 1-3 acciones concretas (la fuga más
   cara primero), en imperativo llano.

**Formatos (decisión pendiente, propuesta):** (a) sección/vista "Reporte" en
el panel; (b) EXPORT HTML autocontenido (como el CSV: una foto, imprimible y
compartible — sinergia con APUESTA #2); (c) push ntfy "tu reporte está listo"
SIN números (privacidad). Texto + tabla de proyectos + gráfica comparativa de
RENDIMIENTO (razón, nunca consumo crudo — el consumo ya tiene su gráfica en
Tendencia diaria y ahí se queda).

**Regla de copy:** cada cifra lleva su frase en llano al lado. El usuario
principiante lee la frase; el avanzado lee el número. Nunca una métrica sin
su "qué significa para ti".

### Decisión de diseño (2026-08-06, con mockups revisados por Oscar)

Se generaron dos mockups con IA externa (prompts en la bitácora de la
conversación): A = reporte tipo documento ejecutivo (820 px, lenguaje muy
llano, tabla de proyectos con "qué lo encareció", tarjetas antes/después,
analogías de gasolina/maleta) y B = pestaña dentro del panel de 446 px.
**Oscar eligió la A como base** — más completa y más entendible para un
usuario común. Plan: adaptar el CONTENIDO de la A a la columna del panel
(pestaña "Reporte" con chips Semana/Mes/Personalizado — el selector
Personalizado reutiliza el calendario de rangos ya existente), y la A casi
tal cual como salida del botón "Exportar reporte (HTML)" — dos vistas del
mismo dato, como ya pasa con el CSV. Los archivos de referencia los tiene
Oscar (opcion a/b michiclaude-reporte-*.html).

**Auditoría de honestidad del mockup A (invariante #8)** — qué datos de la
maqueta son calculables hoy, cuáles necesitan la obra nueva y cuáles NO
existen:
- Calculables ya (retroactivo): tokens por turno útil, días trabajados,
  proyectos de mayor a menor con costo/tokens, hallazgos cerrados/nuevos,
  marcas de arreglo (hacia adelante).
- Necesitan el histórico de cuota (obra nueva ya anotada): "veces que
  topaste el límite de 5 h", "cuándo se acabó el semanal", "mensajes por
  sesión de 5 h".
- NO existe el detector: "pegaste archivos completos 9 veces" (pegar texto
  masivo en el prompt). Detector NUEVO posible — input tokens de usuario
  anormalmente altos por turno — pero hasta no diseñarlo y validarlo, esa
  fila no puede aparecer. El reporte solo enseña "qué lo encareció" con
  frases que salgan de detectores REALES (reread, inflate, mcp_unused,
  claudemd, cachebreak, mech, subagents, hooks_noise, claudemdsize).
- La columna "qué lo encareció" con proyecto sano dice "nada que señalar" —
  eso sí, nunca inventar una causa cuando no hay hallazgo.

**Orden de obra**: 1) motor de datos (tokens/turno en la agregación +
histórico de cuota local + marcas de arreglo) — 3 piezas en sincronía;
2) pestaña Reporte en el panel; 3) export HTML. La pestaña sin el motor
sería una maqueta vacía.

### Fase 1 — motor de datos: IMPLEMENTADA (2026-08-06)

Todo ADITIVO (campos con `#[serde(default)]`, comandos nuevos, colectores
que solo escuchan). Pendiente de `cargo check` en el Windows de Oscar.

- **Turnos útiles (`uturns`)**: `is_user_turn` (Rust y Python, réplica
  exacta) cuenta mensajes HUMANOS reales — fuera `isMeta`, `isSidechain`,
  `toolUseResult`, contenidos con `tool_result`, envoltorios `<command-`,
  `<local-command`, `<ide_…`, `<system-reminder` y `[Request interrupted`
  (el `<ide_…` se cazó en logs reales del VPS: el IDE inyecta avisos con
  rol user sin marcar meta). Dedup global por `uuid` (también cruzan
  archivos). Viven en: `LocalStats.uturns_week`, `ProjectAgg.uturns` y la
  serie `daily` (que ahora lleva `cost`+`tokens`+`uturns` por día — los
  tokens de trabajo por día también hacían falta para el rendimiento
  semanal). Caché de escaneo v2 en AMBOS lados (un caché v1 devolvería 0
  en silencio; el bump fuerza una reconstrucción única). Fusión remota/hub
  suma los tres campos; exportador viejo manda 0 = "sin datos" y la UI
  NUNCA divide entre 0 (invariante #8).
- **Verificación hecha en el VPS** (sin toolchain Rust — `cargo check`
  queda para Windows): regresión con logs CONGELADOS y `--end` fijo →
  campos viejos IDÉNTICOS byte a byte; coherencia de rangos → 7d+7d
  contiguos = 14d exacto en tokens y uturns; muestreo manual de turnos
  detectados → 0 falsos tras el filtro `<ide_`.
- **Histórico de cuota**: `quota_history.json` en el appdata (90 días,
  poda automática, una foto por ciclo con freno de 150 s). Comandos
  `log_quota` (lo llama `refresh()` SOLO con lectura buena del endpoint —
  nunca desde renderQuota, que re-pinta al cambiar idioma y duplicaría;
  nunca con simulador) y `get_quota_history(days)` (clamp 1..90). Campos
  por foto: t, s (% sesión), w (% semanal), sr/wr (resets epoch). LOCAL Y
  PRIVADO: no viaja a hub ni ntfy.
- **Marcas de arreglo**: localStorage `fndHist` (clave→{f: primera vez,
  l: última, t: título}) y `fndMarks` (tope 20). Solo hallazgos de ESTADO
  (mcp_unused, skills_unused, claudemd, claudemdsize, hooks_noise, mech,
  subagents — los de sesión van y vienen y serían ruido). Solo escaneos
  frescos con ventana ≥7 días SIN rango al pasado y sin simulador. Regla:
  visto ≥3 días Y desaparecido ≥2 → marca con la fecha de la última vez
  visto; huella en flowLog ("marca: hallazgo arreglado"). Limitación
  documentada: arreglos ANTERIORES a esta implementación no tienen marca.

### Fase 2 — pestaña Reporte: IMPLEMENTADA (2026-08-07)

Chips Semana/Mes/Personalizado, héroe EFICIENCIA/VOLUMEN, "¿te duró más
o menos?" (histórico de cuota), gráfica de 4 semanas tokens/$, deltas
por proyecto y "qué lo encareció". Reglas vigentes en CLAUDE.md.
Cerrada por Oscar el 2026-08-07: "queda como pendiente por si al usarlo
falta algo". La fase 3 (export HTML del mockup A) sigue sin arrancar.

## La fórmula del % de desperdicio estructural (DISEÑO, 2026-08-14)

> Fila 18 de la tabla y punto 2 de los pendientes. Esto es el diseño previo
> obligatorio: sin él la métrica no se promete. **Todavía sin implementar.**

### La pregunta que responde (y la que NO)

> **De cada $100 que gastaste, ¿cuántos se fueron en cómo está MONTADO el
> entorno, y no en el trabajo?**

No responde "¿cuánto desperdiciaste?" (eso incluiría hábitos, y juzgar hábitos
es "báñate más rápido" — `analizador-fugas.md` §1). Responde por los sellos
rotos: lo que se paga por estar ahí, se haya trabajado o no.

### Por qué "sumar todos los hallazgos ÷ total" está MAL

Tres razones, y las tres invalidan el atajo:

1. **Los detectores se pisan.** `inflate` mide lo que cuesta RELEER el
   contexto (cache_read de la sesión entera): dentro de ese contexto ya viajan
   las líneas del CLAUDE.md, la salida de los hooks y los archivos releídos.
   `mech` cobra el TURNO COMPLETO (input+output+cache_write+cache_read) y no
   excluye subagentes, así que se solapa con `subagents`. Sumarlos cuenta los
   mismos tokens dos y tres veces: es exactamente el error del "210 líneas ×
   40 turnos" que `analizador-fugas.md` §4 marca como el que quema la
   credibilidad de golpe.
2. **Los más estructurales valen $0 hoy.** `mcp_unused`, `skills_unused` y
   `claudemdsize` se emiten con `cost: 0.0` — no están medidos, son resta de
   conjuntos. El hallazgo insignia del pitch (MCPs zombis) aportaría CERO al
   porcentaje.
3. **El tope de 12 los decapita.** `MAX_FINDINGS = 12` ordenado por costo
   descendente: los estructurales son los baratos (pisos de chars/4), o sea
   justo los que el tope corta. El numerador tiene que calcularse **antes**
   del recorte.

### La regla que la hace defendible: una línea de factura, un detector

Cada sumando declara **qué línea de la factura toca**. Dentro de la misma
línea y la misma sesión no puede haber dos detectores:

| Línea | Quién la mide |
|---|---|
| `input` | `claudemd`, `hooks_noise` (y `mcp_unused` el día que se mida) |
| `cache_write` | `cachebreak` |
| `cache_read` | `inflate` — **excluido del numerador** |
| turno entero | `mech`, `subagents` — **excluidos** (mezclan las cuatro) |

Con eso el numerador queda **disjunto por construcción**, sin restar nada a
mano.

### Clasificación de los 10 detectores

| kind | Clase | Costo hoy | ¿Numerador? |
|---|---|---|---|
| `claudemd` | estructural (carga fija por sesión) | piso, chars/4 × sesiones × input | **SÍ** |
| `hooks_noise` | estructural (carga fija por disparo) | piso, chars/4 × input | **SÍ** |
| `cachebreak` | arrastre (contexto reescrito sin trabajo nuevo) | MEDIDO, cache_write | **SÍ** |
| `mcp_unused` | estructural | 0 — sin medir | no, hasta que se mida |
| `skills_unused` | estructural | 0 — sin medir | no, hasta que se mida |
| `claudemdsize` | estructural informativo | 0 por diseño | **NUNCA** (fuga de instrucciones, no de dinero) |
| `inflate` | conductual | MEDIDO, cache_read | no (y se pisa con todo) |
| `reread` | conductual | piso, input | no (vive dentro de `inflate`) |
| `mech` | conductual | MEDIDO, turno entero | no |
| `subagents` | visibilidad | MEDIDO, turno entero | no (no es fuga: es gasto legítimo hecho visible) |

Propiedad de diseño: el numerador es una suma sobre una **CLASE**, no sobre una
lista fija. El día que `mcp_unused` tenga costo medido entra solo, sin tocar la
fórmula.

### La fórmula

```
DE%  =  100 ×  Σ(orígenes) Σ(hallazgos de clase estructural) costo
               ─────────────────────────────────────────────────
               Σ(orígenes) costo total de la MISMA ventana
```

Reglas de cálculo, todas obligatorias:

- **Mismo escaneo.** Numerador y denominador salen de la misma corrida, misma
  ventana (`days`+`end`) y mismos orígenes. Cruzar un `get_findings` de 7 días
  con un `cost_week` de 30 es fabricar un número.
- **Antes del tope de 12.**
- **Fusión multi-origen = suma de numeradores ÷ suma de denominadores.**
  JAMÁS el promedio de los porcentajes de cada máquina (media de razones ≠
  razón de sumas: el portátil que se usó una tarde pesaría igual que el VPS).
  Si un SSH falla, ese origen desaparece de LOS DOS lados — nunca de uno solo.
- **Respeta Ignorar** (`fndIgnore`), y se aplica a los DOS periodos que se
  comparan: como todo se recalcula desde los logs, ambos usan el conjunto de
  ignorados de HOY y la comparación sigue siendo coherente.

### Es un PISO, y se dice "al menos"

Esto resuelve de un golpe la ansiedad del doble conteo. El número **subestima
a propósito**:

- dos detectores estructurales valen 0 (`mcp_unused`, `skills_unused`);
- `claudemd` y `hooks_noise` son pisos declarados (una ingesta, aunque se
  relea en cada turno posterior);
- cada detector tiene umbral (5 líneas, 15 disparos, 300k reescritos): lo que
  no llega al umbral no se cuenta;
- todo lo conductual queda fuera.

El único riesgo de sobreconteo es el prefijo estructural que se REESCRIBE
cuando el caché se rompe (queda dentro de `cachebreak` y también en el piso de
`claudemd`), acotado por `tok_est × rupturas × 1.25 × input` — órdenes de
magnitud menor que lo que se está dejando fuera.

Por eso el copy es **"al menos el X%"**, nunca "el X%". Es la invariante #8
aplicada: un piso solo puede quedarse corto, que es la dirección creíble.

### Trazabilidad: cada punto del porcentaje tiene tarjeta

El numerador usa **los mismos umbrales que las tarjetas**, no las mediciones
crudas. Es menos preciso y es a propósito: cualquier punto del porcentaje se
puede abrir y ver de dónde salió. Regla dura: **si un hallazgo no tiene
tarjeta, no entra al número.**

### Compuertas (cuándo NO se pinta)

- Ventana **< 7 días**: `claudemd` y `skills_unused` ni corren; el porcentaje
  saldría deformado. Chip de 7/30 días únicamente.
- Denominador **< $1** o **< 10 sesiones** en la ventana: "juntando datos"
  (mismo patrón que el mínimo de 20 fotos de cuota del Reporte).
- Denominador 0 → no existe el número. Nunca dividir (invariante #8).
- Con simulador: nunca.
- Algún costo del numerador con `estimated: true` → el número lleva "~".

### Qué obra pide (TRES piezas en sincronía, invariante #1)

1. **`meter-export.py`** — `scan_findings()` devuelve además
   `waste = {struct_cost, struct_tokens, total_cost, sessions, days, end,
   estimated, by_kind{}, items[]}` calculado ANTES de `findings[:MAX_FINDINGS]`.
   `items[]` = clave + costo de los estructurales sin recortar (para que el
   panel pueda descontar los ignorados), tope 100.
2. **Rust** — `struct Waste` con `#[serde(default)]` en todos sus campos
   (exportador viejo = ceros = "sin datos", nunca 0%), réplica exacta en
   `scan_local_findings`, y `get_findings` fusionando sumas por separado. Ojo:
   cambia el tipo de retorno de `get_findings` → grep de TODOS sus usos antes
   de subir (en el VPS no hay compilador).
3. **Panel** — pestaña Reporte: el número, su desglose por kind y la
   comparación con el periodo anterior.

### Lo que este número NO es

- No es un score de salud (fila 19 sigue descartada): es una razón entre dos
  cantidades medidas, y se puede auditar tarjeta por tarjeta.
- No promete ahorro. "Al menos el 14% se fue en el montaje" es historia;
  "vas a ahorrar 14%" es un pronóstico y está prohibido.
- No sustituye a tokens/turno útil: esa mide rendimiento (baja al mejorar
  cómo trabajas), esta mide montaje (baja al arreglar la instalación). Son las
  dos caras y por eso el Reporte enseña ambas.

### Por qué esta sí se puede comparar entre periodos

Es una razón, no un total: sube y baja con la calidad del montaje, casi no con
el volumen de trabajo. Es la métrica natural del antes/después por arreglo
("quitaste esas 47 líneas el 12 de agosto: ibas en 14%, vas en 6%") y el
número que pide la tarjeta compartible de APUESTA #2.

## Qué queda vivo de este doc (actualizado 2026-08-14)

Hecho desde la tabla del 2026-08-05: filas 9-10 (manómetro `press` +
gauge en widget, 2026-08-07), fila 11 en parte (regla `acomp` del coach
avisa de CADA auto-compact <30 min, 2026-08-08), fila 13 (en la pestaña
Reporte vía histórico de cuota), filas 16-17 (fase 1 + fase 2).

Pendiente real, por orden de valor:

1. **Fase 3 — export HTML del mockup A** (reporte imprimible y
   compartible; sinergia con la tarjeta del gatito).
2. **Fila 18 — % de desperdicio estructural**: la FÓRMULA ya está
   definida (§"La fórmula del % de desperdicio estructural", 2026-08-14).
   Falta la obra: las tres piezas en sincronía que lista esa sección.
3. **Fila 14 — botón "copiar resumen de traspaso"** (handoff por
   plantilla desde el recibo). Parcial: la tarjeta de intención del
   relevo ya cubre el caso "sesión al límite"; falta el traspaso a
   voluntad.
4. **Fila 11, la mitad que falta**: la ficha `acomp` avisa del EVENTO;
   no existe el detector de FRECUENCIA en Hallazgos ("N auto-compacts
   esta semana, ~X tokens en resúmenes") con su costo agregado.
5. **Detector de pegado masivo** (input de usuario anormalmente alto):
   diseñarlo y validarlo antes de que el Reporte pueda mencionar esa
   causa.
6. **Formato (c) del reporte**: push ntfy "tu reporte está listo" sin
   números (privacidad). Los formatos (a) pestaña y parte del dato ya
   existen; el push no.
7. **Fila 12 — hábito "sesiones sin /clear"**: viable, medio pelo (se
   solapa con inflate); solo si al usar el Reporte se echa en falta.
8. **Marcas de arreglo manuales**: los arreglos ANTERIORES a la fase 1
   "se pueden anotar a mano" — no hay UI para hacerlo. Solo si duele.
9. **Fila 15 — auditoría semántica de CLAUDE.md**: pospuesta, pide
   modelo (choca con invariante #4 embeberlo).

## Lo descartado (y su porqué, para no rediscutir)

- **Sesión contaminada:** mayor riesgo de falsos positivos del doc entero; la
  confianza es el activo #1. Madurar antes de intentar.
- **Score único de salud:** cifra inventada; contra la filosofía de la app.
- **Modelo local embebido:** peso, distribución, invariante #4. Como mucho,
  descarga opcional en un futuro lejano.
- **Aprendizaje colectivo / telemetría:** contradice invariante #3. Decisión
  de producto de Oscar, no técnica.

## Advertencia operativa

Por el invariante #1, **cada métrica nueva son TRES piezas en sincronía**
(Rust + `meter-export.py` + panel). El costo real de cada punto de la tabla es
~1.5× lo que aparenta. Y las métricas históricas (antes/después) necesitan los
logs retenidos — ya está `cleanupPeriodDays: 365` puesto en VPS y Windows.
