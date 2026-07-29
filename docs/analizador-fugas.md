# Analizador de fugas de tokens — diseño técnico

> Siguiente implementación grande tras el Modo Hub (APUESTA #3 de CLAUDE.md).
> Documento de trabajo: qué se detecta, cómo, y —sobre todo— qué NO se detecta.
> Consolidado de la conversación del 2026-07-29 con Oscar.

---

## 1. La frase que define el producto

> **Los demás te dicen cuánto gastaste. Nosotros te decimos dónde está la
> fuga, cómo taparla, y la semana siguiente te comprobamos que quedó tapada.**

La analogía que hay que tener presente al decidir cualquier detalle:

- Un **medidor de agua** te dice "gastaste 40 m³, el doble que el mes pasado".
  Eso ya lo sabías, te llegó el recibo.
- Un **plomero** te dice "el sello del baño de visitas está vencido, fuga toda
  la noche, cámbialo" — y vuelve el mes siguiente a enseñarte el recibo de 22 m³.

El medidor lo tiene cualquiera (ccusage tiene ~16,500 estrellas y es gratis).
Por el plomero se paga.

**El matiz que decide qué detectores construir:** el plomero NO dijo "báñate
más rápido". Eso sería juzgar cómo vive el cliente, y además no funciona. Dijo
dónde está el sello roto: se arregla una vez y ahorra solo.

Traducido al producto:

| ✅ Sello roto (esto sí) | ❌ Báñate más rápido (esto no) |
|---|---|
| "3 MCP servers inyectan definiciones en cada turno y nunca los usas" | "hubieras usado otro modelo" |
| "estas 47 líneas del CLAUDE.md nunca se relacionaron con nada que usaste" | "escribes mal los prompts" |

---

## 2. Anatomía de un hallazgo

Todo hallazgo útil tiene **cinco partes**. Si le falta una, es consejo de blog:

1. **Evidencia** — el dato crudo, verificable
2. **Causa** — el mecanismo, por qué cuesta
3. **Costo medido** — lo que YA costó (nunca lo que va a costar)
4. **Fix específico** — con nombres, rutas y líneas
5. **Verificación** — cómo se comprueba que sirvió

La 4 es donde todos fallan. *"Recorta tu CLAUDE.md"* es inútil.
*"Estas 47 líneas nunca se relacionaron con ninguna herramienta que usaste"*
ya es otra cosa.

Tarjeta de ejemplo (registro técnico):

```
🔴 2 MCP servers inactivos

postgres-mcp y figma-mcp están conectados. En 7 días,
312 turnos, no se invocaron ni una vez.

Sus definiciones de tools viajan en cada request de
todas formas: ~4,800 tokens por turno.

Costo la semana pasada: ~7% de tu techo semanal.

Fix: quitarlos de .mcp.json (2 líneas). Reconectables.

[Ver diff]  [Aplicar]  [Ignorar]
```

### El tipo de fuga decide el tipo de interacción

| Tipo de fuga | Qué es el fix | Cómo se presenta |
|---|---|---|
| MCP inactivos, archivos releídos | cambio de **configuración** | diff + botón aplicar, reversible |
| CLAUDE.md inflado | cambio de **contenido** | solo sugerir. **Nunca tocarlo.** |
| Sesiones sin `/clear` | cambio de **hábito** | no va en reporte: aviso en el momento |

El del medio necesita cuidado: el CLAUDE.md son *sus* instrucciones. Hay
líneas que parecen redundantes y son justo la razón de que algo no se rompa.

---

## 3. Catálogo de detectores

### Califican (objetivos, estructurales, se arreglan una vez)

- **MCP servers conectados sin invocar** — resta de conjuntos: config vs logs
- **CLAUDE.md sin respaldo** — líneas que mencionan archivos/herramientas inexistentes
- **Sesiones sin `/clear`** — tokens de entrada por turno subiendo monotónicamente
- **Rupturas de caché** — cuándo se recobró a precio de escritura en vez de lectura
- **Archivos releídos** — invocaciones de Read agrupadas por ruta
- **Overhead de subagentes** — subagentes que devolvieron menos de lo que costó arrancarlos
- **Turnos mecánicos** — turno sin edición cuyas herramientas fueron solo comandos
  deterministas (git, tests, formateo, instalar deps, ver logs)

### NO califican

Cualquier cosa que requiera **adivinar qué tan difícil era la tarea**. En
cuanto la app opina *"esto no merecía Opus"* empieza a equivocarse — y a la
tercera, el usuario deja de creerle.

Y algo más de fondo: se pueden medir tokens, pero **no se puede medir si la
calidad del output bajó**. Recomendar "bájale de modelo" se ve precioso en el
dashboard mientras el usuario hace tres rondas extra de "no, arréglalo".

### El catálogo de fixes es corto — a propósito

Quitar MCP inactivos · Recortar CLAUDE.md · `/clear` más seguido ·
`.claudeignore` · Bajar de modelo en subagentes · Mover exploración pesada a
subagente · Acotar el scope · Revisar continuidad de caché · Agrupar turnos
mecánicos al final.

> **La detección es el trabajo real; la recomendación es una tabla de búsqueda.**

---

## 4. Orden de implementación

La parte difícil ya está hecha: la app ya parsea los JSONL (con dedup,
`cache_read` excluido, lectura incremental con `scan_cache.json`), ya lee el
endpoint de OAuth y ya tiene UI en pestañas.

### Baratos — un fin de semana los tres

1. **MCP inactivos** — resta de conjuntos. **Empezar por este.**
2. **Archivos releídos** — contar Reads por ruta dentro de cada sesión.
3. **Sesiones que se inflan** — agrupar por `session_id`, ver si los tokens de
   entrada suben turno a turno.

### Medio — otro fin de semana + esperar datos

4. **El antes/después** — hash de config + CLAUDE.md con timestamp, comparar
   ventanas normalizadas. Lo complicado no es el código: es **necesitar
   semanas de historial a ambos lados**. Por eso la retención de logs se subió
   a 365 días el 2026-07-29 (ver §7).

### ⚠️ La trampa: el CLAUDE.md

Es el hallazgo más vistoso y el más fácil de calcular mal.

```
210 líneas × 40 turnos = X tokens     ← EXAGERA MUCHÍSIMO
```

Porque después del primer turno eso está **cacheado** y se lee como cache
read, no como input fresco. Si se publica un "18%" inflado 5×, se pierde toda
la credibilidad de golpe.

**Forma correcta: no estimar, MEDIR.** Los conteos de cache read ya están en
los JSONL.

### Regla de presupuesto

> El código es fácil. Lo difícil es no equivocarse.
> **70% del tiempo en validar que los números son correctos**, no en escribirlos.

---

## 5. Determinista, nunca un modelo local

### La app no gasta tokens (y eso hay que decirlo)

| Qué | Tokens |
|---|---|
| Analizar los logs | 0 |
| Aviso en vivo | 0 |
| Leer la cuota (OAuth) | 0 — es consultar un saldo |
| Clic en "quitar MCP" | 0 — edita un archivo local |

El medidor de agua no gasta agua para medir el agua. Los conteos ya vienen
calculados dentro del JSONL: nadie está "pensando", se está contando.

*Nota honesta que hay que documentar:* al borrar líneas del CLAUDE.md, el
siguiente mensaje cuesta un poco más (se reconstruye el caché una vez). Se
paga solo en el segundo mensaje — pero mejor decirlo que dejar que lo
descubran.

### Por qué NO un modelo local

El motivo nunca fue el costo, **fue el determinismo**. Un modelo local arregla
el gasto y no lo otro: sigue sin ser reproducible, sigue pudiendo decir algo
distinto con los mismos datos, sigue sin poder testearse. Y para un widget de
bandeja, meter 2 GB de modelo es *un monitor de recursos que se come los
recursos*.

### Todo sale con lógica

**Marcar líneas del CLAUDE.md** — extraer identificadores de cada línea
(rutas, herramientas, comandos, extensiones), buscarlos en 30 días de logs,
tres cubetas:

- menciona algo que sí usaste → verde
- menciona algo que nunca apareció → 🔴 candidata fuerte
- sin nada verificable (*"prefiere claridad sobre brevedad"*) → gris, sin opinión

**La cubeta gris es la clave**: es justo donde un modelo iba a *adivinar* y a
presentarlo como juicio.

**Clasificar sesiones** — heurísticas de herramientas:

| Señal en los logs | Etiqueta |
|---|---|
| Read/Grep/Glob alto, ~cero Edit | research |
| Bash con tests fallando + Edit | debug |
| Archivos nuevos, líneas netas + | feature |
| Solo archivos existentes, netas ≈ 0 | refactor |
| >70% de ediciones en `.md` | docs |

### Las tres razones por las que gana lo determinista

1. **Puede decir "no sé".** Un modelo siempre responde aunque adivine, y no
   avisa cuándo. En un producto que vende credibilidad, poder decir *no sé* es
   una función.
2. **Muestra su trabajo.** *"Lo etiqueté debug porque corriste `pytest` 14
   veces con error entre ediciones"* es auditable. "Gemma dijo debug" es una
   caja negra.
3. **Se puede testear.** Fixture de JSONL, salida esperada, test en CI.

> Si necesitas subir de modelo, casi siempre significa que planteaste mal la
> tarea. Bajarla a clasificación resuelve el 90% de los casos.

---

## 6. Redacción de los mensajes

Mismo hallazgo, mismos datos, dos registros:

**Técnico:** *"31 turnos con invocaciones mecánicas de bash, ~8% de cuota,
aquí tienes un alias"*

**Humano:**

```
💡 Le pediste 8 veces que subiera tus cambios

Subir cambios es mecánico: Claude no tiene que pensar,
solo ejecutar. Pero cada vez que se lo pides, vuelve a
leer toda la conversación del día para hacerlo.

Eso te costó casi medio día de los que te dura el plan.

Dos maneras de evitarlo:
🟢 Pídeselo una sola vez al terminar — un solo cobro
🟢 O hazlo tú en VS Code: panel izquierdo → ✓ → sincronizar  [Ver cómo]
```

Uno lo entiendes; el otro te hace sentir tonto.

### El concepto que le falta a un usuario no técnico

> **Cada vez que le escribes a Claude, él relee toda la conversación desde el
> principio.** No recuerda como una persona: vuelve a leer todo, cada vez.

Por eso el mensaje 50 cuesta mucho más que el 5 aunque escribas lo mismo. Y
por eso *"súbelo a GitHub"* al final de un día largo es carísimo: por cuatro
palabras, Claude se releyó ocho horas de trabajo.

**Si la app enseña solo eso, ya justificó su existencia.**

### Cuatro reglas de redacción

1. **Nómbralo como lo viviste**, no como salió del log.
2. **El costo en algo que se sienta** — "medio día de tu semana", no "8% de tu
   cuota". La gente siente días, no porcentajes.
3. **Nunca "deja de hacer X"** — siempre "hazlo así y te sale gratis". Nadie
   cambia un hábito porque una app lo regañe.
4. **La salida más cómoda primero**, la técnica después.

### Regla de honestidad (es la invariante #8 de CLAUDE.md aplicada aquí)

> **Reporta lo que costó, no lo que vas a ahorrar.**

- *"Esto te costó 7% la semana pasada"* → hecho, defendible.
- *"Vas a ahorrar 7%"* → predicción sobre una semana que no ha pasado. Si
  salen 3%, se perdió la credibilidad completa.

Si hay que proyectar: condicional y conservador — *"si la próxima semana se
parece a esta, esto te devuelve entre 4 y 7 puntos"*.

Y expresar las alternativas en **cobros, no en pronósticos**:

```
Tercera vez que subes cambios hoy
Las 3 llevan ~35 min de tu cuota.

🟢 Déjalo para el final → 1 cobro en vez de 3
🟢 Hazlo tú en VS Code → 0 tokens, siempre  [Ver cómo]
```

- **35 min** → ya ocurrió, es historia
- **1 en vez de 3** → aritmética sobre lo que pasó
- **0** → el único número garantizable al 100%

Convertir tokens a tiempo se hace con **el ritmo de consumo del propio
usuario**, nunca con una constante inventada. Y siempre con "~".

**Máximo dos opciones en el aviso en vivo.** Tres ya es un menú, y nadie lee
menús en medio del trabajo.

---

## 7. El ciclo completo (caso: subir a GitHub)

**Martes — día normal.** Ocho veces le pides a Claude que suba los cambios.
La app **no interrumpe**. Solo cuenta en silencio, turno por turno:

> ¿Este turno modificó algún archivo? **No.** ¿Y qué hizo? Solo comandos de git.
> → marcado como mecánico. Costo del turno: guardado.

**Miércoles — el aviso.** Al tercer "súbelo" del día con la conversación ya
larga, un globito discreto. **Y ya: no vuelve a salir ese día.** Un aviso que
aparece ocho veces se vuelve molestia y lo apagan.

**Lunes — el reporte.** Los ocho turnos de la semana se juntan en la tarjeta,
con las dos salidas.

**Lunes siguiente — la verificación.**

> *Pediste subir cambios 1 vez en vez de 8. Recuperaste ~4 horas de las que te
> dura el plan.*

Sin este paso nunca sabes si el consejo sirvió. **Es exactamente lo que
ninguna otra herramienta hace.**

### El detalle que hace que funcione

**La app nunca se mete en la conversación con Claude.** No es intermediario,
no filtra mensajes, no puede romper nada. Está afuera, mirando un archivo. Si
deja de funcionar mañana, Claude Code sigue igual.

Esa confianza es la mitad de la venta con un usuario no técnico — y es la
misma promesa que ya sostiene el manejo del token (invariante #3).

---

## 8. claude.ai: por qué el analizador no aplica

Hay que separar dos cosas:

- **El medidor sí sirve** — el endpoint de OAuth refleja la cuota compartida,
  que ya incluye lo gastado en el chat.
- **El analizador no** — y no es cuestión de esfuerzo: **no hay materia
  prima**. claude.ai no escribe logs locales. Sin registro por turno no hay
  nada que analizar.

Alternativas evaluadas, ninguna sirve:

- **Enterprise Analytics API** — solo plan Enterprise, requiere provisión del
  Primary Owner, viene agregada y con retraso. Inútil para un Pro o Max individual.
- **Export de conversaciones** — da contenido, no conteos de tokens. Habría
  que re-tokenizar y estimar, perdiendo la distinción cache read / cache
  write, que es donde vive medio análisis.

Las fugas de claude.ai además son de otra naturaleza: pegar el mismo PDF de 40
páginas en cinco chats en vez de subirlo una vez a un Project, o usar el chat
para tareas repetitivas que deberían ser un script. **Eso no lo diagnostica
una herramienta; lo diagnostica una persona viendo trabajar a otra.**

---

## 9. Contexto de mercado (por qué este es el hueco)

La capa de **medir** está saturada y es gratis: ccusage (~16,500 ⭐, ya cubre
Claude Code, Codex, OpenCode, Gemini CLI, Copilot CLI…), Claude-Code-Usage-Monitor,
CCSeva, módulos de Waybar, extensiones de VS Code, apps de menu bar en macOS,
una app .NET de ~6 MB para Windows con 14 idiomas, hasta un monitor en ESP32.

El **enterprise** también está tomado: Torii, Zylo, CloudZero, Portkey,
Langfuse, más la propia Claude Code Analytics Admin API.

Los huecos reales que quedan:

1. **De medir → a reducir con causa raíz atribuida.** El conocimiento de
   optimización existe pero está disperso en blog posts y es manual. Nadie
   conecta consumo → causa → fix → verificación.
2. **El segmento no-developer.** Todas las herramientas son CLI/terminal/menu
   bar. Cero para usuarios de claude.ai, Cowork o Excel.
3. El equipo mediano (mercado chico y difícil de alcanzar).

### Riesgo que hay que tener presente

**Anthropic cierra el hueco solo.** Ya salió `/usage` en la statusline,
`/recap` y la Analytics API. Todo producto de monitoreo tiene fecha de
caducidad; el analizador la tiene más lejos que el medidor, pero la tiene.

---

## 10. Caso de ejemplo: Ana, 3 semanas con Claude Code

Plan Pro, junior. Empieza el lunes y para el **martes a mediodía** está
bloqueada. Cree que Pro "no sirve" y está a punto de pagar Max.

Lo que hizo sin saber que estaba mal:

- Vio un video que decía "instala estos MCP servers". Conectó 6. Usa 2.
- Copió un CLAUDE.md de un blog: 280 líneas sobre Next.js, Prisma y Tailwind.
  Su proyecto es Python con FastAPI.
- Abre la terminal a las 9am y no la cierra en todo el día.

**Hallazgos: 41% de su cuota semanal.** Le toma 12 minutos arreglarlo. Al
lunes siguiente: -38% de consumo por tarea completada, llega al viernes.
**Ana ya no paga Max.**

Ninguno de los tres hallazgos fue *"escribes mal los prompts"*. Los tres eran
**basura estructural**. Lo que la app NO le dijo (peticiones vagas, debuggear
a ciegas, pedir cosas que resolvería más rápido a mano) también le cuesta
cuota — pero eso no sale de los logs.

---

## 11. Estado

- [x] Retención de logs subida a 365 días en el VPS (2026-07-29). Sin esto, el
      antes/después no tiene contra qué comparar: Claude Code borra a los 30
      días por defecto y lo borrado no vuelve.
- [x] Lo mismo en el Windows de Oscar (confirmado 2026-07-29: ya estaba en 365).
- [x] Detector 1: MCP servers inactivos (2026-07-29, validado en vivo).
- [x] Detector 2: archivos releídos (2026-07-29, validado en vivo — y cazó la
      trampa: la estimación por tamaño de archivo exageraba ~100x; se MIDE lo
      devuelto por cada lectura).
- [x] Detector 3: sesiones que se inflan (2026-07-29, validado en vivo;
      cuadró al 0.4% contra la agregación normal por un camino independiente).
- [x] Detector extra: peticiones mecánicas (git/tests; lista corta a propósito).
- [x] Detector 4: rupturas de caché (2026-07-29; cargo check limpio y
      validado en vivo por Oscar ese mismo día, con la tarjeta llegando
      desde el VPS por SSH). Turnos del hilo principal
      donde el prefijo cacheado se PERDIÓ (cache_read cae a menos de la mitad
      del contexto del turno anterior) y la conversación entera se reescribió
      a precio de ESCRITURA (1.25x input) en vez de leerse a 0.1x. Causas
      típicas: pausa mayor al TTL del caché o cambio de modelo (cada modelo
      tiene el suyo). Costo MEDIDO: `min(cache_write, contexto_previo)` — el
      piso, solo lo que ya estaba escrito — a la tarifa de escritura del
      modelo de ese turno. Exclusiones OBLIGATORIAS, las dos verificadas en
      logs reales: subagentes (`isSidechain` — llevan SU contexto y mezclarlos
      fabrica rupturas que no existieron) y compactaciones (`isCompactSummary`
      / `compact_boundary` ±120 s — ahí reescribir es el ahorro, no la fuga).
      Umbrales: prefijo mínimo 20k para evaluar; 300k reescritos por sesión
      para avisar. Validado contra los logs del VPS con una exploración
      independiente ANTES de escribir el detector — cuadre exacto: 21
      rupturas / $80.85 en la sesión monstruo de 1392 turnos (la de los $403:
      cada pausa larga sobre ~900k tokens de contexto costó ~$6 en
      reescritura), 6 / $22.27 y 2 / $4.37 en otras dos. Es la fuga más cara
      del catálogo y ningún otro detector la veía: inflate mide lo que cuesta
      RELEER el contexto; este mide lo que cuesta REESCRIBIRLO cuando el
      caché se pierde.
- [x] Indicador de hallazgos nuevos (2026-07-29, validado en vivo): pilita
      de post-its en el gatito, campana roja en la pastilla y cápsula 9+ en
      la pestaña; pasada diaria ligera; "visto" al abrir la pestaña; primera
      apertura instantánea con el último resultado guardado. Es la versión
      PASIVA del aviso en el momento.
- [ ] SIGUIENTE (acordado 2026-07-29, en este orden): los tres detectores de
      "lo instalado" — (1) skills instaladas sin uso (calca el de MCP:
      disco vs. <command-name> en los logs), (2) subagentes caros (los
      isSidechain traen usage propio: costo exacto, hoy invisible), y
      (3) hooks ruidosos (salida repetida cada turno, tamaño × turnos).
      Regla: señalan lo que NO se usa y lo que cuesta cargarlo — nunca
      califican lo que sí se usa.
- [ ] Detector: líneas de CLAUDE.md sin respaldo (§5, las tres cubetas).
- [ ] El aviso EN EL MOMENTO con texto (globito una vez al día, §7).
- [ ] Fix personalizado por entrypoint (VS Code vs. terminal, respaldo
      genérico — mismo patrón que prettyModel/price_for).
- [ ] El antes/después (necesita semanas de historial).
