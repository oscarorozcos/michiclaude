# Prompts de diseño — remediación (referencia)

Guardados el 2026-08-07 por si se necesitan más adelante. Son los 7
prompts para pasarle a otra IA y que genere maquetas de la propuesta de
remediación TERMINADA (diseño en `remediacion.md` — leerlo primero: los
prompts ya traen sus correcciones de honestidad). El bloque de estilo
usa la paleta y tipografía REALES del panel, extraídas de index.html,
para que la IA no invente otro sistema visual.

REGLA DE USO: lo que devuelva la otra IA es **referencia visual, no
código integrable** — el panel real usa sus propias variables CSS, SVG
inline y CSP estricta; la integración se hace traduciendo el diseño a
nuestro sistema.

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
  observacional, con humor ligero ("Tu disco respira", "Tú sabrás").
- Porcentajes siempre enteros. Idioma: español.
```

## Prompt 1 — Tarjeta de intención (modo consejero, clipboard)

```
Pantalla: tarjeta "Tu sesión ya pesa mucho" dentro del panel de 446px.

Cabecera: icono de gato + título "Tu sesión ya pesa mucho" + badge ámbar "84%".
Subtítulo: "Cada mensaje te está costando más caro. ¿Qué quieres hacer?"

Franja de evidencia (fondo más oscuro, texto pequeño): "Michi detectó: lista de
tareas 8/10 · mismos archivos en los últimos 20 mensajes · último mensaje hace 4 min".

Dos opciones grandes como tarjetas clicables:
1. "Sigo trabajando en lo mismo" — borde acento, badge flotante "Recomendado —
   vas a media tarea", descripción "Comprime el historial pero Claude recuerda
   en qué van", comando /compact en mono pequeño a la derecha.
2. "Ya terminé, empiezo algo nuevo" — borde neutro, descripción "Borrón y cuenta
   nueva, máximo ahorro", advertencia ámbar "⚠ Tienes 2 pendientes en tu lista —
   esto los borraría de la memoria de Claude", comando /clear en mono.

IMPORTANTE: los botones dicen "Copiar comando" (esta sesión no es inyectable);
al pie una nota gris: "Esta sesión no fue lanzada con michi claude — te dejo el
comando listo y tú lo pegas". Botón secundario "Ahora no" abajo a la derecha.

Segundo estado de la misma tarjeta: la sesión SÍ es inyectable — badge teal
"● sesión con relevo" junto al título y los botones cambian a "Aplicar por mí".
```

## Prompt 2 — Panel de Modo automático con desbloqueo progresivo

```
Pantalla: sección "Modo automático" en la pestaña Ajustes del panel de 446px.

Tarjeta principal: icono de rayo + "Modo automático" + toggle maestro encendido.
Subtítulo: "Michi aplica las remediaciones sin preguntarte. Cada acción se
activa por separado."

Lista de 4 acciones, cada una con nombre, badge de riesgo, descripción y su
control a la derecha:
1. "Matar MCPs zombies" — badge verde "Riesgo bajo" — "Procesos MCP sin sesión
   padre viva" — checkbox activo.
2. "Archivar JSONL viejos" — badge verde "Riesgo bajo" — "Sesiones con más de
   365 días, comprimidas a .zip" — checkbox activo.
3. "Aplicar /compact" — badge ámbar "Con countdown" — "Cuando la presión pase
   del 80%. 15 s para cancelar. Solo sesiones con relevo" — en vez de checkbox,
   un CANDADO cerrado con barra de progreso "1/2" y texto pequeño: "Se
   desbloquea cuando apliques /compact tú mismo 2 veces. Michi no automatiza
   lo que no entiendes."
4. "Aplicar /clear" — badge rojo "Destructivo" — "Solo entre tareas detectadas
   como cerradas. 15 s para cancelar. Solo sesiones con relevo" — candado con
   progreso "0/3".

Segundo estado: la acción 3 ya desbloqueada — el candado se convirtió en
checkbox con una animación sutil de candado abierto y micro-texto "desbloqueado
el 7 ago".
```

## Prompt 3 — Countdown de acción automática (superficie propia)

```
Pantalla: tarjeta de countdown que aparece cuando Michi va a actuar solo.
Es una tarjeta destacada con borde ámbar dentro del panel de 446px (NO es una
notificación del sistema).

Contenido: icono de gato + "Michi va a aplicar /compact" + contador grande "12"
en JetBrains Mono ámbar. Subtítulo: "Presión de sesión: 84%. Ya te lo había
dicho." Barra de progreso ámbar que se vacía con el tiempo.

TRES botones: "Cancelar" (fantasma) · "Mejor /clear" (secundario, permite
corregir el rumbo en un clic) · "Aplicar ya" (primario acento).

Nota pequeña al pie: "Antes de inyectar verifico que no estés escribiendo y
que Claude haya terminado su turno."

Segundo estado: acción completada — la tarjeta se transforma en confirmación
verde: "✓ Apliqué /compact. Sesión al 31%, ahorraste ~$0.40. Tu trabajo en
disco sigue intacto."
```

## Prompt 4 — Registro de acciones

```
Pantalla: tarjeta "Registro de acciones" dentro del panel de 446px.

Lista cronológica con hora en JetBrains Mono gris a la izquierda y frase con
icono de estado:
- 14:32 ✓ verde "Maté 2 MCPs zombies. De nada."
- 13:05 ✓ verde "Archivé 12 JSONL (48 MB). Tu disco respira."
- 12:10 ✓ teal "Te dejé /compact en el clipboard; vi que lo aplicaste a las
  12:11. Buen equipo."
- 11:47 ✕ gris "Cancelaste el /compact. Tú sabrás."
- 09:20 ⚠ ámbar "Iba a aplicar /clear pero estabas escribiendo. Me esperé."

Cada fila con etiqueta pequeña de origen: "local", "wsl", "vps-oscar".
Al pie, enlace discreto "ver todo el historial".
```

## Prompt 5 — Manómetro de presión en el widget (pastilla y gatito)

```
Pantalla: dos widgets flotantes de escritorio, lado a lado, sobre un fondo de
escritorio desenfocado.

1. PASTILLA (280×54): cápsula de vidrio esmerilado con asa de puntos ⠿ a la
   izquierda, sticker pequeño del gato, texto "Sesión 62%" y — NUEVO — un
   mini-manómetro de presión de contexto: arco de medidor tipo velocímetro de
   18px con aguja, en teal cuando está bajo. Versión 2 de la misma pastilla:
   el arco en ámbar con "84%" y un puntito pulsante.

2. GATITO: gato pixelado Bongo Cat de ~200px sobre una laptop, con una cápsula
   flotante "Sesión 62%" y el mismo mini-manómetro integrado a la cápsula.
   Versión 2: manómetro en rojo y el gato con cara de alarma.

El manómetro debe leerse a tamaño diminuto; nada de texto extra.
```

## Prompt 6 — Tarjeta educativa (primera vez de cada comando)

```
Pantalla: tarjeta educativa que aparece la PRIMERA vez que Michi sugiere /clear,
en el panel de 446px.

Título: "Antes de tu primer /clear" con icono de gorra de graduación.
Tres líneas fijas con iconos:
- "Qué hace: reinicia el contexto. Claude olvida el chat actual."
- "Qué vas a ver: la pantalla de tu terminal se limpia. Es normal."
- "Qué NO se pierde: tus archivos, código e historial en disco siguen intactos."
Cita en itálica: "Es como cerrar pestañas del navegador, no como formatear
la compu."

Botón primario: "Copiar /clear y aplicarlo yo". Debajo, barra de progreso de
desbloqueo "1 de 3 — cuando lo hayas usado 3 veces, Michi podrá hacerlo solo"
con un candado al final de la barra.
```

## Prompt 7 — El relevo en la terminal (`michi claude`)

```
Pantalla: mockup de una ventana de Windows Terminal (tema oscuro) mostrando el
flujo del relevo de MichiClaude, en dos momentos:

1. El usuario tecleó "michi claude" en C:\proyectos\mi-proyecto. Aparece una
   línea de bienvenida discreta en gris: "🐱 michi: sesión con relevo — Michi
   puede aplicar remediaciones aquí. Todo lo demás es Claude Code normal."
   Debajo, la interfaz normal de Claude Code corriendo.

2. Momento de inyección: el prompt de Claude Code está vacío; se ve aparecer
   "/compact" tecleado con una etiqueta lateral fantasma "← michi" y una línea
   gris posterior: "michi: apliqué /compact (prompt vacío verificado, tú no
   estabas escribiendo)".

Estética de terminal real: JetBrains Mono, fondo casi negro, sin adornos de
app — que se sienta nativo de terminal, no una recreación fantasiosa.
```

## Notas para cuando lleguen los diseños de vuelta

1. La opción "handoff Pro" quedó FUERA de la tarjeta de intención a
   propósito (requiere IA que la app no tiene y decisión de negocio
   pendiente — ver `remediacion.md` §Correcciones). Si se quiere ver en
   el diseño como futuro: pedirla como cuarta opción con badge
   "Próximamente".
2. Los prompts ya traen las correcciones de honestidad: 365 días en
   archivado, "Copiar" vs "Aplicar" según haya relevo, y el countdown
   como tarjeta del panel (no globo, no toast — regla única de globos).
3. La integración real va traduciendo el diseño al sistema del panel
   (variables CSS, SVG inline, I18N ×8, CSP sin fuentes externas).
