# Bitácora de desarrollo — MichiClaude (2026-08-05 → hoy)

HISTORIAL del proyecto: jornadas, validaciones en vivo, decisiones con su
porqué, bugs con su autopsia. Lo VIGENTE (reglas, invariantes, pendientes)
vive en `CLAUDE.md`; aquí va lo que pasó y por qué. El tramo anterior
(julio → 2026-08-04, el CLAUDE.md original de 118k que hubo que destilar)
está íntegro en `bitacora-hasta-2026-08-04.md`. Índice de docs y dónde
mirar al depurar: `docs/README.md`.

Cómo buscar: `grep -n "^## " docs/bitacora.md` lista las jornadas;
`grep -n "<palabra>"` para una decisión o bug concreto. No abrir entero.

## Plantilla de entrada (copiar al cerrar una jornada)

```
## AAAA-MM-DD (n) — título en una frase: qué cambió y por qué importa

QUÉ: lo que se hizo, en llano (features, arreglos, docs).
POR QUÉ: la causa — bug con su autopsia, decisión con su porqué, o
  petición de Oscar; si se descartó una alternativa, decir cuál y por qué.
CÓMO SE VERIFICÓ: cargo check / regresión / prueba en vivo (qué se vio,
  con números si los hay). Si NO se verificó, decirlo.
QUÉ QUEDA: pendientes que abre o cierra (y reflejarlo en CLAUDE.md
  §"Estado / pendientes"; podar CLAUDE.md si se acerca a 40k).
```

Reglas: entrada nueva SIEMPRE al final; título con fecha (`## 2026-08-16`)
para que el grep la encuentre; una entrada por sesión (numerar `(2)`,
`(3)` si hay varias el mismo día). Si la bitácora pasa de ~3.000 líneas,
mover el tramo viejo a
`bitacora-hasta-AAAA-MM-DD.md` como se hizo con el fósil.

---

## Ronda de rediseño UX/UI (2026-08-05) — detalle completo

Se movió aquí desde CLAUDE.md el mismo día, al pasar ese archivo de los
40k caracteres que Claude Code carga (la regla que vigila el detector 10;
nos volvió a pasar en carne propia). En CLAUDE.md queda el contrato de la
ronda y los invariantes; el porqué de cada sección vive aquí.

- [ ] EN CURSO (desde 2026-08-05): ronda de REDISEÑO UX/UI sobre la
      maqueta de Oscar (docs/rediseno-v5.html). RESPALDO COMPLETO del
      estado anterior en el tag `pre-rediseno-20260805` (recuperar:
      `git checkout pre-rediseno-20260805`; comparar: `git diff`).
      CONTRATO DEL REDISEÑO (pedido de Oscar): (1) SOLO reacomodo y
      estética — cero pérdida de funcionalidad, textos, mensajes de
      error, confirmaciones, campos o iconos; (2) sección por sección
      del menú, nunca todo a la vez; (3) toda NOVEDAD funcional de la
      maqueta se consulta con Oscar ANTES de implementarla; (4) si algo
      del diseño choca con un invariante, avisar antes. CHOQUES YA
      DETECTADOS en la maqueta (resolver al portar): fuentes de Google
      (viola CSP/privacidad — van embebidas o tipografía del sistema),
      textos hardcodeados en español (todo pasa por t(), invariante
      #10), buckets y modelos hardcodeados (render dinámico, invariante
      #6), selector de idiomas con Italiano y sin 中文 (son 8 fijos), el
      GATITO desaparecido del selector de estilo y sin sus selectores de
      arte/globos, capa sin la opción "Detrás", Consejos sin las
      tarjetas VIVAS del coach, Hallazgos sin "volver a mostrar", pie
      sin la cifra Semana, ntfy sin interruptor maestro opt-in, y
      "Simular estados" ausente. Después del rediseño: el resto de
      pendientes en orden.
      AVANCE: S1 encabezado+pestañas+paleta base VALIDADA por Oscar
      (capturas 2026-08-05; decisiones: tipografía del SISTEMA se queda —
      sin fuentes web —, y el contraste del degradado aprobado en
      pantalla). S2 hero de Principal implementada (anillo con el mismo
      truco pathLength=100 — JS del medidor intacto salvo la clase warn
      de remQ —, ritmo unificado dentro del hero, eyebrow "A este ritmo"
      retirado a propósito por la maqueta). AJUSTES pedidos por Oscar
      sobre S2 (2026-08-05, con capturas): el anillo COMPLETO se encimaba
      con el reset → vuelve al MEDIO anillo de siempre (misma geometría y
      pathLength); y el texto de las barras se veía apagado sobre el
      violeta → el hero REDEFINE --txt-mut/--txt-dim localmente (todo lo
      de dentro hereda sin repintar reglas) y el % lleva clase `pctv` con
      más peso. REGLA NUEVA del rediseño: toda tarjeta con fondo propio
      redefine esos dos tonos en vez de tocar sus hijos uno por uno.
      Y el acomodo definitivo de las barras (Oscar 2026-08-05): nombre +
      % en UNA línea, y debajo `.bmeta` con un dato por línea TODOS a la
      izquierda — el reset de las filas mini se iba a la derecha y
      rompía la alineación.
- TIPOGRAFÍA (2026-08-05, Oscar la pidió embebida): Inter (texto), Sora
  (`--disp`: títulos y cifras grandes) y JetBrains Mono (`--mono`) viven
  en `src/fonts/` — woff2 variable, subconjuntos latin y latin-ext, 238 KB,
  licencia OFL con su copia en `fonts/LICENSES.md` (obligatoria al
  redistribuir). NUNCA se piden a un CDN: rompería la CSP y la promesa de
  privacidad, y un fallo del servidor dejaría la app sin tipografía. Sin
  glifos CJK: en ja/ko/zh la pila cae sola al sistema — por eso los
  respaldos de --font/--mono se conservan enteros. Se aplican SOLO donde
  Oscar lo indique, sección por sección (por ahora: todo el panel hereda
  Inter/JetBrains, y --disp está en el título y el % del medidor).
- S3 GASTO POR PROYECTO (2026-08-05): filas con AVATAR de iniciales
  (`projInitials`: primera letra de la primera palabra + primera de la
  última — "claude-code-meter"→CM, "MichiClaude"→MC; con una sola palabra,
  sus dos primeras letras), nombre + chip de origen, barrita bajo el
  nombre e importe a la derecha. El avatar hereda el color de PALETTE de
  su barra (el color sigue identificando al proyecto) y su fondo es una
  CAPA a opacidad, NO `color-mix()` — esa función es demasiado reciente
  para darla por segura en WebView2. "Más proyectos (N)" pasa a enlace:
  ya existía y ya desplegaba (projOpen), solo cambió de aspecto.
- S4 TENDENCIA + MODELOS + PIE (2026-08-05): barras de tendencia con
  degradado, esquinas redondeadas y el DÍA MÁS CARO destacado en violeta
  con halo (clase `top`, nueva; `today` y `zero` intactas — el día sin
  actividad sigue siendo hueco, no barra de valor cero). Modelos: barra
  segmentada en cápsula con separación entre tramos y leyenda en píldoras
  (el nombre del modelo va en `<em>` para destacarlo del %; sigue saliendo
  de prettyModel, invariante #6). Pie: tarjeta con degradado y las cifras
  en --disp a 22px.
- BUG DEL REDISEÑO (2026-08-05, lo vio Oscar en captura): la lista de
  proyectos salió descuadrada —avatar e importe a la izquierda, nombre a
  la derecha— porque en CSS Grid los hijos que fijan su fila se colocan
  ANTES que los automáticos, y el nombre acababa en la 3ª columna. Se
  rehízo con FLEX + envoltorio `.ptx`, como la maqueta. REGLA: en filas
  con "algo que ocupa dos líneas" a los lados, flex antes que grid.
- CONTENEDOR BASE: `.sect` deja de ser un bloque separado por línea y pasa
  a ser TARJETA con fondo (--card, radio r-lg). Es transversal a
  propósito, como la paleta: da el lenguaje visual a todas las pestañas de
  una vez y el contenido de cada una se sigue rediseñando por turnos. El
  selector de periodo (.dsel) pasa a cajita con borde — plano sobre la
  tarjeta se perdía.
- S5 FILTROS DE LA TARJETA DE GASTO (2026-08-05, dos maquetas de Oscar).
  El `<select>` de periodos DESAPARECIÓ: ahora hay dos disparadores
  gemelos en el encabezado (embudo = proyectos, calendario = fechas) que
  abren POPOVERS FLOTANTES con velo — `position:fixed` a propósito: el
  panel tiene scroll propio y dentro de él se irían con el scroll.
  Cancelar / ✕ / velo / Esc REVIERTEN (foto del estado al abrir); solo
  Aplicar confirma.
  · FECHAS: presets (Hoy/7/15/30) DENTRO del calendario — "Hoy" ES el
    periodo de 1 día, no un botón aparte — más rango libre a dos clics
    (si se elige al revés se ordena solo). Rejilla de 42 celdas con los
    días vecinos en gris, punto turquesa en hoy, tope 90 días con aviso;
    ni futuro ni más allá de esos 90. Calendario DIBUJADO A MANO
    (invariante #4) y meses/días desde `Intl` con el idioma activo.
    Estado: `curDays` (preset) o `spendRange` (rango libre) en
    localStorage — nunca los dos a la vez.
  · PROYECTOS: filtro solo de FRONTEND (la lista ya viene agregada; con
    filtro el total pasa a ser la suma de los elegidos y por eso lleva
    etiqueta "2 de 8 proyectos" — una cifra sin decir de qué es sería
    justo lo que prohíbe el invariante #8). Conjunto VACÍO = todos, nunca
    "ninguno" por accidente. Buscador, "Todos", contador en el botón y
    CHIPS en la tarjeta para quitar uno a uno o todos. Persiste en
    `projFilter`. Con filtro se enseñan todos los elegidos aunque
    vinieran de la cola plegada.
  · El PIE "Hoy" del final del panel se ELIMINÓ (Oscar 2026-08-05): decía
    lo mismo que el total de arriba en cuanto el periodo era hoy. Su
    contenido —la cifra grande y la nota de privacidad— vive ahora en la
    caja del total, dentro de la tarjeta de gasto. CONSECUENCIA ASUMIDA:
    con un periodo que no sea hoy ya no se ve el gasto del día suelto; la
    cifra que manda es la del periodo elegido, que es lo que se está
    mirando (`cost_today` sigue llegando del backend por si vuelve a
    hacer falta).
  · "Borrar" del calendario vuelve al valor por DEFECTO (hoy) y CIERRA:
    dejarlo vacío obligaba a elegir algo para poder salir, y cerrar sin
    elegir mantenía el periodo anterior — justo el que se quería borrar.
  · Los controles viven en su PROPIA fila alineada a la izquierda: colgados
    del título se descolocaban al pasar el título a dos líneas.
  · Orden de Principal: cuota → gasto → MODELO MÁS USADO → tendencia
    (intercambiadas las dos últimas a petición de Oscar).
  TRADUCCIÓN SIN DICCIONARIO: los nombres de mes y día salen de
  `Intl.DateTimeFormat(lang)` — los 8 idiomas funcionan sin ampliar I18N
  (solo Hoy/Borrar/Aplicar/avisos están en el diccionario). El primer día
  de la semana es lunes salvo en en/ja/ko/zh.
  LÍMITE HONESTO: con rango, las máquinas del HUB quedan FUERA (sus fotos
  son de ventanas que terminan hoy y nadie puede recortarlas); Rust lo
  marca con `hub_skipped` y el panel lo dice en pantalla — callarlo sería
  enseñar un total incompleto (invariante #8). Tampoco se SUBE foto al
  hub mientras hay rango: envenenaría lo que leen las demás máquinas.
  "Hoy" y la serie diaria de 30 días siguen ancladas a AHORA a propósito.
  BUG cazado en la prueba: en Python faltaba el corte superior y el rango
  devolvía todo hasta hoy. Verificación que lo destapó y que conviene
  repetir si se toca esto: dos rangos contiguos de 7 días deben sumar
  EXACTAMENTE la ventana de 14 (dio 0.0000 de diferencia).
- S6 FUENTES DE DATOS (2026-08-05): las cuatro fuentes pasan de lista de
  viñetas a REJILLA DE TARJETAS con icono (se entienden de un vistazo).
  Sus textos salieron del `cfg_note` viejo partiéndolo automáticamente en
  `src1_t/src1_d`…`src4_t/src4_d` ×8 idiomas — sin reescribir traducciones
  a mano. Formulario con campos más altos y foco en acento; botón primario
  en degradado y secundarios (hub/export) en tono apagado. Servidor
  guardado = tarjeta con icono, no fila con línea.
  Y AJUSTES COMPARTIDOS SE MUDA a la pestaña Ajustes (petición de Oscar):
  es un ajuste, no una fuente; solo DEPENDE de un servidor. Como allí no
  se ve la lista, aparece un aviso ámbar (`hub_cfg_needsrv`) cuando no hay
  ninguno — antes el contexto lo daba estar debajo de la lista.
- S7 HALLAZGOS (2026-08-05): el encabezado del analizador queda en su
  tarjeta y las tarjetas de hallazgo van SUELTAS debajo (una tarjeta
  dentro de otra se leía como un cajón). Cada hallazgo estrena ICONO por
  tipo (`FND_ICON`: rayo=cachebreak, gráfico=inflate, hoja=reread,
  terminal=mech, nodos=subagents, enchufe=mcp…) en cuadrito con el color
  de la SEVERIDAD, importe destacado y unidades apagadas, e "Ignorar"
  como píldora en la esquina. El borde izquierdo de color se retiró: con
  fondo de tarjeta y el icono ya coloreado, sobraba.
  OJO al tocar esto: las fichas de CONSEJOS comparten el molde `.fnd`
  (variante `.tip`) — cualquier cambio en .fnd/.fnd-t/.fnd-f les llega
  también, y por eso tienen sus propios overrides.
  · El SELECTOR de Hallazgos pasa al MISMO calendario del gasto: el
    popover es UNO SOLO y `calTarget` ("spend"/"fnd") decide a quién
    aplica lo elegido; Hallazgos guarda su par en `fndDays`/`fndRange`.
    Para que el rango sea de verdad, `get_findings` acepta `end` y el
    analizador (Rust Y Python, invariante #1) gana CORTE SUPERIOR en sus
    tres filtros de ventana — sin él el rango devolvía todo hasta hoy,
    la misma mordida que ya pasó en el gasto. "Borrar" vuelve a HOY en el
    destino que esté abierto. Regresión verificada: sin rango, hallazgos
    y costes idénticos a la versión anterior.
  · PIE en dos piezas: el enlace de recuperar lo ignorado ARRIBA y
    destacado en acento (antes se perdía dentro de una línea gris), con
    el número dentro de la frase ("Volver a mostrar 2 hallazgos que
    ocultaste" — `fnd_restore` pasa a función; `fnd_hidden` se retiró) y
    la nota del "~" debajo en gris.
- ANCHO DEL PANEL 400 → 446 (2026-08-05): lo cazó Oscar comparando con la
  maqueta — a 400 px los textos se apretaban ("Semanal · todos los …"
  cortado con puntos suspensivos, el ritmo partido en dos líneas). 446 =
  los 430 de la maqueta + los 8 px de padding del body por lado.
  `position_panel` usa `outer_size()`, así que el flyout se recoloca solo;
  no hay ningún ancho hardcodeado en Rust. Cambia tauri.conf.json → hay
  que RECOMPILAR para verlo.

### Consejos y remates de Hallazgos (2026-08-05)

- El "¿No encontraste lo que buscabas?" DEJA de ser un pie permanente y
  pasa a ocupar el HUECO de la búsqueda sin resultados: aparece solo
  mientras el filtro no encuentra nada y se va al borrarlo. Razón (mi
  recomendación, aceptada por Oscar): un banner fijo se vuelve invisible a
  los dos días, y el ofrecimiento significa algo justo en el momento en que
  al usuario le falta un consejo. El registro LOCAL de búsquedas fallidas
  del mes (faqMisses, cero telemetría) se mantiene: es lo que viaja en el
  issue, para que la propuesta lleve todo lo buscado y no solo lo último; y
  la búsqueda EN CURSO se añade siempre, porque el registro espera 1.5 s de
  pausa y pulsar rápido no hacía nada. "Descartar" se retiró: ya no hay
  nada permanente que cerrar (tips_dismiss fuera de los 8 idiomas).
- Buscador con ✕ para vaciarlo (lo pidió Oscar señalándolo en captura).
- COMANDOS resaltados en Hallazgos y Consejos con `withCmds()`: envuelve en
  <code> los "/clear", "/compact"… y lo que va entre «comillas angulares»,
  que en el diccionario son siempre órdenes de terminal. Escapa ANTES de
  tocar (nada llega a innerHTML sin escapeHtml).
- El botón de periodo no se parte en dos líneas (white-space:nowrap) y en
  el encabezado el TÍTULO se encoge antes que el control: "03 ago – 05 ago"
  cabía justo y saltaba de línea.
- Copy del pie de Hallazgos reescrito en los 8 idiomas: de "~ = estimado;
  el resto está medido de tus logs" a "Los importes con ~ son aproximados.
  El resto está medido directamente de tus registros" — la abreviatura con
  signo igual se leía como jerga.
- Los hallazgos enseñan CUÁNDO pasaron (`fmtWhen`, a la derecha de la
  fuente): "ahora mismo" / "hace 20 min" / "hace 3 h" hasta las 6 h, luego
  la hora, "ayer HH:MM" y por fin "31 jul 20:38". Con varios hallazgos en
  pantalla, saber cuál es de cuándo era imposible (petición de Oscar
  2026-08-05, con la línea marcada en su captura). Los detectores de
  ESTADO PURO (mcp, skills, claudemd) NO llevan ts y no enseñan nada: no
  describen un momento sino una configuración. Fechas y horas con `Intl`
  en el idioma activo; solo "ahora/min/h/ayer" van al diccionario.
- El campo "Filtrar…" de Consejos llevaba el estilo del sistema y
  desentonaba con todo: ahora usa el mismo campo del rediseño.

### S8 — Ajustes y rastro de los avisos (2026-08-05)

- AJUSTES en tarjetas por tema (General · Avisos · Precios · Exportar ·
  Ajustes compartidos · Acerca de) en vez de una lista corrida. Las filas
  son "etiqueta a la izquierda, control a la derecha" separadas por una
  línea tenue, y las CASILLAS se pintan como interruptores (el checkbox
  nativo desentonaba con todo). Al invertir el orden hubo que mover el
  <input> detrás del <span> en las 6 filas con casilla. "Avisos" agrupa
  alarmas de %, presupuesto y celular: son el mismo tema ("cuándo quiero
  que me avise") y estaban sueltos. Encabezados nuevos `prefs_general` y
  `prefs_alerts` ×8 idiomas.
- RASTRO DE LOS AVISOS en la bitácora del flujo (`flowLog`), a raíz de la
  duda de Oscar ("no sé si los post-its funcionan o si nunca se dan las
  condiciones"): `fndBadgeCalc` y `renderTipsDot` anotan cuando el aviso
  se ENCIENDE o se apaga, y cuando no se enciende dicen por qué ("las N
  tarjetas ya estaban vistas"). Eran los únicos avisos sin huella.
  DIAGNÓSTICO de su caso, leído en su propia bitácora: el circuito está
  intacto (el panel sigue mandando `fnd` y `coach` en quota:update y la
  pastilla los pinta) — lo que pasa es que a las 14:54 hizo un escaneo
  manual con la pestaña abierta (10 tarjetas → vistas) y la pasada diaria
  de las 14:55 encontró esas mismas 10, ya vistas: badge nulo, sin
  campana. Y no hubo ninguna pasada por cierre de sesión porque en
  Windows no nació ningún recibo `sum` en todo el día. Trampa del
  vigilante, cuarta aparición.
- ACERCA DE (2026-08-05, pedido de Oscar: "aunque sea estático"). NO quedó
  estático: reúne cosas que ya existían sueltas — la comprobación de
  versión (que hasta ahora solo corría sola a los 8 s del arranque) con su
  botón, el atajo a Releases (`open_releases`, URL constante en Rust) y
  "Reportar un problema", que abre el formulario de issues con la VERSIÓN
  y el sistema ya escritos: quien reporta casi nunca los incluye y sin
  ellos el reporte no sirve. La versión sale del comando nuevo
  `app_version` (env!("CARGO_PKG_VERSION")): escribirla en el frontend
  sería una segunda verdad que se queda vieja sola.
- Botones de ntfy: "Canal nuevo" y "Enviar prueba" se encimaban. El campo
  del canal manda ahora en su fila (.ntfy-url) y los botones saltan de
  línea si no caben; la confirmación de "Canal nuevo" pasó a fila propia,
  lo que de paso quitó un `style.display` que peleaba con [hidden]
  (invariante 10bis). Los botones secundarios (Copiar, Canal nuevo, CSV,
  JSON, Actualizar ahora, hub) van en tono apagado: el degradado es para
  la acción principal de cada tarjeta.
- BUG del mudanza de Ajustes compartidos (2026-08-05, lo vio Oscar: botones
  muertos): `loadRemotes()` —quien habilita los botones vía
  syncHubButtons— solo corría al abrir Fuentes de datos. Cuando el bloque
  vivía ahí, bastaba; al mudarlo a Ajustes, entrar directo a esa pestaña
  dejaba `remotes=[]` y los botones desactivados para siempre. Arreglo:
  loadRemotes corre al ARRANCAR (get_remotes solo lee remotes.json, local
  y barato) y al abrir cualquiera de las dos pestañas que dependen de los
  servidores. LECCIÓN para el resto del rediseño: al mover un bloque de
  pestaña, buscar qué inicialización dependía de ABRIR la pestaña vieja.
  Y el bloque quedó penúltimo (antes de Acerca de), como en la maqueta.

### Coach multi-fuente (2026-08-05) — local + WSL + servidores SSH

Pedido de Oscar tras entender la limitación con el ejemplo del doctor: los
Hallazgos (análisis de laboratorio) ya veían el VPS, pero el coach (la
enfermera del momento) era ciego fuera de lo local — y él trabaja por SSH
DENTRO del VPS, donde más gasta. Implementación:
- Rust: coach_scan recorre también las distros WSL; CoachHit gana `origin`
  (lo pone get_coach al fusionar, como el origen del export) y
  fetch_remote_coach trae los hits de cada servidor.
- meter-export.py: réplica completa del motor bajo `--coach` (atajo: sin
  agregación de gasto), con TODO el detalle — pendiente fantasma blindado,
  gaps de caché, dedup de tool_use por id, ai-title, leaks del cierre — y
  estado incremental propio en ~/.cache/michiclaude/coach_state.json (el
  exportador es un proceso nuevo por sondeo; sin estado releería sesiones
  enteras cada 3 min). El estado solo guarda sesiones vivas: se poda solo.
- Frontend: fichas, recibos y los pushes de "terminó"/"espera tu
  aprobación" enseñan el origen cuando no es local.
VALIDACIÓN EN VIVO (la mejor posible): el --coach corrido en el VPS
detectó LA PROPIA SESIÓN de trabajo de esta jornada — compact con 802k de
contexto, attach con 26 relecturas, 1021 turnos, $401.86, pending:true
mientras Claude ejecutaba herramientas. Sondeo incremental: 73-91 ms.
VERIFICACIÓN DEL CIRCUITO DE INDICADORES (pedida por Oscar): el resumen
emite fnd+coach antes del early-return de ok:false; la pastilla pinta
fdot/tdotc y el gatito hasfnd/hastip, ambos antes de su early-return; y
desde hoy fndBadgeCalc y renderTipsDot dejan rastro en flowLog al
encenderse/apagarse. PRUEBA EN VIVO PARA WINDOWS: recompilar y esperar el
primer sondeo (60 s) — esta sesión del VPS siempre está enorme, así que
la ficha compact con origen VPS-EU y el post-it turquesa DEBEN aparecer.
VALIDADO EN VIVO EN WINDOWS (2026-08-05, capturas + bitácora de Oscar,
a la primera): el sondeo trajo los consejos del VPS — fichas "compact"
(816k) y "attach" (26 lecturas) con su "michiclaude · VPS-EU", los
comandos /clear y /compact resaltados en cajita, el POST-IT TURQUESA del
gatito encendido con su contador (2, luego 1), el rastro nuevo en la
bitácora ("tips: AVISO ENCENDIDO (2 sin ver)" → "vistas con foco — aviso
apagado") y hasta el push de "Terminó tu sesión en michiclaude · VPS-EU"
con el origen dentro. La campana ROJA no encendió y el rastro dijo por
qué: "fnd: sin aviso — las 10 tarjetas ya estaban vistas" (trampa del
vigilante, ya no muda: ahora se explica sola). Acerca de con su estilo y
el bug rojo: validado.
MATIZ CONOCIDO que salió en la prueba: el push de "terminó" saltó para la
sesión del VPS aunque sigue viva (1727 min, 1041 turnos) — 5 minutos de
silencio entre turnos disparan "done" una vez por sesión, igual que en
local con una pausa larga. Semántica asumida del diseño, no bug: el
banderín notified impide que se repita.
- CAMPANA/POST-IT ROJO VALIDADOS EN VIVO (2026-08-05, tras re-armar
  fndSeen/fndAutoLast por consola): post-it rojo "9+" en el gatito,
  contador 5 en la pestaña, "fnd: AVISO ENCENDIDO" en la bitácora. Con
  esto TODO el sistema de avisos (rojo y turquesa, gatito y pastilla,
  panel y pushes) queda comprobado de punta a punta con datos reales.
- MARCO FANTASMA del panel (lo vio Oscar): la ventana es fija (no puede
  redimensionarse en vivo — regla de las transparentes) y el panel medía
  su contenido: pestañas cortas dejaban un hueco TRANSPARENTE debajo que
  enseñaba lo de atrás. Arreglo: .panel pasa de max-height a HEIGHT — 
  llena siempre la ventana con fondo sólido; lo corto deja espacio vacío
  interior (intencional) y lo largo sigue con scroll interno.

---

## Cierre de jornada — 2026-08-05

RONDA DE REDISEÑO UX/UI: TERMINADA Y VALIDADA, las cinco pestañas con
capturas de Oscar en el mismo día. De la maqueta v5 al panel real:
paleta azul-noche/violeta, tipografía embebida (Inter/Sora/JetBrains,
OFL), tarjetas con fondo, hero con chip de estado, avatares de proyecto,
popovers de filtros (calendario de rango + proyectos con buscador y
chips), hallazgos con icono por tipo y "cuándo pasó", consejos con el
ofrecimiento de proponer en el hueco de la búsqueda, ajustes en tarjetas
con interruptores, y Acerca de con versión real y reporte pre-llenado.

LO GRANDE que cayó además del rediseño: COACH MULTI-FUENTE (local + WSL
+ SSH) — el exportador replica el motor completo bajo --coach con estado
incremental en el servidor (~80 ms/sondeo), validado en vivo con la
propia sesión de trabajo (816k ctx) y con las fichas, el post-it
turquesa, el push con origen y el rastro en flowLog funcionando a la
primera en Windows. Y el AVISO ROJO validado también (post-it 9+ tras
re-armar fndSeen).

BUGS CAZADOS EN LA JORNADA: campo origin sin comodín en done/sum (no
compilaba), ajustes compartidos muertos al mudarse de pestaña (su init
dependía de abrir la pestaña vieja), lista de proyectos descuadrada
(grid vs flex), falta de corte superior en los rangos del analizador
Python, y el marco fantasma del panel (max-height → height).

PRÓXIMA SESIÓN: lo que Oscar traiga. En la lista viven: validación
pasiva natural (alarmas/ntfy/aviso al cierre), decisión del updater
(repo público + tag), capturas del README, y las ideas apuntadas para su
momento (hub con rangos por día; armonizar el widget con la estética v5
si algún día apetece).

---

## Cierre de jornada — 2026-08-06/07

REPORTE EJECUTIVO: DE IDEA A PESTAÑA FUNCIONANDO en dos días. Nació del
documento de estrategia que trajo Oscar (context rot / medir desperdicio
en vez de consumo): primero el análisis con tabla comparativa de 22
puntos (docs/presion-y-rendimiento.md — veredicto: el Nivel 1 ya lo
cubríamos casi entero; lo nuevo viable era rendimiento + antes/después),
luego el diseño del reporte con mockups de IA externa (Oscar eligió el
A, documento ejecutivo en llano), y de ahí las tres fases.

FASE 1 — MOTOR DE DATOS (verificado en el VPS con logs reales):
- Turnos útiles `uturns` (mensajes HUMANOS: fuera meta, sidechain,
  tool_result, comandos, inyecciones <ide_…) en totales, proyectos y
  serie daily (que ganó también tokens/día). is_user_turn réplica exacta
  Rust↔Python; caché de escaneo v2 ambos lados. Regresión con logs
  CONGELADOS y --end fijo: campos viejos idénticos byte a byte;
  coherencia 7d+7d=14d exacta; muestreo del filtro sin falsos (el <ide_
  se cazó ahí: el IDE inyecta avisos con rol user sin marcar meta).
- Histórico de cuota quota_history.json (90 días, una foto por ciclo,
  freno 150 s; log_quota desde refresh() solo con lectura buena).
  Validado en Windows: la primera foto nació con s/w/sr/wr correctos.
- Marcas de arreglo fndHist/fndMarks (solo hallazgos de estado, escaneos
  ≥7d sin rango; visto ≥3d + desaparecido ≥2d = arreglado).

FASE 2 — PESTAÑA REPORTE (validada con capturas de Oscar): 6.ª pestaña,
chips Semana/Mes/Personalizado (calendario compartido, target "rep"),
héroe de rendimiento, "¿te duró más o menos?" del histórico de cuota
(con estado honesto "juntando datos"), gráfica 4 semanas, proyectos con
delta vs periodo anterior y "qué lo encareció" de hallazgos reales,
marcas con antes/después (mínimo 5 días o "midiendo"), y "para los días
que vienen" con recomendación por fuga.

RONDAS DE CAPTURAS (dos): (1) velocidad — caché por periodo + render
progresivo; pasada ligera de hallazgos 20h→3h (el porqué del "nunca vi
el post-it rojo": hallazgos del VPS no disparan cierre local y a 20 h
llegaban tarde — 4.ª mordida de la trampa del vigilante); 6 pestañas en
una fila; re-render al cambiar idioma (gráfica en español dentro del
panel en japonés). (2) maqueta michiclaude-hero-grafica.html de Oscar —
héroe EFICIENCIA/VOLUMEN con ≈$ real pegado a cada dato de tokens, nota
"no es contradicción" solo cuando divergen, regla "1M tok ≈ $X" con la
tarifa MEDIDA del periodo (mejora sobre la maqueta, que traía tarifa
fija), gráfica grande con conmutador tokens/$ estimado, barras de
volumen y detalle al tocar. Margen transparente 8→5→3→1 px; scrollbar
overlay fina.

PRIMERA MEDICIÓN REAL del rendimiento: ~51k tok/turno en el VPS (7d) —
nuestras propias sesiones son intensas; el reporte de Oscar marcó
"empeoró 13%"… con nosotros mismos como causa. El medidor midiéndonos.

PENDIENTE AL CIERRE: fase 3 (export HTML del mockup A), validación
natural del post-it rojo (ahora con la pasada de 3 h tiene cómo), y que
el histórico de cuota junte días para llenar "¿te duró más o menos?".

## 2026-08-07 (segunda sesión) — "Leído" estilo Gmail y diseño de remediación

AVISOS POR TARJETA (pedido de Oscar): abrir la pestaña de Hallazgos o
Consejos —aunque sea por error— ya NO marca nada como visto. Cada
tarjeta se marca LEÍDA con su propio clic (el mismo que pliega/
despliega); el contador de pestaña y el post-it del widget descuentan
una por una, como Gmail descuenta correos abiertos. Ignorar apaga la
suya; restaurar ignorados revive las no leídas; el ✕ del coach despacha.
Cayeron los marcados masivos del render (y con ellos el requisito de
document.hasFocus, que solo existía para que la precarga invisible no
matara el aviso). La TRAMPA DEL VIGILANTE (4 mordidas) queda ENTERRADA:
ya no existe "nace vista por estar mirando la pestaña". Sin claves i18n
nuevas. Detalle fino: en el coach, guardar coachCards ANTES de repintar
el contador (lee de localStorage — al revés quedaba desfasado un clic).

REMEDIACIÓN — DISEÑO DESTILADO en `docs/remediacion.md`: análisis de una
propuesta externa (handoff de otra IA + mockups). Se conservó lo bueno
(intención-no-comando, regla de oro "en la duda pregunta", confianza
progresiva con candados, clasificador de tarea viva por TodoWrite) y se
corrigió lo que chocaba: archivar JSONL a 30d se dejaba ciega a la
propia app (→ ≥365d), el "modelo local" para casos dudosos ya estaba
descartado en presion-y-rendimiento.md, el countdown no puede ser globo
(regla única), los checks "Aplicar /compact//clear" mienten sin canal de
escritura (→ relevo ConPTY `michi claude`, el "tmux nativo" con 5 reglas
anti-choque), y el "handoff Pro" necesita una IA que no hay. 4 etapas,
cada una útil sola; NO arrancar hasta cerrar el reporte.

## 2026-08-07 (tercera sesión) — cierre del reporte y prompts guardados

"LEÍDO" ESTILO GMAIL — VALIDADO por Oscar desde el inspector: los
contadores y post-its descuentan tarjeta por tarjeta al clicarlas y
abrir la pestaña o el post-it ya no borra nada. Tema cerrado.

REPORTE EJECUTIVO — CERRADO HASTA DONDE ESTÁ (decisión de Oscar): las
fases 1 y 2 quedan como están, funcionando; se retoma solo si al usarlo
falta algo o pide ajustes. La fase 3 (export HTML del mockup A) NO se
arranca; queda anotada en el pendiente como lo primero si se retoma.
Siguen vivos de esa área el cargo check de la fase 1 en Windows y la
validación en vivo, que caerán con el uso normal.

PROMPTS DE DISEÑO DE REMEDIACIÓN — guardados como referencia en
`docs/prompts-diseno-remediacion.md` (rescatados del transcript de la
sesión anterior): bloque de estilo común con la paleta y tipografía
REALES del panel + 7 prompts (tarjeta de intención, modo automático con
candados, countdown, registro de acciones, manómetro en widgets,
tarjeta educativa, relevo en terminal) + las 3 notas de uso (handoff
Pro fuera a propósito, correcciones de honestidad ya incluidas, y que
lo que devuelva la otra IA es referencia visual — la integración se
traduce al sistema real del panel). Referenciado desde remediacion.md
y desde el pendiente de REMEDIACIÓN en CLAUDE.md, cuyo candado se
re-redactó: ya no es "hasta cerrar el reporte" (cerrado hoy) sino
"decisión explícita de Oscar" (matar procesos es clase nueva de
capacidad).

## 2026-08-07 (cuarta sesión) — remediación etapa 1a: manómetro de presión

Arranca la etapa 1 de remediación (consejero con intención — la única
que no necesita la decisión pendiente de Oscar: no toca nada, solo
mide). Primera pieza: el MANÓMETRO DE PRESIÓN DE CONTEXTO, puntos 9-10
de presion-y-rendimiento.md ("muy viable y barato: el dato ya existe").

CÓMO: regla nueva `press` en el motor del coach — un hit por sesión con
contexto y quieta <10 min (PRESS_QUIET_MAX), value = tokens de contexto
crudos y campo aditivo `quiet` (minutos quieta). Implementada en las DOS
piezas del motor (Rust `coach_scan` + `--coach` del exportador,
invariante #1); viaja por el canal de siempre (`get_coach`, fusión con
origin intacta — un exportador viejo simplemente no la manda y el
manómetro remoto no existe, degradación honesta). NO es ficha ni aviso:
coachPoll la aparta como done/ask (no gasta tope diario ni tipSeen),
elige la más fresca (menos quieta; empate → más contexto) y emitPill la
monta como campo `press` de quota:update con el % ya redondeado sobre
200k (PRESS_FULL, constante del frontend con su comentario). Sin hit en
un sondeo el manómetro se APAGA solo (la sesión se durmió).

UI: arco de manómetro SVG inline (pathLength=100 + stroke-dasharray, sin
fuentes externas) en la cápsula de la pastilla y del gatito — diminuto,
sin texto ni tooltip (regla de la cápsula); el NÚMERO vive en el detalle
pcard (fila con barra y proyecto·origen) y en el globo del hover del
gatito (bloque con barra). Umbrales PROPIOS 60/85 (presión de contexto,
no ritmo de cuota): calma = acento/tinta, ámbar ≥60, rojo ≥85. Se pinta
ANTES del early-return de ok:false como las campanas: la presión sale de
los logs locales y un fallo del endpoint de cuota no la toca. En
card.html hizo falta `.blk[hidden]{display:none}` — la misma trampa del
10bis (display:flex anula al atributo hidden). Clave i18n `press_lab`
×8. Previews de navegador actualizados en las 4 ventanas.

Decisiones: press NUNCA va a ntfy ni al hub (es lectura local); no pasa
por el interruptor de avisos (es lectura, como el % de sesión); 200k
como techo es constante comentada del frontend — el backend manda tokens
crudos a propósito para que un cambio de techo sea un solo número.

VERIFICADO: node --check en los scripts de las 5 ventanas, py_compile
del exportador, press_lab en los 8 idiomas, cero firmas Rust tocadas
(campo aditivo + regla nueva). PENDIENTE: cargo check en el Windows de
Oscar y verlo en vivo con una sesión real. Siguen 1b (parser TodoWrite +
clasificador) y 1c (tarjeta de intención + clipboard).

## 2026-08-07 (quinta sesión) — remediación etapa 1 COMPLETA: clasificador y tarjeta de intención

La 1a (manómetro) quedó VALIDADA en vivo por Oscar en cuanto compiló:
sus capturas mostraron el arco en la pastilla, el 86% rojo en el detalle
con "michiclaude · VPS-EU" y la fila en el globo del gatito. De paso
preguntó qué significa y qué hacer — la respuesta es justamente 1b+1c,
así que se implementaron en esta misma jornada.

1B — SEÑALES EN EL MOTOR (Rust + Python, invariante #1): el estado del
coach gana todos_open/todos_total (del ÚLTIMO TodoWrite de la sesión —
la señal reina), trail (últimos 20 archivos tocados con
Read/Edit/Write) y commit_clean (hubo `git commit` y nada se editó
después; cualquier edición lo apaga). El hit `press` los lleva como
campos aditivos topen/ttotal/cont/gclean, donde cont = Jaccard % de los
últimos 10 archivos contra los 10 previos (¿sigue en lo mismo?). El
estado viejo del exportador migra solo (setdefault contra el default).

DECISIÓN DE ARQUITECTURA: el motor manda HECHOS crudos; el veredicto
Alive/Boundary/Uncertain vive UNA sola vez, en JS (`intentVerdict`):
topen>0 → alive; lista cerrada al 100% o commit limpio → boundary;
cont≥40 → alive; si no, unsure. Así el invariante #1 solo carga con los
hechos y la lógica no se duplica en tres lados. La señal de "lenguaje de
cierre" sigue FUERA (solo-español vs app de 8 idiomas, ya documentado).

1C — TARJETA DE INTENCIÓN: con presión ≥80% (INTENT_PCT) coachPoll
sintetiza el hit LOCAL `intent` y lo mete al pipeline normal de
tarjetas del coach — hereda gratis el anti-spam por sesión (tipSeen),
el leído estilo Gmail, el ✕, el TTL de 24 h y el aviso
post-it/foco/contador. Exenta del tope diario (perder por tope justo el
aviso que más ahorra sería un contrasentido) y se REFRESCA en cada
sondeo sin renacer (conserva born/min/v; despachada NO resucita). La
tarjeta pregunta la intención en llano — "¿Sigues trabajando en lo
mismo?" / "¿Ya terminaste?" — con el comando pequeño al lado (el
usuario aprende el mapeo), evidencia medida siempre visible ("Michi
detectó: lista 5/6 · sigues en los mismos archivos · último msg hace X
min"), insignia "Recomendado" SOLO cuando el veredicto no es unsure
(regla de oro), advertencia ámbar en /clear si hay pendientes, botón
"Copiar comando" y "Ahora no". El clic de copiar NO pliega ni marca la
tarjeta (stopPropagation): copiar no es terminar de leer.

CLIPBOARD: dep nueva tauri-plugin-clipboard-manager (la justificada en
el diseño), invocada DIRECTO con plugin:clipboard-manager|write_text —
sin wrapper npm (invariante #4). Capability
clipboard-manager:allow-write-text añadida. Escribe al portapapeles
SOLO al clic del usuario.

VALIDACIÓN: node --check en el panel, py_compile, paridad de las 16
claves int_* ×8 por conteo, y prueba de fuego REAL — el exportador
nuevo corrió sobre los logs de este VPS (estado aislado con
XDG_CACHE_HOME para no pisar el del exportador productivo) y detectó
esta misma sesión de trabajo: press con topen=5, ttotal=6 (la lista de
tareas real del momento), cont=50 y quiet=0 → veredicto alive →
recomendaría /compact. El simulador "🧪 Simular hallazgos" gana una
tarjeta intent falsa para probar lo visual sin esperar presión real.
PENDIENTE: cargo check en Windows (la dep nueva se descarga en la
primera compilación) y ver la tarjeta nacer en vivo.

### Sexta sesión (2026-08-07) — la prueba real que se hizo sola

El pendiente "ver la tarjeta nacer en vivo" se resolvió de la forma más
poética posible: la sesión de Claude Code del VPS en la que CONSTRUIMOS
la tarjeta llegó al 100% de presión (se compactó trabajando), y Michi la
cazó en el Windows de Oscar sin simulador ni re-armado — "digamos que
fue prueba real jaja" (Oscar, con capturas).

Lo que confirmaron las capturas, punto por punto:
- La tarjeta nació sola en Consejos: "Tu sesión ya pesa mucho · 100%",
  proyecto "michiclaude · VPS-EU" (el origin remoto pintado bien).
- El VEREDICTO acertó: evidencia "lista de tareas: 0 de 5 sin terminar
  · commit reciente sin cambios después · último mensaje hace 3 min" →
  frontera → insignia RECOMENDADO en /clear. Exactamente lo que el
  clasificador debía concluir con esos hechos (la lista de todos ya
  estaba completada y el último commit no tenía ediciones después).
- Globo del gatito con la fila "Presión de contexto 100%" en rojo entre
  Sesión y Semanal; arco del manómetro en la cápsula conviviendo con el
  % de sesión (94%); contador "1" en la pestaña Consejos.
- De rebote: cargo check y la compilación con la dep nueva
  tauri-plugin-clipboard-manager pasaron en Windows (nada de esto
  existiría en pantalla sin ella).

Con esto la ETAPA 1 de remediación queda validada en vivo de punta a
punta salvo un clic: el botón "Copiar comando" (pegar y ver /compact o
/clear). Las etapas 2-4 siguen sin arrancar a la espera de la decisión
explícita de Oscar (matar procesos = clase nueva de capacidad).

Remate: Oscar probó el botón "Copiar comando" y funcionó. ETAPA 1
COMPLETA Y VALIDADA al 100%, sin pendientes.

## 2026-08-07 (séptima sesión) — remediación etapa 2: automático out-of-band

Oscar dio el GO explícito a las etapas 2-4 (la decisión que faltaba:
matar procesos es clase nueva de capacidad). Se implementó la ETAPA 2
completa; las 3-4 (el relevo ConPTY) esperan a que esta pase cargo check
en Windows y se valide en vivo — misma disciplina por etapas que
funcionó con la 1, y además el relevo construye sobre el registro y el
desbloqueo progresivo que nacen aquí.

Qué se construyó (decisiones detalladas en docs/remediacion.md
§"Decisiones de la etapa 2"):

- **Rust, 5 comandos nuevos** (todos async + spawn_blocking, 10ter):
  `scan_zombies` (foto de procesos por PowerShell/CIM sin deps nuevas;
  zombie = proceso que casa con la firma de un MCP stdio de
  ~/.claude.json Y padre muerto o PID de padre reciclado),
  `kill_zombie` (re-verifica PID+ejecutable+arranque ±2 s justo antes
  del Stop-Process; "gone" si ya no está, ERR_ZOMBIE_CHANGED si el PID
  ya es de otro), `scan_archivable` + `archive_old` (mueve .jsonl ≥365d
  a %APPDATA%\<app>\archive conservando estructura; WSL fuera hasta la
  etapa 4), `get_action_log` (registro actions_log.json, tope 200,
  datos crudos que el panel traduce — invariante #10).
- **Frontend:** sección "Remediación automática" en Ajustes (toggles
  zombie ON / archive OFF por defecto, candado "Michi no automatiza lo
  que no has visto", revisar/cerrar/archivar a mano, registro de
  acciones) + tarjeta de zombies en Consejos por el pipeline normal
  (nace solo cuando el automático no puede actuar; su "Cerrar todos" ES
  la primera manual que desbloquea; clave zombie|arranque-más-nuevo
  para que un lote nuevo re-avise sin resucitar lotes despachados) +
  sondeo `remPoll` horario y archivado auto una vez al día.
- **i18n:** 28 claves × 8 idiomas (paridad verificada por script en la
  sesión).
- Sin tocar meter-export.py: nada de esto viaja por SSH (SOLO LOCAL),
  así que el invariante #1 no se activa.

Trampa evitada sobre la marcha: `#[cfg]` sobre bloques-expresión en
posición de cola dentro de un closure NO compila tras el strip (el
bloque queda en posición de statement); se cambió a la pareja de
funciones cfg'd, el mismo patrón de `wsl_claude_dirs`.

PENDIENTE para validar la etapa 2 (en el Windows de Oscar): cargo
check, ver la sección en Ajustes, "Revisar ahora" con y sin zombies
(fabricar uno: abrir una sesión con un MCP stdio y matar la terminal),
el clic manual que desbloquea, el kill automático a la hora siguiente,
el registro con auto/manual, y el archivado con un .jsonl viejo de
laboratorio (tocar mtime con `(Get-Item f).LastWriteTime=...`).

### Validación en vivo de la etapa 2 (2026-08-07, Windows de Oscar)

Zombies VALIDADO de punta a punta: detección, cierre manual, desbloqueo
del candado, cierre automático a los 90 s del arranque y registro con
sus dos líneas (`03:40 manual` / `03:45 auto`, ambas «fantasma»). El
`cargo check` queda implícito: la app compiló y arrancó con el código
corregido. Archivado, pendiente de la prueba de laboratorio.

Dos bugs que SOLO salían en Windows real — ninguno era visible leyendo
el código, y por eso la regla de validar en la máquina de verdad antes
de dar una etapa por buena:

1. **Barras.** La firma sale de `~/.claude.json` con barra normal
   (`@modelcontextprotocol/server-memory`) y la línea de comando del
   proceso ya resuelto lleva barra invertida
   (`…\node_modules\@modelcontextprotocol\server-memory\dist\index.js`):
   NINGÚN MCP lanzado con npx casaba jamás. Se normalizan ambos lados a
   `/` antes de comparar (commit 68d84e0).
2. **El script del kill moría en el parser de PowerShell.** Iba en UNA
   línea y PowerShell no acepta el `}` de un bloque seguido de otra
   sentencia sin separador: el script no llegaba a ejecutarse, stdout
   salía vacío y TODO cierre acababa en ERR_ZOMBIE_KILL ("No se pudo
   cerrar") mientras `Stop-Process` a mano funcionaba perfecto. Ahora
   lleva saltos de línea reales; REGLA: script de PowerShell escrito
   desde Rust, saltos reales SIEMPRE. El escaneo nunca lo sufrió porque
   es una tubería de una sola sentencia (commit 144986a). De paso, el
   veredicto ya no se decide con `$?` —que con `-ErrorAction
   SilentlyContinue` no distingue "no pude" de "ya no estaba"— sino
   re-consultando el PID, y un veredicto irreconocible deja foto cruda
   en `rem_debug.json` (sin eso el fallo era indistinguible desde la UI:
   nos costó tres rondas de terminal descubrirlo).

Cómo se fabricó el zombie de laboratorio (lo primero que falló): un MCP
bien educado NO sirve. Con `@modelcontextprotocol/server-memory`, al
matar el `cmd` de arriba se cerró toda la cadena sola — cuando su
cliente muere, él se va. Los zombies reales los dejan los MCP que
ignoran el cierre de stdin. Receta que sí funciona: `mcp-fantasma.js`
con `setInterval(function(){},1000000)`, `claude mcp add fantasma --
node <ruta>` y lanzarlo con `powershell -Command "Start-Process node
-ArgumentList '<ruta>' -WindowStyle Hidden"` — ese powershell
intermedio muere en el acto y deja al node huérfano de nacimiento.

Nota de comunicación (Oscar es nuevo en terminal): NUNCA dar comandos
con huecos tipo `<PID>` o `EL_NUMERO_NUEVO` — los pega literales y
PowerShell escupe un error que no dice nada útil. O el número ya
puesto, o un comando que busque por nombre y no necesite sustituir
nada.

Archivado validado el mismo día con un .jsonl de laboratorio (copia de
uno real con `LastWriteTime` a -400 días): lo detectó, apareció el botón
"Archivar ahora" —que solo nace cuando hay algo que archivar— y el
archivo acabó en `%APPDATA%\<app>\archive\C--Users-oscar\` conservando la
estructura, con su línea en el registro. ETAPA 2 CERRADA.

De la validación salieron además dos arreglos de i18n: "1 archivos" y
"1 logs" — todos los textos con contador necesitan su ternario de
singular (los 5 idiomas que lo distinguen; ja/ko/zh no).

### Etapa 3a — el relevo `michi claude`, validado en vivo (2026-08-08)

Crate aparte `relevo/` (paquete `michi`, fuera de `src-tauri` para que la
app no gane dependencias ni el vigilante de `npm run dev` lo recompile).
Compiló a la PRIMERA en el Windows de Oscar y el paso transparente
funcionó de entrada: Claude Code entero dentro de la ConPTY —colores,
flechas, resize, `/login` con navegador— sin enterarse de que hay alguien
en medio del cable. Seis pruebas, todas pasadas: transparencia, `michi
status` desde otra terminal, inyección real de `/compact` (se escribió y
se ejecutó sola), y el candado negándose con texto vivo en el prompt.

Por el camino cayeron TRES fallos, y los tres enseñan algo distinto.

**1. Los avisos del terminal no son teclas.** Por el mismo cable de
entrada llegan cambios de foco (`ESC [ I` / `ESC [ O`), respuestas de
posición del cursor (`R`), identificación (`c`), estado (`n`) y medidas
(`t`). Contaban como actividad humana y reiniciaban la ventana de calma,
así que bastaba con SALIR de la terminal —justo lo que hace el usuario
para ir al panel de MichiClaude— para que nunca se pudiera inyectar.
`KeyWatch::feed` devuelve ahora `human` y solo eso mueve el reloj.

**2. El prompt no se puede modelar solo con lo que entra.** Con `hola`
sin enviar, `status` decía `texto: no` y la inyección se aplicó: salió
`hola/compact` como un solo mensaje. R5 aguantó —no se borró nada, que
era el peor caso previsto— pero el guardián falló. Dos causas de diseño:
`typed` era un booleano APARTE del buffer de la línea (dos fuentes de
verdad; al desincronizarse mandó el booleano, ahora se DERIVA del
buffer), y el Enter limpiaba el modelo a ciegas (ahora aparta la línea a
`pending` y espera a ver si Claude REACCIONA: bytes por la PTY después
del Enter = enviado; 3 s de silencio = no se envió y la línea vuelve).

**3. La causa REAL, que no era ninguna de las dos.** El diagnóstico
nuevo (`michi status --debug`, que enseña CUENTAS de teclas y `line_len`,
nunca contenido) lo destapó en una ronda: con `hola` escrito, `k_print:
0` y `k_esc: 38`. El relevo no había contado una sola tecla en su vida.
En Windows Terminal, **ConPTY pide `win32-input-mode` (`ESC [ ? 9001 h`)
al arrancar y el terminal se lo concede a TODA la ventana** — incluida la
nuestra, que es quien reenvía esa petición sin saberlo. Con ese modo cada
tecla viaja como `ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _` y no llega ni un
carácter suelto. Las letras alcanzaban a Claude porque el relevo reenvía
los bytes intactos; el contador las veía como ruido. Y el terminador `_`
cae dentro de `0x40..0x7e`, así que las secuencias cerraban limpias y
nada chirriaba. `KeyWatch::win32_key` las decodifica (`Uc` es el carácter
en decimal, solo con `Kd` = pulsación). Validado: `hola oscar` = 10, y
`k_print: 10`, `line_len: 10`, `typed: true`, `ERR_RELAY_TYPED`.

Reglas que salen de aquí, para no repetirlas:

- **Envolver una terminal no es reenviar bytes.** Hay un protocolo que el
  terminal y la ConPTY negocian a espaldas de quien está en medio, y el
  de en medio HEREDA esa negociación sin enterarse.
- **Un guardián que cuenta cosas tiene que exponer sus cuentas.** Un
  `k_print: 0` valió más que tres rondas de teoría. Y se puede hacer sin
  romper la privacidad: cuentas y longitudes, jamás contenido.
- **Una sola fuente de verdad.** Un booleano "resumen" al lado del dato
  real acaba mandando él, y mintiendo.
- **Fail-closed de verdad:** mientras no se sabe si un Enter envió, se
  cuenta como que hay texto.

### El manómetro llevaba meses clavado: el techo no era 200k (2026-08-08)

Validando la etapa 3b, la cabecera de Claude Code cantó el bug sin querer:

```
Claude Code v2.1.225
Opus 5 (1M context) · Claude Max
```

**1M.** El manómetro de presión dividía entre `PRESS_FULL = 200000`, una
constante puesta cuando 200k era el techo de TODOS los modelos. Opus y
Sonnet saltaron a 1M en la 4.6, y Fable/Mythos nacieron ahí.

No hizo falta creerse la cabecera: los propios logs lo tenían medido.
Contexto máximo alcanzado por modelo en las sesiones de Oscar:

| modelo | máximo real |
|---|---|
| claude-opus-5 | **998.248** |
| claude-fable-5 | 836.644 |
| claude-opus-4-8 | 641.326 |

Casi un millón. Con el techo viejo esas sesiones marcaban **100%**
permanente: gauge en rojo, gatito alarmado y tarjeta de intención
disparada. Y al revés, la tarjeta saltaba en cuanto se cruzaban 160k
tokens (el 80% de 200k), que en un modelo de 1M es el **16%** del
depósito. La sesión de trabajo de ese mismo día, medida en el VPS, iba
por 480.757 tokens: 48% real, 100% según el panel.

**Por qué se arregló ANTES de la etapa 3c y no después.** La 3c es el
countdown que aplica `/compact` SOLO. Su disparador es este porcentaje.
Construir el automático encima de una cifra 5× equivocada habría hecho
que Michi comprimiera el historial —perdiendo contexto real— con el 84%
del depósito libre. Un número mal calibrado es inofensivo mientras solo
se mira; deja de serlo en cuanto algo actúa sobre él.

**El arreglo no necesitó ni una descarga nueva.** Las tres fuentes de la
cascada de precios publican el techo en el MISMO archivo que ya bajamos
cada 24 h: LiteLLM en `max_input_tokens`, models.dev en `limit.context`,
OpenRouter en `context_length`. `PriceEntry` gana un campo `ctx` y el
caché en disco lo hereda; `ctx_for()` lo lee y, si la fuente no lo dijo
(o el caché es de una versión anterior), cae a `ctx_table()`, respaldo
embebido que decide por VERSIÓN y no por lista de modelos —invariante
#6—, hermano de `price_table()`.

Tres detalles que costaron pensarlos:

- **La duda se resuelve hacia abajo.** Sin dato, 200k. Quedarse corto
  hace que el manómetro avise antes de tiempo (molesto); pasarse haría
  que no avisara nunca (el usuario choca con el muro sin previo aviso).
  El fallo seguro de un avisador es avisar de más.
- **`price_key()` recorta el sufijo `[1m]`** para casar el id del log con
  las tablas públicas. Si el techo se resolviera después de esa
  normalización, una variante de contexto largo se leería como su base de
  200k. Por eso `ctx_for()` mira el id CRUDO antes de buscar en la tabla.
- **Se guarda el modelo, no el techo ya resuelto.** El estado de la
  sesión (Rust y Python) recuerda el id del último turno y el techo se
  calcula en cada sondeo: así una tabla recién descargada corrige la
  cuenta sola, en vez de arrastrar un número viejo hasta que la sesión
  muera.

En el panel el denominador vive en UN solo sitio (`pressFull()` /
`pressPct()`) — la lección de la trampa del booleano resumen, aplicada
antes de que muerda: tres divisiones repartidas por el archivo eran
exactamente la forma en que este bug sobrevivió tanto tiempo.

Regresión: export normal y `--findings` idénticos byte a byte; `--coach`
solo añade la clave nueva.

Lección general: **una constante con el nombre de un límite externo es
una fecha de caducidad esperando.** No estaba mal escrita — estaba mal
envejecida, y nada en el código podía avisarlo. Cuando el límite lo
publica alguien de fuera, el número se busca, no se escribe.

### Auditoría de las tres fuentes de precios (2026-08-08)

Recién metido el techo de contexto en la tabla de precios, Oscar hizo la
pregunta correcta: *"¿y si mañana una fuente dice 2 millones y otra menos?
¿coinciden o manejan parámetros distintos?"*. La cascada es de RESPALDO,
no de verificación cruzada — la primera que responde manda —, así que el
número puede depender de qué servidor estuviera vivo ese día. Se
descargaron las tres y se cruzaron modelo por modelo.

**Precios: coinciden al céntimo.** Cero discrepancias en todos los modelos
que las tres comparten. Fable 5 a 10/50, Opus 5 a 5/25, Sonnet 5 a 2/10.

**Techo: una sola discrepancia.** `claude-sonnet-4-5`: LiteLLM dice 200k,
models.dev dice 1M. Las dos tienen razón a medias — ese modelo es de 200k
con un beta de 1M, así que el número correcto depende de si el beta está
activo. No hay tabla que pueda saberlo; lo sabe la máquina del usuario.

**Y la pregunta destapó un fallo que llevaba ahí desde el principio.**
OpenRouter escribe la versión con PUNTO —`claude-opus-4.8`— donde LiteLLM,
models.dev y los propios logs usan GUIÓN. La tercera fuente casaba **6 de
sus 14** modelos:

```
casan hoy:                   6
casarían con punto→guión:   14
las 8 que faltaban: haiku-4-5, opus-4-1, opus-4-5, opus-4-6,
                    opus-4-7, opus-4-8, sonnet-4-5, sonnet-4-6
```

Nunca explotó porque LiteLLM siempre responde primero. Era un tercer
paracaídas con un agujero que nadie había mirado, y no solo para el techo:
también para los **precios**. `claude-opus-4-8`, uno de los modelos que
Oscar usa a diario, era uno de los ocho.

Arreglo: `price_key()` unifica punto→guión **entre dígitos** (para no tocar
`anthropic.claude-opus-5`). Va dentro de `price_key()` a propósito, que es
el único punto por el que pasan las dos partes —guardar y buscar—, así que
ambas quedan con la misma forma y siguen casando. Normalizarlo solo al
guardar habría roto la búsqueda de `claude-2.1`.

**La evidencia por encima de la tabla.** Para el caso sonnet-4-5 y para
cualquier fuente que se quede corta, `ctx_full()` compara el techo de la
tabla con el contexto MÁXIMO que esa máquina ha alcanzado de verdad. Si lo
medido supera a la tabla, la tabla está demostrablemente mal y mandan los
tokens. Detalle que costó pensarlo: no se puede devolver lo visto a secas
—una sesión de 480k daría 480k de techo y el manómetro volvería a marcar
100%—, así que se sube al primer escalón de `CTX_LADDER`, una lista de
MAGNITUDES (200k/1M/2M/5M), no de modelos.

Lo que este cambio NO resuelve, dicho para que no se olvide: una fuente que
INFLE el techo (2M donde son 1M) silenciaría el aviso, y contra eso la
evidencia no sirve. La respuesta buena es el detector de auto-compacts que
ya está apuntado como pendiente: Claude Code comprime cerca del límite
real, y esa es la medida más honesta que existe.

**Lo que cambió de opinión por el camino.** Al calibrar el techo escribí que
quedarse corto era "el fallo seguro" de un avisador. Eso vale mientras
Michi solo MIRA. En cuanto la etapa 3c aplique `/compact` sola, equivocarse
por abajo deja de ser molesto y pasa a ser destructivo: comprimiría un
historial sano. Con automatización los dos errores duelen, y lo que hace
falta es puntería, no una dirección segura. De ahí sale una regla para la
3c: **el automático se gana con certeza; si el techo no es de fiar, la 3c
aconseja pero no actúa.**

Lección general: **una cascada de respaldo no es una cascada verificada.**
Mientras la primera fuente responda, las otras dos son código que nadie
ejecuta — y el código que nadie ejecuta se pudre sin avisar. Si hay
paracaídas de repuesto, hay que abrirlos de vez en cuando.

### El relevo deja de depender de que te acuerdes (2026-08-08)

Tres piezas del mismo problema, planteado por Oscar: *"los usuarios se les
olvide y empiecen a trabajar y se den cuenta de que MichiClaude no ejecutó
nada"*. Un automático que depende de un hábito no es un automático.

**El atajo del PATH.** La pregunta que lo desatascó fue suya: *"¿se puede
hacer genérico o hay que especificar por herramienta?"*. La respuesta cambió
el diseño: **las terminales y los editores no interpretan `claude`** —
ejecutan un shell, y el shell resuelve el comando. Perseguir "el top 10 de
herramientas" era perseguir el objeto equivocado; el eje real eran cuatro
shells. Y por encima de los cuatro hay algo mejor: un `claude.cmd` propio
primero en el PATH, porque ahí resuelve **Windows**, no el shell. Un
mecanismo, y vale para Windows Terminal, VS Code, Cursor, Warp, Alacritty y
los que salgan mañana. Validado: `claude` a secas abrió con relevo y el panel
lo detectó solo.

Lo que costó de la primera prueba: **una pestaña nueva no es una terminal
nueva.** Windows Terminal heredó su entorno al arrancar y se lo pasa a cada
pestaña, así que el PATH nuevo no llegaba. El aviso decía "abre una terminal
NUEVA" — engañoso, porque una pestaña lo parece. Ahora dice que hay que
cerrar la VENTANA. Y el `.cmd` pasó a ASCII puro: la raya del comentario
salía como `â€”` porque un `.cmd` no declara codificación y cmd.exe lo lee
con la página de códigos que toque. En un comentario es cosmético; en un
archivo de órdenes, una bomba de relojería.

**Y el indicador estaba en el sitio equivocado.** Lo levantó Oscar: *"¿no
estaría bien ver de forma visual qué sesión está activa con relevo, y no
darme cuenta al final de que no?"*. Tenía razón y era un fallo de diseño mío
— el indicador vivía en el panel, que es donde NO tienes los ojos. Trabajas
en la terminal.

Plan A (poner el título al arrancar) **no sobrevivió**: Claude Code pone
«Claude Code» en cuanto arranca. Estaba declarado como best-effort antes de
probarlo, así que el plan B ya estaba pensado: como el relevo ve pasar todos
los bytes, `TitleMark` intercepta la secuencia OSC del título y le antepone
la marca. La pestaña queda «michi · MichiClaude · Claude Code» y la marca
sobrevive a cada reescritura de Claude porque se pega a todas.

Es la ÚNICA excepción al paso transparente del relevo, así que va acotada:
solo `ESC ] 0|1|2 ;`, **leyendo el número entero y no el primer dígito**
—`ESC]10;` es el color de primer plano y tratarlo como título le habría
metido la marca dentro—, sin apilar marcas, y con tope de 1024 bytes que
suelta lo retenido tal cual. Fail-open: lo peor posible es quedarse sin
marca, jamás comerse la salida. Diez casos probados con un puerto de la
máquina de estados antes de tocar un compilador.

Lecciones:

- **Cuando algo "hay que acordarse de hacerlo", el diseño está incompleto.**
  No es un problema de documentación ni de disciplina del usuario.
- **Antes de integrar N herramientas, buscar qué tienen debajo.** Diez
  terminales eran cuatro shells, y cuatro shells eran un PATH.
- **Un indicador va donde están los ojos**, no donde es cómodo ponerlo.
- **Declarar "best-effort" antes de probar** convirtió un fallo en un paso
  previsto: el plan B ya estaba pensado cuando el plan A cayó.

## 2026-08-08/09 (cierre) — el automático se prueba solo en vivo y foto completa de pendientes

VALIDADO EN VIVO, y con la mejor evidencia posible: el ciclo completo del
automático sobre la sesión de chat del VPS (Windows → SSH → relevo →
extensión). Primera corrida: countdown y silencio — dos fallos de UX
(rechazo `ERR_RELAY_BUSY` quemaba la sesión para siempre; la cuenta
acababa sin veredicto). Arreglados (reintento a 10 min + cierre ✓/✕) y la
SEGUNDA corrida la vivió Oscar sin tocar nada: cuenta atrás, ✓ verde,
`auto · aplicó /compact en «VPS-EU»` en el registro, 872 960 tokens
liberados — y ese /compact cayó sobre la conversación de trabajo real.

También de estas sesiones:
- **Manómetro tras compactar**: todo `compact_boundary` pone `last_ctx=0`
  (mentía hasta 10 min y causaba el /compact redundante "No messages to
  compact"). Rust + exportador, regresión byte a byte.
- **Auto-compactación de Claude Code** investigada sobre el binario
  v2.1.226 y decisión tomada: no se apaga ni se sugiere (red de
  seguridad + precompute); entramos al 80% vs su ~94%. Un /compact
  inyectado se registra `manual` → `acomp` nunca se avisa a sí mismo.
- **La compactación no deja `usage`** en el log: no es facturable desde
  los .jsonl; solo se ve en cuota. Si un día se enseña: estimado o nada.
- **Interruptor del chat** (`set_chat_relay` + `CHAT_WRAP_PY`): el
  wrapper de VS Code en servidores SSH se enciende desde Ajustes; 8
  casos en banco (ajeno no se pisa, ilegible no se toca, backup,
  NOWRAP). Validado por Oscar: "VPS-EU ✓".
- **Lista blanca analizada y cerrada en 2** con regla de entrada
  (libera + no destruye + verificable); /usage//context//cost nativos
  son lo que el widget vuelve innecesario; /doctor se recomienda, jamás
  se inyecta.
- **presion-y-rendimiento.md** llevaba desde el 05 diciendo "sin
  arrancar" con las fases 1-2 vivas: corregido y añadida la sección
  "Qué queda vivo de este doc".

FOTO DE PENDIENTES al cierre (consolidada por Oscar, 2026-08-09):

- BLOQUEADOS POR DECISIÓN DE OSCAR: updater (repo público + tag v* +
  probar), michi.exe en el instalador (release.yml, solo desde la web),
  capturas del README, lanzamiento (repo privado hasta que diga).
- CÓDIGO DEL RELEVO: alias `~/.bashrc` del VPS (el más corto; cierra
  chat ✓ + terminal SSH ✗), WSL entero (relevo en la distro + alias),
  chat del Windows local (modo wrap en michi.exe).
- VALIDACIÓN PASIVA (usar la app): alarmas reales (umbral/100%/ventana
  nueva), camino ntfy completo (con PC apagada), hallazgo naciendo
  natural con panel cerrado, y el automático arreglado (✓/✕ + reintento)
  — la primera ya pasó en vivo este mismo día.
- APARCADOS CON DISEÑO: HUB+rangos (espera 2.ª máquina), Reporte fase 3
  (export HTML mockup A), detector de pegado masivo, apuesta #2 (tarjeta
  del gatito + gamificación), /export como red pre-/clear, ficha
  recomendando /doctor.
- DE presion-y-rendimiento §"Qué queda vivo" (orden de valor): fórmula
  del % de desperdicio (DISEÑO previo), botón "copiar resumen de
  traspaso", frecuencia de auto-compacts como hallazgo (la señal ya
  está; con el relevo debería BAJAR — prueba medible de que Michi
  trabaja), push ntfy "reporte listo", hábito sin /clear, marcas de
  arreglo manuales, auditoría semántica de CLAUDE.md (pide modelo).
- DE consejos-coach.md: hooks opt-in ("futuro, quizá nunca"), fix
  personalizado por entrypoint (cimiento puesto), botón de issues útil
  al hacerse público el repo.
- DECISIONES CERRADAS (no rediscutir sin leer su doc): lista blanca de
  2, auto-compact no se apaga, compactación no facturable, marketing
  honesto (cuota real + momento elegido + fugas medibles, jamás "ahorra
  compactando"), y los descartes de raíz (score único, modelo local,
  telemetría, rastrear otras herramientas, BD historial, modo empresa,
  podar CLAUDE.md automático).

Lecciones:

- **Una cuenta atrás que acaba sin decir qué pasó es peor que no haber
  avisado**: deja al usuario adivinando si actuaste.
- **Un rechazo transitorio no puede costar un castigo permanente**:
  `done` solo tras aplicar de verdad.
- **Las cabeceras de estado de los docs también se regresionan**: un
  "sin arrancar" viejo esconde pendientes reales y hace rediscutir lo
  hecho. Al cerrar etapa, actualizar el doc que la diseñó.
- **"$0" sin decir QUÉ cuesta cero confunde**: la inyección es gratis,
  la compactación no — y la diferencia importa para el pitch.

## 2026-08-09 — auto-/clear con red: /export verificado antes de borrar

Oscar pidió, con la captura de una conversación de 729 turnos delante,
que Michi decida el /clear como ya decide el /compact. El "jamás" del
auto-/clear tenía escrita su propia salida (remediacion.md §lista blanca:
"candidato razonable: inyectar /export antes como red") y eso exacto se
construyó. Diseño completo en remediacion.md §El auto-/clear con red;
aquí el resumen y lo que se aprendió.

- La regla (a)(b)(c) de la lista blanca no se relajó: a /clear se le
  CONSTRUYÓ la (b). El relevo teclea `/export <ruta>` (ruta que genera
  ÉL — jamás viaja por el canal; la lista del canal sigue en 2),
  VERIFICA que la copia existe con contenido, y solo entonces borra.
  Sin copia: `ERR_RELAY_EXPORT` y cero /clear.
- Verificado en el binario 2.1.226 ANTES de escribir código: `/export`
  a secas abre un menú interactivo (inyectarlo así atraparía el REPL);
  con argumento escribe directo. Por eso la ruta es obligatoria.
- La secuencia corre en un hilo propio en las TRES piezas: esperar la
  copia tarda segundos y bloquear el bucle principal habría dejado el
  estado sin refrescar >15 s — el panel habría dado la sesión por
  muerta. De ahí también `ERR_RELAY_BUSY` mientras dura.
- `STATE_V` 1→2 como compuerta de compatibilidad: un relevo viejo
  ignoraría la marca `export` y borraría SIN copia — el panel no le
  pide la red a un v1 (ni manual ni automático).
- El automático del /clear exige, además de todo lo del /compact:
  interruptor propio `relayClear` (nace OFF), 3 manuales ganadas,
  veredicto Boundary del clasificador (en la duda gana /compact, que
  no borra) y relevo v≥2. El manual de la tarjeta de intención lleva
  la red siempre que el relevo sepa (v2).
- VALIDADO en banco de PTY real en el VPS: terminal 13/13 (regresión
  /compact intacta, /export ANTES de /clear, copia en disco, claude
  sordo → ERR_RELAY_EXPORT y nada borrado) y chat stream-json 6/6
  (sid casado, orden, eco de ambas inyecciones visible). PENDIENTE:
  cargo check y validación en vivo en el Windows de Oscar.
- Copias en `<datos>/handoff/` (Windows) y `~/.michiclaude/handoff/`
  (Linux), nombre `handoff-<pid>-<epoch>.md` sin ni un dato del
  usuario; caducan a los 90 días al arrancar el relevo.

Lecciones:

- **A una prohibición sana no se le quita el candado: se le construye
  la condición que le faltaba.** /clear no cumplía "no destruye";
  con copia verificada en disco, la cumple. La prohibición de fondo
  (jamás borrar sin red) sigue intacta y ahora es código.
- **La verificación buena es un hecho del disco, no un texto en
  pantalla**: el archivo existe con contenido. Los textos cambian de
  idioma y de versión; los archivos no.
- **Todo lo que espera dentro del relevo va en su hilo**: la misma
  lección que el 10ter de la app (síncrono congela), versión PTY.

## 2026-08-09 (segunda) — el Enter pegado al texto: por qué falló la red en la primera prueba real

Oscar compiló, abrió sesión con relevo (`v:2`, `ready:true`) y corrió
`michi inject /clear --export`. Respuesta: `ERR_RELAY_EXPORT`. La red
hizo exactamente lo que promete —no se borró nada— pero la copia no
aparecía y en pantalla no salía ningún error.

Reproducido en el VPS contra Claude Code REAL (dos sondas de PTY que
solo se diferencian en el ritmo del tecleo):

- `"/export <ruta>\r"` escrito de una vez → la línea se queda ESCRITA
  en el prompt y no se ejecuta. Cero salida, cero error.
- El texto, 0,6 s de pausa, y el `\r` aparte → `Conversation exported
  to:` y archivo de 762 bytes en disco.

Causa: la TUI de Claude Code trata el texto y el Enter que llegan en la
MISMA ráfaga de lectura como un PEGADO, y un pegado no se envía solo.
Con `/compact` (9 bytes) colaba; con una ruta de ~110 bytes, jamás.

Arreglo: `type_line()` en las dos piezas de PTY escribe el texto, duerme
250 ms (`ENTER_GAP_MS`) y manda el Enter aparte. Se aplica a TODOS los
comandos: el fallo dependía del largo de la línea y de la velocidad de
la máquina, así que el `/compact` validado el 2026-08-08 estaba vivo de
suerte y podía morder en cualquier momento. El modo chat no lo necesita
(ahí un mensaje es una línea JSON, no teclas).

Validado end-to-end contra Claude Code real: `aplicado: /clear (copia:
…)` con una copia de 912 bytes que CONTIENE la conversación. Banco de
falso claude: 13/13 sin regresión.

Nota de método: el banco de falso claude NO podía cazar esto — su
"claude" lee líneas de stdin, así que el ritmo le da igual. Un banco
prueba tu código contra tu idea del mundo; solo el programa real prueba
tu idea del mundo.

Lecciones:

- **Escribir en una PTY no es mandar bytes: es imitar a un humano.** Y
  un humano no teclea 110 caracteres y el Enter en el mismo instante.
- **Si la TUI no reacciona a algo que SE VE escrito en pantalla,
  sospechar del RITMO antes que del contenido.** La ruta era correcta,
  los permisos también, el comando también.
- **Un fallo que depende del largo de la entrada es un fallo dormido**:
  el /compact "funcionaba" y solo escondía el mismo defecto.

## 2026-08-09 (tercera) — el binario que no se recompiló: empate de mtime

Tras subir el arreglo del Enter, Oscar hizo `git pull` (confirmado:
`git log` en `0e9a283`), recompiló y el fallo SEGUÍA. Media hora de
diagnóstico para algo que no estaba en el código:

- `Select-String ENTER_GAP_MS src\main.rs` → el fuente SÍ traía el
  arreglo (tres coincidencias).
- `cargo build --release` → `Finished` en **0.11 s**, sin ninguna línea
  `Compiling`.
- `cargo clean -p michi` → **"Removed 0 files"**.
- `dir michi.exe` → `08:26`. `dir src\main.rs` → **`08:26` también**.

El `git pull` cayó en el MISMO MINUTO que la compilación anterior.
Cargo decide por fecha de modificación y ante un empate no recompila:
el `.exe` que se ejecutaba seguía siendo el de antes del arreglo. El
"sigue sin funcionar" era literalmente cierto — nunca llegó a probarse
el código nuevo.

Arreglo: `(Get-Item src\main.rs).LastWriteTime = Get-Date` + `cargo
build --release` → `Compiling michi v0.1.0` en 7,55 s y binario de las
08:40. Anotado como regla en CLAUDE.md §Comandos.

Lecciones:

- **Antes de dudar del arreglo, comprobar que el arreglo se ejecutó.**
  Tres señales lo decían y ninguna era el mensaje de error: build
  instantáneo, `clean` que no borra nada, y la hora del binario.
- **`cargo clean -p <paquete>` no es garantía**: dijo "Removed 0 files"
  y nadie se alarmó. La prueba buena es la HORA del ejecutable, no lo
  que diga la herramienta.
- **Al guiar a alguien por comandos, pedir la hora del binario después
  de compilar.** Es una línea y corta en seco toda esta clase de
  diagnóstico fantasma.

## 2026-08-09 (cierre) — el /clear automático nace, y dos bugs que lo tapaban

Jornada larga que empezó con una pregunta de Oscar ("¿puede Michi decidir
el /clear como decide el /compact?") y acabó con la función construida,
validada a mano en Windows y esperando su primer disparo automático.

**Lo entregado** (6 commits, todos con su porqué):

- `8b4bd40` auto-/clear con red: `/export` verificado antes de borrar.
  Detalle y decisiones en remediacion.md §El auto-/clear con red.
- `0e9a283` el Enter va SEPARADO del texto. Bug real que también
  amenazaba al /compact ya validado — vivía de suerte por ser corto.
- `c53cf7f` la lección del binario que no se recompiló (empate de mtime).
- `65f391f` validación en vivo en Windows.
- `5d7c6be` botón «abrir la copia» en el registro de acciones.
- `137d881` regla de lectura de archivos grandes en CLAUDE.md.

**Estado del auto-/clear al cerrar** (medido, no supuesto): interruptores
ON, desbloqueo ganado (/compact 2/2, /clear 5/3), relevo del chat del VPS
vivo en v2 y casado por `sid` EXACTO, veredicto **Boundary** (topen 0,
ttotal 5, gclean true). Lo único que falta es presión: **676k de 1M =
68%**, y el umbral son 80. Oscar decidió NO bajar `INTENT_PCT` para
forzarlo — se gana por diseño, no por carrera.

**Idea de producto de Oscar, anotada para cuando haya datos:** que
MichiClaude no solo señale la fuga sino que enseñe la práctica, con un
bloque copiable para el CLAUDE.md. Tres fichas candidatas, cada una
colgada de un detector que YA existe: claudemdsize (partir el archivo en
índice + historial), reread (leer por rangos) y la fuga al cierre. Regla
de diseño acordada: **una ficha entra solo si hay una señal medible que
la dispare** — un consejo sin dato medido es un post de blog, y eso no es
lo que hace fuerte a este producto. La regla anti-relectura que se puso
hoy en CLAUDE.md es el banco de pruebas: si el detector `reread` deja de
dispararse, la ficha se escribe con el antes y el después.

**Qué queda vivo, por orden:** ver el auto-/clear dispararse solo; medir
el efecto de la regla de lectura; el indicador de relevo en el widget
(lo pidió Oscar al ver que el chat de VS Code no dice si está relevado);
el alias de `~/.bashrc` en el VPS; `michi.exe` en el instalador (toca el
workflow, invariante #9); WSL y el chat del Windows local. Bloqueados por
decisión: updater (repo público + tag) y capturas del README.

Lecciones:

- **Un bug que depende del largo de la entrada es un bug dormido**: el
  /compact "funcionaba" y escondía exactamente el mismo defecto.
- **Antes de dudar del arreglo, comprobar que el arreglo se ejecutó.**
- **Un banco prueba tu código contra tu idea del mundo; solo el programa
  real prueba tu idea del mundo.**
- **A una prohibición sana no se le quita el candado: se le construye la
  condición que le faltaba.**

## 2026-08-10 — etapa 4: el alias de ~/.bashrc para las terminales SSH

El fleco de las terminales de los servidores, cerrado desde el propio VPS
(la máquina donde va a vivir). Detalle de diseño en remediacion.md §"El
alias de ~/.bashrc"; aquí la jornada y sus lecciones.

**Qué se hizo:** guion `TERM_ALIAS_PY` embebido en lib.rs (viaja por
SSH-STDIN, jamás interpolado; veredictos de una palabra), comandos
`term_relay_status`/`set_term_relay` (misma coreografía que el wrapper del
chat: re-subir el relevo ANTES de encender), ejecutor SSH generalizado
(`remote_verdict_py`, ahora compartido con `chat_wrap_remote` sin cambiar
su firma), interruptor en Ajustes bajo el del chat (oculto sin servidores,
invariante #8) y claves `rly_term_*` en los 8 idiomas.

**El enganche es una FUNCIÓN de bash, no un alias ni un shim:** necesita
decidir (¿TTY?, ¿está el relevo?, ¿ya relevado?) y `~/.bashrc` solo lo
leen las shells interactivas — los scripts ni se enteran, que es el
reparto correcto. Fail-open en cascada al `command claude`. Sin bucle por
construcción: las funciones de bash no viajan a subprocesos, así que el
relevo resuelve `claude` por PATH y da con el binario real.

**Validación: banco de 29 comprobaciones contra un HOME falso, 29/29.**
Ciclo on/off que devuelve el archivo byte a byte, backup exacto una sola
vez, idempotencia, marcas rotas = MANUAL sin tocar nada, bloque viejo
reemplazado entero, permisos 600 conservados, `bash -n`, y la función
corriendo de verdad: TTY simulada con `script(1)` → banner del relevo y
el claude real debajo; sin TTY o con `MICHI_RELEVO` → directo al real.
La invocación por STDIN (`python3 - status`) probada tal cual la hará
Rust, y el guion re-extraído del lib.rs para confirmar que lo embebido es
idéntico a lo probado.

Lecciones:

- **El único fallo de la primera pasada era el guard funcionando:** el
  banco corría DENTRO de una sesión ya relevada (este Claude Code del VPS
  va bajo michi-relevo.py) y el `MICHI_RELEVO` heredado disparaba el
  fail-open anti-anidamiento — el relevo hijo cedió al claude real, que
  es exactamente lo prometido. Validación en vivo gratis; el banco ahora
  limpia la variable con `env -u`.
- **`"#` dentro de un raw string de Rust lo CIERRA:** las marcas del
  bloque (`A = "# >>> …"`) contienen comilla+almohadilla y matan un
  `r#"…"#` — el guion va en `r##"…"##`. Sin toolchain en el VPS lo cazó
  la revisión a mano; cargo check en Windows lo habría dicho, pero mejor
  no viajar roto.
- **CLAUDE.md rozó su tope de 40k al anotar el avance** (40.314): el
  diseño aplazado de HUB+rangos se movió ÍNTEGRO a hub-modo-equipo.md y
  quedó el puntero. La regla del archivo aplicada al archivo.

**Pendiente que abre:** cargo check en el Windows de Oscar (aquí no hay
toolchain) y la validación de punta a punta desde el panel (encender el
interruptor, abrir una SSH nueva, ver el banner). De la etapa 4 quedan:
WSL, chat del Windows local y michi.exe en el instalador.

**Cierre del pendiente (mismo día, más tarde):** cargo check limpio en el
Windows de Oscar (11.63 s, con `Compiling` de verdad — no hubo empate de
fechas) y la validación de punta a punta COMPLETA: interruptor nuevo en
Ajustes → «VPS-EU ✓ — abre una sesión SSH nueva para que lo tome», y en
una SSH nueva `claude` mostró el banner «michi · relevo activo (sesión
N)». Verificado además del lado del servidor: bloque con marcas en
`~/.bashrc` (líneas 120–130), backup `.michi-backup` creado en el
instante del encendido, `michi-relevo.py` re-subido fresco y `bash -n`
limpio. El alias de ~/.bashrc queda CERRADO.

## 2026-08-10 (tarde) — el banner del relevo dentro del chat de VS Code

Oscar pidió una señal visible de que el chat va relevado (maqueta previa
con otra IA: banner como primer mensaje y pestaña con su nombre). Se
implementó en el modo `wrap` y costó TRES intentos, cada uno con su
lección:

1. **Pegado al init:** el banner se emitía justo detrás del
   `system/init`. No se pintó nunca — en el arranque la interfaz del chat
   todavía no está lista y la línea se pierde sin dejar rastro.
2. **Delante del primer mensaje:** movido a la primera actividad de
   usuario (con re-armado al cambiar el `session_id`, para que cada
   conversación estrene el suyo). Tampoco se pintó.
3. **La causa real — la FORMA de la línea:** medido contra el binario de
   la extensión (2.1.226), el replay del CLI no es un `user` a secas:
   lleva `session_id`, `uuid`, `parent_tool_use_id`, `timestamp` e
   `isReplay`. La extensión DESCARTA EN SILENCIO lo que no case con la
   sesión. `replay_line()` imita esa forma campo a campo y el banner
   apareció a la primera. Al hijo se le sigue mandando la forma corta.

Lecciones:

- **El eco al chat y el mensaje al hijo son DOS formas distintas** y no
  se pueden confundir: `user_line()` hacia Claude, `replay_line()` hacia
  la extensión. El eco de las INYECCIONES iba con la forma corta desde
  siempre — o sea que el `/compact` inyectado podía no verse en el chat
  pese a que el diseño lo exige («nada a tus espaldas»). El banner
  destapó un fallo silencioso que llevaba tiempo ahí.
- **Un descarte silencioso se diagnostica midiendo, no leyendo:** la
  forma buena salió de correr el binario real con
  `--replay-user-messages` y mirar la línea que emite él.
- Banco propio (10/10) con un claude falso que habla stream-json: init
  intacto, banner único por conversación, re-armado al cambiar de sesión,
  el hijo recibiendo solo los mensajes reales y el paso directo sin
  protocolo. VALIDADO EN VIVO en el chat del VPS.

**Pendiente que abre:** el guion viaja EMBEBIDO en la app
(`include_str!`), así que hasta que Oscar recompile en Windows su
MichiClaude re-subirá la versión vieja al arrancar. `git pull` + build
para que quede permanente.

## 2026-08-10 (cierre) — michi.exe viaja en el instalador

Tercer fleco de la etapa 4, y el de más valor de producto: sin él, todo lo
construido en la etapa 3 solo existía en la copia de desarrollo de Oscar.
Detalle de diseño en remediacion.md §"michi.exe dentro del instalador".

Lo que enseñó la jornada:

- **El pendiente decía "workflow, invariante #9" y no hacía falta ningún
  workflow.** `beforeBuildCommand` mueve la construcción del crate al
  propio Tauri, así que el CI lo hace solo. Un pendiente puede estar
  bloqueado solo por cómo se enunció.
- **Una verificación con `git pull` que dice "Already up to date" no es
  una verificación:** Oscar compiló 6m24s con la configuración vieja
  porque yo le di los comandos ANTES de empujar los cambios. Empujar
  primero, pedir después.
- Su salida de PowerShell delató que `npm run dev` funciona desde
  `src-tauri` (npm sube a buscar el package.json) — de ahí el doble
  intento de ruta en el comando previo. Leer la salida entera del usuario
  paga: ahí venía un dato que yo no había pedido.

## 2026-08-10 (noche) — WSL, y dos fallos que solo salen probando

Etapa 4d cerrada y VALIDADA EN VIVO (detalle y diseño en remediacion.md
§"WSL, la tercera máquina"). Oscar puso el correctivo que la abrió: yo
proponía dejar WSL dormido porque ÉL trabaja por Remote-SSH, y su
respuesta fue que la app es para más gente que él. Tenía razón: el modo
que uno no usa sigue siendo el modo de alguien.

Lo que enseñó la jornada:

- **Lo que compila y parece razonable puede no ejecutarse nunca.**
  `wsl.exe -- sh -c 'guion' michi <arg>` no entrega `$1` (ssh sí). El
  código era simétrico al de SSH, pasó `cargo check`, y estaba roto.
  Solo se vio ejecutando el comando A MANO en la máquina de verdad.
- **El fallo grave no era ese, era el silencio.** Con la operación vacía
  los guiones caían en la rama de "apagar", no encontraban nada que
  quitar y contestaban OK: el interruptor enseñaba ✓ de algo que jamás
  tocó la distro. Escribimos la regla "nada a tus espaldas" para el
  relevo y la incumplió el propio panel. Ahora una operación que no se
  reconoce contesta BADOP. Callar es peor que fallar.
- **Diagnosticar por hipótesis tiene un límite.** Perseguí "¿corre como
  root?" con dos comandos antes de rendirme a lo obvio: ejecutar a mano
  el comando exacto que hace la app. Ese fue el que habló, y a la
  primera. Cuando dos observaciones se contradicen, deja de teorizar y
  reproduce.
- **Un `git pull` que dice "Already up to date" no es una verificación:**
  antes, Oscar compiló 6m24s con la configuración vieja porque le di los
  comandos antes de empujar. Empujar primero, pedir después.
- **Probar sin el programa real vale:** `tests/claude-falso.sh` (cinco
  líneas que imprimen lo que llega) permitió validar la cadena entera sin
  instalar Claude Code en la distro ni gastar cuota. Al relevo le basta
  una PTY viva que reaccione.

De paso quedó probado que la marca del título (`TitleMark`) también
funciona en WSL, que no lo habíamos mirado.

## 2026-08-10 (cierre) — el chat de Windows, y la ETAPA 4 COMPLETA

Último fleco del relevo, pedido por Oscar con el argumento que lo cambió
todo hoy: "es para más usuarios que yo". Diseño y detalle en
remediacion.md §"El chat de VS Code en Windows".

El relevo llega ya a las TRES máquinas (Windows local, SSH y WSL) por las
DOS vías (terminal y chat), y todo está validado en vivo.

Lecciones de la jornada:

- **Un enganche invisible necesita rastro desde el primer día.** La
  extensión se come stderr: un wrapper que no arranca se ve EXACTAMENTE
  igual que uno que funciona (la conversación sigue, el relevo no
  aparece). Estuvimos dos rondas suponiendo; `wrap_debug.txt` contestó a
  la primera y además demostró que el paso directo funciona (la llamada
  `auth status --json`, sin protocolo, se dejó pasar tal cual).
- **32 minutos de diferencia entre dos binarios nos tuvieron ciegos:** el
  ajuste apuntaba a la copia que `tauri dev` deja junto al ejecutable, y
  esa copia era de ANTES de compilar el rastro. Al ver un `michi.exe`
  VIVO en la lista de procesos se entendió todo: el enganche funcionaba
  desde el principio; lo que faltaba era el binario nuevo.
- **El banco encontró un fallo que habría borrado ajustes ajenos:** con un
  settings.json escrito en una sola línea, nuestra clave compartía renglón
  con los ajustes del usuario y al apagar el interruptor se los llevaba.
  Se cazó antes de tocar un archivo de verdad. De ahí que la línea se
  inserte siempre sola en su renglón.
- **No duplicar la coreografía:** en vez de escribir un segundo attend
  para el chat, se extrajo un `Speaker` — la terminal teclea, el chat
  manda protocolo, y R1-R5 y la red del /export son una sola
  implementación. Es la misma decisión que en `relay_inject_fs` y
  `relay_from_json` esta misma tarde: si hay dos copias de una regla, un
  día divergen.

**Residual:** el banner del chat no se pinta en Windows (en Linux sí). No
es el mecanismo —el eco del /compact inyectado usa la MISMA línea de
replay y sale perfecto—, es cuándo se emite. Pendiente de rastro,
cosmético.

**Cierre real del 4e (misma noche):** el aviso del chat de Windows YA SE
PINTA. Tres rondas creyendo que el mecanismo fallaba, y no era eso:

1. Dos rondas con un binario viejo — el ajuste apuntaba a la copia que
   `tauri dev` rehace en cada arranque. Corregido: en debug manda el
   michi.exe que uno compila. Un fantasma solo se caza mirando QUÉ
   ejecutable está corriendo (`Get-Process michi | Select Path`), no
   releyendo el código.
2. Y la última: el aviso llegaba mientras el mensaje del usuario iba en
   vuelo, y ahí la extensión no lo pinta; el eco de un /compact inyectado
   —la MISMA línea— sí salía porque llega con el chat en reposo. Se emite
   ahora tras el `result` del primer turno. La pista no vino de una idea
   nueva sino de comparar dos usos del mismo mecanismo, uno que funcionaba
   y otro que no, y preguntar en qué se diferenciaban.

De paso: el interruptor tiene que reconocer sus PROPIAS rutas anteriores.
Si no, tras mover cuál michi.exe se usa, ve su ruta vieja como "wrapper
ajeno", se niega a pisarla (regla correcta con uno de verdad ajeno) y se
queda encallado en OTHER sin forma de salir desde la interfaz.

## 2026-08-11 — la presión de contexto deja de ser un arco y pasa a ser una idea

Petición de Oscar con dos bocetos HTML propios: la presión de contexto del
gatito, contada como una BOMBILLA que se degrada, con el gato "pensándola".
La columna que pidió, de abajo arriba: gato → bombilla en medio → cápsula del
% de sesión, y "que no quede amontonado".

El manómetro anterior era un arco SVG de 13 px metido dentro de la cápsula.
Funcionaba y era honesto, pero competía por el espacio con el %, el rótulo y
la cuenta atrás del automático: se VEÍA y no se LEÍA. Un dibujo que cambia de
forma —filamento limpio, onda, maraña, dos trozos y una grieta— se entiende
sin mirarlo fijo, que es justo lo que hace un widget de bandeja.

**Lo que se conservó tal cual** (era la mitad del trabajo): niveles con los
umbrales de siempre (60 y 85, más un paso nuevo en 40 que solo cambia el
dibujo), el punto del RELEVO —ahora en el casquillo—, la bombilla fuera del
early-return de la cuota (la presión sale de los logs, no del endpoint), y el
número exacto con su proyecto en el globo del hover. La pastilla NO se tocó:
sigue con su arco.

**El truco que hizo barato el cambio: `.stage`.** La ventana tenía que crecer
hacia arriba, y sobre esos 210x157 estaba calibrado casi todo el widget en
PORCENTAJES: el recorte de los gifs, la zona de clic de la cabeza, los dos
post-its. Recalcularlos habría sido una tarde y un rosario de bugs finos. En
vez de eso, el gato y todo lo suyo se metieron en un `.stage` que mide
EXACTAMENTE lo que medía la ventana (210x157) y va pegado al fondo: los
porcentajes resuelven contra él y ni uno cambió de significado. Verificado
pintando la zona de la cabeza de rojo y forzando los post-its: caen donde
caían.

**Tres trampas que no se ven leyendo el código:**

1. **El gato se hundía.** La posición guardada es la esquina SUPERIOR
   izquierda; al crecer la ventana 48 px hacia arriba, quien tuviera el gatito
   posado sobre la barra de tareas (la posición por defecto) se lo habría
   encontrado medio tapado. `migrate_cat_geometry` conserva el borde INFERIOR
   una sola vez (campo `geom`), que es lo que ya hacía `set_pill_style` al
   alternar pastilla ↔ gatito. Y va en píxeles FÍSICOS: con pantalla al 150%
   son 72, no 48 — de ahí el factor de escala.
2. **Los globos caían sobre la bombilla.** Su solape está medido contra el
   BORDE de la ventana, no contra el gato. Sumarles `CAT_TOP_H` los devuelve
   exactamente donde estaban respecto a la cabeza. Por eso el alto de la
   franja es una constante y no un número suelto en tres sitios.
3. **El vidrio no contrastaba.** El primer render salió correcto de geometría
   y mudo de lectura: el cristal casi blanco sobre el papel del globo (casi
   blanco también) desaparecía, y a escala 0.85 el filamento no se distinguía.
   Se arregló con lo que sí se lee a 26 px — la TEMPERATURA del vidrio, cálida
   encendida y fría muerta — y subiendo la bombilla a escala 1:1, con los
   rayos acortados para que no rocen el borde de la nube.

**Cómo se verificó sin Windows.** El VPS no tiene ni toolchain de Rust ni
Pillow, así que: (a) un decodificador GIF en stdlib para MIRAR el arte y medir
dónde caen las llamas del estado `fire` (arriba-izquierda) y las Z del `zzz`
(arriba-derecha) — la bombilla se colocó en el pasillo libre que queda entre
las dos; (b) una composición de la ventana nueva con las cajas de la columna
encima, para ver choques y aire; (c) chromium headless renderizando un banco
que EXTRAE el `<style>` y el marcado reales de cat.html —nada retecleado— en
los cuatro niveles y las dos pieles. `cargo check` sigue pendiente del Windows
de Oscar: aquí no hay cargo.

De paso, el simulador recorre los cuatro niveles (`p` en SIM_CAT → `simPress`,
resuelto DENTRO de emitPill y no parcheando `lastPill`, que la regla prohíbe).
Sin eso, ver el estado "muerta" costaba llenar un contexto de verdad hasta el
85%.

**Y una cuarta trampa, de regalo: `.hidden` no existe en SVG.** Al copiar el
patrón del punto del relevo (`$("x").hidden = !relay`) saltó la duda: `hidden`
es una propiedad de `HTMLElement`, y ese punto es un `<circle>`. Comprobado en
Chromium: no está en `SVGElement.prototype`, así que la asignación crea una
propiedad suelta en JS y NO toca el atributo — el punto nace oculto y se queda
oculto PARA SIEMPRE, sin un solo error en consola. El CSS sí funcionaba y por
eso engañaba: `[hidden]` es un selector de ATRIBUTO y le da igual el
namespace. Lo mismo pasaba en `pill.html` desde la etapa 3b: **el punto del
relevo de la pastilla no se ha enseñado nunca**. Los dos van ya con
`toggleAttribute`. Lección: una propiedad que no existe no avisa, solo no hace
nada; cuando el mismo patrón se copia a otro tipo de elemento, hay que
comprobar que el patrón siga siendo válido ahí.

**Lo que queda peor y hay que saberlo:** la ventana es 48 px más alta, y esos
48 px son transparentes pero SÍ atrapan el clic (una ventana es un rectángulo;
no hay hit-testing por píxel). Sobre el escritorio no molesta; encima de otra
ventana, es un poco más de superficie muerta.

## 2026-08-11 (segunda) — la bombilla, en su sitio: pequeña, suelta y con su propia ficha

Oscar probó la primera versión y volvió con capturas y ajustes. Todos apuntaban
al mismo sitio: la bombilla se había comido el widget en vez de sumarse a él.

Lo que pidió y quedó: bombilla PEQUEÑA (34x44 en vez de 76x96), SIN globo de
pensamiento, animada en cada estado, en el eje de la cápsula y con poco aire
entre las tres piezas; la cápsula vuelve a su alineación de siempre —posada
sobre la cabeza y ladeada 15.5°— y solo SUBE cuando hay bombilla que alojar
(`body.hasidea`), bajando sola cuando no hay sesión; la información de contexto
sale del globo de resumen y pasa a una ficha propia al pasar el mouse por la
bombilla.

**Al quitar el globo, la ventana volvió a caber en sí misma.** Los 48 px de
franja que había ganado esta mañana se devolvieron: `CAT_TOP_H` desaparece, los
dos globos recuperan sus solapes de siempre y se acaba la zona muerta
transparente que se tragaba clics. Queda `CAT_GEOM_V1_TOP` con un único
cometido: DESHACER el desplazamiento en las configuraciones que alcanzaron a
guardar la versión 1 (migración `geom` 1 → 2). Una corrección de posición no se
puede "revertir con el código": el archivo del usuario ya cambió.

`.stage` se queda aunque hoy mida lo mismo que la ventana. Sale gratis, ancla al
gato abajo y, si algún día vuelve a crecer por arriba, ninguna de las
calibraciones en porcentajes cambia de significado. Ese fue el trabajo de la
mañana; conservarlo cuesta una línea.

**Separar cuota de contexto era lo correcto y no solo una preferencia.** El
globo del resumen habla del PLAN (sesión, semanal, buckets por modelo) y la
presión de la SESIÓN que tienes abierta; mezcladas, había que desplegar el
resumen entero para mirar un número. La ficha nueva vive DENTRO de la ventana
del gatito, no en una ventana Tauri: cada WebView2 arranca en ~57 MB y esto es
una etiqueta de dos renglones.

**El bug que solo se ve renderizando.** La clase de estado del `<body>` se
llamaba igual que la clase del elemento: `ptip`. Como la regla del elemento es
`.ptip{display:none}`, el selector casaba TAMBIÉN con el `<body>` — y al pasar
el mouse por la bombilla el widget ENTERO desaparecía. No hay error en consola,
no hay nada raro en el diff: solo una ventana que se apaga. Salió a la primera
captura del estado de hover, y de ahí la regla: **el nombre de una clase de
ESTADO en el body nunca puede coincidir con el de una clase de ELEMENTO**
(ahora `body.showtip` frente a `.ptip`).

De paso, dos trampas del banco de pruebas que conviene no repetir: en
`chromium --headless`, `--window-size` va en píxeles de DISPOSITIVO (con
`--force-device-scale-factor=2` el viewport CSS es la mitad, y las columnas de
más se quedan fuera del recorte pareciendo vacías), y con factores altos este
chromium de snap devuelve la captura en blanco — se amplía con `zoom` en CSS.
Y el banco necesita un ancestro `position:relative`: en vivo ese papel lo hace
la ventana, y sin él todo lo absoluto se va al fondo del viewport.

El simulador de la bombilla ya no viaja dentro del guion del gatito: tiene botón
propio (💡 Simular contexto, solo dev) porque prueban cosas distintas —aquel
recorre estados de ánimo y avisos, este los cuatro dibujos y el salto de la
cápsula—, y empuja con `emitPill`, así que cualquier refresco durante la prueba
sigue enseñando el nivel simulado.

**Calibración final de la columna (misma tarde, con capturas de Oscar).** Tres
números y el porqué, para que nadie los "arregle" luego: la bombilla va en el
eje de la CABEZA (`left:68%`), no en el de la cápsula (72.4%), y casi posada en
ella (`top:47px`, ~3 px de aire); la cápsula con bombilla baja al 20%. Repartido
por el hueco, el trío se leía como tres cosas sueltas; junto y pegado al gato se
lee como algo SUYO. Comprobado contra los tres estados que podían chocar: las
llamas del `fire` quedan al otro lado, las Z del `zzz` libres —moverla a la
izquierda ayudó— y la ficha del hover no la toca. Único roce: en `zzz` la
bombilla se posa sobre la punta del gorro de dormir; con su trazo y su sombra se
lee como encima, y se deja así antes que meter una excepción por estado
(además apenas puede darse: el gato duerme por el semanal agotado y la bombilla
exige sesión tocada hace <10 min, así que solo coinciden en esa cola).
Rematado con dos detalles de Oscar: la bombilla lleva la MISMA inclinación que
la cápsula (15.5°) —así se leen como piezas del mismo juego y no como un icono
pegado— y los tipos de la ficha son los del globo de modelos (12.5/11.5/10):
el mismo dato no puede leerse más chico en una superficie que en otra.

## 2026-08-11 (noche) — nace el análisis local: la insignia inteligente de /clear vs /compact

Primera pieza de IA local en MichiClaude, tras la investigación de Oscar en
`docs/modelos-locales-cpu.md` (Qwen3.5-2B + llama.cpp medidos en su i7 sin
GPU) y una conversación larga sobre dónde SÍ aporta un modelo chico y dónde
no. Diseño completo en `docs/analisis-local.md`; aquí el porqué de lo
construido y las trampas.

**El caso elegido** (de Oscar, con su captura de la tarjeta genérica): cuando
la tarjeta de intención sale sin insignia —veredicto `unsure`, ni TodoWrite ni
commit limpio que decidan—, hoy la pregunta clave ("¿lo que sigue necesita lo
ya hablado?") se le devuelve al usuario. El modelo local la contesta leyendo
el ai-title y los últimos 3 mensajes humanos, y pinta una insignia PUNTEADA
("Análisis local · tema nuevo") distinta a propósito del "Recomendado" sólido:
una inferencia no puede vestirse de hecho.

**Las decisiones que ordenaron todo:**

1. **El modelo consume hechos, jamás vive en el motor.** El exportador es
   stdlib puro y así se queda (invariante #1): la evidencia viaja en el hit
   `press` (campos aditivos `title`+`msgs`) por el mismo SSH de siempre, y el
   análisis corre SOLO en la máquina del panel. Las sesiones del VPS se
   analizan igual sin que el VPS sepa que existe un modelo.
2. **`user_turn_text` es el único filtro.** La evidencia necesita el TEXTO de
   los turnos humanos y `is_user_turn` solo contaba: se refactorizó para que
   el texto sea la fuente y el bool la envuelva — en Rust y en Python. Dos
   implementaciones del mismo filtro habrían divergido tarde o temprano.
3. **HTTP a mano sobre TcpStream.** reqwest está sin la feature `blocking` y
   el patrón de la casa es async → spawn_blocking; antes que añadir features
   o deps (invariante #4), un POST HTTP/1.1 contra 127.0.0.1 son 40 líneas de
   std — con des-chunkeo a nivel de BYTES (los tamaños de chunk son bytes;
   por chars se descuadraría con UTF-8).
4. **El truncado de mensajes va por CHARS, no bytes** (300): un corte por
   bytes parte un carácter UTF-8 por la mitad y revienta el JSON del hit.
   `chars().take(300)` en Rust ≙ `[:300]` en Python — la réplica coincide.
5. **llama-server nace y muere en cada análisis** (guard con Drop que cubre
   todos los `?`): la app pesa 276 MB y un residente de 2 GB mata el pitch.
   Flags directos de la investigación: -ngl 0, --no-mmap, sin razonamiento
   (12x), temp 0 y gramática GBNF — el enum se FUERZA, no se pide.
6. **Una invocación por sesión aunque falle** (`aiTried` en la tarjeta):
   reintentar en cada sondeo sería arrancar un servidor de 1.3 GB en bucle
   cada 3 minutos contra un fallo persistente.
7. **La evidencia no se persiste.** `msgs` vive en el hit en memoria; al
   almacén de tarjetas solo entra el veredicto `{rec, reason}`. Y el sesgo
   asimétrico va cosido en DOS capas: el prompt ("when in doubt NEVER answer
   clear") y el render (la insignia de /clear solo con razón `tema_nuevo`).
8. **Los hechos mandan hasta el final**: la insignia se pinta solo si el
   veredicto determinista SIGUE en unsure al momento de pintar — si entre
   tanto apareció un TodoWrite, la inferencia se calla sola.

**Lo que NO hace, por diseño y para siempre:** tocar el automático. El
auto-/clear sigue exigiendo Boundary determinista + relayClear + 3 manuales +
red de /export; un "tema_nuevo" del modelo no abre ni una compuerta.

**v1 sin embeddings a propósito:** la escalera completa (hechos → embeddings
→ 2B) está en el diseño, pero lo que decide si esto sirve es la CALIDAD del
veredicto del 2B, y Oscar ya tiene llama-server y el GGUF instalados — cero
descargas para empezar a probar hoy. Los embeddings son un atajo de velocidad
y llegan en la etapa 2 si el veredicto demuestra valer.

**Cómo se prueba sin esperar una sesión al 80%:** Ajustes → Análisis local
(IA) → encender, ruta del .gguf → **Probar**: es la MISMA tubería real
(`ai_intent`) con evidencia de ejemplo que cambia claramente de tema — lo
esperado es `clear · tema nuevo` en segundos (arranque frío ~10-20 s).

Pendiente de `cargo check` en el Windows de Oscar (aquí no hay toolchain);
el JS y el Python pasaron sus verificadores. 18 claves i18n nuevas ×8
idiomas.

## 2026-08-11 (cierre) — descarga guiada: el análisis local sin escribir rutas

Oscar probó la pantalla nueva y vio lo que vería un usuario nuevo: dos cajas
de ruta vacías y un error. Pregunta suya: "¿hay manera de que lo haga en
automático cuando active?". La hay, y era la mitad de la etapa 2 que valía la
pena adelantar (la otra mitad, los embeddings, siguen esperando su turno).

Al encender el interruptor, si falta algo aparece **Descargar todo
(~1.4 GB)** — o "(~17 MB)" si solo falta llama.cpp — con progreso en vivo y
una nota que dice de dónde viene cada cosa. `ai_setup` baja el zip del
release de GitHub y el GGUF de Hugging Face, verifica las huellas SHA-256,
descomprime, rellena la config y enciende. Las cajas de ruta quedan como
ajuste avanzado: quien ya tiene los archivos (Oscar) no ve el botón.

Decisiones y por qué:

- **URLs y huellas en CUATRO CONSTANTES del binario** (b10362 de llama.cpp y
  el GGUF exacto de la investigación, con sus SHA-256 consultadas de las
  fuentes al implementar). Es la regla del updater: nada de esto puede salir
  jamás de un archivo descargado. Al actualizar el pin, las cuatro juntas.
- **Verificación con `Get-FileHash` y descompresión con `Expand-Archive`**:
  PowerShell del sistema antes que un crate de sha256 o de zip (invariante
  #4, la misma decisión que la etapa 2 de remediación). Si la huella no
  casa, el archivo SE BORRA — medio archivo corrupto no puede quedarse
  esperando a que alguien confíe en él.
- **`llama-server.exe` se BUSCA dentro de lo descomprimido** (`find_ls`): el
  zip de llama.cpp ha cambiado de forma entre builds y suponer la ruta es
  apostar a que no vuelva a cambiar.
- **Sin resume**: media descarga se rehace entera. La verificación es por
  huella del archivo completo; reanudar añadiría estados a medias por
  ahorrar minutos de una operación que se hace UNA vez.
- **Idempotente**: el botón baja solo lo que falte, así que "reintentar"
  tras un fallo es el mismo clic.
- **Es la única conexión de la app que no va a api.anthropic.com** — GitHub
  y Hugging Face, una vez, opt-in y anunciada en la propia interfaz. Quedó
  escrito en CLAUDE.md porque toca el matiz del invariante #3.

Pendiente igual que la v1: `cargo check` y la prueba en vivo en el Windows
de Oscar (aquí ni toolchain ni Windows). El camino feliz del usuario nuevo
quedó en: encender → Descargar → esperar la barra → Probar.

**Postdata del mismo día — "no me llega el consejo de /clear".** Oscar abrió
una sesión de prueba aparte, cambió de tema varias veces y no salió nada;
preguntó si se había acabado el límite diario. No: la tarjeta de intención
está EXENTA del tope de 10. Lo que pasaba es que el detonante no es el cambio
de tema sino la PRESIÓN ≥80% del techo de esa sesión — una sesión recién
abierta anda por el 1-2%, y con techo de 1M harían falta ~800k tokens. El
tema solo decide CUÁL de las dos sugerencias sale, una vez que la tarjeta ya
nació. Segunda intuición suya, también descartada: dejarla quieta unos
minutos tampoco la saca — el hit `press` exige sesión tocada hace <10 min
(`PRESS_QUIET_MAX`), así que la quietud la APAGA en vez de encenderla (lo que
sí nace con una sesión quieta es la ficha `cache`, y solo con ≥30k de
contexto). Queda escrito en el diseño porque es la pregunta que cualquiera
se hará la primera vez.

De ahí salió el botón **🎯 Simular intención** (solo dev): crea la tarjeta con
veredicto `unsure` y corre el `ai_intent` DE VERDAD sobre la evidencia de tu
sesión activa más fresca — tus mensajes reales, no un ejemplo, cuando los
hay. Sin él, validar la insignia significaba esperar días a que una sesión
larga cayera además en la zona gris. Detalle de implementación: en modo
simulación las tarjetas se reconstruyen desde `coachHits` en cada render, así
que el veredicto se cuelga del HIT (`_ai`) y no del envoltorio persistido —
si no, se perdía en el primer repintado.

**Primera prueba del 🎯 y el fallo del propio simulador (2026-08-12, madrugada).**
La tarjeta salió perfecta —86%, proyecto y origen reales, las dos opciones—
pero con la insignia RECOMENDADO determinista, no con la del modelo. No era
un fallo del análisis: `siFakeIntent` copiaba el `cont` REAL de la sesión
viva, y en una sesión de trabajo ese Jaccard va alto ("sigues en los mismos
archivos") → `intentVerdict` = **alive** → el render suprime la inferencia,
tal y como manda la regla #2 del diseño (los hechos ganan). O sea: el
mecanismo funcionó exactamente como debía y lo que estaba mal era el banco
de pruebas. Las señales deterministas del hit simulado van ahora NEUTRAS
(topen/ttotal/cont/gclean en cero); la evidencia —título y mensajes— sigue
siendo la real. De paso, el veredicto se escribe también en `flowLog`: el
`simMsg` vive en Ajustes y la tarjeta en Consejos, así que un "unsure"
—que por diseño no pinta insignia— se veía igual que un fallo.

Lección: un simulador que hereda demasiado del estado real puede reproducir
el camino EQUIVOCADO con total fidelidad. Al forzar un escenario hay que
neutralizar justo las variables que lo definen.

## 2026-08-12 — el primer veredicto del modelo local, y el mecanismo equivocado

Segunda prueba del 🎯 (ya con las señales deterministas neutras) y el rastro
del flujo dio la respuesta en una línea: `sim intención: ERR_AI_BADOUT`. El
modelo arrancaba, contestaba, y su salida no se podía leer.

**La causa, verificada en la documentación de llama.cpp:** el parámetro
`grammar` (GBNF) SOLO existe en el endpoint NATIVO `/completion`. En
`/v1/chat/completions` —el que usamos, porque es el que aplica la plantilla
de chat del modelo— se ignora **en silencio**: no da error, simplemente no
restringe nada. Así que el 2B contestaba en prosa libre y
`serde_json::from_str` moría. La vía correcta en ese endpoint es
`response_format` con esquema (`{"type":"json_object","schema":{…}}`), que
llama-server convierte él mismo a gramática: mismo blindaje, endpoint
correcto — los `enum` se cumplen al MUESTREAR, no al validar.

Lección para el archivo: **un parámetro ignorado en silencio es peor que uno
rechazado.** Si la petición hubiera fallado con 400, el diagnóstico habría
sido inmediato; al aceptarla y no aplicarla, el fallo aparece tres capas más
abajo, disfrazado de "el modelo no sabe responder". Antes de dar por bueno un
mecanismo de restricción, hay que comprobar que el ENDPOINT concreto lo
implementa.

De ahí salió también **`ai_debug.txt`** (carpeta de datos, se sobrescribe):
petición y respuesta CRUDA del último intento. Es la misma familia que
`quota_debug.json`, `wrap_debug.txt` y `rem_debug.json`, y la misma lección
que dejó el chat de VS Code el 2026-08-10: *un enganche invisible necesita
rastro desde el primer día*. Aquí se saltó ese paso al construir y costó una
ronda entera de adivinar.

De paso, el parseo se volvió tolerante: mira `reasoning_content` si `content`
viene vacío, recorta al primer `{…}` por si el modelo pone algo delante, y
detecta el campo `error` del servidor en vez de tratarlo como salida ilegible.

**Lo que la prueba SÍ validó** (todo lo demás de la cadena funciona): la
tarjeta nace con datos reales (86%, proyecto y origen correctos), el
clasificador determinista manda —en la primera prueba dio `alive` por el
`cont` real y suprimió la inferencia, exactamente como está diseñado—, los
botones de copiar comando funcionan, el simulador no ensucia el almacén real
("Ahora no" filtra `coachHits`, no localStorage) y el modelo carga y responde
en el tiempo esperado.

**Segunda autopsia, mismo día — el modelo sí contestaba, pero pensando.**
Con el `ai_debug.txt` ya escribiendo, el segundo `ERR_AI_BADOUT` se resolvió
en una lectura:

```
"finish_reason":"length", "content":"",
"reasoning_content":"Thinking Process:\n\n1. **Analyze the Request:**..."
```

Qwen3.5 **razona por defecto**. El `--reasoning-budget 0` que le pasamos a
llama-server es solo un DEFAULT del servidor y la plantilla de chat lo pisa,
así que el modelo gastó sus 40 tokens redactando un "Thinking Process:" y
dejó `content` vacío. Y un detalle que conviene recordar: la gramática del
`response_format` restringe SOLO el canal `content` — lo que el modelo
escriba razonando no pasa por ella, así que no hay blindaje que valga si el
razonamiento está encendido.

Lo humillante y lo útil: **la solución llevaba escrita desde el principio en
`modelos-locales-cpu.md` §3**, en la sección de configuración del cliente —
`{"chat_template_kwargs": {"enable_thinking": false}}` y, como alternativa "a
prueba de balas", `/no_think` al final del mensaje. Yo leí ese documento
entero para diseñar esto y aun así implementé solo la mitad de la receta: la
del servidor. Ahora van las dos, cinturón y tirantes.

Regla que queda: **cuando un documento de investigación dice "el servidor
solo pone un default y el cliente lo pisa", eso es una instrucción para el
CLIENTE, no una curiosidad.**

Datos buenos del mismo volcado: el prefill fue de 208 tokens a 60.8 tok/s
(3.4 s) y la generación a 13.9 tok/s — o sea que el análisis completo saldrá
en ~6-8 s con el servidor ya caliente, dentro de lo prometido. Y confirmó que
corre el GGUF descargado por la app y el build `b10362` que pineamos.

**FUNCIONA (2026-08-12, 00:39).** `sim intención: clear · tema_nuevo`, con la
insignia punteada "Análisis local · tema nuevo" sobre la opción `/clear` y
claramente distinta del "RECOMENDADO" sólido del clasificador determinista.
La evidencia era la de ejemplo —"commit y push de la bombilla" seguido de
"planeemos las capturas del README"— y el veredicto es el correcto: tema
nuevo, no necesita lo anterior.

Cadena validada de punta a punta: motor (Rust + exportador) → evidencia en el
hit `press` → llama-server bajo demanda → esquema que fuerza el enum →
insignia que dice de dónde viene. Tres autopsias hicieron falta y ninguna fue
del diseño: binario viejo (empate de mtime), `grammar` ignorado en el
endpoint de chat, y el razonamiento encendido por defecto.

**Lo que queda para cerrar la v1** (validación pasiva, con el uso):
1. Ver la insignia en una tarjeta REAL —sesión al 80% con veredicto
   `unsure`—, no simulada.
2. Anotar unos días si ACIERTA. Ese es el dato que decide la etapa 2
   (embeddings como peldaño previo) o si hay que afinar el prompt.
3. Cuando el repo sea público: el espejo de modelos en GitHub Releases.

## 2026-08-12 (tarde) — el automático por INFERENCIA: el modelo puede disparar el /clear

Oscar lo pidió con todas las letras: que el `/clear` se aplique solo cuando lo
recomiende el modelo, con las reglas y la red que ya existen, para probarlo
unos días. Eso cruza la que yo había escrito como **regla #1** del análisis
local ("el modelo jamás sustituye una compuerta"), así que lo primero fue
decírselo y lo segundo diseñarlo de forma que la red aguante. Su decisión,
implementada — y la regla #1 REESCRITA en el diseño en vez de dejarla
mintiendo.

**La forma: camino PARALELO, no sustitución.** El auto-/clear tiene ahora DOS
razones válidas, cada una con su interruptor:

| | (a) HECHO | (b) INFERENCIA |
|---|---|---|
| Dispara | `Boundary` (lista al 100% o commit limpio) | `unsure` + modelo dice `clear`/`tema_nuevo` |
| Interruptor | `relayClear` | `relayClearAi` (cuelga del anterior, nace OFF) |
| Cuenta atrás | 15 s | **30 s** |

Todo lo demás se exige IGUAL: interruptor maestro, 3 manuales de `/clear`
ganadas a mano, relevo v≥2, widget A LA VISTA, una vez por sesión sellada
antes de empezar, cualquier toque la para, R1-R4 al escribir, y la **copia
`/export` verificada en disco o no hay `/clear`**. Esa red es lo que hace la
apuesta defendible: un `/clear` por inferencia equivocada cuesta una copia que
sigue en `<datos>/handoff/`, no la conversación.

**Dos exigencias extra que solo tiene el camino (b):**

1. **`topen === 0`.** El veredicto `unsure` ya lo implica (con tareas abiertas
   sería `alive`), pero se comprueba OTRA VEZ a propósito. Defensa en
   profundidad: el día que alguien toque `intentVerdict`, esta puerta sigue
   cerrada. Un hecho no se sobreescribe con una opinión.
2. **`reason === "tema_nuevo"`.** El sesgo asimétrico llevado al automático:
   `tema_cruzado`, `tarea_viva` y `cierre` NO borran, caen al `/compact`.

**El detalle que decidía si esto servía de algo: el automático tiene que
ESPERAR el veredicto.** Al llegar al 80% con `unsure`, el automático de
siempre aplicaría `/compact` en el PRIMER sondeo — antes de que el modelo
alcance a hablar — y el camino nuevo nunca se usaría. Ahora, con (b) armado y
el análisis en marcha, el sondeo se abstiene y espera al siguiente
(`aiPending`). Acotado: 10 min desde que nació la tarjeta, y un fallo del
análisis marca `aiErr` para dejar de esperar de inmediato. La presión solo
sube, así que esperar nunca empeora nada. Sin esta pieza el resto era decorado.

**La cuenta atrás es el doble (30 s)** y con eso queda completa una escalera
que el proyecto ya venía usando sin nombrarla: **5 s cuando lo pides tú, 15
cuando lo decide un hecho medido, 30 cuando lo decide una inferencia.** Cuanto
más blanda la razón, más tiempo para pararla.

**Cómo se audita la prueba:** el rastro del flujo distingue quién decidió —
`relevo auto: aplicado /clear por IA (tema_nuevo)` frente a `… por hecho`. Ese
es EL dato de estos días. Y si aparece un `por IA` donde no debía, la copia
está a un clic desde el registro de acciones.

**Orden de retirada si sale mal** (escrito ANTES de probar, que es cuando se
piensa con la cabeza fría): apagar `relayClearAi` y el resto del automático
sigue como estaba → si el problema es el veredicto, afinar el prompt → si es
sistemático, volver a la v1 (solo insignia). El camino (a) nunca depende del
modelo.

**Nota de mantenimiento:** CLAUDE.md quedó otra vez pegado al tope de 40k
(39.982). Es la tercera vez en el día que meter una regla nueva obliga a
recortar prosa de otras. Cuando vuelva a apretar, lo sano es mover el bloque
de REMEDIACIÓN —7.5 k, y su propio doc ya dice tenerlo todo— y dejar aquí solo
el puntero.

**La cuenta atrás no decía QUÉ iba a aplicar (encontrado 2026-08-12 al
escribir los ejemplos).** Oscar pidió los casos del `/clear` explicados con
ejemplos del día y "qué voy a ver en cada uno". Al ir a describir lo que se ve
—en vez de asumirlo— salió el hueco: el widget pintaba SOLO el segundero, así
que la cuenta de un `/compact` y la de un `/clear` eran idénticas en pantalla.
Una resume y la otra BORRA, y con dos razones posibles (hecho o inferencia) la
ambigüedad crecía justo el día que se enciende el camino nuevo.

Lo más llamativo: el texto completo —"Michi va a aplicar /clear en 30 s, toca
para parar"— ya viajaba en el evento `relay:auto` desde la etapa 3c-2. Estaba
construido y **nadie lo pintaba**. Se emitía, se traducía a 8 idiomas y se
tiraba.

Arreglado: el chip lleva el comando y el color habla — ÁMBAR `/compact 15`,
ROJO `/clear 30`. En la pastilla cabe entero; en el gatito no caben las dos
cosas, así que mientras la cuenta corre el "Sesión X%" se aparta (`body.autorun`)
y la cápsula queda dedicada a lo único que importa esos segundos. Verificado
renderizando los tres estados con el marcado idéntico al de producción —la
primera captura mentía porque al banco le faltaba el `id` del `%` y no aplicaba
la regla que lo esconde—.

Regla que queda, hermana de la del veredicto ✓/✕: **una cuenta atrás que no
dice qué va a hacer deja al usuario adivinando igual que una que acaba en
silencio.** Y la lección de proceso: escribir la documentación de cara al
usuario ("qué vas a ver") encuentra huecos que revisar el código no encuentra,
porque obliga a mirar la pantalla y no la lógica.

**La mudanza anunciada (2026-08-12, tarde).** CLAUDE.md tocó su tope por
tercera vez en el día al apuntar el pendiente de la ficha proporcional, y se
ejecutó lo que la nota de ayer dejaba dicho: el bloque de REMEDIACIÓN (7,3k)
se mudó ÍNTEGRO a `remediacion.md` §"REGLAS VIGENTES — resumen operativo", y
en CLAUDE.md queda un puntero de ~15 líneas con solo lo transversal (crate
aparte, lista blanca, la red del /export, las dos razones del auto-/clear, la
cuenta atrás y el invariante del workflow). De 40.248 a 33.942: seis mil
bytes de margen para dejar de pellizcar palabras en cada regla nueva.
Verificado byte a byte que el bloque llegó entero antes de borrarlo del
origen.

## 2026-08-12 (tarde) — auditoría pre-público: el repo está listo, quedan dos decisiones

Oscar puso en palabras el freno real del lanzamiento: el miedo a que un
usuario descargue algo roto y no pueda actualizarlo. El antídoto es probar el
updater, y para eso el repo tiene que ser público — así que se auditó TODO lo
que se publicaría, historial incluido (lo que está en el historial se publica
con el repo y ya no se puede retirar después sin reescribirlo).

**Limpio, verificado:**
- gitleaks sobre los 397 commits: cero fugas. Barrido manual extra de
  patrones (ghp_, sk-ant, AKIA, llaves SSH, correos personales, IPs
  públicas) sobre TODO el historial: nada.
- Las notas de negocio del analizador: solo se referencia su RUTA externa
  (`~/.michiclaude/`), el contenido jamás entró al repo — el diseño funcionó.
- Archivos borrados en el historial: arte viejo y un .pyc; inofensivos.
- Workflow de release: los secretos van por `secrets.*` de GitHub, nada
  incrustado. Updater: pubkey (pública por diseño), endpoint y RELEASES_URL
  correctos y constantes.
- README: usuarios ficticios e IPs de documentación (TEST-NET). Un solo tag
  (`pre-rediseno-20260805`), sin ramas sueltas ni stashes.

**Las dos decisiones que solo puede tomar Oscar, ANTES de abrir:**
1. **Los correos de autor de los commits** (396 con una dirección personal,
   9 con otra) se publican con el repo. Opciones: aceptarlo (normal en open
   source) o reescribir el historial AHORA al correo noreply de GitHub —
   gratis mientras el repo es privado y sin colaboradores; imposible de
   deshacer limpiamente después.
2. **`docs/modelos-locales-cpu.md`** trae contexto de OTRO negocio (despliegue
   en equipos de clientes, pipeline de destilación). Ya está en el historial:
   quitarlo de verdad = la misma reescritura. Opciones: publicarlo (es
   investigación honesta y da credibilidad) o sacarlo en la misma pasada que
   el punto 1.

La bitácora misma se publica y se queda: es la transparencia que el producto
vende. Si Oscar decide reescribir (1 y/o 2), es una sola operación con
`git-filter-repo` + force push; después, repo público → tag pre-release →
probar el updater de punta a punta.

## 2026-08-12 (tarde) — limpieza pre-público: el historial se reescribe UNA vez

Decisión de Oscar sobre las dos preguntas de la auditoría: las dos cosas
fuera, y en general nada personal en el repo. Como lo que está en el
historial se publica con el repo, la única forma real es reescribirlo — y el
momento es AHORA, con el repo privado y sin colaboradores: gratis hoy,
imposible de hacer limpio mañana.

Lo que cambia (con respaldo `.bundle` completo en `~/.michiclaude` antes de
tocar nada):

1. **Correos de autor → noreply de GitHub** (los dos personales, 405 commits)
   y nombre normalizado a "Oscar". Los commits futuros nacen ya con el
   noreply (config del repo en las dos máquinas).
2. **`docs/modelos-locales-cpu.md` fuera de TODO el historial**: trae
   contexto de otro proyecto (despliegue en equipos de clientes,
   destilación). Se muda ÍNTEGRO a `~/.michiclaude/`, junto a las notas de
   negocio — mismo patrón: el conocimiento se usa, el contexto no se
   publica. Las referencias del código y los docs apuntan ahora "a la
   investigación de modelos (fuera del repo)"; las menciones narrativas de
   la bitácora se quedan (cuentan QUÉ se aprendió, no exponen el contexto).
3. **El username viejo de GitHub** (contenía el prefijo del correo personal)
   sustituido por el actual en los contenidos históricos de CLAUDE.md y en
   un mensaje de commit.

Nota para el futuro: el clon de Windows queda desincronizado por la
reescritura — `git fetch origin && git reset --hard origin/main` (y
`git fetch --tags --force`), NUNCA `git pull`, que intentaría fusionar las
dos historias.

**Ejecutado y verificado (misma tarde).** 408 → 407 commits (el que solo
añadía el doc se podó solo). Verificación completa post-reescritura: UNA sola
identidad de autor (el noreply con ID), el doc fuera de TODO el historial,
cero rastros de los correos/username viejo en contenidos y mensajes, gitleaks
limpio sobre la historia nueva, y el tag `pre-rediseno-20260805` reescrito y
re-empujado. El force push llegó al remoto (verificado con ls-remote: main =
hash nuevo). El token de GitHub salió del config del repo (filter-repo había
tirado el remoto original): ahora un credential.helper lo lee de
`~/.secrets/github-token` al momento del push, y los commits futuros nacen
con el noreply en este clon — falta la MISMA config en el clon de Windows.
Matiz honesto: GitHub conserva un tiempo los objetos viejos inalcanzables en
su servidor; como el repo jamás fue público y nadie más tiene los hashes, el
riesgo práctico es cero, y al hacerse público solo se clona lo alcanzable.

## 2026-08-12 (noche) — REPO PÚBLICO, y el release #1 muere por los iconos

Oscar lo hizo público. Primer tag `v0.1.0` → primera ejecución REAL del
workflow de release (escrito hace semanas, jamás corrido) → rojo a los 7m51s:
`icons/icon.ico not found`. La causa: `.gitignore` ignoraba `src-tauri/icons/`
entero — en el Windows de Oscar los iconos existen porque los generó una vez
con `npm run icons`, pero el runner parte de un clon limpio. El clásico
"funciona en mi máquina" en su forma más pura, y nunca se pudo ver antes
porque el workflow nunca había corrido.

Arreglo: los iconos generados van COMMITEADOS, como en la plantilla oficial
de Tauri (son artefactos deterministas de app-icon.png y el build los
necesita); fuera del repo quedan solo las variantes móviles (android/ios).
Regenerar: `npm run icons` si algún día cambia app-icon.png.

Lección: un workflow que nunca ha corrido es una promesa, no una pieza.
La primera ejecución ES parte de la validación — por eso el updater se
prueba completo ANTES de que exista un solo usuario.

**Release #2 verde… a medias (misma noche).** El instalador se publicó, pero
sin `.sig` ni `latest.json`: el endpoint del updater daba 404 — o sea, app
instalable pero incapaz de enterarse de versiones nuevas, que era EL punto de
toda la prueba. Causa: faltaba `"createUpdaterArtifacts": true` en el
`bundle` de tauri.conf.json — sin ella Tauri v2 no firma los artefactos y el
workflow no tiene con qué armar el latest.json (su includeUpdaterJson por
defecto no encuentra nada que incluir). Segunda pieza del updater que solo se
podía descubrir EJECUTANDO: el workflow nunca había corrido y la config nunca
había empaquetado un updater de verdad.

**Release #3: VERDE Y COMPLETO (2026-08-12, 18:45).** Tercera ejecución, la
buena: instalador + firma + latest.json publicados, y el endpoint del updater
responde el JSON firmado (verificado desde fuera con curl, el mismo camino
que recorrerá cada instalación). El primer release público de MichiClaude
existe: v0.1.0. Dos fallos quemados por el camino —iconos ignorados y
createUpdaterArtifacts ausente— que solo la ejecución real podía enseñar.
Queda el cierre del círculo: instalar el exe de Releases, publicar v0.1.1 y
ver a la app actualizarse sola.

## 2026-08-12 (noche) — EL UPDATER FUNCIONA: el círculo completo en una tarde

La tarde empezó con Oscar confesando el freno real del lanzamiento: "no
quiero que un usuario descargue algo roto y no pueda actualizarlo". Terminó
con la v0.1.0 instalada desde Releases detectando, descargando, verificando
la firma e instalándose la v0.1.1 sola, con la configuración intacta. El
miedo ya no tiene objeto: el canal de reparación existe y está probado.

Cuatro mordidas en el camino, ninguna evitable sin ejecutar:

1. **Release #1, rojo:** `icons/icon.ico not found` — los iconos estaban
   gitignorados; en la máquina de desarrollo existen, el runner parte de un
   clon limpio. Van commiteados, como en la plantilla oficial de Tauri.
2. **Release #2, verde a medias:** instalador sin `.sig` ni `latest.json` —
   faltaba `createUpdaterArtifacts: true` en el bundle. App instalable pero
   sorda a versiones nuevas, que era EL punto.
3. **Release #3 (v0.1.1), verde con versión vieja:** el tag se creó sin
   `git pull` previo y apuntó al commit anterior al bump. La red funcionó
   sola: latest.json anunciaba 0.1.0 y ninguna app se habría "actualizado"
   a lo mismo. Se borró release+tag y se re-etiquetó desde el VPS.
4. **La franja nunca llegó sola:** el check automático corría UNA vez, 8 s
   tras arrancar — y la app llevaba abierta desde antes del release. Para
   una app de bandeja que vive semanas sin reiniciar, eso es no enterarse
   nunca. Ahora: al arrancar Y cada 12 h, con guarda `v===updVer` para no
   re-anunciar lo ya anunciado (la REGLA ÚNICA de los globos: cerrado no
   vuelve).

Lección de la jornada, la misma cuatro veces: **cada pieza de la tubería
que nunca había corrido escondía exactamente un fallo, y ninguno era
visible leyendo el código.** El workflow, el empaquetado del updater, el
proceso humano de etiquetar y la cadencia del check — los cuatro se
estrenaron hoy y los cuatro mordieron una vez. Por eso se prueba con cero
usuarios.

Estado: MichiClaude es público, con dos releases reales y un canal de
actualización validado. Lo que queda para el LANZAMIENTO (cuando Oscar
quiera ser encontrado): capturas del README, el espejo de modelos en un
release, y la apuesta #2 (tarjeta compartible + gamificación) como pieza
de crecimiento.

## 2026-08-12 (tarde-2) — El espejo de modelos: el análisis local ya no depende de servidores ajenos

Idea de Oscar del 2026-08-11 ("¿y si Hugging Face quita la URL o el
modelo deja de existir? ¿no es mejor dejarlo en mi GitHub cuando sea
público?"), ejecutada el mismo día que el repo se abrió — era el único
bloqueador.

**Qué se hizo:** release `modelos-v1` en el propio repo con copias byte a
byte del GGUF (Qwen3.5-2B, 1.8 GB) y el zip de llama.cpp (b10362). En el
código, dos constantes nuevas (`AI_LS_URL_MIRROR` / `AI_MODEL_URL_MIRROR`)
y `ai_fetch()`: intenta la fuente original, y si falla la RED **o la
HUELLA**, cae al espejo. El fallo de huella importa tanto como el de red:
que Hugging Face responda 200 con OTRO archivo (lo reemplazaron) es
exactamente el escenario que el espejo cubre. La misma SHA-256 valida
ambas fuentes — la autoridad es la huella, no el servidor.

**Dos detalles que evitaron romper lo de la mañana:**

1. El release va como **PRERELEASE**: `releases/latest` (el endpoint del
   updater validado horas antes) ignora prereleases. Sin esa marca,
   `modelos-v1` habría tapado a la v0.1.1 y el updater se habría quedado
   ciego. Verificado tras subir: `latest.json` sigue anunciando 0.1.1.
2. El tag NO empieza con `v` → el workflow de release (`tags: v*`) no se
   dispara: no compila nada, no publica instaladores fantasma.

**Verificación en vivo, círculo completo:** bajados los originales al VPS
→ huellas idénticas a las constantes → subidos con `gh release upload` →
descargado el zip DE VUELTA del espejo sin ninguna autenticación → huella
idéntica otra vez. El camino que recorrería la app de un usuario nuevo con
Hugging Face caído está probado de punta a punta, salvo el salto mismo
(imposible de probar sin tumbar la fuente original; el código del salto
son 10 líneas de bucle sobre las mismas dos funciones ya validadas).

**Regla para el futuro:** cambio de build o de modelo = actualizar las
SEIS constantes juntas Y subir las copias a un release `modelos-v2` —
no se reutiliza el viejo, misma regla que el updater: un binario ya
publicado no se reemplaza.

**Cerrado el 2026-08-13:** `cargo check` limpio en el Windows de Oscar
(`Compiling michiclaude` presente — no fue el empate de mtime —, sin
warnings). Antes se auditó desde el VPS lo que no necesita compilador: las
seis constantes emparejadas, `ai_fetch` con un único llamador de
`ai_download` y tipos que cuadran, y ningún uso huérfano de la firma
vieja. Comprobadas además las cuatro URLs en vivo (espejo y originales,
200 las cuatro) y que `releases/latest` sigue anunciando la v0.1.1 con su
`.sig` — el prerelease `modelos-v1` no tapó al updater, que era el riesgo
real de haber subido un release el mismo día.

## 2026-08-13 — la ficha `compact` deja de avisar al 12%: el umbral se hace proporcional

El último bug conocido vivo, y era un déjà vu: la ficha `compact` del
coach (y, se descubrió al hacerlo, también el ⚠ "ctx" del recibo de
cierre en `coach_leaks()`) disparaba a los 120k FIJOS de `COACH_CTX_HIGH`.
Con un modelo de techo 1M eso es el 12% del contexto — la app gritaba
"¡compacta!" con la sesión recién empezada, y Oscar salta entre modelos a
diario. Exactamente el mismo bug que tuvo el manómetro clavado en 200k
durante meses (§2026-08-08).

**El arreglo:** `COACH_CTX_HIGH` (120k) muere y nace `COACH_CTX_PCT`
(60): el umbral es ahora el 60% de `ctx_full(model, ctx_seen)` — la misma
función, ya validada, que le da el techo al manómetro, con la evidencia
medida de la máquina incluida. Con modelo desconocido `ctx_for()` cae a
200k y el umbral queda en los 120k de siempre: el comportamiento viejo es
el caso degenerado del nuevo. Cuatro sitios, dos por lado (invariante #1):
la ficha y `coach_leaks` en `lib.rs`, y sus réplicas en `meter-export.py`.
Sin `coach_leaks` el recibo habría contado otra historia que la ficha
(fuga a 120k en una sesión que la ficha consideraba holgada con techo 1M).

**Por qué se pudo hacer sin esperar el cierre de la prueba en vivo:** el
pendiente decía "al cerrar la prueba" por prudencia, pero se verificó en
el código que el `/compact` AUTOMÁTICO va por otro camino —
`relayAutoCheck` dispara con `pressPct ≥ INTENT_PCT` (80% del techo, del
hit `press`) — y no lee la ficha ni `COACH_CTX_HIGH`. Cambiar la ficha no
mueve nada de lo que Oscar está midiendo.

**Verificado:** `py_compile` limpio; grep sin referencias huérfanas a la
constante vieja; `st`/`CoachSess` llevan `model` y `ctx_seen` en los
cuatro sitios (el hit `press` vecino ya los usaba). Docs en sincronía:
consejos-coach.md (dos menciones), CLAUDE.md (regla + pendiente cerrado).
Pendiente de Windows: `cargo check` (el VPS sigue sin toolchain).
Cerrado el mismo día: `cargo check` limpio en el Windows de Oscar
(`Checking michiclaude` en 8.68 s tras el pull de 1a3fb8a y el toque de
mtime — trabajo real, no el empate). El arreglo queda VERIFICADO en los
dos lados; el VPS recibirá el exportador nuevo cuando Oscar recompile y
arranque la app (viaja embebido).

## 2026-08-13 — la rampa invisible: compás adaptativo del coach y candado antes de la cuenta

**La prueba en vivo que lo destapó.** Primer intento del auto-/clear por
HECHO: sesión Haiku en el VPS llenada con lecturas masivas (lib.rs +
meter-export.py ≈ 197k tok). El manómetro nunca pasó de ~30% en pantalla
y Claude Code auto-compactó al ~94% ("Compacted chat · auto · 197k tokens
freed"). El automático de Michi jamás vio el 80%.

**Autopsia (dos causas, las dos de diseño):**
1. `coachPoll` corría FIJO cada 3 min y el manómetro reporta `last_ctx`
   (el contexto del ÚLTIMO turno, no el pico). La rampa 60k→197k cupo
   entera entre dos sondeos: el panel midió 30%, y al siguiente sondeo la
   compactación ya había puesto `last_ctx=0`. El pico existió pero pasó
   entre dos fotos. No es solo un caso de laboratorio: skills, subagentes
   o un prompt de "lee todo" hacen exactamente esa rampa en uso real.
2. Aunque se hubiera detectado: durante la rampa Claude está GENERANDO,
   el candado (R2) habría rechazado la inyección al final de la cuenta y
   `relayAutoCheck` sellaba el reintento a 10 min (`AUTO_RETRY_MIN`) —
   carrera perdida contra el auto-compact del ~94%.

**Arreglo (solo frontend, cero Rust, invariante #1 intacto):**
- Compás adaptativo `coachSched()`: el sondeo se auto-agenda según lo
  visto — 3 min sin sesión activa (el compás de siempre), 60 s con hit
  `press` vivo, 20 s con presión ≥55 (`COACH_WARM_PCT`), 10 s con ≥70
  (`COACH_HOT_PCT`) O salto de contexto ≥15k tok entre sondeos
  (`COACH_RAMP_TOK`, con prev>0: estrenar sesión no es rampa). El costo
  es acotado: `get_coach` es incremental por offset y el SSH de las
  remotas solo se paga mientras dura la banda alta, que se autolimita
  (o dispara o la sesión se calma). Las transiciones van al `flowLog`
  ("coach: compás 10 s (presión 72%, rampa)"). La cadencia de CUOTA no
  se tocó (3 min, regla del 429 — el coach no habla con la API).
- `relayAutoCheck` ahora exige `rly.ready` ANTES de `autoStamp`: ocupado
  es transitorio, no se sella nada y el siguiente sondeo (rápido bajo
  presión) reintenta gratis. El flujo en rampa queda: sondeo caliente ve
  ≥80% a mitad del turno gigante → espera en silencio → el turno termina
  → relevo `listo` → cuenta atrás → inyección con el candado en verde.
- Sellado del intento y resto de compuertas: SIN CAMBIOS (una vez por
  sesión, widget a la vista, desbloqueo manual, veredictos).

**Además quedó validado de la prueba fallida:** el fallback a `/compact`
con lista abierta funcionó de punta a punta EN REAL (tarjeta de intención
con "3 de 5 sin terminar", ⚠ en la opción /clear, auto-/compact aplicado
y registrado). Y la mordida conocida de siempre: la sesión de la prueba
quedó sellada como "done" — el próximo intento necesita sesión nueva.

**Verificado:** sintaxis del bloque `<script>` completo con `node --check`
limpio (el VPS no corre la app; la prueba funcional queda para el Windows
de Oscar tras el pull). Docs en sincronía: CLAUDE.md (regla del compás y
del candado en §Coach) y el comentario de `rlyPoll` sobre el compás.

## 2026-08-13 (2) — primera corrida REAL del camino por inferencia; la cuenta arranca con el veredicto

**Lo que pasó (prueba en vivo, sesión Haiku en el VPS):** el compás
adaptativo funcionó a la primera — "coach: compás 10 s (presión 80%)" en
el flowLog, tarjeta de intención y post-it casi inmediatos. Claude Code
SE SALTÓ el TodoWrite del guion (leyó y dijo "listo" a secas), así que no
hubo Boundary… y eso destapó el estreno involuntario del camino DIFÍCIL:
veredicto `unsure` → análisis local REAL (primera vez fuera del
simulador: "ai: veredicto clear · tema_nuevo", 13 s) → cuenta de 30 s
para /clear con red /export y la insignia punteada en la tarjeta. Oscar
tocó el gatito durante la cuenta (abrió el panel) y "cancelado (el
usuario)" — la ventana de cancelación TAMBIÉN quedó validada en real.
Pendiente de ver: la inyección completándose (✓ + copia en
`~/.michiclaude/handoff/` del VPS). El análisis local v1 ya tiene su
primer punto de la muestra: acierto razonable (sesión de lecturas sueltas
juzgada tema nuevo).

**Cronometría del hueco que molestó a Oscar:** aviso 18:21:08, veredicto
18:21:21 (13 s de modelo local), cuenta 18:21:33 (hasta 10 s extra
esperando el siguiente sondeo). Ese último tramo era evitable: ahora
`maybeAiIntent`, al guardar el veredicto, llama `relayAutoCheck(pr)` EN
EL ACTO — el pr del último sondeo (≤10 s bajo presión) sigue fresco y la
función re-verifica todas sus compuertas igual (autoRun, sellado, ready,
widget). El hueco restante es el costo del modelo (~13 s en la máquina de
Oscar) y no se puede comprimir sin cambiar de peldaño (embeddings, etapa
2). Por el camino del HECHO (Boundary, sin IA) aviso y cuenta ya nacían
en el mismo sondeo.

**Verificado:** `node --check` limpio del script completo. Nota de
mordida conocida: el intento cancelado sella `relayAuto[sid]` con
timestamp — la MISMA sesión reintenta sola pasados 10 min si vuelve a
estar activa (un mensajito la despierta); no hace falta sesión nueva.

## 2026-08-13 (3) — la carrera del primer sondeo caliente: /compact ganándole al veredicto

**Vista en vivo (18:32, segunda prueba del día):** en el MISMO segundo
nacieron la cuenta de /compact, la tarjeta de intención y el "ai:
analizando"; el veredicto "clear · tema_nuevo" llegó 10 s tarde a una
cuenta ya corriendo y la sesión quedó sellada con /compact — el /clear
por inferencia no se pudo ver. De paso quedaron validados el compás con
rampa ("compás 10 s (presión 85%, rampa)") y el segundo acierto seguido
del análisis local (2 de 2 tema_nuevo en sesiones de lecturas sueltas).

**Autopsia:** `relayAutoCheck(pr)` se llamaba ARRIBA en coachPoll, antes
de que el bucle de tarjetas guardara la de intención y antes de
`maybeAiIntent`. En el PRIMER sondeo que ve presión ≥80, `aiPending`
buscaba la tarjeta en el almacén, no la encontraba (aún no existía),
contestaba "no hay análisis en camino" y la cuenta de /compact arrancaba
sin esperar. En los sondeos siguientes la tarjeta ya existía y la espera
funcionaba — por eso la prueba de las 18:21 no lo destapó (ese primer
sondeo lo frenó la compuerta `ready`; el orden correcto se dio solo).
Con el compás de 3 min la carrera era casi imposible de ver; el compás
de 10 s la volvió reproducible al primer intento.

**Arreglo (tres piezas, solo frontend):**
1. `relayAutoCheck(pr)` se movió DESPUÉS de `saveCoachCards` y
   `maybeAiIntent` en coachPoll: cuando decide, la tarjeta existe y el
   análisis (si toca) ya está lanzado y sellado con `aiTried`.
2. `aiPending` ahora exige `aiTried`: sin lanzamiento real (IA apagada,
   exportador viejo sin `msgs`, veredicto no-unsure) no hay espera — se
   decide con lo determinista al momento, como siempre. Evita el plantón
   de 10 min (`AI_WAIT_MIN`) esperando un veredicto que jamás saldrá.
3. Simétrico en el `catch` del análisis: si el modelo falla, se llama
   `relayAutoCheck(pr)` en el acto (antes solo pasaba en el éxito).

**Verificado:** `node --check` limpio. La sesión 4043897 quedó sellada
("done" por el /compact aplicado): la próxima prueba del /clear necesita
sesión nueva.

## 2026-08-13 (4) — tercera prueba: la tubería entera bien, y un límite nuevo: el chat no tiene /export

**Lo validado (registro 18:45):** la carrera del sondeo caliente quedó
arreglada (tarjeta + "ai: analizando" SIN cuenta atrás), la cuenta
arrancó EN EL MISMO SEGUNDO que el veredicto (18:45:25, el ajuste de
maybeAiIntent), tercer acierto seguido del análisis local (3 de 3
tema_nuevo) y el fail-closed de la red actuó tal como promete.

**El límite descubierto:** el relevo tecleó
`/export <ruta handoff>` y el chat de VS Code contestó "/export isn't
available in this environment". El comando /export existe en la TUI de
la terminal pero NO en el entorno del chat de la extensión — la copia no
puede nacer, la verificación no la encuentra y el /clear se niega
(ERR_RELAY_EXPORT, no se borró nada: el fail-closed es la estrella de la
prueba). La validación anterior del /clear con red fue por terminal; en
CHAT el auto-/clear hoy NO puede completarse.

**Camino de fondo (diseño pendiente, zona de reglas duras del relevo):**
en modo chat el relevo conoce el `session_id` exacto — puede hacer la
copia ÉL MISMO desde el jsonl de la sesión (~/.claude/projects/…) en vez
de teclear /export: mismo destino handoff/, misma verificación en disco,
sin depender de un comando que el entorno no tiene. Tocaría
michi-relevo.py y michi.exe EN SINCRONÍA (mismas constantes/esquema) —
leer docs/remediacion.md antes; no se hace en caliente.

**Mientras tanto:** la prueba del final feliz del auto-/clear va por
TERMINAL (ahí /export sí existe). Ojo con el casado: una sesión de
terminal casa por cwd y es fail-closed ante ambigüedad — si en el mismo
cwd vive otro relevo (p. ej. el chat de trabajo en el proyecto), no casa
nunca. Truco: lanzar `claude` desde una carpeta neutra (~) y leer con
rutas ABSOLUTAS.

## 2026-08-13 (5) — FINAL FELIZ: el auto-/clear por inferencia, completo en vivo

**Registro (18:56-18:57, sesión de terminal en el VPS, cwd ~ para el
casado sin ambigüedad):** tarjeta + "ai: analizando" + "compás 10 s
(presión 88%, rampa)" en el mismo tick; veredicto "clear · tema_nuevo" a
los 22 s y cuenta atrás EN EL ACTO; a los 34 s "relevo auto: aplicado
/clear por IA (tema_nuevo)". El relevo tecleó `/export <handoff>` en la
TUI (ahí el comando SÍ existe), verificó la copia y aplicó el /clear.
Primera vez que el camino entero — rampa → compás caliente → análisis
local → inferencia → red /export → /clear — corre de punta a punta sin
intervención. El ✓ cerró la cuenta en la cápsula y el registro de
acciones guarda el "auto · aplicó /clear".

**Detalles que la corrida validó de propina:** la cuenta SOBREVIVIÓ al
cambio de sesión reina a mitad de camino (18:56:22, un sondeo vio la
sesión del chat al 20% — autoRun es independiente del sondeo, como debe);
el análisis local queda 4 de 4 (tema_nuevo, correcto) en sesiones de
prueba; y el contraste chat/terminal quedó medido el mismo día con la
misma tubería: chat = ERR_RELAY_EXPORT (fail-closed, /export no existe en
la extensión), terminal = aplicado. El comportamiento divergente es del
ENTORNO, no del relevo.

**Cerrado en CLAUDE.md:** auto-/compact y auto-/clear por inferencia
pasan de "en prueba" a COMPLETOS en vivo; el pendiente nuevo es el diseño
de la copia propia del relevo en modo chat (jsonl + sid, zona de reglas
duras) y la muestra del análisis local en uso natural.

## 2026-08-13 (6) — el auto-/clear llega al chat: la copia sin /export

**Qué se construyó:** la red del /clear en modo CHAT ya no depende de
`/export` (que la extensión no tiene, medido en vivo el mismo día): el
relevo hace la copia ÉL MISMO. `session_jsonl(sid)` localiza el JSONL de
la sesión por NOMBRE (`projects/*/<sid>.jsonl` — el sid es UUID único,
sin reproducir la transformación de carpetas; respeta CLAUDE_CONFIG_DIR),
copia con tmp+rename a `handoff/…jsonl` y verifica el hecho del disco.
R4 tras la copia (busy/hijo muerto → el /clear pierde, la copia queda).
Lista blanca, generación de ruta y fail-closed: SIN CAMBIOS. Terminal:
SIN CAMBIOS (/export ahí funciona y está validado). STATE_V sigue en 2.
Detalle en remediacion.md §"La copia SIN /export en el chat"; réplica
exacta wrap_handoff (.py) ↔ rama chat de handoff() (main.rs, por sp.sid).

**Banco en el VPS (claude falso SIN /export, como el real):** 3/3 —
(1) /clear con red: acuse ✓ con ruta, copia IDÉNTICA byte a byte (diff),
banner + eco del /clear pintados en el chat, cero /export tecleado;
(2) regresión: /compact a secas sin copia nueva; (3) fail-closed: sid
sin jsonl → ERR_RELAY_EXPORT y CERO /clear. py_compile limpio.

**Pendiente:** `cargo check` del crate en el Windows de Oscar (el VPS no
compila Rust; `npm run dev` compila el relevo solo con su
beforeDevCommand) y la prueba en vivo con el chat real — el .py viaja
embebido: recompilar en Windows y arrancar la app lo re-sube al VPS, y
la sesión de chat tiene que ser NUEVA (el wrap viejo en marcha no se
entera).

**Mordida del día (ajena al código):** al apagar el banco se mató por
patrón `michi-relevo.py wrap` — que también casaba con los relevos
REALES de las pestañas de chat del VPS: una pestaña de Oscar murió con
SIGTERM (sin pérdida: el jsonl queda y la pestaña se reabre). Regla para
el futuro: en máquinas con relevos vivos, matar por PID exacto, jamás
por patrón.

## 2026-08-13 (7) — el auto-/clear del CHAT validado en vivo, y la sesión que arde gana el trono

**FINAL FELIZ EN EL CHAT (19:43-19:44):** primera corrida real de la
copia sin /export — tarjeta a las 19:43:11 (presión 89%), veredicto
tema_nuevo a los 26 s, cuenta de 30 s, y "aplicado /clear por IA" a las
19:44:10 con la pestaña del chat renacida y la copia .jsonl en
handoff/. La tabla queda 4 de 4: /compact y /clear automáticos, en
terminal y en chat. El análisis local: 5 de 5 aciertos.

**El culpable de los "minutos muertos" que preguntó Oscar:** la sesión
REINA se elegía SOLO por frescura. Registro en vivo: 19:39:53 la pesada
al 77% (compás 10 s) → 19:41:06 la reina era OTRA sesión al 29% (un
mensaje en el chat de trabajo la volvió más fresca) y el compás se abrió
a 60 s → 19:43:11 la pesada (ya al 89%) recuperó el trono. Dos minutos
de sombra que en uso real (varios chats vivos) serían constantes.
Arreglo: `arde()` — una sesión ≥ INTENT_PCT gana el trono SIEMPRE; la
frescura solo desempata (entre dos que arden o dos que no). Con eso el
compás caliente se queda con la sesión peligrosa y el automático no
pierde de vista al incendio por un mensaje en otra ventana.

**Lo que queda de latencia y su porqué (medido en esta corrida):**
detección ≤10-20 s (compás caliente), veredicto 13-26 s (llama en CPU —
LA cifra que los embeddings de la etapa 2 bajarían a ms), cuenta 30 s
(DELIBERADA: es la ventana de cancelación, no se recorta). Piso real
actual ≈ 1 min desde que la sesión cruza el 80% estando quieta.

## 2026-08-13 (8) — visor de copias handoff y etapa 2 del análisis local (embeddings)

**Visor de copias (pedido de Oscar: "¿hay manera de verlo si lo
requiero?"):** el "abrir la copia" del registro de acciones solo servía
para copias LOCALES (Explorador) y las de Oscar viven en el VPS. Ahora el
botón dice "ver la copia" y abre un overlay del panel (patrón .pop
reusado) con el CONTENIDO: .jsonl como transcript legible, .md tal cual.
Piezas: RemAction.origin (aditivo), relay_inject_remote devolviendo la
ruta del acuse (se tiraba), read_handoff(name, origin) con nombre
validado a [A-Za-z0-9._-] antes de componer nada (en remoto viaja en un
comando ssh), tope 4 MB, i18n en los 8 idiomas. "Abrir en la carpeta"
queda solo para locales.

**Etapa 2 del análisis local — embeddings (pedido del mismo día):**
la escalera queda completa (determinista → embeddings → 2B → nada).
`ai_emb_verdict()` corre DENTRO de ai_intent_impl antes de arrancar el
2B: mismo llama-server con `--embeddings --pooling mean -c 512` (puerto
+1, guard kill-on-drop), prefijo `query: ` en ambos lados (e5 lo exige),
coseno TEMA (título+viejos) ↔ RECIENTE (último msg); <0.45 →
clear·tema_nuevo, >0.65 → compact·tema_cruzado, banda media → el 2B.
Fail-quiet en cadena: sin GGUF o con cualquier fallo, v1 exacta. via/sim
aditivos en AiVerdict → flowLog ("(embeddings 0.38)" / "(modelo)") y
ai_debug.txt con tema/reciente/sim (vectores no). Modelo:
multilingual-e5-small-q8_0 (~126 MB, cstr/multilingual-e5-small-GGUF),
SUBIDO al release-estante modelos-v1 como asset NUEVO (aditivo — la
regla del modelos-v2 es para reemplazos) y verificado con descarga
anónima + huella idéntica (0a34067a…53e8). Constantes de la descarga:
NUEVE. ai_setup baja solo lo que falte → con la v1 instalada el botón
ofrece "Descargar el modelo rápido (~126 MB)".

**Verificado:** node --check limpio (visor + i18n + escalera JS);
espejo round-trip con huella idéntica. PENDIENTE Windows: cargo check
(visor Rust + embeddings Rust comparten commit con el relevo del chat),
descargar el e5 con el botón nuevo y ver el primer `via:emb` en vivo.
Los umbrales EMB_NEW/EMB_CROSS no se afinan hasta tener muestra natural.

## 2026-08-13 (9) — la etapa 2 estrenada en vivo (decidió el 2B) y la similitud que viajaba a ciegas

**Cuarta corrida del auto-/clear del día, primera con la escalera
completa (20:25-20:27):** todo el camino otra vez perfecto — 98% de
presión, veredicto a los 31 s, /clear con red aplicado, y el VISOR
estrenado con la copia remota (transcript legible del jsonl traído por
SSH, botón "ver la copia" en el registro). PERO el veredicto vino
"(modelo)": el peldaño de embeddings NO decidió, y con el diseño de esa
mañana era imposible saber por qué — el ai_debug.txt del 2B PISA el
rastro del emb (se sobrescribe, por diseño de esa familia), así que
"banda media legítima" y "el peldaño falló en silencio" se veían
idénticos.

**Arreglo (mismo día):** ai_emb_sim (la medida) se separó de
ai_emb_verdict (la decisión), y la similitud viaja AHORA con el veredicto
del 2B: `sim` va en AiVerdict también cuando decide el llm. El flowLog
distingue: "(modelo · sim 0.52)" = midió banda media y el 2B decidió;
"(modelo)" a secas = el emb no pudo medir (GGUF ausente o fallo
silencioso). Esa diferencia ES el diagnóstico de la etapa 2 en campo.

**Pendiente de la próxima corrida:** ver si sale "sim" en el flowLog. Si
sale a secas "(modelo)", el peldaño está fallando en el Windows de Oscar
(sospechosos: flag --embeddings del build de llama.cpp, o el GGUF de e5
con su versión) — ai_debug.txt tras un "Probar" lo dirá, porque el
Probar con la evidencia de ejemplo debería dar sim baja y decidir por
embeddings en segundos.

## 2026-08-13 (10) — el e5 estaba roto: autopsia con banco propio y cambio a EmbeddingGemma

**El diagnóstico llegó por los micrófonos nuevos:** el emb_server.log de
Oscar dijo la causa exacta — "bert model needs to define token type
count": la conversión GGUF de cstr es vieja y no trae un metadato que el
llama.cpp moderno exige. El peldaño moría al arrancar, siempre.

**Banco de embeddings EN EL VPS (primera vez):** se bajó el build Linux
de llama.cpp b10362 (el MISMO pineado para Windows) + libgomp extraída
de un .deb sin sudo — y con eso las pruebas que antes exigían
ida-y-vuelta con el Windows de Oscar se hicieron aquí en minutos:
- cstr e5: reproduce el fallo exacto de Oscar. mili e5: core dump.
  keisuke e5: carga… pero el tokenizer está dañado — "receta de
  carbonara"↔"CSS del widget" da 0.93, MÁS que una subtarea del mismo
  proyecto (0.90). Matriz completa pooling{mean,cls,last}×prefijo{query,
  sin}: TODAS solapadas. Sin separación no hay umbral: e5-small en GGUF
  comunitario está muerto como opción.
- EmbeddingGemma-300M (GGUF OFICIAL ggml-org, 500k descargas): separa
  limpio SIN prefijos — tema nuevo 0.15-0.36, continuación 0.53, mismo
  tema entre idiomas 0.84 — y CALZA con los umbrales 0.45/0.65 del
  diseño (el prefijo STS de su ficha comprime hacia la banda media: se
  descartó con medida, no con opinión). Validación final con los flags
  EXACTOS del Rust: probar=0.358→clear·emb, carbonara=0.155,
  idiomas=0.844→compact·emb; 3 pares en 0.3 s ya cargado.

**Cambios:** constantes AI_EMB_* → gemma (HF oficial + espejo modelos-v1
verificado con descarga anónima y huella b5ce9d77…0d63); el e5 roto
RETIRADO del estante (jamás lo referenció un release de la app — no es
"reemplazar un binario publicado"); ai_emb_path() ignora la ruta del e5
aunque siga en la config (sin esto, quien lo descargó hoy quedaba
bloqueado para siempre) y ai_setup borra el archivo huérfano y pisa la
ruta muerta; flags: -c 1024, sin --pooling (el GGUF oficial trae el
suyo), sin prefijos; tamaños de la UI 126→319 MB y total 1.4→1.7 GB ×8
idiomas.

**Verificado:** carga + salud + separación con los flags exactos en el
banco del VPS; espejo round-trip; node --check limpio. Pendiente Windows:
cargo check (npm run dev) + Probar — esperado "✓ clear · tema nuevo ·
embeddings 0.36" en segundos.

## Cierre 2026-08-13 — la jornada de los automáticos

Día récord: 14 commits, y el proyecto cruzó su meta fundacional — Michi
aplicando /compact y /clear SOLO, de punta a punta, en terminal Y en
chat (tabla 4/4, validada en vivo por Oscar en cuatro corridas).

**Lo construido hoy, en orden:** compás adaptativo del coach con cazador
de rampas (3 min → 10 s bajo presión) · cuenta pegada al veredicto ·
compuerta `ready` antes de la cuenta · arreglo de la carrera del primer
sondeo caliente · la sesión que arde gana el trono de la reina · la
copia SIN /export para el chat (el límite del /export de la extensión,
descubierto, diseñado, implementado y validado el mismo día) · visor de
copias handoff (local/SSH/WSL, transcript legible) · etapa 2 del
análisis local con EmbeddingGemma (banco de llama.cpp en el VPS, autopsia
de los e5 rotos, espejo verificado, Probar en Windows clavando el número
del banco: 0.36).

**Estado al cierre:** todo pusheado (`ba06442`), cargo check implícito
pasado (la app compiló y corrió todo en el Windows de Oscar). El sistema
queda EN VALIDACIÓN PASIVA: Oscar lo usa normal y reporta cualquier
rareza de clear/compact — el rastro para revisarlas es flowLog +
emb_debug.txt + registro de acciones con su "ver la copia". Los
pendientes vivos quedan en CLAUDE.md §Estado: primer via:emb en sesión
real, muestra natural para los umbrales, validación pasiva de alarmas/
ntfy/hallazgos, y el ruteo inteligente sigue BLOQUEADO hasta confirmar
estas pruebas del día a día.

---

## 2026-08-14 — Etapa 0 del ruteo: el hook SÍ impone el modelo del subagente (A/B en el VPS)

Primera pieza del ruteo inteligente, y la única que el plan permitía
tocar con las pruebas del día a día abiertas: es un experimento aparte,
no comparte código con el coach ni con el gatito.

**La pregunta:** ¿un `PreToolUse` puede reescribir el modelo con el que
NACE un subagente, devolviendo `hookSpecificOutput.updatedInput`? De
ella cuelga el Hook B entero (el ahorrador silencioso).

**Cómo se probó** (`scripts/ruteo-etapa0/`, commit del experimento):
hook de juguete que solo actúa con la marca `RUTEO-TEST` y falla
callado; sesión headless de Claude Code 2.1.231 con el hook en settings
de proyecto, padre en Sonnet, subagente `general-purpose`; el veredicto
NO se le pregunta al subagente (los modelos se equivocan sobre sí
mismos) sino al `agent-*.jsonl` que escribe Claude Code.

**Resultado — A/B con 27 s de diferencia, todo lo demás igual:**

| Corrida | Modelo real en el transcript |
|---|---|
| Con marca (hook actúa) | `claude-haiku-4-5-20251001` |
| Control sin marca (hook calla) | `claude-sonnet-5` (hereda del padre) |

ÉXITO. La apuesta técnica del Hook B se sostiene y no hizo falta el
plan B (frontmatter `model:` / `CLAUDE_CODE_SUBAGENT_MODEL`).

**Lo que el log enseñó y el diseño no sabía:**

1. **El nombre de la herramienta no es estable**: en este build llega
   como `Agent`, no `Task`. El matcher `Task|Agent` la agarró por los
   pelos — el matcher doble es OBLIGATORIO, no adorno. Si un día no
   dispara, sospechar del nombre ANTES que del script.
2. **El input NO trae `model`**: llegó `antes=(no venía)` y el hook lo
   AÑADIÓ. `updatedInput` no solo reescribe campos, también agrega los
   que no existen. Y el input traía `run_in_background`, que es
   justamente por qué hay que devolver el objeto COMPLETO (§10.1).
3. **La variante A basta**: `updatedInput` a secas, sin
   `permissionDecision: allow`. La variante B queda documentada por si
   una versión futura la exige.
4. **Contexto gratis para el Hook B real**: el payload del hook trae
   `cwd`, `session_id`, `transcript_path`, `permission_mode` y
   `effort:{level}`. El `cwd` da el proyecto sin adivinar — el
   `modo_proyecto` de `router_state.json` se puede resolver ahí mismo.

**Lo que queda de esta etapa:** la corrida en Windows nativo (el
`hook-model-test.ps1` es traducción literal del Python y NO se pudo
ejecutar en el VPS — no hay PowerShell). Mecánicamente WSL y VPS son el
mismo caso (Linux, mismo script), así que la matriz de Oscar se cierra
con esa única corrida pendiente.

**Lo que NO cambia:** la compuerta sigue puesta. Etapa 1 en adelante
espera a cerrar las pruebas en vivo del auto-/clear y del análisis
local — comparten zona de código y contaminarían la medición.

### Apéndice del 2026-08-14 — tres trampas de Windows en la etapa 0

Anotadas mientras Oscar corría el experimento en su Windows (Claude
Code v2.1.232); ninguna es del mecanismo, las tres son del entorno:

1. **PowerShell 5.1 lee los `.ps1` sin BOM como ANSI.** Mis cuatro
   scripts iban con tildes y rayas largas en los comentarios: el `—` se
   convierte en `â€"` y ese `"` CIERRA la cadena a media línea →
   `MissingEndCurlyBrace` y el script no compila. Arreglado pasando los
   cuatro a ASCII puro. REGLA para el Hook B de verdad (irá embebido con
   `include_str!`, donde nadie avisa en compilación): grep de no-ASCII.
   El fallo, eso sí, se comportó como debía — `hook error ... non-blocking`
   y el subagente corrió igual.
2. **El menú `/hooks` es de SOLO LECTURA** en esta versión ("To add or
   modify hooks, edit settings.json directly"). El README lo daba como
   camino recomendado para INSTALAR; corregido: sirve para verificar.
   Instalar es `instalar-hook.ps1`, que fusiona con los hooks que ya
   haya en vez de pisarlos.
3. **El modelo puede estar CLAVADO** en `.claude\settings.json`
   ("pins Haiku 4.5 — that applies on restart"). `/model` cambia la
   sesión de ya, pero reiniciar la devuelve al clavado. Orden correcto:
   hook → reiniciar → `/model` → prueba. Y si la sesión ya está en
   Haiku, el experimento no demuestra nada: el subagente nacería en
   Haiku con hook o sin él.

Cuarto dato, este a favor: en Windows la herramienta TAMBIÉN llega como
`Agent` (lo dijo el error: `PreToolUse:Agent hook error`). Dos builds
distintos, mismo nombre no-documentado — el matcher `Task|Agent` se
queda.

### Cierre de la etapa 0 (2026-08-14, tarde) — validada también en Windows nativo

El experimento corrió en el Windows de Oscar (Claude Code v2.1.232,
sesión en Sonnet 5) y el A/B salió solo, gracias al fallo de
codificación de la primera corrida:

| Hora | Estado del hook | Modelo real del subagente |
|---|---|---|
| 12:34:23 | roto (`hook error`, no bloqueante) | `claude-sonnet-5` (hereda del padre) |
| 12:39:54 | ya en ASCII, funcionando | `claude-haiku-4-5-20251001` |

Misma máquina, misma sesión, mismo `general-purpose`, 5 min de
diferencia: la única variable fue que el `.ps1` compilara. El error de
codificación regaló el grupo de control.

El log de Windows confirma los tres hechos del VPS, ahora en el otro
mundo: la herramienta llega como `tool_name: "Agent"`; el input NO trae
`model` (`antes=(no venia)`) y `updatedInput` lo AÑADE; y basta la
forma mínima, sin `permissionDecision`.

**ETAPA 0 CERRADA.** Los dos mundos donde corre Claude Code están
cubiertos: Linux (VPS por SSH; WSL es el mismo caso mecánico — mismo
`hook-model-test.py`, mismo `~/.claude`) y Windows nativo (PowerShell).
La apuesta técnica del Hook B se sostiene y el plan B queda de respaldo.

La compuerta NO se mueve: etapa 1 sigue esperando a que cierren las
pruebas en vivo del auto-/clear y del análisis local. Comparten zona de
código (coach/gatito) y arrancar ahora contaminaría esa medición.

## 2026-08-14 (2) — % de desperdicio estructural: fórmula, obra y dos arreglos del panel

Jornada en el VPS (chat de VS Code). Tres frentes, los tres cerrados aquí
y pendientes solo de `cargo check` + vistazo visual en Windows.

**1. La fórmula (fila 18 de presion-y-rendimiento.md).** Era el diseño
previo obligatorio y quedó escrito en su § propia. Lo esencial: sumar
todos los hallazgos y dividir está MAL por tres razones verificadas en el
código — los detectores se pisan (inflate contiene a reread vía
cache_read; mech cobra el turno entero sin excluir subagentes), los más
estructurales valen $0 (mcp/skills, resta de conjuntos), y el tope de 12
por costo decapita justo a los baratos. La salida: UNA LÍNEA DE FACTURA
POR DETECTOR (input: claudemd+hooks_noise; cache_write: cachebreak;
cache_read/turno entero: excluidos) → numerador disjunto por
construcción, sin restas a mano. Como deja fuera más de lo que arriesga,
el número es un PISO y el copy dice "al menos" — invariante #8 con
dirección segura. Fusión multi-origen: suma de numeradores ÷ suma de
denominadores, JAMÁS promedio de porcentajes.

**2. La obra (tres piezas, invariante #1).** `scan_findings` (Python) y
`scan_local_findings` (Rust) calculan `waste` ANTES del tope de 12:
{struct_cost, struct_tokens, total_cost, sessions, days, end, estimated,
items[]} con `items` = tarjetas estructurales sin recortar (tope 100)
para que el panel descuente las ignoradas con `fndKey`. `get_findings`
ahora devuelve `FindingsPack{findings, waste}` — los 3 usos del frontend
desempaquetan `.findings`. Tarjeta en Reporte bajo el héroe con los 3
estados degradados diseñados (ventana corta / juntando datos / nada que
señalar), comparación "antes: Y%" (segunda pasada con --end corrido) y
nota "no contamos" con MCP/skills. i18n `wst_*` ×8.

VALIDACIÓN en el VPS: regresión `--end` congelado 7d y 30d → findings y
campos viejos byte-idénticos; y `waste.total_cost` == `cost_week` de la
agregación normal AL CÉNTIMO en ambas ventanas — dos caminos
independientes que cuadran exactos. Dato real del VPS: 11.2% de
desperdicio en 30d ($230 de $2,057), TODO cachebreak — la fuga más cara
del catálogo también manda aquí. La maqueta de la otra IA (prompt del
doc) sirvió de referencia con 4 correcciones anotadas en el chat: su
tarjeta de "subagentes sin rastro" era falsa, el "trabajo real 86%"
afirmaba lo no demostrado, el cachebreak no lleva "~" (es MEDIDO) y
mezclaba ventanas de 7 y 30 días.

**3. Panel, dos peticiones de Oscar del día:** (a) adiós a la rendija
transparente — era el padding de 1px del body que el anillo del borde
necesitaba; ahora el borde es `outline` con offset -1px (hacia adentro,
por encima del contenido: un inset lo tapaba el sticky) y padding 0;
(b) el panel ya NO se cierra al perder el foco — era flyout y estorbaba
al consultarlo trabajando; solo cierran el ✕ y el menú del tray. CLAUDE.md
actualizado en ambos.

## 2026-08-14 (3) — Detector 11: frecuencia de auto-compacts (kind acompact)

La mitad que faltaba de la fila 11: la regla `acomp` del coach avisa del
EVENTO; ahora Hallazgos mide el HÁBITO. Tarjeta por PROYECTO con ≥3
auto-compacts en la ventana (por sesión sería confeti), costo PISO
obligatorio: la compactación NO trae usage, lo único medible es
`preTokens` (mismo campo que la regla acomp) cobrado UNA vez al input del
modelo dominante, con "~". Solo `trigger != manual` — las del relevo
entran como manual y quedan fuera solas. Dedup por uuid (reanudaciones).
NO entra al numerador del % de desperdicio. Tres piezas en sincronía;
marcas de arreglo lo incluyen.

Validación con moraleja: la regresión congelada (7d/30d) dio el resto de
tarjetas y el waste byte-idénticos, pero NO salió la tarjeta acompact en
30d pese a haber 3 autos reales en los logs — la instrumentación línea a
línea enseñó que los 3 se CUENTAN bien y caen 2+1 en dos proyectos
distintos (las sesiones de julio llevan el disp viejo claude-code-meter):
bajo umbral, silencio honesto — el detector funcionando exactamente como
se diseñó. El cuadre exacto quedó en un fixture sintético: 3 autos =
175k pre = $0.175 a precio haiku, con el uuid duplicado deduplicado, la
manual y la fuera-de-ventana excluidas, y callando con solo 2. De paso se
explicó el `cost_today` "inestable" de la regresión: es relativo a AHORA
(no al --end) y le pasa igual al exportador viejo — ruido del reloj, no
de los cambios.

## 2026-08-14 (4) — Detector 12: pegado masivo — y el bug de uturns que cazó el diseño

El pendiente decía "diseñarlo y validarlo antes de prometerlo", y el
diseño pagó: la exploración de los 1,025 mensajes humanos reales del VPS
(mediana tecleada 290 chars, p90 1.7k) enseñó que los 10 "mensajes" más
grandes NO eran pegotes — eran los resúmenes de continuación de la
compactación ("This session is being continued…"), que viajan con rol
user y PASABAN el filtro user_turn_text. Doble consecuencia: el detector
habría acusado pegotes del sistema, y —el bug de regalo— uturns llevaba
contándolos como turnos útiles desde la fase 1, diluyendo el rendimiento.

Arreglo en la raíz: isCompactSummary fuera de user_turn_text (AMBOS
lados) + caché de escaneo v2→v3 (el patrón documentado: un caché viejo
devolvería los uturns de antes en silencio). Delta verificado EXACTO:
842→824 uturns en 30d = los 18 resúmenes únicos de la ventana, ni uno
más.

El detector: kind `paste`, umbral POR MENSAJE 5k chars (~17× la mediana
real), tarjeta por PROYECTO con ≥3 pegotes y ≥10k tokens, costo PISO
chars/4 × input dominante ("~"), dedup uuid, fuera del waste
(conductual), fix que no regaña (un error de consola no tiene ruta que
mencionar). Réplica Rust con chars().count() — bytes divergiría con
tildes. Fixture con cuadre exacto (50k chars = 12.5k tok = $0.0125
haiku; resumen/meta/corto/fuera-de-ventana excluidos; el volumen calla 3
pegotes chicos). En los datos reales del VPS la tarjeta VIVE (7d: 6
pegotes, 11.4k tok en michiclaude) pero cae al puesto 16 — el tope de 12
la deja fuera porque aquí dominan los inflates; en la máquina de un
usuario típico saldría. cargo check pendiente en Windows.

## 2026-08-15 — Las 4 piezas de integridad: que un borrado no se disfrace de mejora

Oscar trajo un ADR externo (multi-harness + persistencia con SQLite).
Veredicto y análisis completo en `docs/adr-multiharness-y-persistencia.md`:
la Parte 1 se rechaza (choca con el NO vigente y con el foso
Claude-específico; la capa "medidor" está saturada y gratis), la Parte 2
diagnostica un riesgo REAL pero con una solución sobredimensionada. Oscar
aprobó la versión ligera: 4 piezas, cero SQLite.

**El riesgo, en una frase:** los `.jsonl` no son nuestros. Si un limpiador
tipo conversation-reclaim los recorta, MichiClaude leería menos y cantaría
"mejoraste" — la mentira exacta que prohíbe el invariante #8. El fixture lo
enseña sin piedad: 504,000 → 30,200 tokens, una "mejora" del 94% que era un
borrado.

**Lo construido.** (1) Detector pasivo montado sobre el caché de escaneo,
que YA guardaba tamaño+mtime: archivo que encogió o desapareció →
`integrity.json` (local, no viaja), con réplica en el exportador y el
origen puesto por Rust. (2) Comparaciones NO CONCLUYENTES en el Reporte
(héroe, volumen, contradicción, desperdicio) cuando el tramo está tocado.
(3) `daily_history.json`, la serie diaria fusionada de 400 días — RESPALDO,
no jefe. (4) Las marcas de arreglo congelan su "antes" al nacer, sacado del
cuadernito.

**Las dos decisiones de diseño que más costaron pensar.** La primera: el
cuadernito NO manda. Ayer mismo el fix de `uturns` corrigió 30 días hacia
atrás porque los logs crudos seguían ahí; con rollups congelados al mando
—como pedía el ADR— ese bug habría quedado fosilizado en la historia. Un
store protege contra borrados Y congela errores: por eso lo vivo manda
siempre y el cuadernito solo rellena lo que ya no se puede ver. La segunda:
un recorte se DETECTA en una fecha, pero sus bytes pueden ser de cualquier
día. No se puede atribuir a un periodo, así que cualquier hecho desde el
arranque del periodo más viejo ensucia la comparación entera — fingir
precisión ahí habría sido peor que el hueco.

**Falsos positivos, cazados antes de nacer.** Dos guardas, ambas probadas:
solo se juzgan las raíces que se pudieron LEER (con WSL apagado sus
archivos "faltan" sin haberse borrado — habría sido una alarma falsa
diaria en el Windows de Oscar) y solo cuenta si el archivo de verdad no
existe (envejecer fuera de la ventana ≠ borrarse). Y el archivador propio
no puede dispararla: mueve archivos ≥365d y el caché solo guarda los de
~32 días. Cero solape.

**Validación.** Fixture de extremo a extremo con cuadre AL BYTE
(56,379−5,659=50,720); silencio en la primera corrida sin caché, sin
cambios y tras avisar (no repite); regresión con logs reales 7d/30d
byte-idéntica en findings, waste, totales y serie diaria; 9 casos de la
lógica de cobertura en node, incluidos los negativos; forma del JSON
verificada contra el struct de Rust; i18n ×8. Pendiente: `cargo check` en
Windows y WSL, que queda verificado POR CONSTRUCCIÓN (misma función, otra
raíz) pero no ejecutado.

**Aviso de mantenimiento:** CLAUDE.md quedó en ~39.7k de los 40k. La
próxima entrada que se le añada debería venir con una poda: lo que ya está
en los docs de diseño no necesita repetirse ahí.

## 2026-08-15 (2) — Purga del archivo: el ciclo de vida completo, y WSL entra al archivador

Nació de la pregunta de Oscar sobre el caso viral de los 60 GB de logs.
Es creíble (tool results enteros en el log, enjambres de agentes, y las
reanudaciones que COPIAN el archivo entero — la razón de nuestra dedup
por uuid), y destapó una verdad incómoda: con `cleanupPeriodDays: 365`
hacemos crecer el disco 12× más que la fábrica, y el archivador de la
etapa 2 solo MOVÍA. El disco nunca bajaba. Faltaba el último escalón.

Diseño completo en remediacion.md §"Purga del archivo": ciclo VIVO →
ARCHIVADO → PURGADO; siete reglas de seguridad en Rust que el panel no
puede saltarse (suelo 180 d, doble reloj con sidecar `.arch`, allowlist
canónica, simulacro, palabra de confirmación proporcional, tope por
pasada, solo .jsonl); nace en "nunca" y el automático es opt-in con el
candado de primera manual. Decisiones de Oscar: purga apagada de
nacimiento (rotundo), el usuario elige el plazo con advertencia, y el
VPS SOLO INFORMA (`--du` + un `find -mtime +365` acotado) — desde la app
nunca se borra por SSH.

Hallazgo de paso: WSL NUNCA se archivaba (`archivable_files` solo miraba
`~/.claude` local). Arreglado: `archive_roots()` cubre las distros, cada
una a su subcarpeta.

Validación sin toolchain: réplica línea a línea del algoritmo en Python,
18/18 — incluida la trampa de un symlink dentro del archivo apuntando a
un log VIVO (no entra; el vivo queda intacto). `--du` contra logs reales
y fixture. `cargo check` pendiente en Windows.

Nota de mantenimiento: CLAUDE.md pasó por su primera PODA (el bloque de
validación pasiva narraba historia que ya vive en remediacion.md y la
bitácora); quedó en 39.7k. La regla desde hoy: cada entrada nueva ahí
viene con una poda equivalente.

## 2026-08-15 (noche) — falso positivo del coach: capturas ≠ relecturas

Oscar monitoreó con MichiClaude una sesión de otro proyecto (sparky-site,
VPS) y llegó la ficha "Menciona el archivo, no lo pegues" con "un mismo
archivo se leyó 19 veces". Él no había pegado nada dos veces. Autopsia
en `coach_state.json`: `reads` tenía `scratchpad/revision.png` ×19 —
Claude iteraba el diseño mirando la captura tras cada retoque. Dos
errores en uno: la regla `attach` mezclaba "releer el mismo texto" con
"mirar una captura regenerada", y la ficha hablaba sin sujeto ("se
leyó"), así que el usuario se creía el culpable.

Arreglo en las tres piezas (invariante #1): `Read` sobre imagen
(`IMG_EXT`) → `shots`, aparte de `reads`; regla nueva `shots` (≥10, ficha
"Muchas capturas en una sesión" ×8 idiomas, ⚠ en el recibo); hits
`attach`/`shots` llevan `file` (nombre sin ruta, aditivo) y la línea
"Ahora:" dice "Claude leyó revision.png 19 veces". `trail` (continuidad)
sigue contando las imágenes: son archivos con los que se trabaja. El
`reread` de Hallazgos no cambia (mide chars; una imagen da 0). La ficha
`cache` de esa misma sesión (6 min de pausa con 235k) era legítima.
`cargo check` pendiente en Windows (el VPS no tiene toolchain).

Segundo falso positivo, mismo día, en los casos anteriores de michiclaude:
`lib.rs` leído UNA vez en 6 tandas de 1000 líneas (offset 0…5000) contaba
6 relecturas → ficha attach. Es justo lo que la ficha recomienda hacer.
Arreglo: la relectura se cuenta por ARCHIVO + RANGO (`read_key` →
`ruta#Lini-fin`), en el coach y en el detector `reread` de Hallazgos,
Rust y Python. Regresión: 12/12 hallazgos iguales sobre 30 días de logs
reales; la sesión de los 6 trozos ya no dispara ni attach ni reread.

## 2026-08-16 — cargo check limpio en Windows: se cierra la deuda de compilación

`cargo check` en el Windows de Oscar (`michiclaude v0.1.2`, `Finished dev
en 3.36s`) sin errores ni avisos, con TODO lo escrito desde el VPS del
14 al 15: fila 18 `waste`, las 4 piezas de integridad, purga del archivo
(archivador + WSL), detectores `acompact`/`paste` y el fix
relectura=archivo+rango / regla `shots`. Los dos "cargo check pendiente"
de CLAUDE.md quedan cerrados. Lo que sigue vivo es solo VALIDACIÓN
PASIVA con el uso (auto-/compact, auto-/clear, `via:emb`, alarmas,
ntfy, purga real) — nada bloquea a Oscar y el ruteo (etapa 1+) sigue
esperando esas pruebas.

## 2026-08-16 (2) — orden en docs/: índice, "dónde mirar", plantilla y bitácora rotada

QUÉ: (1) `docs/README.md` nuevo: tabla "qué doc abrir según lo que
toques" + tabla "dónde mirar cuando algo falla" (rastro por área:
quota_debug, coach_debug/flowLog, ai/emb_debug, rem/wrap_debug,
integrity, scan_cache, prices_cache, ntfy/hub_debug, coach_state en el
VPS) + convenciones. (2) `docs/img/` con `README.md` de convención
(`AAAA-MM-DD-area.png`, sin datos personales: el repo se publica).
(3) Plantilla de entrada de bitácora (QUÉ / POR QUÉ / CÓMO SE VERIFICÓ /
QUÉ QUEDA) en la cabecera de este archivo. (4) Bitácora ROTADA: el
tramo fósil (el CLAUDE.md original de 118k, líneas 15–1862, hasta
2026-08-04) pasa íntegro a `bitacora-hasta-2026-08-04.md`; este archivo
arranca en el rediseño del 2026-08-05 (4.800 → 2.950 líneas). (5)
CLAUDE.md apunta al índice y a la plantilla; podado para quedar en
39.874 bytes.
POR QUÉ: Oscar preguntó qué prácticas del proyecto valen la pena
generalizar (posible skill aparte) y qué faltaba; lo que faltaba era
esto: el rastro de depuración estaba disperso en 6 docs, la bitácora
crecía sin freno y las entradas no tenían forma común.
CÓMO SE VERIFICÓ: las 9 referencias "bitácora §…" de CLAUDE.md apuntan
todas a jornadas del 2026-08-05 en adelante (siguen en `bitacora.md`);
`pill_debug.json` NO existe ya en el código (solo en .gitignore) y se
quitó de la tabla; los demás rastros comprobados con grep en lib.rs /
relevo / index.html.
QUÉ QUEDA: usar la plantilla en la próxima jornada; capturas del README
a `docs/img/`; el skill genérico ("nuevo proyecto con este método") se
decide aparte, fuera de este repo.

## 2026-08-16 (3) — Etapa 3 del análisis local DISEÑADA: temas sobre `inflate` (contar → demostrar)

QUÉ: diseño (sin código) en `analisis-local.md` §"Etapa 3": la tarjeta
`inflate` ("una conversación siguió creciendo N turnos") gana una capa
ADITIVA que parte la sesión en TRAMOS de tema con embeddings
(EmbeddingGemma, mismo llama-server de la etapa 2, nunca el 2B) y
CALCULA el ahorro de cada `/clear` que no se hizo (reparto del
`cache_read` por tramo, mismo `price_for` que `cr_cost`); un solo tramo
→ "aquí conviene /compact, no /clear". Reglas: hallazgo determinista y
`fndKey` intactos; fail-quiet (sin GGUF/opt-in = tarjeta de hoy);
evidencia = mensajes humanos vía `user_turn_text` ≤300 chars, NO se
persisten; solo en la pasada completa de Hallazgos, caché por sesión
`inflate_topics.json`; algoritmo con centro de tramo, frontera sostenida
2 mensajes (`TOPIC_HOLD`), tramo mínimo 4, mensajes ≤3 palabras no
votan; constantes propias `TOPIC_*` (arrancan en EMB_NEW/EMB_CROSS).
Tres piezas: Rust local/WSL (`topics_for_inflates`, `ai_emb_vecs`),
exportador manda `umsgs` por SSH y el Windows embebe (el modelo NO va al
VPS), panel pinta chips de tramos + ahorro (`fnd_inflate_one/multi` ×8),
`topics` no viaja al hub. Puntero en `analizador-fugas.md` §5 (por qué
un embedding no viola "determinista, nunca un modelo local").
POR QUÉ: Oscar vio dos tarjetas inflate (71 y 30 turnos, VPS-EU) y
preguntó si Michi "detecta" cambios de tema; la respuesta honesta es
que la tarjeta cuenta y la ficha SUPONE ("al cambiar de tema…"). Pidió
que fuera "más inteligente aparte de contar o suponer". Alternativa
descartada: pasar el 2B por los logs (genera, no reproducible, choca con
analizador §5 y con la RAM del widget).
CÓMO SE VERIFICÓ: solo docs — no hay código que verificar. Se comprobó
en el código dónde engancha: `Finding` (serde default), emisión de
`inflate` en Rust (`cr_cost`) y en el exportador, `ai_emb_verdict` /
`EMB_NEW`/`EMB_CROSS`, `user_turn_text` en los dos lados, `fndKey` en
el panel. CLAUDE.md en 39.925 bytes tras podar dos [x] ya cerrados.
QUÉ QUEDA: la etapa 3 queda BLOQUEADA detrás de las pruebas en vivo de
auto-/compact y auto-/clear (misma zona `ai_emb_*`), como el ruteo.
Orden al arrancar: Rust local + panel → medir fronteras con las
sesiones reales de la semana → exportador `umsgs` → fixture de test.

## 2026-08-16 (2) — el /clear ya no te quita la conversación de la vista: globo post-/clear + visor de la sesión borrada

QUÉ: (1) Kind de globo nuevo `cleared`: cualquier /clear —tecleado por el
usuario (lo publica `user_cmd` del relevo, mismo sello anti-doble del
desbloqueo) o aplicado por el AUTOMÁTICO— saca el globo persistente "la
conversación quedó guardada"; el clic abre el visor con la conversación
(la ventana notif ahora emite `notif:open` con el kind antes de
`show_panel`). (2) Comando Rust `read_cleared(sid,cwd,ts,origin)`: trae el
.jsonl de la sesión borrada cuando no hay copia handoff — Claude Code no
lo borra con /clear (verificado contra sessions.md/hooks.md oficiales);
seguridad familia `read_handoff` (sin rutas del frontend, sid completo
validado, búsqueda por cwd+ts que excluye a la sesión recién nacida, 4 MB,
solo lectura). (3) Fichas calientes cache/compact con relevo casado ganan
el botón "Aplicar" de la tarjeta de intención (`relayApply` tal cual, con
su cuenta atrás y la red /export en /clear v2); para el casado en terminal
los hits `cache`/`compact` ganan `scwd` ADITIVO (réplica exacta en
meter-export.py, invariante #1). (4) 3 claves i18n × 8 idiomas. (5) Diseño
completo en remediacion.md §"El globo post-/clear" + resumen en sus REGLAS
VIGENTES; antes, en la misma jornada (commit 655cf65), entraron dos
análisis externos a docs/ con nota de encaje (hooks token-saving y widget
en taskbar).

POR QUÉ: pedido de Oscar con escena real — se aleja unos minutos, el
automático (o él, siguiendo la ficha de caché) hace /clear, y al volver la
terminal está vacía y la copia enterrada en el registro de acciones. La
decisión de diseño: NO reinyectar contexto al modelo (un auto-/clear
existe porque el contexto viejo estorba; la escalera "Copiar arranque" +
hook SessionStart(clear) queda diseñada para después, junto a la
maquinaria de hooks del ruteo) y NO tocar relevo ni lista blanca (cero
cambios en relevo/ y michi-relevo.py). El botón manual del panel no saca
globo: quien lo pulsa está viendo el registro. La presencia
(GetLastInputInfo) se pospone: tocaría relayAutoCheck en plena validación
pasiva.

CÓMO SE VERIFICÓ: py_compile limpio (exportador) y node --check limpio
sobre los scripts extraídos de index.html/notif.html; grep de coherencia
(comando registrado, 26 líneas con las claves nuevas, disparadores). cargo
check NO corrido (clon VPS sin toolchain): PENDIENTE en el Windows de
Oscar, más la prueba en vivo de los 3 caminos (tecleado terminal, tecleado
chat con sid, automático con copia).

QUÉ QUEDA: cargo check en Windows; validación en vivo de los 3 caminos
(añadida al pendiente VALIDACIÓN PASIVA de CLAUDE.md, que quedó a 39,967
de 40k — la próxima adición YA exige poda); las piezas 2 (Copiar
arranque / hook SessionStart) y 3 (presencia) del diseño, bloqueadas
tras la validación pasiva y la maquinaria de hooks del ruteo.

## 2026-08-16 (3) — el globo post-/clear, validado en vivo y extendido a WSL y al VPS

QUÉ: (1) VALIDACIÓN EN VIVO del camino principal (Oscar, Windows): compiló
limpio en 27.96 s —cierra la deuda del cargo check— y el /clear tecleado en
una terminal con relevo sacó el globo y el visor enseñó la conversación
borrada, legible. (2) Se cerró el hueco que destapó la pregunta de Oscar
("¿y en WSL y el VPS?"): en un servidor SSH el Windows no puede mirar el
disco ajeno, así que la búsqueda la hace ALLÁ el exportador con
`--cleared-stdin` (`find_cleared`, réplica exacta de `read_cleared`,
invariante #1) y las señas {sid,cwd,ts} viajan por STDIN — nunca en la
línea de comandos. Guarda de compatibilidad: se exigen ≥2 líneas jsonl, así
que un exportador viejo (que ignora el flag y contesta su JSON de gasto en
una línea) se descarta y el visor dice GONE. (3) Bug PREEXISTENTE cazado de
paso: el sello anti-doble de relayUserCmds usaba el pid A SECAS y el pid
solo es único dentro de una máquina — con WSL y el VPS en la lista dos
sesiones podían compartirlo y la segunda se daba por vista (sin globo y sin
contar para el desbloqueo); la clave ahora es `origin#pid` y acepta el sello
viejo para no recontar al actualizar. (4) Matriz de cobertura por origen en
remediacion.md.

POR QUÉ: la pregunta de Oscar no era retórica — el hueco existía y era del
tipo peor: el globo SÍ salía en el VPS (los relevos remotos llegan por el
compás del coach) y el visor habría contestado "esa conversación ya no está
en el disco". Un globo que promete lo que no puede cumplir es peor que no
tener globo. Se descartó buscar por SSH interpolando el cwd en el comando
(un cwd con espacios o comillas es exactamente la puerta que
relay_inject_remote cerró): el patrón bueno ya existía en el proyecto y es
el de --prices-stdin.

CÓMO SE VERIFICÓ: en vivo lo dicho arriba (flowLog: `relevo: /clear
tecleado por el usuario en pid 21048` → `globo cleared:` → visor con la
conversación). `find_cleared` probado contra los .jsonl REALES de este VPS,
5/5 — por cwd, por sid completo, cwd falso→nada, sid corto (el de 8 del
coach)→nada, y el caso fino: simulando el /clear en el instante en que
nació una sesión, descarta la recién nacida y elige la que murió.
Tubería `--cleared-stdin` corrida tal como la invocará Rust (418 líneas de
jsonl; entradas falsas → 0 bytes). py_compile y node --check limpios.
PENDIENTE: cargo check del cambio SSH (llegó después del build de Oscar).

QUÉ QUEDA: cargo check en Windows y validación en vivo de los caminos que
faltan (chat con sid, automático con copia, WSL, VPS). Prueba 2 (el botón
"Aplicar" de la ficha de caché) sigue esperando a que la regla se dé sola:
pide ≥6 min de pausa Y ≥30k de contexto, y tras un /clear el contexto nace
casi vacío — por eso la espera de Oscar no la disparó.

## 2026-08-16 (4) — cargo check limpio del camino SSH: la pieza del globo post-/clear queda sin deuda de compilación

QUÉ: Oscar compiló en Windows el commit 811e9ef (`Compiling michiclaude` →
`Finished` en 34.86 s, sin errores). Con eso las DOS entregas del día
—globo post-/clear (e34feeb) y su extensión a WSL/VPS (811e9ef)— tienen
cargo check limpio.

POR QUÉ: el VPS es espejo de código sin toolchain de Rust; todo cambio en
Rust necesita el check en la máquina de Oscar antes de darse por cerrado.

CÓMO SE VERIFICÓ: la salida del build (línea `Compiling` presente: sin
ella, cargo no recompiló y se estaría probando el exe viejo — la trampa
del empate de mtime, que hoy mordió DOS veces por otra razón: el clon
estaba varios commits atrás y el `git pull` faltaba, no la fecha).

QUÉ QUEDA: solo validación en vivo de los caminos que faltan (chat con
sid, automático con copia, WSL, VPS). El de Windows/terminal ya está
validado (entrada 3).

## 2026-08-17 — el botón "Aplicar" de la ficha, validado en vivo: dos observaciones de UI (ninguna es un fallo del código nuevo)

QUÉ: Oscar probó en Windows el camino que estrena la pieza: la ficha de
caché nació caliente ("26 min de pausa con contexto grande"), su botón
"Aplicar" corrió la cuenta atrás, el /clear se aplicó en el pid 21048 y la
copia `handoff-21048-1786924911.md` quedó en disco y se vio en el visor.
Funcionó de punta a punta. Dos observaciones suyas:

1. NO salió globo. Es la REGLA DE DISEÑO (el botón manual del panel no
   dispara globo: quien lo pulsa está delante). Y además el globo se
   suprimiría solo al volver a enfocar el panel. PERO el usuario se quedó
   sin camino corto a la copia: tuvo que ir a Ajustes → registro de
   acciones. Mejora candidata: enlace "ver la copia" en la propia fila del
   botón tras aplicar.
2. La ficha "salió doble". No lo fue: a las 00:00:54 nació
   `cache|7ba4ce66`, la leyó, y a las 00:02:59 nació `cache|f6395676` —
   OTRA sesión del MISMO proyecto (C:\Users\oscar) cruzando los 6 min de
   pausa; la "Ahora:" lo delata (26 min vs 6 min). La regla "una tarjeta
   viva por regla" sustituye la vieja por la nueva, así que nunca hubo dos
   a la vez: lo que se vio fue el post-it reencendiéndose. La causa de la
   confusión es que la ficha se identifica por PROYECTO + origen, y con
   varias sesiones en la misma carpeta eso no distingue nada. Mejora
   candidata: que la ficha diga a qué sesión se refiere.

POR QUÉ: las dos son de la UI del coach y PREEXISTEN a la pieza del globo;
se anotan aquí porque salieron a la luz al haber por fin un botón que
ACTÚA sobre la ficha — antes el consejo se quedaba en texto y daba igual
de qué sesión hablara.

CÓMO SE VERIFICÓ: flowLog de Oscar (`nace tarjeta cache|7ba4ce66` →
`/clear aplicado a mano (18/3)` → `aplicado /clear en pid 21048` →
`nace tarjeta cache|f6395676`), capturas del botón en cuenta atrás, del
registro con "ver la copia", del visor y de la carpeta handoff con el .md.
Las líneas del log salen de relayApply (index.html:10102 y :10157), lo que
confirma que fue el BOTÓN y no un comando tecleado — por eso no hubo globo.

QUÉ QUEDA: decidir las dos mejoras de UI (enlace a la copia tras aplicar;
identificar la sesión en la ficha). Validación en vivo pendiente: chat con
sid, automático con copia, WSL y VPS.

## 2026-08-17 (2) — las dos mejoras de UI del coach: enlace a la copia y la ficha dice qué sesión

QUÉ: (1) Tras aplicar `/clear` desde una tarjeta aparece "ver la copia" en
la misma fila del botón, abriendo el visor de siempre; el nombre sale del
registro de acciones y se guarda en `tipCopies` para sobrevivir a los
repintados. Vale igual para la ficha caliente y para la tarjeta de
intención. (2) Las fichas `cache` y `compact` dicen a qué sesión se
refieren: el campo `title` viaja ahora en esos hits (aditivo, réplica en
meter-export.py — invariante #1) y la ficha lo enseña recortado a 42
chars, con el sid corto de respaldo si la sesión aún no tiene título.

POR QUÉ: las dos salieron de la primera prueba real del botón (entrada
anterior). La segunda dejó de ser cosmética en cuanto la ficha ganó un
botón que ACTÚA: mientras se escribía esta misma respuesta le llegó a
Oscar una ficha de caché de la sesión `michiclaude · VPS-EU` — la que
estábamos usando— con su botón "Aplicar" al lado. Sin identificar la
sesión, un clic ahí borra la conversación equivocada (con copia, pero
cortada). El título de Claude Code es el identificador humano que ya
existía: el `sum` lo usaba desde siempre.

CÓMO SE VERIFICÓ: `node --check` y `py_compile` limpios; el coach del VPS
devolvió el hit `press` con `title: "Analizar viabilidad de dos documentos
.md para implementaci…"` — o sea, el campo está poblado y es legible, que
era la duda; regresión de `--cleared-stdin` OK (sigue devolviendo jsonl y
eligiendo por ventana de tiempo). **cargo check LIMPIO en Windows** (Oscar,
2026-08-17: `Compiling michiclaude` → `Finished` en 26.42 s).

QUÉ QUEDA: ver las dos mejoras en vivo. Validación pendiente de siempre:
chat con sid, automático con copia, WSL y VPS.

## 2026-08-17 (3) — la ficha caliente dejaba de decir la verdad: se refresca y confiesa su edad

QUÉ: las fichas `cache`, `compact`, `attach` y `shots` se refrescan con la
medición de cada sondeo sin renacer (conservan born/min/v, como la de
intención) y llevan `ts` (última medición); cuando su regla deja de
dispararse, la ficha muestra "medido hace X min" a partir de 3. Clave
`tip_ago` en los 8 idiomas. `sum` y `acomp` quedan fuera: son fotos de algo
terminado.

POR QUÉ: Oscar sospechó que la detección era LENTA ("me avisa mucho
después"). Se midió y era lo contrario — la reconstrucción de la sesión
4652b615 (chat de VS Code por SSH al VPS) contra su propio .jsonl:

  00:31:04  último byte escrito → empieza la pausa
  00:37:04  nace cache|4652b615   → 6 min y 0 s EXACTOS, el primer instante
            en que la regla podía dispararse (el push de `done` salió a la
            vez, por lo mismo)
  00:40:38  vuelve a escribir (la pausa duró 9 min 34 s)

Lo que fallaba no era el reloj: la tarjeta NACÍA Y SE CONGELABA. A las
00:40, con Oscar ya trabajando otra vez, seguía diciendo "Ahora: 6 min de
pausa" — un dato fósil con la palabra "Ahora" delante. De ahí la sensación
de aviso tardío. El estado del coach en el VPS confirmó lo demás: 66.587
tokens de contexto, 20 turnos, título "Qué es michiclaude" (la mejora de
identificar la sesión, funcionando en vivo).

Queda dicho también el techo REAL de latencia para sesiones remotas: 6 min
de regla + hasta 3 de sondeo (compás del coach en reposo; con actividad son
60 s, que es lo que pasó aquí). Y una limitación de fondo que NO se
arregla con código: este consejo es forense, no preventivo — el caché
caduca a los ~5 min y avisar antes no serviría, porque si hay pausa es que
no estás delante. Su valor es a la vuelta: "¿sigue siendo el mismo tema?".

CÓMO SE VERIFICÓ: reconstrucción del .jsonl real y del coach_state.json del
VPS (arriba); `node --check` limpio; simulacro de la lógica de refresco —
refresca el valor, conserva leída/plegada/born, y NO toca la tarjeta de
otra sesión ni resucita una despachada. Sin cambios en Rust: no hace falta
cargo check.

QUÉ QUEDA: verlo en vivo (que el número de la ficha se mueva y aparezca
"medido hace X min" al reanudar). Validación pendiente de siempre: /clear
del chat, automático con copia, WSL y VPS.

## 2026-08-17 (4) — cierre de jornada y poda de CLAUDE.md

QUÉ: se cierra el tema del /clear. CLAUDE.md recoge la regla nueva del
coach (ficha caliente que se refresca y lleva `ts`) y, para que cupiera,
se PODARON dos bloques cuyo detalle vive verificado en otro sitio: la
entrada de PURGA (reglas completas en remediacion.md §"Purga del archivo";
quedan el puntero y lo que no se puede olvidar — allowlist jamás
`~/.claude`, VPS solo informa) y la línea "VISOR DE COPIAS handoff
validado en vivo" (registro de validación ya cumplido; el diseño vive en
remediacion.md:1458). Antes de cada poda se comprobó por grep que la
regla existía en el doc de destino.

ESTADO AL CIERRE: CLAUDE.md a 39.931 de 40.000 — 69 caracteres de margen.
Sigue estructuralmente EN EL TOPE: la próxima jornada que necesite anotar
algo ahí debe empezar por una PODA GRANDE (mover a docs/ lo que ya esté
explicado allí), no por improvisar recortes al final como hoy.

LO ENTREGADO HOY (4 commits de código + docs): globo post-/clear con
visor de la sesión borrada (Windows, WSL y VPS, este último vía
`--cleared-stdin` del exportador); botón "Aplicar" en las fichas
cache/compact con la red /export; enlace a la copia en la propia fila;
las fichas dicen de qué sesión hablan (title) y se refrescan en vez de
congelarse. Tres bugs preexistentes cazados de paso: sello del relevo por
pid sin origen, fichas que parecían duplicadas y el dato fósil bajo la
palabra "Ahora".

QUÉ QUEDA: solo validación PASIVA con el uso (Oscar avisa si ve algo
raro): /clear del chat con sid, automático con copia, WSL y VPS, y ver el
refresco de la ficha en vivo. Y la poda grande de CLAUDE.md.

## 2026-08-17 (5) — el globo post-/clear abría la conversación equivocada: la trampa del sid vivo

QUÉ: dos arreglos en el visor de la sesión borrada. (1) La guarda del SID
VIVO en `read_cleared` (Rust) y su réplica `find_cleared` (exportador,
invariante #1): si el archivo del sid nació CON el /clear, se cae a la
búsqueda por cwd. (2) `handRender` deja de pintar los envoltorios que
Claude Code inyecta con rol user (misma lista que `user_turn_text`),
recorta el `<system-reminder>` pegado al mensaje bueno y marca con 🖼 la
captura sin texto. Reglas y porqués en `remediacion.md` §"La trampa del
sid vivo".

POR QUÉ: Oscar mandó una captura del visor con un único apunte,
`<command-name>/clear</command-name>` en crudo, y lo dio por detalle
estético. No lo era: el visor estaba abriendo la sesión RECIÉN NACIDA. En
el chat el relevo publica el sid de la sesión VIVA y el /clear estrena
sesión en el acto, así que el sid que llega al panel es el de la nueva; la
rama del sid de `read_cleared` se fiaba de él a ciegas — la guarda del
"nació con el /clear" existía SOLO en la rama de terminal, que es justo la
única fila de la matriz que estaba validada en vivo. Prueba: el estado del
relevo del VPS (`~/.michiclaude/relevo/2263534.json`) con
`sid = e84443f4…` (nacida 01:47:57) y `user_cmd = /clear` del MISMO
segundo, mientras la conversación borrada, `d35db79a…`, seguía intacta con
815 KB. La captura engañaba porque TODA sesión nacida de un /clear empieza
por ese mismo apunte: por arriba, el archivo bueno y el malo se ven
idénticos. Se descartó el arreglo "más fino" (que el relevo publique el
sid del instante del `user_cmd`): obliga a campo nuevo en los DOS relevos
y deja rotos los ya instalados, mientras que la guarda arregla también a
los viejos y no toca `relevo/`.

CÓMO SE VERIFICÓ: exportador contra los .jsonl reales del VPS con las
señas EXACTAS del fallo — `HEAD` devuelve `e84443f4…` (el bug), el
arreglado devuelve `d35db79a…` completa (815 955 bytes). Batería de 8
casos, 8/8, y el único que cambia de respuesta es el del bug (sid bueno
con ts posterior, sid bueno sin momento, sid inexistente, sid corto,
terminal sin sid, cwd falso, sin momento). `handRender` extraído del
index.html y corrido en node contra dos conversaciones reales: 130→129 y
72→71 apuntes, cae exactamente el del /clear, cero tripas en la salida y
la captura marcada. NO verificado aquí: `cargo check` del cambio en Rust
(el VPS no tiene toolchain) y el clic real en el globo — ambos quedan para
el Windows de Oscar.

QUÉ QUEDA: `cargo check` + la prueba en vivo del globo en el chat, que era
justo el camino que la matriz daba por pendiente. Lo demás de la
validación pasiva sigue igual.

## 2026-08-17 (6) — el /clear que tecleas tú ya sale en el registro, con su propia etiqueta

QUÉ: el "Registro de acciones" de Ajustes pinta ahora también los `/clear`
que TECLEA el usuario, mezclados por fecha con los demás pero con etiqueta
y verbo propios: **tú · tecleaste /clear en «proyecto · servidor»**, con su
botón "ver la copia". Tres etiquetas en total — `auto` (lo decidió Michi),
`manual` (pulsaste el botón y lo tecleó Michi), `tú` (lo escribiste en la
terminal) —. Piezas: `clearedLog` en localStorage (tope 10) que
`clearedRemember` alimenta solo con los marcados `you:true`, fusión por
`ts` desc en `remLogLoad`, `clearedView(info)` acepta ahora una fila
concreta, y dos claves nuevas por idioma (`rem_you_lab`, `rem_log_typed`,
×8). Backend, exportador y `RemAction` SIN TOCAR.

POR QUÉ: lo cazó Oscar mirando su propio registro tras un `/clear` a mano
("no sale en el registro de abajo con los demás, ¿es normal?"). La primera
respuesta fue "sí, es de diseño" — y lo era, está escrito en
`remediacion.md` §relevo y en el comentario de `relayUserCmds`: el registro
es de lo que aplica MICHI, no tú. Pero al revisar salió un agujero de
verdad: el globo post-`/clear` era la ÚNICA puerta a esa conversación, y
por la regla única de los globos no vuelve una vez cerrado; encima
`clearedInfo` es UNA sola ranura que el siguiente `/clear` pisa. Cerrar el
globo con la ✕ sin clicar, o encadenar dos `/clear`, dejaba la conversación
viva en disco y sin ningún botón en toda la app que la pidiera. El registro
era el sitio natural para rescatarla.

Se descartó hacerlo en Rust (que era la lectura obvia de "meterlo en el
registro"): un `/clear` tuyo no deja copia handoff y se recupera con
`read_cleared(sid, cwd, ts, origin)` — señas que no caben en `RemAction` y
que obligarían a campos nuevos + réplica en `meter-export.py` por el
invariante #1, todo para pintar una fila. En el frontend los datos ya
estaban ahí: son los mismos que `clearedRemember` recibe desde 2026-08-16.

Dos trampas que se vieron antes de caer en ellas: (1) la marca de "esto lo
tecleaste tú" es EXPLÍCITA (`you:true`) y no se deduce de `file` vacío — un
automático cuyo nombre de copia no se pudo leer llega igual de vacío y
habría salido dos veces, una por Rust y otra por la lista; (2) el clic usa
la foto `remLogMine` que se PINTÓ, no `clearedLog()` releído, porque un
`/clear` nuevo se mete por delante (unshift) y el índice abriría la
conversación de al lado.

CÓMO SE VERIFICÓ: sintaxis del JS embebido con `node --check` (bloque
único, OK) y simulación del render fusionado con 4 acciones de Rust + 1
`/clear` tecleado — sale exactamente la tabla del ejemplo, con la fila
`tú` arriba por fecha y el `⚠ falló` intacto en la suya. Las 8 claves
nuevas verificadas por conteo (9 apariciones = 8 diccionarios + 1 uso).
Rust sin cambios, así que no hay `cargo check` que correr. NO probado en
vivo todavía: falta que Oscar teclee un `/clear` y vea la fila nacer y su
"ver la copia" abrir el visor.

QUÉ QUEDA: la prueba en vivo de arriba, dentro de la validación pasiva que
ya estaba abierta. No abre pendientes nuevos.

## 2026-08-17 — la fila «tú» existía pero nadie la mandaba pintar

QUÉ: `relayUserCmds()` repinta el registro de acciones (`remLogLoad()`)
cuando acaba de anotar un `/clear` tecleado y la pestaña Ajustes está a la
vista. Un `let nuevoClear` y una línea; sin tocar Rust.

POR QUÉ: Oscar hizo la prueba en vivo de la fila «tú» (commit 79160d6) y
no la vio. La cadena estaba entera y correcta —el relevo publica
`user_cmd`, `clearedRemember` guarda con `you:true` en `clearedLog`,
`remLogLoad` fusiona por `ts` y pinta— pero `remLogLoad` SOLO se llama al
entrar en una pestaña (`showTab("prefs")`) y tras una acción de Michi.
Estando ya en Ajustes esperando a que la fila naciera, no había ningún
render entre el `/clear` y la mirada: la lista era la de hace un rato. El
globo sí salía (ese lo dispara `clearedRemember`), y ese contraste —globo
sí, fila no— es lo que hacía pensar que la fila no se había implementado.
Autopsia en una frase: **se pintó la fila y se olvidó quién la manda
pintar.** Segunda causa posible que NO se descarta desde el VPS: que el
binario de Windows fuera el de antes del commit (mismo síntoma exacto);
por eso el arreglo se acompaña de la comprobación de compilar/instalar.

CÓMO SE VERIFICÓ: solo lectura del código (frontend, sin Rust — no procede
`cargo check`). La prueba en vivo la hace Oscar: con Ajustes abierto en el
registro, teclear `/clear` y ver la fila aparecer sola, sin cambiar de
pestaña.

QUÉ QUEDA: esa prueba, dentro de la validación pasiva ya abierta. No abre
pendientes nuevos.

## 2026-08-17 (8) — cerrada la validación en vivo del relevo: el ruteo queda desbloqueado

QUÉ: Oscar probó en vivo la cadena completa del `/clear` tecleado por él y
salió entera. Con eso se da por CERRADO el bloque de validación pasiva que
frenaba al ruteo inteligente. Se actualiza CLAUDE.md §"Estado / pendientes"
en tres sitios: la validación pasiva anota qué quedó cerrado, el ruteo pasa
de BLOQUEADO a EN CURSO, y la etapa 3 del análisis local queda desbloqueada
pero APARCADA detrás del ruteo. Sin cambios de código.

POR QUÉ: la etapa 1 del ruteo se paró a propósito el 2026-08-13 para no
tocar la misma zona mientras los automáticos se probaban en vivo. Esa razón
ya no existe.

CÓMO SE VERIFICÓ: cuatro capturas de Oscar, una por eslabón. (1) Globo del
gatito tras teclear `/clear` en la terminal del VPS: "/clear en michiclaude
· VPS-EU — la conversación anterior quedó guardada. Clic para verla."
(2) Clic en el globo: el visor abre la conversación BORRADA, no la recién
nacida — vuelve a confirmar el arreglo del sid vivo (e2ca447). (3) Pestaña
Ajustes, ya abierta y sin tocar nada: la fila nace sola arriba del registro,
`16/08 08:37 p.m. · tú · tecleaste /clear en «michiclaude · VPS-EU»`, con su
botón "ver la copia" — el repintado de 5422bf7 funcionando. (4) Ese botón
abre el MISMO visor con la misma conversación, y debajo siguen en su sitio
las filas `auto` del 13/08, ordenadas por `ts` desc. La fecha de la fila es
hora LOCAL de Windows (UTC-6) contra un VPS en UTC: `17/08 02:37 UTC` =
`16/08 08:37 p.m.` — cuadra, no es un desfase.

QUÉ QUEDA: de la validación pasiva siguen abiertas las alarmas reales, el
camino ntfy con la PC apagada, el aviso de hallazgos naciendo natural y el
primer `via:emb` en sesión real al 80%. Ninguna frena al ruteo: no tocan
`ai_emb_*` ni el relevo. Siguiente paso, etapa 1 de
`docs/ruteo-inteligente.md` §10-11.

## 2026-08-17 (9) — ruteo inteligente, etapas 1 y 2: el Hook B existe y ya ruteó un subagente de verdad

QUÉ: las dos primeras etapas construibles del ruteo
(docs/ruteo-inteligente.md §11). (1) La «nota del refri»:
`pushRouterState()` en el frontend (junto a logQuota, mismo guard
simRunning) manda el estado GRUESO de la cuota (% a múltiplos de 5, horas
al reset) a `save_router_state` (Rust), que lo deja en `~/.michiclaude/`
de esta máquina, de cada home WSL (\\wsl.localhost, fs puro) y de cada
servidor SSH — solo con el interruptor encendido. (2) El Hook B:
`scripts/router-hook.py` y `router-hook.ps1` (réplicas exactas, embebidos
con include_str!), un PreToolUse sobre `Task|Agent` que impone el modelo
del subagente vía updatedInput con el objeto COMPLETO: exploración→haiku
siempre, implementación→sonnet, análisis→sonnet solo con el peor bucket
≥70; `model` explícito se respeta, prompt con `~` es escotilla, estado
ausente o >10 min = silencio absoluto. Decisiones a `ruteo_log.jsonl`
(JSON plano, rota a .1 en 512 KB). El alta/baja: guion `RUTEO_PY` por
STDIN en SSH/WSL y `ruteo_local()` en Windows, misma lógica — respaldo
`.michi-backup` una vez, merge atómico, MANUAL si el settings no parsea,
NOHOOK sin script, BADOP para op desconocida. Interruptor en Ajustes
(claves `rt_*` ×8 idiomas) con una fila por máquina, patrón del wrapper
del chat. Comandos nuevos: `get_ruteo`, `set_ruteo`, `save_router_state`
(los tres async+spawn_blocking, invariante 10ter).

POR QUÉ: es el PRÓXIMO GRANDE decidido el 2026-08-13, desbloqueado esta
misma jornada al cerrar las pruebas del relevo. El orden (nota primero,
hook después) es el del §11; el motor quedó LOCAL-only a propósito
(§10.3c): los hooks viven donde corre Claude Code y el exportador no
participa, así que el invariante #1 no obliga réplica — documentado para
no morder después.

CÓMO SE VERIFICÓ: (a) matriz sintética del router-hook.py con HOME falso,
16/16 (clases, presión por sesión/semana/null, explícito, bypass, estado
viejo, basura, objeto completo conservado, log y rotación). (b) El guion
RUTEO_PY extraído DEL lib.rs (se probó lo embebido, no una copia): alta
sobre un settings con hook AJENO que quedó intacto, idempotencia, baja
que poda, MANUAL con JSON roto, BADOP, python absoluto en el command.
(c) EN VIVO en el VPS, ciclo completo real: alta sobre el settings.json
de Oscar (con respaldo), nota fresca a mano, sesión headless `claude -p`
padre en Sonnet lanzando un subagente Explore → el agent-*.jsonl dice
`claude-haiku-4-5-20251001` y el log anota route/light/haiku; control SIN
nota 27 líneas después: el mismo subagente hereda `claude-sonnet-5` y el
log NI CRECE; baja final y settings.json IDÉNTICO al de antes (diff
semántico). (d) node --check del script embebido del panel, claves rt_*
9/9 (8 diccionarios + 1 uso), .ps1 verificado ASCII PURO. NO verificado:
`cargo check` (el VPS no tiene toolchain — pendiente en el Windows de
Oscar) y la primera corrida real del .ps1 (aquí no hay PowerShell, como
en la etapa 0).

QUÉ QUEDA: cargo check + `npm run build` en Windows, la prueba en vivo
del lado Windows (interruptor de Ajustes, .ps1 ruteando, nota viajando a
WSL y al VPS desde la app real) y las etapas 3-6 (medición en Reporte,
gatito consejero, Hook A). Reflejado en CLAUDE.md §pendientes y en el
§11 del doc.

## 2026-08-17 (10) — el ruteo, cerrado en el lado Windows: la etapa 2 queda validada en producción

QUÉ: cierre de la validación de las etapas 1-2 con la app REAL en el
Windows de Oscar. Sin cambios de código: solo CLAUDE.md y esta entrada.

CÓMO SE VERIFICÓ: (1) `cargo check` limpio en Windows (con `Checking` de
9.2 s — no fue el empate de hora del binario). (2) Interruptor de Ajustes
en modo dev: `local ✓ · VPS-EU ✓ · WSL: Ubuntu ✓` y el recordatorio de
sesiones nuevas. (3) Sesión nueva de Claude Code v2.1.233, padre clavado
en Sonnet, «lanza un subagente Explore que solo diga OK»: el
`ruteo_log.jsonl` de Windows anota `route/light/haiku` — primera corrida
real del `.ps1`, la pieza que el VPS no podía probar. (4) Verificado
DESDE el VPS que la app de Windows hizo su parte por SSH sola: hook
subido a las 03:14 (la hora del interruptor), entrada `Task|Agent` en el
settings.json de acá, y `router_state.json` con 17 s de edad y cuota
REAL (sesión 35, semana 25) — el ciclo de 3 min riega la nota solo.
Matiz de honestidad: el log de Windows prueba que el hook DECIDIÓ haiku;
que Claude Code OBEDECE el updatedInput en Windows lo probó la etapa 0
en esa misma máquina (A/B del 2026-08-14) — por eso se cierra sin
re-mirar el agent-*.jsonl, y el comando para el triple candado quedó
dicho en el chat.

QUÉ QUEDA: etapa 3 (medición en la pestaña Reporte: log de decisiones ×
JSONL reales × quota_history), luego 4-6. La validación del día a día
del ruteo es pasiva desde hoy: Oscar trabaja normal y el log acumula.

## 2026-08-17 (11) — ruteo etapas 3, 4 y 5 de un tirón: guardián, registro visible, medición y consejero

QUÉ: en una sesión, a petición de Oscar ("haz todos los puntos, pruébalos
2-3 veces de maneras distintas, no rompas nada"), cuatro piezas:
(1) REGISTRO VISIBLE del ruteo en Ajustes: latido del día (N ruteados,
→Haiku/→Sonnet, frenos, último dónde) + últimas 15 decisiones de las
tres máquinas con su porqué (`get_ruteo_log`). (2) GUARDIÁN (Hook A,
etapa 5, ADELANTADA): `guard-hook.py/.ps1` en UserPromptSubmit —
prompt pesado en haiku/sonnet = bloqueo ANTES de gastar; interruptores
"guardián" y "contexto inyectado" en Ajustes, banderas dentro de la
nota. UN alta para los dos hooks. (3) MEDICIÓN (etapa 3): `scan_ruteo`
Rust+exportador, tarjeta en Reporte, el Hook B anota `parent`.
(4) CONSEJERO (etapa 4): racha `light` en el motor del coach (réplicas),
compuerta en el panel, tarjeta con botones, `set_default_model` a
local/WSL/SSH, tarjeta de vuelta al reset. Detalle de reglas en
docs/ruteo-inteligente.md §11 (etapas 3, 4, 5 marcadas HECHAS).

POR QUÉ: la pregunta de Oscar tras ver el ruteo funcionar por detrás —
"¿cómo VEO yo que funciona, que no falla, que no se queda en el caro?" —
no tenía respuesta en la app: el log era un jsonl por terminal. Y su
preocupación ("¿estoy en lo caro para algo sencillo, o en lo básico
pidiendo cosas complejas?") partió el plan en dos mitades con costo
distinto: la B (error CARO) es el guardián, que se adelantó a las etapas
3-4; la A (error barato) es el consejero, que aconseja solo por patrón
sostenido y hacia la SIGUIENTE sesión (§4.1: cambiar a media sesión tira
el caché y sale más caro que la pregunta).

AUTOPSIAS de la jornada (mordidas antes de salir): (a) el "largo" del
guardián contaba palabras: 313 caracteres de japonés eran 33 "palabras"
y no disparaba — ahora ≥60 palabras O ≥300 caracteres. (b) `parse_file`
del exportador compara con datetime, no epoch: `RUTEO_EPOCH`. (c) El
hook anota `ts` en segundos enteros y el transcript lleva milisegundos:
el padre "del mismo turno" iba 0.4 s DESPUÉS y no casaba — tolerancia +5
s y, mejor, el Hook B ya anota `parent`. (d) En el turno 1 de una sesión
`--model sonnet` no está en ningún archivo: el guardián NO adivina y deja
pasar (diseño); del turno 2 en adelante bloquea. (e) `[ordered]@{}+$base`
en PowerShell 5.1 es resbaloso: filas construidas explícitas.

CÓMO SE VERIFICÓ (todo en el VPS, con datos reales donde los había):
guardián: matriz 24/24 con HOME falso (idiomas es/ja, insistencia,
escotilla, opus/fable nunca, fallback settings, ctx on/off, privacidad
del log) + EN VIVO con `claude -p --resume` en sesión Sonnet: bloqueo
real sin gastar (assistant turns siguió en 1), insistencia pasó, `~`
pasó, Opus no bloqueó, y con ctx ON Claude citó LITERAL el contexto con
la cuota real (25 % semana, 50 % sesión). Instalador v2 (dos hooks): alta
con hooks AJENOS en ambos eventos intactos, alta a medias repuesta sin
duplicar, off deja solo lo ajeno + `{}` en poda total, MANUAL/BADOP.
Registro: render con filas reales en es/en/ja. Medición: `--ruteo --days
1` sobre el log real → 4/4 casados, 51k tok, $0.28→$0.12 (ahorro $0.15),
un subagente a mano al céntimo (9.3k tok: $0.012 haiku vs $0.059 opus);
ventanas 1/7, `--end` ayer = 0, precios por stdin; tarjeta con la salida
real + 4 casos límite. Consejero: sesión sintética (3 código + 10
preguntas → hit 9; edición reinicia; salida >1500 reinicia; no re-emite);
`--coach` REAL: mi sesión de 378 turnos y 11 archivos editados da
light=0 (no molesta en sesión mixta); guion SETMODEL 8 casos (cwd hostil
en b64 → NOCWD, no ejecuta); b64 Rust-réplica vs estándar 800 casos;
compuerta 9/9 (presión, tier, sin lectura, 3 «no» = manual por
proyecto). NO verificado: `cargo check` (VPS sin toolchain), los `.ps1`
en Windows, y las tarjetas en la app viva. Todo el JS con node --check.

QUÉ QUEDA — PLAN DE PRUEBAS PARA OSCAR (Windows), en orden:
1. `git pull` + `cargo check` (src-tauri) — si algo no compila, pegar el
   error tal cual: son ~900 líneas nuevas de Rust escritas sin compilador.
2. `npm run dev` → Ajustes → tarjeta del ruteo: latido y registro deben
   pintar las decisiones de hoy (VPS + Windows). Encender "Guardián".
3. Sesión NUEVA de Claude Code en Windows, `/model sonnet`, un prompt
   trivial primero (turno 1) y luego pegar un prompt con un bloque de
   código y dos rutas: debe FRENAR con el mensaje bilingüe. Reenviarlo
   igual: pasa. Otro pesado con `~` delante: pasa. Comprobar en el
   registro de Ajustes: «el guardián frenó…», «insististe…», «~ enviado».
4. Reporte → tarjeta "Ruteo inteligente": ahorrado, casados, autoconsumo.
5. Consejero: en una sesión Opus/Fable con cuota ≥70 hacer 9 preguntas
   seguidas sin tocar código → tarjeta en Consejos con tres botones;
   pulsar "Sí, solo este proyecto" y comprobar `.claude/settings.local.
   json` del proyecto con `"model":"sonnet"` (respaldo `.michi-backup` al
   lado si existía). Con cuota <70 la tarjeta NO debe salir (compuerta).
6. WSL: la corrida pendiente del Hook B (una sesión con subagente Explore
   → `route/light/haiku` en el log de la distro).
Etapa 6 (v2: análisis en frío con modelo local, embeddings en el
guardián) sigue en su sitio, después de la validación pasiva.

## 2026-08-17 (12) — el guardián escala solo: `/model <alias>` entra a la lista blanca del relevo

QUÉ: la etapa 5b del ruteo, pedida por Oscar tras probar el guardián en
su chat («después del stop me gustaría que fuera automático eso que hice
yo manual»). Con el interruptor «Escalar solo» (`esc`, apagado, exige el
guardián), al frenar un prompt pesado en haiku/sonnet el hook le deja al
RELEVO de esa sesión la orden `/model <peldaño>` (escalera haiku→sonnet
con una señal, →opus con dos o código; JAMÁS a fable solo) y el usuario
solo reenvía (↑ + Enter). Piezas: `allowed()` en las TRES piezas del
relevo (michi-relevo.py, relevo/main.rs, lib.rs `relay_allowed`) —
`/model <alias>` con alias de lista cerrada, la ÚNICA entrada con
argumento; `destino()`/`relevo_de()`/`escalar()` en guard-hook.py/.ps1;
el relevo espera ≤8 s a quedar libre SOLO para /model (chat: espera en
attend, que corre en hilo; terminal: orden PENDIENTE reintentada por el
bucle de la PTY, para no congelar la pantalla); nota con `esc`; filas
`escalate` en registro y Reporte (`esc` en scan_ruteo, réplicas); globo
del gatito `rtEscBalloon` al compás del coach (el chat de VS Code NO
pinta el motivo del hook — el gatito lo dice); textos del freno «estoy
subiendo la sesión a X — dale ~10 s y reenvía». Reglas en
remediacion.md §"La ÚNICA ampliación con argumento".

POR QUÉ: la mitad B del miedo de Oscar («estoy en lo básico pidiendo
cosas complejas») ya estaba cubierta por el freno; lo que faltaba era
que el remedio no costara tres gestos. Y se decidió NO reenviar el
prompt por él: el hook tendría que guardar su texto (regla de
privacidad) y un multilínea inyectado se manda al primer salto de línea.

AUTOPSIAS de la jornada, tres mordidas seguidas antes de que saliera:
(1) `esc:true` puesto a mano en la nota desaparecía: la app de Windows la
riega cada ciclo con SUS banderas — correcto, y por eso la bandera vive
en ruteo.json y viaja en la nota; en la prueba hubo que "regarla" a la
par. (2) ERR_RELAY_BUSY con la orden escrita 0.2 s tras el bloqueo: el
relevo del chat marca busy al entrar el mensaje y solo lo suelta con el
`result` del bloqueo… que Claude Code emite DESPUÉS de que el hook
termine. El hook esperaba el acuse → abrazo mortal de 8 s. Arreglo: el
hook deja la orden y SALE; el relevo espera él (≤8 s). (3) Con eso el
acuse llegó a los 8 s — y el reenvío de prueba a los 6 s corrió aún en
sonnet: el texto del freno pasó de "espera un segundo" a "dale ~10 s".
De regalo, dos hechos medidos que valen oro: un prompt bloqueado SÍ
produce `result` (`UserPromptSubmit operation blocked by hook: …`, con
el motivo dentro — la extensión decide no pintarlo), y `/model opus`
como mensaje `user` en stream-json cambia el modelo de la sesión («Set
model to Opus 5 for this session only», el turno siguiente en opus-5).

CÓMO SE VERIFICÓ: allowed() 11/11 (incluye `/model opus; rm -rf` y
`/model` a secas → NO); destino() 8/8; matriz del guardián 24/24; y EN
VIVO en el VPS con el relevo REAL en modo chat (`michi-relevo.py wrap`
+ stream-json, `--replay-user-messages`): USER prompt pesado en Sonnet
→ RESULT blocked («…I'm switching this session to opus…») → USER
`/model opus` (tecleado por el relevo, acuse ok a los 8 s) → RESULT «Set
model to Opus 5» → USER reenvío → ASSIST `claude-opus-5`. Sin `esc` en
la nota: freno de siempre, sin intento. Datos reales del día ya cuentan
2 escaladas ok en `--ruteo`. NO verificado: `cargo check` de lo de hoy
(RuteoCfg.esc, set_ruteo_flags(esc: Option), RuteoRow.to/ok, esc en
RuteoReport, relay_allowed; y `relevo/` con allowed()), el `.ps1` en
Windows, y el globo en la app viva.

QUÉ QUEDA (para Oscar, en orden): (1) `git pull` + `cargo check` en
src-tauri Y `cargo build --release` en relevo/ (el michi.exe también
cambió). (2) `npm run dev` → Ajustes → encender «Escalar solo» (bajo el
guardián). (3) En el chat del VPS, sesión Sonnet: «hola», luego el
prompt del `def suma` → freno; a los ~10 s ↑ + Enter → la respuesta debe
venir en Opus, el gatito debe haber sacado el globo «Frené tu prompt…
subí la sesión a Opus», y el registro: «el guardián frenó… y subió la
sesión a Opus». (4) Reporte: fila «N de esos se escalaron solos».
Pendientes anteriores intactos (consejero en vivo, WSL, etapa 6).

## 2026-08-17 (13) — la escalada validada en el chat de Oscar; el globo tenía compás de coach

QUÉ: prueba en vivo de la etapa 5b en el chat Remote-SSH de Oscar
(sesión Sonnet, prompt del `def suma`): freno 16:52:54 → el relevo
tecleó `/model opus` a las 16:53:02 (8 s) → «Set model to Opus 5 for
this session only» → el reenvío corrió en Opus (el selector del chat lo
confirma). Registro: «el guardián frenó… y subió la sesión a Opus».
Dos observaciones de Oscar: (1) el globo del gatito tardó minutos —
estaba colgado del compás del coach (3 min en sesión recién abierta):
ahora tiene sondeo PROPIO `rtEscSched` cada 12 s, solo con el guardián
encendido. (2) Un «hola» posterior siguió en Opus: correcto y a
propósito — `/model` es "for this session only"; MichiClaude solo SUBE
(el error caro), bajar a media sesión es la mitad barata y ahí mandan el
consejero (siguiente sesión) o el usuario. El propio Claude, por el
contexto inyectado, sugirió «/model sonnet te sale mucho más barato».

CÓMO SE VERIFICÓ: capturas de Oscar + log del VPS y acuse del relevo
(esc-… ok a los 8 s). node --check del JS. QUÉ QUEDA: cargo check de
5b en Windows (lib.rs y relevo/), consejero en vivo, WSL, etapa 6.

## 2026-08-17 (14) — 5b cerrada con el globo a tiempo; el segundo /model era de Oscar

QUÉ: segunda prueba en vivo de Oscar tras el sondeo de 12 s: freno
17:17:48 → `/model opus` del relevo 17:17:56 (8 s, acuse esc-…068006) →
globo del gatito «casi instantáneo» (palabras de Oscar). En el chat
salieron DOS `/model opus`: el transcript enseña el segundo a las
17:18:36, 40 s después, sin ninguna orden nueva en el log ni en el
relevo — lo tecleó Oscar (como en la prueba anterior). Inofensivo. Un
solo hook registrado, un solo escalate, un solo acuse. Cerrada la
validación en vivo de la etapa 5b (chat Remote-SSH). Nota de método:
`/clear` en la misma ventana vale como "conversación nueva" (mismo
relevo, sesión nueva de Claude).

QUÉ QUEDA: cargo check de src-tauri con la 5b lo hizo `npm run dev`
(arrancó y la nota llegó con `esc`); relevo/ `cargo check` limpio en
Windows. Pendientes: consejero en vivo (cuota ≥70), WSL, etapa 6.

## 2026-08-17 (15) — 5c: el guardián reenvía por ti (100 % automático en el chat)

QUÉ: la última pieza que Oscar quería desde el principio: tras el freno
y el `/model`, el relevo REENVÍA el prompt frenado y el usuario no toca
nada. Interruptor «…y reenviarlo por mí» (`rs`, apagado, exige «Escalar
solo»); bandera en la nota; el guardián (py+ps1) manda `then` con el
prompt SOLO si el relevo de esa sesión es de chat (`mode: chat`); el
relevo (py+rs) espera el `result` del /model y manda `then` como
mensaje `user` sin eco (`send_user(echo=False)` / `say_echo`) —con eco
salía dos veces, medido—; acuse con `resent`; memoria de insistencia con
`auto` → el reenvío se anota `resent` (Michi), no `insist` (tú). Rust
además ganó la espera de 8 s para /model que solo tenía Python (un
`attend` para los dos modos; la I/O va en hilos, esperar ahí no congela).
Textos rt_rs_*/rt_ev_esc_rs/rt_ev_resent/rt_rep_resent/rt_balloon_esc_rs
×8. Privacidad: el texto viaja en el `.cmd` (borrado al leer) y en la
variable del hilo; ni acuse, ni estado, ni log (verificado: grep del
prompt en el log = 0). Terminal: sigue «reenvía tú» (un multilínea
tecleado se manda al primer salto de línea).

CÓMO SE VERIFICÓ: en vivo (VPS, relevo real, chat stream-json): freno →
/model opus (relevo) → «Set model to Opus 5» → prompt reenviado por el
relevo (multilínea entero) → respuesta en claude-opus-5; el "usuario"
(alimentador) no mandó nada tras el prompt. Segunda corrida con el eco
corregido: un solo mensaje. Matriz del guardián 24/24; --ruteo cuenta
esc=6, resent=2 con el log real del día. NO verificado: cargo check
(lib.rs: RuteoCfg.rs, set_ruteo_flags(rs), RuteoRow.resend, resent en
report; relevo/: is_model_cmd, espera, then, say_echo), el .ps1, la app.

QUÉ QUEDA: Oscar — git pull, cargo check en src-tauri y relevo/, npm run
dev, encender «…y reenviarlo por mí», y la prueba de siempre en el chat
del VPS: tras el freno NO tocar nada — el globo debe decir «…y lo reenvié
yo», y la respuesta llegar en Opus sola.

## 2026-08-17 (16) — 5c en terminal: la TUI pide confirmación y guarda el default — se escala solo SOLO en chat

QUÉ: intento de llevar el reenvío automático (5c) al modo terminal con
pegado entre marcas (`type_paste`, bracketed paste + Enter aparte, en
Python y Rust), más `sid` publicado por el relevo terminal (`guess_sid`
en los dos) y `then` en el pendiente del bucle de la PTY. Todo eso QUEDA
en el relevo — pero el guardián NO lo pide en terminal, por lo medido:
(1) en la TUI 2.1.233 `/model opus` abre un diálogo «Switch model? 1.
Yes / 2. No» — el pegado del reenvío caía encima y su Enter era el «Yes»;
(2) al confirmar, «Set model to Opus 5 and saved as your default for
new sessions»: escribió `model: opus` en el settings.json del VPS
(revertido a mano al instante). En el chat no pasa ninguna de las dos
(«for this session only», sin diálogo). Decisión: escalar/reenviar SOLO
si el relevo publica `mode: chat`; en terminal, freno con mensaje y
`err: TERMINAL` en el log. Y `MODEL_WAIT` a 20 s (la TUI repinta tras el
bloqueo y 8 s se quedaban cortos), pegado solo con calma TOTAL y 1 s
antes del Enter.

AUTOPSIAS de la tarde (varias mordidas de mi propio banco de pruebas):
la PTY hija heredaba `CLAUDE_CODE_CHILD_SESSION` y no guardaba
transcript (sin transcript ni sid ni modelo → el guardián callaba);
Claude Code se actualizó a 2.1.233 a media tarde y estrenó el diálogo
«Set up auto mode?» tras el primer turno (mi pegado lo confirmaba y
cerraba la sesión); y `wait_for("❯")` cazaba el prompt de más y
re-pegaba. Nada de eso es del código de MichiClaude, pero costó tres
corridas distinguirlo del fallo real (el diálogo de /model).

CÓMO SE VERIFICÓ: terminal (PTY real, entorno limpio, diálogo auto mode
contestado): freno con mensaje manual, `escalate ok=False err=TERMINAL`,
settings sin `model`. Chat (regresión): freno → /model opus «for this
session only» → reenvío → claude-opus-5, settings sin `model`. Matriz del
guardián 24/24. NO verificado: cargo check de relevo/ (guess_sid,
type_paste, then en terminal, calm=ready) y del guardián .ps1.

QUÉ QUEDA: si una versión futura de la TUI deja de guardar el default
con /model (o de pedir confirmación), quitar la compuerta `mode==chat`
del guardián y probar el pegado (`type_paste` ya está). Oscar: git pull,
cargo check en relevo/ y src-tauri, npm run dev; el chat sigue igual.

## 2026-08-17 (17) — terminal también escala y reenvía solo: la coreografía del /model

QUÉ: la respuesta a «¿hay manera de arreglar lo de terminal?»: sí. Dos
hipótesis medidas en PTY real (probe): (1) el diálogo «Switch model?» se
confirma con un Enter y un Enter sobre la caja vacía no hace nada; (2)
la sesión NO relee settings.json a media sesión — tras restaurar el
default el turno siguiente siguió en opus. Con eso `type_model` (py y
rs): guarda `model` de settings, teclea `/model`, +1.5 s Enter (solo si
el usuario no tecleó), +3.5 s restaura el default; en Python va como
pasos del bucle de la PTY (`mstep`) para no congelar la pantalla; en
Rust bloquea dentro del hilo (la I/O va aparte). El guardián vuelve a
pedir la escalada también con relevo terminal (fuera la compuerta
`mode==chat`). Textos del interruptor «…y reenviarlo por mí» ya sin el
"(solo chat)" ×8. remediacion.md y CLAUDE.md al día.

CÓMO SE VERIFICÓ: PTY real, entorno limpio: hola (sonnet) → prompt
pesado pegado UNA vez → freno → `/model` del relevo (+Enter al diálogo)
→ default restaurado → pegado del prompt a los ~26 s → ASSIST claude-
opus-5; log `escalate(resend) → resent`; `settings.json` sin `model` al
final. Chat: regresión previa intacta. NO verificado: cargo check de
relevo/ (type_model, settings_model*, guess_sid, type_paste, say_echo) —
Oscar; y el michi.exe en terminal de Windows nativo (ConPTY): la misma
coreografía, sin probar aquí.

QUÉ QUEDA: Oscar: git pull, cargo check en relevo/ y src-tauri, npm run
dev; probar en una TERMINAL de Windows (`michi claude` o el alias),
sesión Sonnet, prompt pesado y no tocar nada — debe frenar, cambiar a
Opus, reenviar y responder en Opus, con el default de settings.json
intacto (mirar `~/.claude/settings.json` antes y después).

## 2026-08-17 (18) — cierre del ruteo inteligente (etapas 0-5c)

QUÉ: se da por CERRADO el bloque de construcción del ruteo. Cargo check
limpio en Windows de todo lo del día (relevo/ y src-tauri, con
`Compiling` en el dev). Estado: nota + Hook B (subagentes → Haiku/
Sonnet, validado en VPS, Windows y desde el chat de Oscar), medición en
Reporte, consejero (motor + tarjeta + set_default_model), guardián
(freno, insistencia, ~, contexto), escalar solo (chat y terminal, con
la coreografía del /model), reenviar por mí (chat y terminal), registro
visible y globo. Aclarado con Oscar: los subagentes NO necesitan
guardián ni escalada porque nacen ya con el modelo correcto — son lo
más automático del sistema.

QUÉ QUEDA (validación PASIVA, con el uso — nada que programar): ver
nacer el consejero (cuota ≥70 + sesión de consultas en Opus/Fable), la
corrida del Hook B en WSL, la terminal de Windows nativo (michi.exe
por ConPTY: misma coreografía, sin probar aquí), y en unos días decidir
la etapa 6 con los datos del Reporte. Cualquier rareza: registro del
ruteo en Ajustes, `ruteo_log.jsonl` de la máquina y flowLog.

## 2026-08-17 (19) — poda a fondo de CLAUDE.md: de 39.5k a 36.0k, solo reglas

QUÉ: CLAUDE.md rozó el techo de 40k cinco veces en la jornada (cada
cierre obligaba a limar 100-500 bytes). Poda estructural: se quitó la
NARRATIVA (fechas de cuándo se decidió, "autopsia en la bitácora",
"validado en vivo", "pico de 197k invisible", cifras de la anécdota) y se
dejó la REGLA con su puntero al doc. Cambios de forma:
- Los `[x]` de "Estado / pendientes" (rediseño, purga, remediación,
  ruteo, métricas) dejaron de narrar su historia: pasan a una sección
  nueva "Bloques cerrados — reglas duras viven en su doc" (solo lo
  transversal que no puede olvidarse + LEERLO) o a un invariante (#12 =
  reglas vigentes del rediseño). "Integridad de las fuentes" y
  "Retención de logs" se plegaron ahí también.
- "Estado / pendientes" queda con lo VIVO: validación pasiva (ahora
  incluye ruteo y análisis local en una sola viñeta), hub+rangos, Etapa 3
  del análisis local, apuesta #2.
- Auto-updater pasó a una viñeta de "Reglas de comportamiento".
- El bloque de IA del coach se condensó a lo que `analisis-local.md` NO
  dice explícitamente (una invocación por sesión, `emb_debug.txt`,
  `topen==0`); el resto (llama-server, espejo `modelos-v1`, e5 rotos,
  9 constantes SHA-256, `response_format`) ya vive en ese doc — verificado
  con grep antes de recortar.
- `relevo/` entra al árbol de Arquitectura (faltaba).
- Nueva instrucción en la cabecera: al podar, lo cerrado que aún cuenta su
  historia se MUEVE a la bitácora (entrada "poda").

QUÉ SALIÓ (para el grep del futuro — todo está en las entradas de su día):
- Fuentes de datos: la fecha del hallazgo de subagentes (2026-08-04) y la
  cifra "la 3.ª fuente casaba 6 de 14" de `price_key()`; "verificado con
  quota_debug.json real" del plan que no llega.
- Ventanas: "(Oscar 2026-08-14; antes era flyout y estorbaba)" y "la
  'línea de la orilla' era el propio borde --stroke".
- Invariante #1: la fecha 2026-08-05 y "verificado con regresión byte a
  byte" de `end:`.
- Coach: "(2026-08-05) multi-fuente", "validado en vivo, sondeo ~80 ms"
  del exportador viejo, "ETAPA 2 HECHA (2026-08-13)", "banco en bitácora"
  de los e5, "Autopsia en la bitácora" del CTX_LADDER, la fecha 2026-08-15
  del attach/shots, "(2026-08-13)" del compás y "pico de 197k invisible".
- Hallazgos: "(20 h era mucho para los nacidos en el VPS)".
- Auto-updater: "PROBADO DE PUNTA A PUNTA (2026-08-12; autopsias de los
  3 releases en la bitácora)".
- Rediseño: fecha, tag `pre-rediseno-20260805` (sigue en la entrada
  "Ronda de rediseño UX/UI").
- Métricas: "CERRADO HASTA DONDE ESTÁ (Oscar 2026-08-07)", "FILA 18 HECHA
  2026-08-14", "cargo check limpio (2026-08-16)", "caché v3".
- Remediación/ruteo: "4 ETAPAS COMPLETAS Y VALIDADAS EN VIVO
  (2026-08-07/10)", "CERRADO 2026-08-17 (etapas 0-5c)", "Cargo check
  limpio en Windows", los ejemplos de validación por etapa.
- Análisis local: "v1 validada en vivo (5/5 tema_nuevo)", "Probar en el
  Windows de Oscar dio 'embeddings 0.36' — clavado con el banco del VPS",
  "EN VALIDACIÓN PASIVA desde 2026-08-13".
- Purga: "(2026-08-15)".

CÓMO SE VERIFICÓ: `wc -c` 39459 → 35956; diff leído sección por sección
comprobando que cada regla, umbral, constante y "NO hacer" siga (solo
salió narrativa); grep en `docs/analisis-local.md` de cada término
recortado del bloque de IA antes de condensarlo. Sin cambios de código.

QUÉ QUEDA: nada nuevo. Margen ~4k para las próximas jornadas; si vuelve a
rozar, el siguiente candidato es el detalle CSS del gatito (Ventanas), que
podría vivir en un `docs/widget-gatito.md` con puntero.

## 2026-08-17 (20) — el interruptor del modelo TOP: fable (y el que venga) entra al ruteo solo si Oscar lo enciende

QUÉ: casilla propia en Ajustes → ruteo («Usar también el modelo top
(Fable)»), `RuteoCfg.top` en `ruteo.json`, campo `top: "<alias>"` en la
nota (solo encendido), Hook B da el top a subagentes de análisis con cuota
sobrada (<50, `TOP_ROOM`) y el guardián escala hasta él con peso ≥3
(`TOP_PESO`) — opus pasa a ser escalable con umbral 3. Réplicas .py/.ps1
tocadas a la par; `rt_why_top` y textos en 8 idiomas; `get_ruteo_cfg`
devuelve `top_alias` para pintar el nombre real. Docs: ruteo-inteligente
§11 «El interruptor del modelo top», remediacion.md (la frase «JAMÁS a
fable solo» lleva la salvedad), CLAUDE.md (puntero).
POR QUÉ: Oscar (2026-08-17): el ruteo funcionaba en sus tres piezas pero
sin el modelo más caro; quiere un interruptor EXCLUSIVO para «el más caro
e inteligente del momento» (hoy fable; mañana otro) — apagado = como
está, encendido = entra a la lógica igual que los demás. Decisiones: el
alias NO se configura ni se busca en tablas de precios (un alias sin
versión no se sabe cobrar; `price_table("opus")` daría 75 > 50 y elegiría
mal): es el ÚLTIMO de la lista cerrada del relevo, que ya es la escalera
de barato a caro — un modelo nuevo se añade AL FINAL y todo lo sigue.
Umbrales conservadores a propósito: al top solo con cuota SOBRADA (no
«holgada» a secas) y con TRES señales, porque es el que más gasta.
CÓMO SE VERIFICÓ: matriz sintética 29/29 de los dos .py (detalle en el
doc); .ps1 a ojo (sin pwsh aquí), ASCII puro comprobado; sintaxis del
index.html con node. NO verificado: `cargo check` (sin toolchain en el
VPS) ni la casilla en vivo — quedan para el Windows de Oscar.
QUÉ QUEDA: en Windows: `cargo check`, encender la casilla, ver `top` en
la nota y una fila `think-top` real en el registro; se suma a la
validación pasiva del ruteo.

## 2026-08-17 (21) — prueba en vivo del top: los hooks embebidos se refrescan al arrancar; latido dinámico

QUÉ: `ruteo_refresh_scripts()` en el arranque (ruteo ON → reescribe los
scripts locales, SSH y WSL; settings.json intacto); latido del registro
con desglose por destino dinámico (`det`, rtModelName); `insist`/`resent`
anotan `to` (guard_last.json lo guarda), .py/.ps1 a la par.
POR QUÉ: al probar en vivo desde el VPS, con `top:"fable"` ya en la nota
el guardián frenó hacia OPUS: el `~/.michiclaude/guard-hook.py` era la
copia vieja (19:06) — el exe solo la re-subía al encender el interruptor.
Y el latido «0 → Haiku, 0 → Sonnet» no habría enseñado un `→ Fable`.
CÓMO SE VERIFICÓ: en vivo (VPS, `claude -p --resume` de una sesión en
sonnet): prompt pesado → «/model fable y reenvía» sin gastar; el mismo
prompt otra vez → pasa (`insist`, `to: fable`). Hook B real desde esta
sesión (padre fable): Explore → haiku, transcript del subagente lo
confirma. Globo del gatito y registro del panel en Windows lo pintaron
(capturas de Oscar). Matriz 29/29 sigue verde; js ok. NO probado:
`think-top` real (sesión al 50-55 %, umbral <50) ni `cargo check` del
refresco (VPS sin toolchain).
QUÉ QUEDA: cargo check en Windows; `think-top` real tras el reset de
sesión.

## 2026-08-17 (22) — «bajar solo»: la sesión principal también baja a Sonnet sola (con compuerta y cuenta atrás)

QUÉ: interruptor `down` en `ruteo.json` (Ajustes → Ruteo, bajo «reenviar
por mí»; exige «Escalar solo»). Con él, el hecho `light` del motor que
pasa la compuerta de la tarjeta (modelo caro + peor gauge ≥70 + sin 3
«no») se encola (`downQ`, 10 min) y `relayDownCheck` arranca la MISMA
cuenta atrás del auto-/compact (15 s en la cápsula, un toque = «no» vía
`autoRun.onStop`), y al terminar `relay_inject` con `/model sonnet`.
Suelo Sonnet. Sello `relayAuto["down:<sid>"]`. La tarjeta `light` queda
`applied:"now"` (`lgt_applied_now` ×8). Textos `rt_down_*` ×8.
POR QUÉ: Oscar («no solo la sugerencia, también automático» en su día a
día). Preguntado y decidido: SOLO con cuota apretada (misma compuerta que
la tarjeta) y con cuenta atrás visible/cancelable — no silenciosa. Cambia
la regla dura de remediacion.md («/model lo pide el guardián, NUNCA el
panel a ciegas»): ahora hay una excepción CON compuerta, documentada en
§5d; se exige `esc` porque la red que devuelve la sesión arriba es el
guardián.
CÓMO SE VERIFICÓ: js ok (parse); Rust son dos campos con `#[serde
(default)]` (cargo check en Windows, VPS sin toolchain). NO probado en
vivo: hace falta una sesión real en opus/fable con 8 turnos ligeros y
cuota ≥70 — validación pasiva.
QUÉ QUEDA: primera bajada en vivo; ver que en terminal la coreografía del
`/model` (diálogo + restaurar default) sirve igual para bajar que para
subir.

## 2026-08-17 (23) — TEMAS en los hallazgos `inflate`: la tarjeta deja de suponer y demuestra (etapa 3 del análisis local)

QUÉ: implementada ENTERA la etapa 3. `Finding` gana `topics`
(tramos + ahorro + sim_min + sampled) y, solo de transporte,
`umsgs`/`crs`/`usampled`. Motor nuevo en lib.rs: `ai_emb_start` +
`ai_emb_vecs` (extraídos de `ai_emb_sim`, que ahora los usa: UN server
para cientos de mensajes en vez de uno por medida), `topic_split` con
confirmación de frontera, `topic_saved`, caché `inflate_topics.json` y
`topics_for_inflates`. El exportador recoge la misma evidencia
(`topic_sample` réplica exacta). Panel: resumen pegado al costo, chips de
tramos y consejo que SUSTITUYE al genérico, 7 claves ×8 idiomas.
POR QUÉ: Oscar (2026-08-16) «me gustaría que fuera más inteligente aparte
de contar o suponer» — la ficha decía "un /clear al cambiar de tema" sin
saber si hubo cambio de tema, y a una sesión de UN tema el consejo
correcto es /compact.
DECISIONES: (a) interruptor = el del análisis local, SIN casilla nueva
(sin ese modelo la capa no puede correr; una segunda casilla que depende
de la primera confunde) — el texto de la casilla lo dice ahora en 8
idiomas; (b) la evidencia se recoge en la pasada QUE YA EXISTE en vez de
reabrir los .jsonl como decía el diseño: se midió primero (551 mensajes
humanos = 0.17 MB en 9 días), unifica los tres modos en UN camino y
resuelve gratis las sesiones reanudadas (dedup por uuid).
DOS BUGS CAZADOS PROBANDO (los dos con datos reales, ninguno por lectura):
1. El ahorro no era monótono: en una sesión con rupturas de caché, AÑADIR
   una frontera lo bajaba de $38.06 a $14.23. Causa: se tomaba el
   `cache_read` del instante de la frontera y una ruptura lo hace caer
   aunque la conversación siga entera. Arreglo: MÁXIMO CORRIDO,
   conservando el tope por turno. 1200 combinaciones de cortes sobre las
   sesiones del VPS: 0 fallos.
2. El centro del primer tramo salía de un mensaje que NO vota si el
   primero era corto ("dale"). Arreglo: lo define el primero que vota.
CÓMO SE VERIFICÓ (el VPS no tiene toolchain de Rust NI el GGUF): banco del
algoritmo portado línea por línea a Python con vectores de similitud
CONOCIDA, 22/22; render del panel 22/22 con el ESCAPADO de las etiquetas
(son texto del usuario: se probó con `<img onerror>` y `<script>`);
exportador corrido de verdad contra los logs del VPS (7 inflates, umsgs y
crs correctos, ordinales crecientes, recorte a 300 chars); y REGRESIÓN con
ventana congelada (`--end`): hallazgos y `waste` IDÉNTICOS byte a byte sin
las claves nuevas.
QUÉ QUEDA: `cargo check` en el Windows de Oscar y la primera tarjeta con
temas REALES (el modelo de embeddings vive ahí). Umbrales
`TOPIC_NEW`/`TOPIC_HOLD`/`TOPIC_MIN_MSGS` son constantes A PROPÓSITO hasta
tener muestra real, como los de la etapa 2.

AÑADIDO EL MISMO DÍA (23b): los temas se movieron a una SEGUNDA PASADA de
fondo. Metida en `get_findings` a secas, la pestaña habría esperado la
carga fría del modelo (~20 s) más el presupuesto de 25 s antes de pintar
NADA. Ahora `loadFindings` pinta sin temas y `fndTopicsLater()` repite con
`topics:true` solo si `ai_get_config` dice que el análisis está encendido;
descarta si cambió el periodo o entró un escaneo más nuevo. Tercer bug del
día cazado verificando: la función usaba `$("tab-fnd")`, id que NO EXISTE
(es `tab-findings`) — un repintado que nunca habría ocurrido. Se comprobó
con un barrido de TODOS los `$("…")` del archivo contra los `id=`
declarados: ninguno huérfano.

## 2026-08-19 (y madrugada del 20) — arranca la validación pasiva CON EL EXE: nace el checklist dev/release, el vigía y R6

QUÉ:

- **`docs/validacion-pasiva.md`**, checklist vivo a DOS COLUMNAS (Dev /
  Exe) con 88 filas en 12 áreas. Cada fila dice qué se probó en desarrollo
  —distinguiendo `✅` en vivo de `🧪` solo simulador— y qué se ha
  confirmado ya con el instalador. Al cierre: **32 ✅ en Exe**, 4 a medias,
  52 pendientes.
- **Bitácora PRO** (`feat`, commit 287b559): fila propia en Ajustes que
  copia `flowLog` desde el exe. `flog()` ya grababa siempre en release; lo
  único escondido era el botón 📜, que vive en la fila del simulador.
  Copia por el portapapeles de Tauri (bajo la CSP de release
  `navigator.clipboard` no es de fiar), Mayús+clic la vacía, 8 idiomas.
- **R1 y R2 arreglados** (commit cfbe369): el marcador de desbloqueo se
  esconde al cumplirse los cupos (enseñaba "/clear 8 de 3" y prometía un
  desbloqueo ya ocurrido) y el recibo concuerda en plural ("1 archivo
  editado"). En R1 se DESCARTÓ capar el número y añadir una frase nueva en
  8 idiomas: la señal de desbloqueo ya la dan los candados al irse.
- **Vigía** (`~/.michiclaude/vigia.py`, FUERA del repo por ser herramienta
  personal): lee cada 60 s los rastros del VPS (`router_state.json`,
  `coach_debug.json`, `ruteo_log.jsonl`, `relevo/*.json`, `handoff/`) y
  anota en `vigia.log` solo lo que cambia, con una línea "→ en el panel:"
  que dice qué debería verse. No habla con la API: cero cuota.
- **Plan por fases** acordado con Oscar: apagar TODOS los interruptores y
  encenderlos de uno en uno, lo manual antes que lo automático.

POR QUÉ:

- Oscar empezó a usar el instalador como usuario y a mandar capturas; sin
  un sitio donde marcarlas, las validaciones se perdían. El doc nació de
  ahí y la separación dev/exe la pidió él para poder CASAR una con otra.
- La bitácora PRO nació de un atasco real: R3 no se podía diagnosticar
  porque el flowLog es de dev y los debug del panel viven en el AppData de
  Windows, que desde el VPS no se ve.
- El plan por fases salió de una premisa equivocada de Oscar ("integrar
  poco a poco lo de dev al exe") que hubo que corregir: dev y el exe son
  EL MISMO código. Lo que sí tenía sentido escalonar eran los
  interruptores, y el criterio "manual antes que automático" coincide con
  el que la app ya aplica en su compuerta de aprendizaje.

CÓMO SE VERIFICÓ (todo en release, sin simulador):

- Cayeron por primera vez fuera de dev: **la alarma real** ("Sesión al 10%
  · Reset en 3 h 36 min" con `cat-fire`), el globo anclado, los post-its
  rojo y turquesa a la vez cuadrando con los badges, el hallazgo naciendo
  solo, el recibo `sum` completo, la ficha del coach sobre una sesión
  REMOTA por SSH, "Copiar comando" → "Copiado ✓", y la ficha de contexto
  al hover con origen y marca de relevo.
- El **contexto inyectado del ruteo** se vio funcionando en un chat real:
  el Claude de `sparky-site` sugirió bajar a Sonnet por su cuenta. Se
  anotó separado del consejero `light`, que es otra pieza.
- La **bitácora PRO** cerró R3 en una pegada: la capa de temas va
  llenándose pasada a pasada (`temas listos en 1 sesión(es)` con dos
  tarjetas, `en 2` con tres más tarde) — es el presupuesto de 25 s
  trabajando, no un fallo. Y demostró que **ntfy publica** (umbral, "terminó"
  y el `ask` de la herramienta colgada), que "Aplicar" del panel se
  distingue de lo tecleado a mano (`aplicado /clear en pid 4122038` + su
  copia a la misma hora) y que el compás llega a la rampa de 10 s.

DOS ERRORES PROPIOS, con su autopsia:

1. **Falsa alarma del 93%.** El vigía nació con el techo de contexto
   clavado en 200k mientras la app da 1M a `claude-opus-5`; anunció "93%,
   urgente" cuando eran 19%. Prueba de que la app acertaba: la
   auto-compactación de Claude Code (~94%) habría entrado a 188k y la
   sesión pasó de 185k a 197k sin compactar. Regla que queda: **cuando el
   vigía y el panel discrepen, el sospechoso es el vigía** — él trabaja con
   una COPIA de las reglas (`ctx_table`, umbrales) y la app tiene el dato
   de primera mano.
2. **"Con ntfy apagado no te enteraste"** — dicho sin comprobarlo. La
   bitácora enseñó el `push ok` del `ask`. Estaba encendido.

QUÉ QUEDA:

- **R6, el hallazgo de la jornada** (ya en CLAUDE.md §Estado): las reglas
  de presión miden % del techo, y con 1M eso son 600k/800k de contexto —
  inalcanzables. La presión máxima medida en 11 días de bitácora fue
  **23%**. Duermen manómetro, ficha de compactar, tarjeta de intención, el
  ⚠ `ctx` de fugas y TODOS los automáticos; siguen vivas las reglas de
  umbral absoluto. Solo se ve USANDO la app: en dev lo tapaba el simulador.
- R4 (dos `inflate` con 61k tok idénticos), R5 (el primer prompt de cada
  sesión va sin modelo, así que escapa al guardián) y R7 (una alarma deja
  3-4 renglones de globo, con un solo push: falta ver si en pantalla sale
  uno o varios).
- El plan por fases, sin arrancar. Fase 1 = apagar todo.

## 2026-08-21 — gating v1: la app decide qué enseña para el lanzamiento del 23

QUÉ:

- **Respaldo previo por dos vías**: etiqueta `respaldo-2026-08-21` empujada
  a GitHub y `michiclaude-2026-08-21.tar.gz` (83 MB, sin targets ni
  node_modules) en `/opt/projects/respaldos/` del VPS.
- **Gating v1** en index.html: dos listas (`V1_HIDE`, `V1_SOON_TABS`) y una
  función `applyV1()` que corre al arrancar y en cada `applyI18n()`.
  Escondidos con `hidden` (invariante 10bis los blinda): IA local
  (`aiSect`), remediación+purga+relevo (`remSect`, comparten tarjeta),
  ruteo (`rtSect`) y config compartida del HUB (`hubCfgSect`). La pestaña
  **Reporte** queda visible pero gris (`.tab.soon`), con tooltip
  "Próximamente" (`tab_soon`, 8 idiomas) y sin abrir por ninguna vía
  (guardia al inicio de `showTab`).
- **Gesto de desbloqueo**: Mayús+clic en "MichiClaude vX.Y" (Acerca de)
  alterna `v1all` en localStorage y repinta al momento — Oscar puede
  validar el modo completo con el MISMO exe, sin recompilar. Si se apaga
  con una pestaña gateada abierta, salta a Principal.
- **ntfy queda VISIBLE**: Oscar confirmó hoy que los pushes llegan a su
  celular — la fila del checklist pasa a ✅ (21/08) y el bloque de Avisos
  se queda entero en la v1.
- Regla VIGENTE anotada en CLAUDE.md §Estado; fila de ntfy marcada en
  `validacion-pasiva.md`.

POR QUÉ:

- Lanzamiento objetivo 2026-08-23: dar la app por partes para que los
  usuarios se familiaricen sin revolverse, y que instalen ANTES de los
  cambios de cuota de Anthropic de septiembre (el Reporte gris es el
  teaser: sus datos se juntan desde el día uno y una update lo enciende
  con el "antes" ya guardado).
- Criterio de la tabla acordada con Oscar: ✅ visible = solo lee y está
  validado en exe; 🔒 gris = solo lee pero sin validar (se anuncia);
  🙈 oculto = toca sesiones o archivos del usuario, o depende de R6.
  Mensaje del lanzamiento: "MichiClaude solo mira, nunca toca".
- Se DESCARTÓ deshabilitar-visible para todo (idea inicial de Oscar): un
  bloque gris de automáticos anuncia justo lo que no se quiere prometer
  aún, y cada explicación son 8 traducciones. El híbrido (solo Reporte
  gris) da el teaser sin el ruido. También se descartó esconder Hallazgos
  o Consejos: están validados en exe y son el diferencial.
- `var` y no `const` para las listas del gating: `showTab` las consulta
  con `typeof` y con `const` en TDZ el `typeof` lanza en vez de responder.

CÓMO SE VERIFICÓ:

- `node --check` limpio sobre el script extraído de index.html (619k).
- Sin cambios en Rust: no aplica cargo check. Pendiente la pasada visual
  en el exe de Windows (recompilar allí; ojo a la trampa de la hora del
  binario tras el pull).

QUÉ QUEDA:

- Probar en release: pestañas visibles completas, Reporte gris con
  tooltip, Mayús+clic alterna todo, cambio de idioma repinta el tooltip,
  y la primera ejecución en un Windows limpio (nunca probada desde cero).
- R7 antes del anuncio (¿un globo o varios en pantalla?); redactar el
  anuncio con la transparencia por delante; decidir si la update de
  septiembre que enciende el Reporte va acompañada del relevo manual.

## 2026-08-21 (2) — tokens primero, dólares después: Principal deja claro qué mide

QUÉ:

- **Gasto por proyecto**: el número grande de cada fila pasa a ser los
  TOKENS (`fmtTok`, el mismo formato k/M de los hallazgos) y el $ baja a
  segunda línea, atenuado. La barra ya no es proporcional al coste sino a
  los tokens.
- **Total de la ventana**: tokens en el display grande con la unidad en
  cuerpo chico (`.st-u`), y debajo `$X USD` — la palabra USD escrita, que
  era la duda original de Oscar ("aclara que son dólares").
- **La gráfica se llamaba "Tendencia diaria"** y no decía de qué: ahora es
  **"Consumo por día"** (8 idiomas) y dibuja TOKENS. El globo da las dos
  cifras: `21/Ago/26 · 12.4M tok · $219.00`.
- Demo del navegador enriquecido (proyectos con tokens y serie diaria de
  30 días) para poder ver la pantalla sin compilar.

POR QUÉ:

- Oscar, mirando el panel del build de lanzamiento: "$394.45 a secas no
  dice ni que son dólares", y "¿tendencia diaria de qué?". Decidió él el
  orden: **la prioridad son los tokens, luego el $**.
- Encaja con la doctrina de la casa: el $ es NOCIONAL (equiv. API) y la
  suscripción se gasta en tokens, así que el dato honesto es el que ahora
  manda en la pantalla. El invariante #8 gana su 8bis.
- El RESPALDO de la gráfica no es adorno: `DailyAgg.tokens` lleva
  `#[serde(default)]`, así que un exportador viejo en un servidor manda la
  serie con tokens en 0 — sin el respaldo la gráfica saldría plana con
  gasto real. Si no hay tokens en toda la serie, se dibuja el coste.
- Se DESCARTÓ reordenar la lista por tokens: el orden lo fija el backend
  (y el exportador, invariante #1) y tocarlo obligaba a mover dos motores
  para un cambio de pintura. Queda anotado que la barra puede romper el
  orden — es información, no un fallo: ese proyecto usó un modelo más caro
  por token.

CÓMO SE VERIFICÓ:

- `node --check` limpio sobre el script extraído.
- **Render real headless** (chromium sobre `index.html` en modo demo): el
  total sale "31.0M tok / $47.20 USD", las filas con tokens arriba y el $
  debajo, las barras proporcionales a los tokens y la gráfica bajo el
  título "Usage per day". Sin compilar en Windows todavía.

QUÉ QUEDA:

- Verlo en el exe con datos reales (los proyectos de VPS-EU) y confirmar
  que a 446 px las dos líneas del total no aprietan la nota de privacidad.

## 2026-08-21 (3) — el aro de foco, la huella de marca y la memoria de la conversación

QUÉ:

- **Arreglado el recuadro blanco sobre el gatito** (`d19daae`): la zona
  `.head` es un `<button>` de verdad, y al clicar WebView2 pintaba su aro
  de foco, que sobre un gif transparente se lee como una línea blanca.
  `button:focus{outline:none}` en las CINCO ventanas del widget; el panel
  conserva el suyo (ahí sí se navega con teclado).
- **«Presión de contexto» → «Memoria de la conversación»** (`e0c1cd1`), 8
  idiomas. Se alinearon además las dos frases largas que hablaban del
  «manómetro de presión» (tooltip de precios y ficha `acomp` del coach):
  dejar dos nombres para lo mismo era justo la confusión que se quería
  quitar. Motor y umbrales intactos.
- **Marca del panel: la HUELLA.** El sol de rayos (`sunburst`) se cambia
  por `pawMark()`, SVG dibujado en código igual que el anterior: sin
  archivo nuevo, nítido a cualquier tamaño y con `var(--brand)` en los dos
  temas. Oscar puso además su propia huella como `app-icon.png` (1024²).

POR QUÉ:

- La huella y no el gatito: **Bongo Cat tiene dueño**. Investigado hoy —
  el arte original es de @StrayRogue (7 mayo 2018), @DitzyFlama hizo la
  versión con bongos, y StrayRogue **vende merchandising oficial** del
  personaje. El permiso que circula es específico (por escrito, a la web
  bongo.cat en 2018), y el MIT de esos repos cubre el CÓDIGO, no el
  dibujo. Como mascota dentro de la app, con la excepción del LICENSE,
  es la zona templada; como ICONO y LOGO sería la cara de un producto que
  se va a promocionar. Una huella dice "gato" sin usar a nadie.
- El nombre nuevo salió de Oscar usando la app: "presión de contexto está
  algo confuso". El usuario objetivo no sabe qué es una ventana de
  contexto, pero sabe que una memoria se llena.

CÓMO SE VERIFICÓ:

- `node --check` en las seis ventanas y **render headless** de la huella a
  160/44/22 px y en el encabezado real: lee como huella al tamaño que
  importa (22 px). Se afinó una vez (dedos +0.5, almohadilla más
  estrecha) tras ver el primer render.
- **La validación del post-it se cerró SOLA**, sin montar el proyecto de
  prueba que se iba a fabricar. Bitácora PRO de Oscar, 22:03:03-04:
  `nace tarjeta sum` → `fnd: pasada por cierre de sesión ok, 2 tarjetas
  (1d)` → `fnd: AVISO ENCENDIDO (1 sin ver, de 2)`, y a las 22:11 los dos
  avisos apagándose al clicar cada tarjeta. La cadena entera en 2 s. Dos
  filas del checklist marcadas; la sospecha de "el post-it no funciona"
  era la CADENCIA (pasada ligera de 1 día: al nacer un recibo, o cada 3 h).

QUÉ QUEDA:

- Oscar: `npm run icons` en Windows con su app-icon.png y COMMITEAR los
  iconos (el updater los necesita commiteados, invariante del auto-update).
- Opcional: `src/icon-mini-panel.png` (la marca del `pcard`) sigue siendo
  el sol; se puede sustituir copiando `src-tauri/icons/128x128.png`.
- R7 sigue abierta (¿un globo o varios en pantalla?).

## 2026-08-21 (4) — la huella se vuelve marca: cápsula, detalle e instalador

QUÉ:

- **La marca de la pastilla y del detalle pasa a ser la huella.** Eran dos
  PNG (`sticker-black/white.png` en la cápsula, `icon-mini-panel.png` en el
  `pcard`) y ahora es el MISMO SVG del panel, con `fill:currentColor`: el
  tema lo resuelve el CSS y `syncMk()` se queda sin nada que sincronizar
  (se conserva vacía porque la llama el ciclo del tema).
- **Arte del instalador** en estilo cómic: `scripts/make-installer-art.py`
  (solo stdlib) dibuja `src-tauri/installer/header.bmp` (150x57) y
  `sidebar.bmp` (164x314) — azul #251590 con lunares y la huella en blanco,
  con supermuestreo x3 porque NSIS no suaviza. `tauri.conf.json` los
  referencia junto a `installerIcon`.
- Los gifs del gatito NO se tocan: la mascota sigue siendo el gato; la
  huella es la MARCA (icono, logo, cápsula, instalador).

POR QUÉ:

- Oscar quiso unificar: huella en el icono del exe, en el panel, en la
  cápsula y en el instalador. Y de las dos maquetas HTML que pasó (una
  azul-lavanda, otra crema-ámbar) pidió **un solo color**: se eligió el
  azul, que es el de la app.
- El arte se DIBUJA en un script en vez de exportarse de un editor: NSIS
  exige BMP de 24 bits con tamaños fijos, y así el instalador se regenera
  cuando cambie la marca sin depender de un archivo que solo existe en la
  máquina de alguien. Misma geometría que el SVG del panel (viewBox 64) a
  propósito: si cambia la huella, cambia en los dos sitios o se nota.

TRAMPA QUE MORDIÓ (y queda anotada): un `<button>` **no hereda el color
del texto** en Chromium — le pone el suyo, negro. La huella de la cápsula
se pinta con `currentColor`, así que salía NEGRA sobre el cristal oscuro.
Se arregla con `color:var(--txt-strong)` en `.mkbtn`. Con el `<img>`
anterior el problema no existía: por eso no estaba.

CÓMO SE VERIFICÓ:

- `node --check` en las dos ventanas y **render headless a 3x** de la
  cápsula: la huella sale blanca sobre el cristal, del tamaño del gatito
  que sustituye. El arte del instalador se revisó abriendo los BMP en el
  navegador.
- Lo que NO se puede verificar aquí: cómo se ven los BMP dentro del
  instalador real. Eso sale en el `npm run build` de Windows.

QUÉ QUEDA:

- **El aspecto cómic COMPLETO del instalador no es posible con NSIS**: las
  maquetas HTML llevan botones, tipografías y bordes propios, y NSIS dibuja
  controles NATIVOS de Windows. Lo que sí cambia son las dos imágenes, el
  icono y el título. Un instalador con esa piel exige una app instaladora
  propia — proyecto aparte, no para esta semana.

## 2026-08-21 (5) — la marca queda en un solo dibujo: huella naranja sobre azul

QUÉ:

- `scripts/make-installer-art.py` crece: además de los dos BMP del
  instalador genera **`app-icon.png` de 1024²** con esquinas redondeadas
  (radio 22%, el de Windows 11) y fondo transparente fuera de la pastilla.
  Se corrió `npm run icons` y el juego entero de iconos va commiteado.
- **Colores de la app, no inventados**: fondo `--card` #151F3A y huella
  `--brand` #E08B63, copiados de index.html. El icono casa con el avatar
  del panel porque es literalmente la misma pareja de colores.
- Fuera los lunares del instalador: Oscar pidió el fondo plano de su
  captura. Queda una sola marca —misma geometría, mismos colores— en el
  icono del exe, el instalador, el panel, la cápsula y el detalle.

POR QUÉ:

- Oscar mandó una captura del avatar del panel: "esta huella naranja y ese
  fondo, en todos lados". Antes el icono era su PNG azul claro y el
  instalador azul-violeta: tres marcas distintas para la misma app.
- El PNG se dibuja con `zlib` + `struct` de la stdlib (encoder propio de
  ~10 líneas, filtro 0 por fila). Cero dependencias nuevas.
- Detalle que se cuidó: al promediar el supermuestreo, el borde de la
  pastilla mezcla color y alfa; sin premultiplicar tiraba a negro sobre
  fondos claros.

CÓMO SE VERIFICÓ:

- Render headless del icono a 160/48/32/**16 px**: la huella sigue
  reconociéndose al tamaño de la barra de tareas, que es donde mueren los
  iconos con detalle fino. También los PNG que genera `tauri icon`.
- Los BMP se abrieron en el navegador. Falta verlos DENTRO del instalador
  real: eso sale en el build de Windows.

QUÉ QUEDA:

- Oscar: `git pull` y `npm run build`. Ya NO tiene que correr
  `npm run icons` — los iconos van commiteados desde el VPS.
- El icono viejo del escritorio es caché de Windows, no del build.

## 2026-08-22 — v0.2.0 PUBLICADA: el primer release para usuarios, con su cara y su historia

QUÉ:

- **Release v0.2.0 publicado y firmado** (tag de Oscar, workflow verde en
  ~12 min): instalador 7.4 MB + .sig + latest.json correcto. Notas de
  release bilingües escritas y publicadas: qué es, qué trae, **qué viene
  apagado a propósito** y transparencia (endpoint no oficial dicho por
  nosotros; crédito a Bongo Cat).
- **README de lanzamiento** (ambos idiomas): fila de badges (descarga,
  versión auto, GPL, Discussions, LinkedIn), badge de **beta**, bloque
  "qué abre la v1" (solo mira, nunca toca), y **14 imágenes** de Oscar
  colocadas donde explican algo — gif principal por idioma, 3 de
  Principal, fugas, recibo del coach y gif de estados del gatito.
  Discussions activado. Roadmap poda "capturas" (hecho).
- **Camino del lanzamiento en Facebook** acordado con las ENCUESTAS de
  Oscar como base (58% Windows nativo, 46% /clear a mano + 15% "me entero
  cuando ya se acabó", 87% conoce el límite inflado hasta el 31/08):
  post para los grupos con esa narrativa, y post fundacional de la página
  IA Sparky **ya publicado y fijado**. Grupo: sábado 9:30-11:00 CDMX.

POR QUÉ:

- El desajuste README-vs-app gateada era el riesgo de confianza del día
  uno: el README describe Reporte/automáticos/IA que la v1 esconde. El
  bloque nuevo lo convierte en promesa ("se abrirá validado") en vez de
  sorpresa.
- Las cifras reales de las capturas ($1,324/15d, proyectos con nombre) se
  quedan A PROPÓSITO: Oscar decidió que la honestidad vende más que el
  pulido. El dato de los $34 del inflate de 269 turnos pasó al post como
  prueba verificable.
- "No retar" al grupo: el post cerró en agradecimiento (las encuestas le
  dieron forma) y Mac quedó como puerta abierta según interés — sirve de
  termómetro para decidir el port.

QUÉ QUEDA (validación en curso, no bloqueó el release):

- R7 (¿un globo o varios?), reconfigurar ntfy/VPS-EU si el desinstalador
  los borró, ensayo de descarga desde el enlace público, y el post del
  grupo el sábado por la mañana con Oscar respondiendo la primera hora.
- Pendientes acordados: LinkedIn (texto largo ya redactado), página
  iasparky.com como portafolio enlazado, respuestas preparadas para
  "¿y Mac?", "¿mi token?", "¿vs ccusage?".

## 2026-08-24 — jornada de validación en modo usuario: 13 filas cerradas y la tanda de 7 rarezas (R5 a R13), R6 incluida

QUÉ. Dos cosas, en este orden. (1) Validación pasiva con Oscar usando el
exe como usuario y mandando capturas: cayeron 13 filas del checklist —el
`/clear` tecleado por ti detectado y contado, el globo `cleared` con su
visor, el ✕ que despacha una ficha, el post-it turquesa, restaurar un
hallazgo ignorado, el export CSV verificado en Excel, el idioma llegando
al globo del gatito, el globo resumen con buckets por modelo, el
presupuesto semanal, la INTEGRIDAD («comparación no concluyente»), la
capa sobre pantalla completa, el globo popover con la pastilla, y el
Reporte entero en sus tres modos— más tres filas de RUTEO que el propio
Reporte demostró medidas (Hook B, guardián con 3 frenados y 1
insistencia, `scan_ruteo`). (2) La tanda de arreglos: R5, R6, R8, R9,
R10, R11, R12 y R13.

POR QUÉ, una por una:

- **R6 (la gorda).** Las reglas de presión medían SOLO % del techo. Con
  Opus/Sonnet de 1M, el 60% son 600k y el 80% son 800k: inalcanzables
  (presión máxima real medida en 11 días, 23%). Dormían la ficha de
  compactar, la tarjeta de intención, el ⚠ `ctx`, el color del manómetro
  y los AUTOMÁTICOS — justo en los modelos que más cuestan. El daño de un
  contexto grande es ABSOLUTO: releer 200k cuesta lo mismo con techo de
  200k que de 1M. Ahora entra lo que ocurra ANTES: `ctx_alto()` (60% ∨
  150k) en Rust y su réplica en el exportador, `pressHot()` (80% ∨ 200k)
  y `pressLevel()` en el panel, y el compás del coach con los mismos
  suelos. El DIBUJO no cambia (sigue siendo % del techo, que es lo
  honesto); cambia CUÁNDO avisa. El `lvl` se decide UNA vez en el panel y
  viaja dentro de `press`: que pill/pcard/cat recalcularan cada una su
  60/85 era el mismo bug escrito tres veces.
- **R12.** El Reporte enseñaba dos totales del mismo periodo ($2418 arriba,
  $2381 en desperdicio). Se midió en el VPS: los dos caminos coinciden AL
  CÉNTIMO en una máquina (7 d y 30 d, diferencia 0.0000). El hueco lo
  ponía el HUB, que manda su foto agregada con `waste` en ceros — la de
  OSCAR-HUAWEI suma $37.03 en 30 d, exactamente el hueco. Se descartó la
  primera hipótesis (desfase de instantes) con esos números. El
  denominador pasa a ser el total del héroe: el % sale más conservador,
  que es el único lado seguro para un dato que se anuncia como «al menos».
  Contradice el «mismos orígenes» de la fórmula, así que queda ESCRITO en
  presion-y-rendimiento.md, no escondido.
- **R10.** «Esta semana» daba 5.3M/$164 arriba y 3.9M/$117 en la gráfica:
  ventana de 7×24 h rodando (con la cola del 17/08, un día de 4.5M) contra
  7 días naturales. Las ventanas del Reporte terminan ahora al cierre del
  día, en UTC porque así agrupa la serie diaria. El motor no se tocó.
- **R8.** La compuerta de aprendizaje del relevo vivía en localStorage y
  la desinstalación con «borrar datos locales» del 21/08 se la llevó
  entera (de `/compact 2 de 2 · /clear 8 de 3` a `0 de 2 · 1 de 3`, con el
  automático bloqueado otra vez). Es confianza GANADA: ahora vive en
  `relay_gate.json` y localStorage queda de espejo; la primera carga sube
  lo que hubiera como `seed` y Rust fusiona por el máximo.
- **R9.** `flog()` sellaba en UTC y la interfaz en local: el mismo /clear
  salía como «18:37» en la Bitácora PRO y «12:34 p.m.» en el registro.
- **R13.** El presupuesto semanal guardaba en el evento `change`, sin
  confirmación, al lado de unas alarmas que sí tienen chips y botón.
  Ahora tiene la misma forma; el botón reusa `cal_apply` (cero texto
  nuevo en 8 idiomas).
- **R11.** «DESPERDICIO ESTRUCTUR…»: la cabecera mutilaba el título para
  salvar la coletilla. Ahora envuelve.
- **R5.** El primer prompt de cada sesión escapaba al guardián (13 de 16
  eventos sin modelo eran eso). Último recurso: el modelo que esa carpeta
  ya usaba (`lastModelUsage`), y SOLO si hay uno; si aun así no se sabe,
  `ev:"noeval"` en el registro — un agujero contable se puede medir, uno
  silencioso no.

CÓMO SE VERIFICÓ. `node --check` limpio sobre el JS de las seis ventanas;
`ast.parse` limpio en los dos .py; el respaldo de R5 probado a mano (una
carpeta con un modelo devuelve el modelo, una con varios devuelve None);
R12 medido con el exportador real en el VPS (tabla de arriba). **`cargo
check` PENDIENTE en el Windows de Oscar**: el VPS no tiene toolchain de
Rust — se tocó `lib.rs` (comando `relay_gate` nuevo, `ctx_alto()`, dos
llamadas sustituidas) y se hizo grep de todos los usos, que es lo que
manda CLAUDE.md cuando no hay compilador.

QUÉ QUEDA. R6 está ARREGLADA pero no VALIDADA: falta ver la primera ficha
de compactar y la primera tarjeta de intención con un modelo de 1M en uso
real. Los automáticos siguen APAGADOS y la compuerta pide 2 `/compact` y 2
`/clear` manuales: la fase 4 no puede empezar hasta que Oscar los aplique
desde el panel. R4 y R7 siguen abiertas (verificar), el HUB sigue
bloqueado sin segunda máquina, y el bloque B del reparto —alarmas
repetidas, semanal al 100%, ntfy con la PC apagada, `cat-zzz`/`cat-break`,
caducidad de 24 h— solo se puede cerrar esperando a que ocurra.

## 2026-08-25 — R14 y R15 cerradas: el último texto en inglés y el atajo que parecía roto; la fase 4 arranca

QUÉ. Jornada corta de validación en modo usuario, dos rarezas y una poda.
(1) **R14** cerrada del todo: el menú del tray ya sale en español —«Abrir
panel · Widget flotante · Salir»— y con eso NO queda un solo texto de la
app en inglés teniendo la interfaz en español. (2) **R15**: el interruptor
«Hacer que «claude» pase por el relevo» APAGADO mientras el relevo
funcionaba; no era fallo, era un texto que no acotaba su alcance, y se
acotó en los 8 idiomas. (3) La **compuerta de aprendizaje se reganó** y
Oscar encendió los dos automáticos: empieza la fase 4. (4) CLAUDE.md
volvió a pasar de 40k y se podó.

POR QUÉ, una por una.

**R14 — el arreglo era de HILO, no de idioma.** El panel siempre mandaba
las tres etiquetas traducidas con `set_tray_menu`; lo que fallaba es que
un comando de Tauri corre en el pool del runtime y en Windows los menús
nativos solo se pueden crear y asignar desde el hilo del bucle de eventos.
`set_menu` se negaba EN SILENCIO —sin error que devolver al `.catch()`—
así que el panel creía haber cumplido. Dos cinturones: `run_on_main_thread`
en Rust, y en JS un `sendTrayMenu()` aparte que `updateTray` REINTENTA la
primera vez que el tray responde (al arrancar, el icono puede no existir
cuando el panel pinta, y como el idioma no vuelve a cambiar en toda la
sesión el menú se quedaba en inglés para siempre). El TOOLTIP, que se
sospechó roto, nunca lo estuvo: seguía al idioma elegido y el idioma era
inglés. Esa sospecha quedó como HISTORIAL en `docs/validacion-pasiva.md`
§R14 — se documentó el síntoma y se esperó al dato antes de tocar nada.

**R15 — el alcance escrito solo en el doc no evita la confusión.** Oscar
trabaja en el chat de VS Code contra el VPS. Vio el atajo del PATH apagado,
la lista enseñando `michiclaude · VPS-EU · chat · listo` y la bombilla
midiendo 7% de contexto, y lo dio por fallo. Son TRES puertas distintas al
relevo y esa sesión entra por otra: el atajo (`set_relay_alias`) escribe un
shim en el PATH de USUARIO de Windows, mientras el chat de VS Code y las
terminales Linux tienen interruptor propio —los dos encendidos—. El
alcance ya estaba en `docs/remediacion.md` §"El atajo del PATH" («NO cubre
WSL desde dentro ni SSH»), pero la etiqueta decía solo «Hacer que «claude»
pase por el relevo» y la nota «cualquier terminal». Arreglo de texto en
los 8 idiomas: «(terminales de Windows)». Se decidió meterlo en la
ETIQUETA y no en la nota porque sin servidores dados de alta las filas del
chat y de las terminales Linux están OCULTAS (`rlyChatRow`/`rlyTermRow`) —
una nota del tipo «SSH y WSL tienen su propio interruptor» señalaría algo
invisible (invariante #8). LECCIÓN: si el interruptor de al lado hace algo
parecido, el alcance va EN la etiqueta.

**La compuerta y la fase 4.** Tras el borrón de R8 (la reinstalación se
llevó `relayDone` de localStorage) la cuenta volvía a 0/2 y 1/3. Oscar
aplicó los que faltaban y el marcador desapareció, que es la señal de
desbloqueo por diseño (R1). De paso se corrigió algo que se le había dicho
mal: cuentan las DOS vías, el comando TECLEADO por él —el relevo lo detecta
y lo apunta, index.html:11463— y el botón «Aplicar» (11606); no hacía falta
pasar por el panel. Con los cupos cumplidos encendió auto-`/compact` y
auto-`/clear`, y dejó apagado el `/clear` por análisis local (pide la IA
local, que sigue apagada). El ruteo entero sigue apagado: es la fase 5.

**Poda de CLAUDE.md.** Llegó a 40.375 caracteres, por encima del techo
duro. Se condensaron dos bloques CERRADOS cuyo detalle ya vive en su doc
—«TEMAS de inflate» (→ `docs/analisis-local.md` §Etapa 3) y «Modo HUB» (→
`docs/hub-modo-equipo.md`)— dejando en cada uno el puntero y lo que no
puede olvidarse, y el párrafo de R6 se fundió en el pendiente de
validación. Queda en 39.789.

CÓMO SE VERIFICÓ. R14 **visto en pantalla** por Oscar sobre el exe
recompilado: menú en español. Su build dejó un `warning: unused import:
tauri::Manager` que se quitó — `run_on_main_thread` es inherente de
`AppHandle` y no pide el trait. R15 es solo texto: `node --check` limpio
sobre el bloque de script del panel (los 627k de `index.html` extraídos y
comprobados aparte), y **visto en pantalla** tras el `git pull` + build de
Oscar («Compiling michiclaude», 7m56s: no cayó la trampa de la hora del
binario). ntfy REconfirmado en el celular sobre el build nuevo — el canal
sobrevive a la reinstalación porque el topic vive en `ntfy_config.json`,
no en localStorage. Sin `cargo check` en el VPS, como siempre.

QUÉ QUEDA. La fase 4 está ARMADA pero SIN VER: falta la primera cuenta
atrás de 15 s en la cápsula, que diga el comando, que un toque la pare, que
el `/clear` deje su copia verificada y que el comando aterrice. R6 sigue
arreglada y sin verla disparar. El ruteo (fase 5) ni se ha tocado: si algo
suyo «no dispara», la causa es el interruptor, y el estado VIGENTE de todos
ellos vive ahora en `docs/validacion-pasiva.md` para no volver a
perseguirlo. Sigue esperando el bloque B —alarmas repetidas, semanal al
100%, ntfy con la PC apagada, `cat-zzz`/`cat-break`, un 429 real,
caducidad de 24 h—, R4 abierta y el HUB bloqueado sin segunda máquina.

## 2026-09-02 — R16: el auto-/clear decapitaba procesos autónomos; compuerta de reposo + tarifas de los modelos nuevos

QUÉ: compuerta de REPOSO para el auto-/clear (`AUTO_REST_MIN = 5` en
`relayAutoCheck`, index.html): sin `quiet` ≥5 min el veredicto Boundary
se degrada a /compact y JAMÁS se borra; rastro nuevo en la Bitácora PRO
("[sin reposo: /clear degradado, quiet X min]"). Y tablas de respaldo al
día con los modelos de septiembre: Sonnet 5+ a $2/$10 (la introductoria
se hizo permanente) y lectura de caché de Fable/Mythos 5.1+ a $0.25/M en
absoluto (0.025×, no el 0.1× estándar) — en `price_table()` de lib.rs y
`price_for()` del exportador (invariante #1). R16 con autopsia en
validacion-pasiva.md; regla en remediacion.md §"El auto-/clear con red"
y en el resumen de CLAUDE.md.
POR QUÉ: mordida real de Oscar (bitácora del flujo, 09-02 12:39:56):
trabajando en polymarket-bot (chat VS Code sobre SSH, modelo de 1M) un
análisis autónomo largo pasó de 200k absolutos, el clasificador dio
Boundary — el proceso cierra su lista de TODOs y commitea A MITAD del
trabajo, así que "tarea cerrada" salía con Claude generando (quiet 0) —
y el /clear entró en el hueco entre dos pasos: el candado del relevo
(`ready()`) solo ve el instante (2 s de PTY / turno del chat), no una
tarea en segundo plano. El umbral de 5 min es el mismo que la regla
`done` usa para "terminó tu sesión": para BORRAR hace falta sesión
terminada, no hito intermedio. La ventana queda en quiet ∈ [5, 10) —
a los 10 el hit press deja de salir (PRESS_QUIET_MAX) y ya no hay
automático, asumido: /compact ya resolvió la presión. Se descartó
endurecer el candado del relevo (QUIET_MS más largo): frenaría también
los manuales y el /compact, que con Claude activo es justo el que gana
la carrera al auto-compact del ~94%. Los alias del relevo NO cambian:
`/model fable` resuelve a Fable 5.1 solito (Claude Code v2.1.257 lo
hace default). Investigado con fuentes oficiales: Fable 5.1 salió el
01/09 ($10/$50, 1M, alias `fable`), Opus 5 $5/$25 1M, Sonnet 5 $2/$10
1M, Haiku 4.5 $1/$5 200k — la lógica por VERSIÓN de las tablas ya los
cubría; solo las dos tarifas de arriba estaban desfasadas.
CÓMO SE VERIFICÓ: py_compile del exportador limpio; el cambio de
index.html es una condición nueva sobre campos que ya viajan (`quiet`
del hit press, en minutos). cargo check PENDIENTE (el VPS no tiene
toolchain): correrlo en el Windows de Oscar antes de recompilar — el
cambio de lib.rs es solo la tabla de precios, sin firmas nuevas. Lo
POSITIVO del día también cuenta: Oscar confirmó los dos automáticos
disparando en vivo con su cuenta atrás (lo que faltaba de la fase 4).
QUÉ QUEDA: ver el /clear disparar CON reposo y ver la degradación con su
rastro (pendiente en R16); fase 4 actualizada en CLAUDE.md (02/09).

## 2026-09-02 (2) — R17: el sello del automático era perpetuo y dejaba a la sesión sin relevo el resto de su vida

QUÉ: el sello "ya se aplicó" del automático (`relayAuto` en localStorage)
deja de ser perpetuo y pasa a durar UN CICLO DE CONTEXTO. Al aplicar se
guarda la medida del momento (`{done:<tokens>}` en vez de la cadena
`"done"`) y `autoRearm()` —nuevo, llamado desde `coachPoll` sobre TODAS
las `press` de cada sondeo— levanta el sello en cuanto una lectura de esa
sesión cae a la mitad o menos de lo sellado, dejando línea en la Bitácora
PRO. `autoBlocked()` no cambia de firma: la cadena `"done"` a secas sigue
bloqueando para siempre, que es el formato viejo y el sello del "bajar
solo" (bajar de modelo una vez y ya). Documentado en
`docs/remediacion.md` §REGLAS VIGENTES, `docs/validacion-pasiva.md` §R17
y el resumen de CLAUDE.md.

POR QUÉ: Oscar preguntó por qué le llegaban los consejos y no lo
automático, y la Bitácora PRO no lo explicaba (todas las salidas de
`relayAutoCheck` son mudas). Autopsia sobre los `.jsonl` de este VPS:
polymarket-bot `dc321d80` (Fable 5.1, techo 1M) recibió el auto-/compact
a las 12:59:54 (`relevo/3737148.json`, `id: app-…`, `ok:true`; el
`compact_boundary` de las 13:01:41 trae preTokens 259.023 y el contexto
cayó a ~114k), se volvió a llenar hasta **344.265 tok** y cerró a las
15:43. A las ~15:48 se dieron a la vez TODAS las condiciones del /clear
de R16 —relevo `ready:true`, quieta 5-10 min, tarea cerrada con recibo—
y no disparó: `autoStamp(sid,"done")` es para siempre y `/compact` no
cambia el `sessionId` (a diferencia de `/clear`, que estrena sesión y por
eso sí se re-armaba solo). O sea: un `/compact` temprano desarmaba a la
sesión justo cuando más falta hace, y encima se comió el escenario que
llevábamos esperando para validar R16.
La señal para levantar el sello NO es una corazonada: todo
`compact_boundary` —de quien sea— pone `last_ctx = 0` (lib.rs 4180), el
hit `press` exige >0 y deja de emitirse, y al turno siguiente reaparece
con lo que quedó; dentro de un mismo ciclo el contexto solo crece, así
que una caída a la mitad solo la produce un vaciado. Se descartó pedirle
al motor un dato nuevo (contador de compactaciones): obligaría a tocar
Rust + `meter-export.py` + las sesiones remotas por el invariante #1
para algo que ya se deduce de lo que viaja. Y se descartó mirarlo solo
sobre la sesión reina cuando arde: a 344k el rastro del 259k → 114k ya no
existe, el vaciado solo se ve en el tramo BAJO y ese tramo no arde — por
eso la pasada va sobre todas las `press` de cada sondeo.

CÓMO SE VERIFICÓ: sintaxis del JS de index.html con `node --check` sobre
los `<script>` extraídos, limpia. Banco de pruebas en seco (scratchpad,
16 casos, todos en verde) con los números REALES del día: sesión virgen,
intento fallido dentro y fuera de los 10 min, sellada que sube a 300k
(sigue bloqueada), el vaciado 259k → 114k (levanta, con su rastro, una
sola vez) y ya rellenada a 344k (sigue libre), sello viejo `"done"` y
`down:<sid>` intactos, frontera exacta de la mitad, y lectura de 0 o hit
sin sesión sin efecto. Sin Rust tocado, así que no hace falta cargo
check. NO verificado en vivo todavía: falta ver el rastro nuevo y un
segundo automático en la misma sesión.

QUÉ QUEDA: checklist repasado el mismo día. La fase 4 marca YA en Exe el
auto-/compact (02/09 12:59:54, preTokens 259.023) y el auto-/clear
(12:40:14, «por hecho»), la alarma de cuota suma dos umbrales del día
(81% y 96%, uno cada uno) y se abren tres filas nuevas: la degradación de
R16, el /clear en reposo y el sello levantado de R17. Y §"Lo que falta,
en orden" reúne por primera vez TODO lo pendiente agrupado por la fase
que lo desbloquea. De la fase 4 quedan cuatro cosas, y solo una es
forzable a voluntad: parar la cuenta atrás con un toque. En la fase 4 de
la validación pasiva, ver en vivo "sello levantado" y detrás el segundo
automático; sigue pendiente lo de R16
(el /clear con reposo y la degradación a /compact), que este arreglo
desbloquea. Anotado también que `relayAutoCheck` calla en sus ~8
salidas: distinguir "no tocaba" de "estaba bloqueado" costó una tarde de
autopsia, y una línea con freno lo evitaría — no se hizo aquí para no
mezclar dos cambios en la misma compuerta.

## 2026-09-02 (3) — la sesión llega al 100%: cae la pieza de ntfy que llevaba pendiente desde el principio

QUÉ: validación en vivo, sin tocar código. La sesión de 5 h de Oscar llegó
al 100% y con ese único evento caen tres filas del checklist: el aviso de
ntfy al 100% CON su «ya volvió» programado (fase 6 CERRADA), el globo de
descanso anclado al gatito, y el estado `cat-break`. `docs/validacion-
pasiva.md` actualizado (§2, §10, §"Lo que falta, en orden" y la
actualización de cabecera).

POR QUÉ: era la ocasión más barata que iba a haber — a las 16:06 la
bitácora ya marcaba 96%, así que se sugirió aprovecharla en vez de abrir
la fase 5. Un solo evento cubría cuatro filas pendientes.

CÓMO SE VERIFICÓ: capturas de Oscar. Globo «Sin cuota de sesión. Vuelvo
en 2 min.» con la cápsula «Sesión 100%» en rojo y el gatito tumbado; y en
el celular LOS DOS avisos, el inmediato y el de cuota restablecida. Ese
segundo solo puede ser el PROGRAMADO: `ntfyLimit` (index.html:12232-12240)
lo publica en el mismo instante del 100% con el delay del reset +120 s, y
no hay otro sitio en el código que emita ese texto — o sea que ntfy lo
retuvo y lo entregó él solo, que es justo lo que había que probar.
Comprobación cruzada del reloj de la cuota, de regalo: al 99% Claude Code
decía «resets in 5m» y tres minutos después, ya al 100%, nuestro globo
decía «Vuelvo en 2 min» — las dos fuentes cuadran al minuto.
Oscar nota que el DIBUJO del gatito tarda un poco en cambiar: no es un
fallo, es la cadencia de cuota (3 min). El widget nunca llama al endpoint;
espera a que el panel le emita `quota:update`. Está anotado en la fila
para que no vuelva a abrirse como rareza.

La CUARTA fila también cayó, con captura aparte: globo «Cuota de sesión
restablecida. A trabajar.» anclado al gatito y la cápsula en «Sesión 0%»
— el restablecimiento con confirmación, que solo sale porque la ventana
anterior llegó al 100%. De paso corrige una afirmación equivocada de esta
misma sesión: se le dijo a Oscar que buscara el gatito en `cat-fire`, y
`cat-fire` es para una ALARMA sin confirmar (`ackPending:alarm`,
index.html:7178). El restablecimiento se manifiesta como el GLOBO; al
resetearse la ventana se borra `hit:session` y el dibujo vuelve a
`normal`. Queda anotado en la fila para que nadie lo depure al revés.

QUÉ QUEDA: de ntfy, nada. Del gatito falta `cat-zzz`, que pide la SEMANA
al tope. La fase 4 sigue con sus cuatro.

## 2026-09-02 (4) — cierre de la jornada: R16 y R17 en el código, fase 6 cerrada, la fase 4 a cuatro filas del final

QUÉ: cierre del día. Tres entradas antes que esta cuentan el detalle; el
resumen es que hoy salieron dos arreglos del automático y una tanda de
validación en vivo que cerró una fase entera.

- **R16** (commit `73433f1`): el auto-`/clear` exige la sesión en REPOSO
  (`quiet` ≥ `AUTO_REST_MIN` = 5 min); sin reposo degrada a `/compact`.
  Además, tarifas de Sonnet 5 y caché de Fable 5.1.
- **R17** (commit `0bafea4`): el sello `done` del automático dura UN CICLO
  DE CONTEXTO, no toda la vida de la sesión. `autoRearm()` corre en cada
  sondeo sobre TODAS las press y levanta el sello al ver el vaciado.
- **Validación** (commits `00d2954`, `8144ae4`, `2d79017`): fase 6 (ntfy)
  CERRADA con el 100% real de Oscar — aviso inmediato, «ya volvió»
  programado, globo de descanso, `cat-break` y restablecimiento con
  confirmación. Cinco filas con un solo evento.

POR QUÉ: el día empezó con una pregunta de Oscar —«no me llegó el
automático, ¿está bien o no?»— y la respuesta fue que NO estaba bien: la
autopsia del flowLog encontró el sello perpetuo de un `/compact` de las
12:59 bloqueando todo lo demás hasta el final del día, con la sesión en
344k. De ahí R17. La validación de ntfy se hizo porque la cuota ya iba al
96%: era la ocasión más barata que iba a haber, más barata que abrir la
fase 5.

CÓMO SE VERIFICÓ: `node --check` limpio y banco de pruebas de 16 casos con
los números reales de la sesión (259k→114k→344k), todo en verde; el resto,
capturas de Oscar en vivo, detalladas en las entradas (2) y (3).

QUÉ QUEDA: la fase 4 es la ÚNICA abierta, con cuatro filas —parar la
cuenta atrás con un toque (la única forzable a voluntad), la degradación
de R16, el `/clear` en reposo de verdad y el rastro «sello levantado» de
R17 con su segundo automático—. Fase 5 (ruteo) sin abrir y todo apagado;
fase 7 (archivador y purga) igual; HUB bloqueado hasta la segunda máquina.
Sueltos: `cat-zzz` (pide la SEMANA al tope) y el primer `via:emb` del
análisis local. Aviso de mantenimiento: la bitácora va por 4.7k líneas y
la regla de este archivo manda mover el tramo viejo a
`bitacora-hasta-AAAA-MM-DD.md` pasadas las ~3.000 — pendiente de hacer, no
urgente pero ya toca.
