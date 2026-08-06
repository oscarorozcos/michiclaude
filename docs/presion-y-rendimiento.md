# Presión de contexto y métricas de rendimiento — análisis de viabilidad

> Estado: **PENDIENTE — sin arrancar**. Análisis hecho el 2026-08-05 a partir de
> un documento de estrategia externo (sesión de producto de Oscar:
> "MichiClaude — Fugas de contexto, desperdicio y estrategia de producto").
> LEER ESTE ARCHIVO COMPLETO antes de implementar nada de aquí.

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
| 18 | % de desperdicio estructural | ❌ | — | Viable DESPUÉS de definir la fórmula (el propio doc lo deja pendiente) |
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
