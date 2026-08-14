# Prompts de diseño — % de desperdicio estructural

Para pasarle a otra IA y que genere la maqueta visual de la métrica cuya
FÓRMULA está definida en `presion-y-rendimiento.md` §"La fórmula del % de
desperdicio estructural". Leer esa sección antes: los prompts ya traen sus
reglas de honestidad y si se relajan, la maqueta miente.

REGLA DE USO (igual que en `prompts-diseno-remediacion.md`): lo que devuelva
la otra IA es **referencia visual, no código integrable** — el panel real usa
sus propias variables CSS, SVG inline y CSP estricta.

## Bloque de estilo común (pegarlo antes de cada prompt)

```
Diseña una interfaz para MichiClaude, una app de bandeja para Windows 11 (Tauri) que
monitorea el uso de Claude Code. Sistema visual OBLIGATORIO:

- Tema oscuro por defecto: fondo #0F1830, tarjetas #151F3A (hover #1B2748),
  bordes rgba(148,170,255,.16), texto #EAF0FF, secundario #93A0C4, apagado #5E6B92.
- Acento índigo #7C8CFF (gradiente a #A46BFF para elementos hero), teal #4FD1E0
  para positivo, ok #3FD68F, advertencia ámbar #F2B443, peligro #FF6B5E,
  marca naranja #E08B63 (solo el gatito).
- Tipografías: Inter (cuerpo), Sora (títulos, bold, letter-spacing -0.2px),
  JetBrains Mono (números, comandos, horas).
- Radios: 20px tarjetas grandes, 14px medianas, 10px chips. Bordes de 1px sutiles.
- Panel angosto: 446px de ancho. Todo debe caber en esa columna.
- Iconos: SVG de línea simple (estilo Tabler), nunca emojis ni icon-fonts.
- La mascota es un gato pixelado estilo Bongo Cat; tono de los textos: seco,
  observacional, con humor ligero.
- Porcentajes siempre enteros. Idioma: español.
- Entrega un HTML autocontenido (CSS embebido, sin CDN ni imágenes externas),
  con tema oscuro y tema claro.
```

## Contexto de la métrica (pegarlo también, es lo que no puedes inventar)

```
QUÉ MIDE el número que vas a maquetar:

"Desperdicio estructural" = de todo el dinero gastado con Claude Code en un
periodo, la parte que se fue en cómo está MONTADO el entorno y no en el trabajo:
reglas del archivo de configuración CLAUDE.md que nunca se usaron pero viajan en
cada sesión, salida de hooks que se inyecta en cada disparo, y contexto que hubo
que REESCRIBIR cuando se perdió la caché. Se calcula dividiendo el costo medido de
esos hallazgos entre el costo total del periodo. Las dos fuentes son los logs
locales de Claude Code; la app no gasta tokens para medir.

REGLAS DE HONESTIDAD — son la personalidad del producto, no adorno:
1. El número es un PISO: hay desperdicio que la app no sabe medir (servidores MCP
   conectados y nunca invocados, skills instaladas sin usar) y queda fuera. Por eso
   SIEMPRE se escribe "al menos el 14%", nunca "el 14%".
2. Prohibido prometer ahorro. "Al menos $23 se te fueron en el montaje" es historia
   y se puede defender; "vas a ahorrar $23" es un pronóstico y no se dice jamás.
3. Los costos en dólares son NOCIONALES (equivalente a precio de API; el usuario
   paga una suscripción fija). Va etiquetado como "estimado" y con "~" delante.
4. Nada de inventar cifras: si no hay datos suficientes, se dice, no se rellena.
5. Cada punto del porcentaje se puede abrir y ver de dónde salió: el desglose es
   parte del número, no un extra escondido.

DATOS REALES DISPONIBLES para el desglose (no inventes otros):
- "Reglas del CLAUDE.md que nadie usó" — nº de líneas, archivo, costo estimado.
- "Salida de hooks inyectada" — nombre del hook, nº de disparos, costo estimado.
- "Contexto reescrito al perderse la caché" — nº de rupturas, tokens, costo MEDIDO.
- Sin costo medible (se muestran como transparencia, con $0 y la nota
  "no lo contamos"): servidores MCP inactivos, skills sin usar, CLAUDE.md
  demasiado grande.
```

## Prompt 1 — La tarjeta en la pestaña Reporte (estado normal)

```
Pantalla: tarjeta "Desperdicio estructural" dentro de la pestaña Reporte del panel
de 446px de ancho. Va debajo del héroe de eficiencia que ya existe, así que NO debe
competir con él: es la segunda cosa más importante de la pantalla, no la primera.

Contenido:
- Título "Desperdicio estructural" con un icono de línea (una tubería con una gota,
  o una junta) y, a la derecha, un chip discreto con el periodo: "Últimos 7 días".
- EL NÚMERO: "al menos 14%" — el "al menos" en tamaño pequeño y color secundario,
  el "14%" grande en Sora bold. Debajo, en una sola frase llana:
  "De cada $100 que gastaste, al menos $14 se fueron en el montaje, no en el trabajo."
- Una barra horizontal de proporción (no una dona) que ocupe el ancho: el tramo de
  desperdicio en ámbar, el resto en un gris azulado neutro que NO parezca "bueno"
  ni "malo" — el resto es simplemente trabajo. La barra lleva su cifra al lado:
  "~$23 de ~$164 (estimado)".
- Comparación con el periodo anterior en una línea con flecha: "Antes: 21% ↓ 7 puntos".
  Si mejora, teal; si empeora, ámbar; si el cambio es de 1 punto o menos, gris con
  la palabra "igual". Nunca porcentaje de porcentaje: siempre "puntos".
- Desglose plegable ("Ver de dónde sale", cerrado por defecto): 3 filas con
  nombre en llano a la izquierda, costo en mono a la derecha y una micro-barra de
  peso relativo debajo del nombre. Filas de ejemplo:
    · "Contexto reescrito al perderse la caché — 4 veces"  ~$16
    · "Reglas del CLAUDE.md que nadie usó — 31 líneas"     ~$5
    · "Salida de hooks inyectada — PostToolUse:Bash"       ~$2
  Cada fila es clicable (lleva a su tarjeta en la pestaña Hallazgos): indícalo con
  un chevron pequeño, no con un botón.
- Al pie del desglose, una nota gris de dos líneas con el icono de información:
  "No contamos lo que no sabemos medir: 2 servidores MCP inactivos y 5 skills sin
  usar también pesan, pero su costo no es medible desde los logs. Por eso el número
  dice 'al menos'."

Prohibido en esta tarjeta: barras de progreso que sugieran una meta, semáforos
verde/rojo, la palabra "ahorro", cualquier cifra que no esté en la lista de datos
disponibles.
```

## Prompt 2 — Los estados degradados (los tres)

```
Tres variantes de la MISMA tarjeta de 446px, apiladas para comparar. La app prefiere
callar antes que inventar, y esos estados hay que diseñarlos igual de bien que el
bueno:

A) "Juntando datos" — hay menos de 10 sesiones o menos de $1 en el periodo. Sin
   número, sin barra: el hueco del número lo ocupa un guion largo apagado y la
   frase "Juntando datos — necesito unos días más de uso para que el porcentaje
   signifique algo". Debajo, un progreso discreto y honesto: "6 de 10 sesiones".

B) "Ventana corta" — el usuario eligió el periodo de 1 día. Los detectores de
   configuración no corren en ventanas tan cortas. Mensaje: "Este número solo tiene
   sentido con 7 días o más" y un chip clicable "Ver 7 días" que cambia el periodo.

C) "Sin desperdicio detectado" — el numerador es 0. NO celebrar de más: título
   "Nada que señalar", frase "No detecté desperdicio estructural en estos 7 días.
   Eso no significa que sea cero: significa que nada llegó a los umbrales."
   Tono seco, sin confeti, sin verde de "aprobado" — como mucho un teal apagado.

Las tres deben verse hermanas de la tarjeta normal: mismo alto aproximado, misma
posición de los elementos, sin saltos de layout al pasar de un estado a otro.
```

## Prompt 3 — El bloque del reporte ancho (export HTML, 820px)

```
Mismo dato, otra vista: el bloque de "Desperdicio estructural" para un reporte
imprimible y compartible de 820px de ancho (documento ejecutivo para un usuario
que está EMPEZANDO y no sabe de tokens ni de caché). Aquí hay sitio para explicar.

Estructura del bloque:
- Titular grande con la frase, no con la jerga: "De cada $100, al menos $14 se te
  fueron en el montaje". El "14%" como cifra secundaria a un lado.
- Una analogía en una línea, en cursiva y color secundario, del registro de un
  plomero: la casa no gasta más agua por bañarse más, gasta más porque una junta
  está vencida. No repitas la analogía en el resto del bloque.
- Tabla de tres columnas: "Qué fue" (nombre en llano) · "Cuánto pesó" (costo
  estimado en mono) · "Qué se hace" (una acción concreta en imperativo, corta:
  "quita esas 31 líneas del CLAUDE.md"). Tres filas, las del desglose del Prompt 1.
- Barra comparativa de DOS periodos, una encima de otra (esta semana / semana
  anterior), con la etiqueta de puntos de diferencia al final. Nunca una gráfica de
  consumo crudo: aquí solo se compara la RAZÓN.
- Recuadro final "Lo que no está contado" con la nota de honestidad del Prompt 1,
  redactado para alguien no técnico.

El bloque tiene que verse bien impreso en blanco y negro: no dependas solo del
color para distinguir el tramo de desperdicio del resto (usa también textura o
grosor de borde).
```
