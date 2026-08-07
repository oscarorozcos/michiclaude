# Remediación — de consejero a plomero que también arregla

Diseño acordado el 2026-08-07 a partir de una propuesta externa (handoff
de otra IA + mockups). Este documento es la versión DESTILADA Y CORREGIDA:
la propuesta original traía contexto falso del proyecto y varias piezas
que chocan con invariantes — aquí queda solo lo viable, con su porqué y
sus etapas. LEER ANTES de tocar cualquier cosa de remediación.
Los prompts para generar las maquetas con otra IA están guardados en
`prompts-diseno-remediacion.md` (referencia visual, no código).

## La idea en una frase

MichiClaude ya diagnostica (Hallazgos) y aconseja (Consejos). La
evolución: que además APLIQUE remediaciones — las que controla directo,
en automático opt-in; las que viven dentro de la terminal de Claude Code,
primero a un clic (clipboard) y después de verdad, vía el relevo.

## Principios de diseño (no negociables)

1. **Intención, no comando:** Michi nunca pregunta "¿/compact o /clear?";
   pregunta "¿sigues trabajando o ya terminaste?" y ÉL pone el comando.
   El nombre del comando aparece pequeño al lado — el usuario aprende el
   mapeo sin que sea requisito.
2. **Regla de oro:** en la duda, Michi pregunta, no actúa. El peor caso
   permitido del modo automático es "me preguntó de más" (recuperable),
   nunca "borró mi contexto".
3. **Honestidad de UI (espíritu del invariante #8):** jamás prometer lo
   que no se puede hacer ni afirmar lo que no se puede saber. Sin relevo
   no existe "Aplicar /compact" — existe "Copiar comando". "Michi aplicó
   X" solo se afirma si de verdad lo inyectó (o si lo VIO aplicado en el
   JSONL después).
4. **Confianza progresiva:** cada acción automática se gana. Candado con
   contador de aplicaciones manuales, primera vez de cada tipo SIEMPRE
   manual, re-bloqueo automático tras 2 cancelaciones seguidas del
   countdown, ventana de 30 días para que las manuales cuenten.
5. **Evidencia visible:** toda tarjeta de intención enseña POR QUÉ
   ("lista 8/10 · mismos archivos en 20 msgs · último msg hace 4 min").
   Mostrar evidencia = diagnóstico, no adivinanza.
6. **Las manos del usuario siempre ganan** (ver reglas anti-choque del
   relevo).

## Dos clases de remediación

**Out-of-band** — Michi las ejecuta directo, sin tocar la terminal:
matar MCPs zombies, archivar JSONL viejos. Riesgo bajo real, reversibles.
Candidatas al modo automático desde la etapa 2.

**In-band** — `/compact`, `/clear`: viven DENTRO del REPL de Claude Code
y MichiClaude no tiene canal de escritura. Sin relevo: modo
semi-automático (tarjeta + clipboard al clic). Con relevo: inyección
real. La inyección de teclado simulado (SendInput) queda DESCARTADA como
mecanismo (frágil, puede teclear encima del usuario); como mucho, un
opt-in experimental futuro.

## El relevo (`michi claude`) — el "tmux nativo" sin tmux

Un mini-ejecutable relevo que viaja con la app. El usuario teclea
`michi claude` en su terminal de siempre (Windows Terminal, VS Code,
la que sea) en su carpeta de siempre — cero configuración, nada se
mueve, `~/.claude` intacto. El relevo:

1. Lanza Claude Code dentro de una **ConPTY** que él controla (API
   nativa de Windows; en Rust, crates tipo `portable-pty` — escribir
   contra la abstracción portable desde el día 1: en Mac/Linux la PTY es
   nativa y el relevo portaría casi gratis).
2. Reenvía TODO transparente (teclas, pantalla, resize, Ctrl+C, modo
   raw). Claude Code ni se entera.
3. Abre un canal local (named pipe) por donde MichiClaude le pide
   inyectar un comando.
4. Como ve pasar cada tecla y cada byte de salida, SABE (no adivina) si
   el prompt está vacío y si Claude está generando.

**Reglas anti-choque de la inyección (todas obligatorias):**
- R1: NUNCA inyectar si hay texto del usuario desde su último Enter (el
  relevo cuenta las teclas — es certeza, no heurística).
- R2: NUNCA inyectar mientras Claude genera su turno.
- R3: ventana de calma (5-10 s sin una sola tecla) antes de actuar.
- R4: countdown visible primero; al vencer se RE-verifican R1-R3 en ese
  instante; si fallan, se aborta y degrada a clipboard.
- R5 (sagrada): Michi JAMÁS borra texto del usuario (nada de backspaces
  ni Ctrl+U para "limpiar" la línea). Si hay texto, no se inyecta, punto.
- Si el usuario aplica su propio comando primero, el relevo lo ve pasar
  y cancela el suyo (y lo anota en el registro).

**Degradación:** sesiones no lanzadas por el relevo funcionan como hoy
(detección por logs + modo clipboard). Nunca un error; solo "esta sesión
la puedo remediar / esta te dejo el comando listo". Alias opcional
(un clic en Ajustes, reversible) para que `claude` pase por el relevo
sin cambiar el hábito.

**Los 3 modos:** el relevo no sabe qué hay al otro lado del tubo —
local directo; WSL envolviendo `wsl claude`; SSH envolviendo el cliente
`ssh` local (la inyección viaja por el tubo como tecleo). Matiz SSH: los
DATOS de esa sesión llegan por el exportador con retraso de sondeo, y
casar "esta terminal" con "esta sesión de los logs remotos" necesita
heurística (carpeta + hora de inicio) — es el modo que más pruebas pide.

## Clasificador de tarea viva (señales del JSONL)

`Alive` / `Boundary` / `Uncertain`, determinista (nivel 1):

| Señal | Peso | Nota |
|---|---|---|
| TodoWrite: items con status != completed | Alta (la reina) | último TodoWrite de la sesión |
| Actividad: último evento <15 min / >60 min | Alta | viva / probable frontera |
| Continuidad de archivos (Jaccard últimos 10 vs 10 anteriores >0.4) | Media | misma tarea |
| `git commit` reciente sin ediciones después | Media | señal de cierre |
| Densidad de tool calls | Baja | ráfaga de edits = faena |

La señal de "lenguaje de cierre" por regex ("listo", "ya quedó") queda
FUERA: es solo-español y la app es de 8 idiomas.

Mapeo: `Alive` → recomendar /compact, /clear con advertencia (y en
automático, auto-/clear JAMÁS); `Boundary` → recomendar /clear;
`Uncertain` → SIEMPRE preguntar (la propuesta original resolvía
Uncertain con un modelo local — descartado, ver abajo). Los todos
abiertos alimentan la advertencia contextual del /clear ("tienes 2
pendientes — esto los borraría de la memoria de Claude").

## Desbloqueo progresivo por acción

| Acción | Default | Desbloqueo del automático |
|---|---|---|
| Matar MCPs zombies | on | inmediato (riesgo bajo real) |
| Archivar JSONL ≥365d | off | inmediato |
| /compact (con relevo) | candado | 2 aplicaciones manuales |
| /clear (con relevo) | candado | 3 manuales; solo en Boundary |

Microcopy del candado: "Michi no automatiza lo que no entiendes" —
restricción convertida en argumento. Persistencia: contador por acción
con timestamps (solo cuentan los últimos 30 días). Tarjeta educativa en
la primera vez de cada comando (qué hace / qué vas a ver / qué NO se
pierde — la línea "qué vas a ver" es la clave anti-susto).

## Etapas (cada una útil por sí sola)

1. **Consejero con intención** (sin relevo, TODAS las sesiones):
   manómetro de presión en pastilla/gatito (puntos 9-10 de
   presion-y-rendimiento.md — dato ya existe, viaja en quota:update),
   parser de TodoWrite + clasificador (TRES piezas: Rust +
   meter-export.py + panel, invariante #1), tarjeta de intención con
   evidencia y botón "Copiar comando" (clipboard SOLO al clic — pisar el
   clipboard sin pedirlo es invasivo; dep nueva justificada:
   tauri-plugin-clipboard-manager).
2. **Automático out-of-band:** matar zombies (con re-verificación
   anti-reciclaje de PID: nombre de ejecutable + hora de inicio antes
   del taskkill) + archivar JSONL **≥365 días** + registro de acciones +
   desbloqueo progresivo. Todo async + spawn_blocking (invariante
   10ter). SOLO LOCAL — nada de matar procesos ni mover archivos por
   SSH; las tarjetas de origen remoto no ofrecen botón.
3. **El relevo** (`michi claude`): inyección real de /compact//clear con
   countdown, solo sesiones del relevo; los checks "Aplicar" APARECEN
   solo cuando existen sesiones inyectables. El countdown va en una
   SUPERFICIE PROPIA (tarjeta del panel o ventana nueva) — NUNCA
   reutilizar los globos (regla única: ningún globo se cierra solo) ni
   toast de Windows con widget vivo.
4. **Relevo en WSL y SSH** — el modo automático completo en los 3 modos.

## Correcciones sobre la propuesta original (para no rediscutir)

- **Contexto falso que traía el handoff:** licencia "MIT/Apache
  open-core" (somos GPL-3.0), tier "Pro" con precios (el negocio vive
  FUERA del repo), tipografías/colores inventados a medias, iconos
  Tabler por icon-font (rompería la CSP — aquí todo icono es SVG
  inline).
- **"L2: modelo local" para Uncertain:** DESCARTADO — ya estaba
  descartado con porqué en presion-y-rendimiento.md §Lo descartado
  (peso, distribución, invariante #4). Uncertain = preguntar.
- **"Archivar JSONL >30 días" etiquetado riesgo bajo:** era la acción
  MÁS peligrosa — el analizador, el Reporte y las marcas de arreglo
  necesitan ese historial (cleanupPeriodDays 365). Umbral corregido a
  ≥365 días.
- **"Handoff Pro" (resumen de traspaso + /clear):** APLAZADO — necesita
  una IA que la app no tiene (coach sin IA por diseño, modelo local
  descartado, y usar el token OAuth gastaría la cuota del usuario).
  Decisión de negocio de Oscar, no técnica.
- **Podar CLAUDE.md / editar settings.json del usuario:** APLAZADO — el
  mayor salto de confianza; decidir QUÉ reglas sobran no lo sabe una
  regla determinista (nos pasó con el corte de 118.8k: solo Oscar sabía
  qué sobraba). El detector claudemdsize que avisa ya existe y basta.
- **Wrapper tmux/WSL:** INNECESARIO — el relevo ConPTY cubre WSL
  envolviendo `wsl claude`.
- **Todo el microcopy va por I18N** (códigos desde Rust, t() en el
  panel, invariante #10): son ~30-40 claves × 8 idiomas, y hay que
  validar que el tono seco de Michi sobreviva la traducción.
- **"Telemetría local"** (ajustar heurística si el usuario corrige 2
  veces): válida SOLO como contadores en localStorage — nada viaja
  (invariante #3). Y nada del registro de acciones va a ntfy con rutas
  ni nombres de proyecto.

## Pendiente de decisión de Oscar antes de arrancar

- Matar procesos es una CLASE NUEVA de capacidad (hoy la app solo lee
  logs y llama un endpoint) — decisión consciente, no colarse en un PR.
- Cuándo arrancar: hay obra abierta (fase 1 del reporte pendiente de
  cargo check en Windows, fase 2 de capturas, fase 3 sin arrancar,
  validación pasiva). Recomendación: cerrar el reporte primero.
