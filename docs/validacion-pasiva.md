# Validación pasiva — checklist vivo

Qué es: la lista de TODO lo que solo se puede dar por bueno **usando la
app de verdad** (no en el simulador). Oscar manda capturas o notas
mientras trabaja; aquí se marca qué quedó validado, con qué evidencia y
qué salió raro. CLAUDE.md solo apunta aquí.

Leyenda:

- `[x]` **validado** — visto funcionando en uso real, con evidencia.
- `[~]` **parcial / raro** — se vio, pero con un pero anotado.
- `[ ]` **pendiente** — todavía no ocurre en uso real.

**CÓMO se está validando (2026-08-19):** Oscar usa el **instalador de
release**, como usuario, no `npm run dev`. Consecuencias que hay que
tener presentes al diagnosticar:

- **No hay flowLog (📜) ni DevTools** — son de dev. Todo rastro tiene que
  salir de `%APPDATA%\com.oscarorozco.michiclaude\` (`coach_debug.json`,
  `rem_debug.json`, `emb_debug.txt`, `inflate_topics.json`,
  `quota_debug.json`…). No pedir el flowLog.
- **A favor**: lo que se ve bien aquí está probado CON la CSP de release
  (invariante #3), que es donde suelen romperse los estilos.
- Las sesiones observadas viven en el VPS y llegan por SSH; el panel
  corre en el Windows de Oscar.

Regla: nada se marca `[x]` por haberlo visto en el simulador ni por
"debería funcionar". La evidencia va con fecha y una frase de qué se vio.
Cuando algo sale raro se abre línea propia en §"Rarezas" con el rastro
que toca mirar (`docs/README.md` §"Dónde mirar cuando algo falla").

---

## 1. Cuota y alarmas

- [x] **Alarma de sesión por umbral con cuota real**: globo "Sesión al
      10% de tu límite de 5 h. · Reset en 3 h 36 min". *2026-08-19,
      release.* Trae el porcentaje y la hora de reset, y el gatito pasa a
      `cat-fire`.
- [ ] Repetición cada 5 min hasta abrir el panel.
- [ ] Varios umbrales cruzados de golpe → solo el más alto.
- [ ] Alarma semanal al 100% (uno por ventana).
- [ ] Restablecimiento de ventana (sesión y semanal) con confirmación.
- [ ] 429 real: el gauge conserva el último dato bueno 15 min, sin cifras
      inventadas.
- [ ] Tray con cuota en error: "–" gris.

## 2. Avisos al celular (ntfy)

- [ ] Push de umbral llega al celular.
- [ ] Camino completo del 100%: aviso inmediato + "ya volvió" programado
      **con la PC apagada**.
- [ ] Un push por ventana (no se repite).
- [ ] Nombre de proyecto solo si la casilla `names` está encendida.

## 3. Hallazgos (analizador de fugas)

- [x] **Un hallazgo NACE natural** (sin simulador): post-it rojo con `2`
      en la tapa del gatito y badge rojo `2` en la pestaña Hallazgos,
      encendidos solos. *2026-08-19.*
- [x] **"Leído" al clicar descuenta contador y post-it**: tras leer los
      dos hallazgos, el badge rojo de la pestaña y el post-it rojo del
      gatito se apagaron, y el turquesa del coach siguió con su `1`.
      *2026-08-19.*
- [ ] Ignorar persiste; restaurar ignorados revive las no leídas.
- [ ] Pasada ligera al cerrar una sesión (recibo) enciende el aviso.
- [~] **Temas de `inflate` (etapa 3)**: *2026-08-19*, de dos tarjetas
      `inflate` del mismo proyecto, UNA trae la capa semántica ("un solo
      tema" pegado al costo, y el consejo correcto: «un solo tema, nada
      más que muy largo → /compact, no /clear») y la otra —la más
      fresca— cae al consejo genérico. Funciona; falta entender por qué
      no llegó a la segunda. Ver rareza R3.
- [ ] Marcas de arreglo (`fndHist`): un hallazgo de estado desaparece y
      sale como arreglado.

## 4. Coach (Consejos)

- [x] **Ficha caliente en sesión REMOTA por SSH** — regla `cache`
      (pausa ≥6 min con ctx ≥30k). *2026-08-19: "El caché caduca en
      minutos · Ahora: 7 min de pausa con contexto grande — el caché ya
      venció — sparky-site · VPS-EU · «Imagen.webp a zorro-final-webp»".*
      Valida de paso: motor replicado en `meter-export.py --coach`,
      fusión con `origin`, `pname` resuelto (carpeta real `sparky-site`)
      y título de sesión en la línea "Ahora:".
- [x] **Contador de la pestaña Consejos** enciende con ficha nueva
      (badge `1` sobre "Consejos"). *2026-08-19.*
- [x] **Botonera de la ficha**: chip `/clear`, "Copiar comando",
      "Aplicar" y "ver la copia". *2026-08-19.*
- [x] **"Copiar comando" copia de verdad**: el botón pasa a "Copiado ✓".
      *2026-08-19, release* — o sea que `clipboard-manager|write_text`
      invocado a pelo funciona en el build firmado.
- [x] **Ficha caliente sobre la sesión LOCAL en curso**: la regla `cache`
      disparó sobre «Validación de funcionalidades» (michiclaude ·
      VPS-EU) mientras se hacía esta misma validación. *2026-08-19.*
- [x] **Recibo `sum` al cerrar una sesión**, completo. *2026-08-19:
      «Imagen.webp a zorro-final-webp» · Resumen de la sesión ·
      sparky-site · VPS-EU · "1 min · 4 comandos · 1 archivos editados ·
      ~$0.48" y el ⚠ "cerró con 37k tokens de contexto — el caché venció
      en la pausa".* Valida título AI, línea de hechos, `~$X` y
      `coach_leaks()` al cierre. (Ver rareza R2: el plural.)
- [ ] Push `done` / `ask` al celular.
- [ ] Tope diario de 10 fichas (con `sum` exento).
- [~] Ficha caliente que se REFRESCA sin renacer: la de `cache` se vio
      con "6 min de pausa" y antes con "7 min" en otra sesión — falta
      verla cambiar el minutaje SIN saltar de sitio, en la misma tarjeta.
- [x] **Contraer + leído**: al clicar el recibo se pliega a título +
      subtítulo y deja de contar. *2026-08-19.*
- [ ] Caducidad a 24 h.
- [ ] Post-it turquesa del coach en el gatito y clic → panel en Consejos.

## 5. Presión / contexto (`press`)

- [x] **Bombilla encendida en el gatito** con sesión con contexto.
      *2026-08-19: bombilla visible sobre la cápsula, cápsula desplazada
      hacia arriba (`body.hasidea`).*
- [x] **Ficha de contexto al hover en la bombilla**, en la MISMA ventana.
      *2026-08-19: "7% Presión de contexto · michiclaude · VPS-EU ·
      relevo" — trae número, proyecto, origen y la marca de que esa
      sesión va por el relevo.*
- [x] **Coherencia del manómetro**: el 7% de la bombilla es el mismo que
      Ajustes enseña para `pid 4020410`. *2026-08-19.*
- [ ] Arco de presión en la pastilla y número en `pcard`.
- [ ] Techo por modelo correcto (`full` del hit) — verlo con un modelo de
      1M y con uno de 200k.
- [ ] `compact_boundary` deja el manómetro en "sin medida" hasta el
      siguiente turno (no miente 10 min).

## 6. Intención (contexto ≥80%)

- [ ] Tarjeta de intención aparece sola al 80%.
- [ ] Insignia "Recomendado" solo con veredicto (unsure = sin insignia).
- [ ] "Copiar comando" pega en el portapapeles.
- [ ] Advertencia si hay pendientes.

## 7. Análisis local (IA)

- [ ] Primer `via:emb` en sesión REAL al 80%.
- [ ] **Primer auto-`/clear` por `tema_nuevo`** — interruptor
      `relayClearAi` ENCENDIDO desde 2026-08-19 (visto en Ajustes), o
      sea que la segunda razón ya está armada y solo falta que dispare.
- [ ] Muestra natural suficiente antes de tocar `EMB_NEW`/`EMB_CROSS`.
- [ ] `ai_intent` con veredicto unsure → insignia punteada propia.
- [ ] Fail-quiet: sin GGUF se comporta exactamente como la v1.
- [ ] llama-server arranca bajo demanda y **se mata** al terminar.

## 8. Relevo y automáticos (remediación)

- [x] **El relevo se anuncia en la ventana de Claude Code** — "michi ·
      relevo activo (sesión 4122038) — MichiClaude puede aplicar
      /compact y /clear en esta ventana". *2026-08-19, extensión de
      VS Code sobre SSH.*
- [x] **Copia `/export` verificada en disco y visible desde el panel** —
      "ver la copia" abre `HANDOFF-4122038-1787174044.JSONL · VPS-EU` con
      la conversación real dentro. *2026-08-19.* Es la pieza fail-closed
      del /clear automático.
- [x] **Registro de acciones** con una fila por aplicación (fecha, si fue
      `manual` o `auto`, en qué máquina) y **"ver la copia"** en cada
      una. *2026-08-19: filas del 13, 16 y 19 de agosto, en «VPS-EU» y
      «oscar».*
- [x] **Compuerta de aprendizaje** cumplida en uso real: `/compact 2 de 2`
      y `/clear 8 de 3`; los candados de los interruptores ya no se
      enseñan. *2026-08-19.* (Ver rareza R1: el marcador sigue
      prometiendo un desbloqueo que ya ocurrió.)
- [x] **Lista de sesiones con relevo** en Ajustes: proyecto, máquina,
      tipo (`chat`), pid, presión y estado `listo`, con su botón
      "Aplicar /compact" por sesión. *2026-08-19: `michiclaude` pid
      4020410 · 7% y `sparky-site` pid 4122038 · 4%.*
- [~] "Aplicar" desde el panel inyecta el comando. *2026-08-19: el
      registro anota 8 `/clear` manuales y las copias existen, así que
      la cadena entera dejó rastro — falta ver con los ojos el comando
      ATERRIZANDO en la ventana de Claude Code.*
- [ ] Auto-`/compact` real con cuenta atrás de 15 s que DICE el comando.
- [~] Auto-`/clear` automático: hay filas `auto` del 13/08 en el
      registro, así que ya disparó solo alguna vez. Falta saber POR QUÉ
      razón (Boundary vs `tema_nuevo`) — se ve en `rem_debug.json` /
      flowLog — y ver uno en vivo con el widget a la vista.
- [ ] Cualquier toque durante la cuenta atrás la para.
- [ ] Archivador (≥365 d) mueve; la purga solo borra lo ya archivado.
- [x] **El VPS solo informa** (`--du`): "LOGS EN TUS SERVIDORES (SOLO
      INFORMACIÓN) · VPS-EU · 124 archivos · 245 MB", sin botón de
      borrar al lado. *2026-08-19.*

## 9. Ruteo inteligente

- [ ] Consejero `light` en vivo con cuota ≥70.
- [ ] Primer `think-top → fable` real con cuota <50 (interruptor TOP).
- [ ] Guardián: prompt pesado frenado en haiku/sonnet, con insistencia y
      `~`.
- [ ] Escalado por el relevo (`/model <alias>`) y reenvío del prompt
      (`then`).
- [ ] Primera BAJADA SOLA real (8 ligeros + cuota ≥70 → `/model sonnet`).
- [ ] `/model` en **terminal ConPTY** — subir y bajar, con el default de
      la TUI restaurado.
- [ ] Ruteo en **WSL**.
- [ ] Medición `scan_ruteo`: lo que no casa no se factura.

## 10. Widget (pastilla, gatito, globos)

- [x] **Cápsula "Sesión X%" sobre el gatito con lectura real** (5%).
      *2026-08-19.*
- [x] **Globo de alarma anclado al gatito**, con cola apuntando al
      widget y su ✕. *2026-08-19, release* — y bien pintado, o sea que
      la CSP de release no se comió los estilos de `notif.html`.
- [ ] Que se quede hasta ✕ o abrir el panel, **y no vuelva**.
- [ ] Con el widget puesto, ese aviso NO sale además como toast de
      Windows (el toast es solo para quien no tiene widget).
- [ ] Hover lo esconde pero no cuenta como leído.
- [ ] Cerrar el globo NO cambia el dibujo del gatito.
- [~] Estados por gravedad: **`cat-fire` visto** con la alarma pendiente
      de confirmar (llamas en la laptop). *2026-08-19.* Faltan `cat-zzz`
      (semana al tope) y `cat-break` (sesión al tope).
- [x] **Post-its con sus números, los dos a la vez**: rojo `2`
      (hallazgos) y turquesa `1` (coach) en la tapa, coherentes con los
      badges de las pestañas del panel. *2026-08-19.*
- [ ] Capa: el widget no se hunde tras usar otra app a pantalla completa.
- [ ] Globo como popover con la pastilla (`body.cap`).

## 11. Panel, Reporte y fuentes

- [ ] Reporte con ≥20 fotos de cuota (sale de "juntando datos").
- [ ] "1M tok ≈ $X" con tarifa real del periodo.
- [ ] Export CSV/JSON: una fila por hecho, BOM, sin totales.
- [ ] Presupuesto semanal contra los últimos 7 días de la serie diaria.
- [ ] Detector de integridad: un `.jsonl` que encoge → "no comparable",
      nunca "bajó el consumo".
- [ ] Multiidioma: cambiar idioma repinta TODO, incluido el menú del tray.
- [ ] Auto-updater: check al arrancar y globo de versión nueva.

## 12. HUB (bloqueado)

- [ ] Todo el bloque HUB + rangos de fecha: **NO sin una segunda máquina
      con MichiClaude** (`hub-modo-equipo.md`).

---

## Rarezas / a revisar

Cada entrada: fecha, qué se vio, qué rastro mirar, y si se arregló.
**Nada se arregla sobre la marcha**: las rarezas se acumulan aquí con su
arreglo propuesto y se atacan en UNA tanda cuando Oscar lo diga (así la
sesión de validación no se convierte en sesión de código a medias).

### R1 · El marcador de desbloqueo no sabe que ya terminó — 2026-08-19

**Qué se vio:** Ajustes enseña "Aplicado por ti: /compact 2 de 2 ·
/clear 8 de 3 — el automático se desbloquea al completarlos", con los
dos cupos ya cumplidos (y el `/clear` pasado de rosca).

**Por qué pasa:** `rly_unlock` (index.html, las 8 traducciones) es una
frase ÚNICA con la coletilla en futuro, y `remUi()` la pinta siempre con
el contador crudo. Los candados `rlyAutoLock` / `rlyClearLock` sí se
esconden bien al completarse — el fallo es solo el marcador.

**Impacto:** cosmético, pero contradice a la propia UI: dice que falta
algo que ya está hecho, y enseña "8 de 3".

**Arreglo propuesto (sin hacer):** capar el contador a su tope y cambiar
la coletilla a "desbloqueado" cuando ambos cupos estén completos.

### R2 · El recibo no distingue singular de plural — 2026-08-19

**Qué se vio:** "1 min · 4 comandos · **1 archivos editados**".

**Por qué pasa:** `tip_sum_line` es una plantilla plana sin concordancia.
Afecta a 5 idiomas (ES/EN/PT/FR/DE: "1 commands", "1 files edited",
"1 comandos"…). Los tres asiáticos usan contadores y están bien.

**Impacto:** cosmético, pero es la tarjeta más visible del coach.

**Arreglo propuesto (sin hacer):** concordancia por cantidad en las
cinco plantillas afectadas (ya existe el patrón en `rly_auto_lock`).

### R3 · Un `inflate` fresco se quedó sin capa de temas — 2026-08-19

**Qué se vio:** dos tarjetas `inflate` de `michiclaude · VPS-EU`. La de
hace 2 h trae "un solo tema" y el consejo bueno (/compact). La de hace
6 min NO trae temas y cae al genérico «un /clear al cambiar de tema» —
justo el consejo que la etapa 3 vino a evitar.

**Por qué puede pasar (sin confirmar):** la segunda pasada
(`fndTopicsLater`) corre UNA vez y repinta; si el hallazgo fresco nació
después, o si el presupuesto duro de 25 s se agotó, o si no había
evidencia (`umsgs`/`crs`) suficiente, se queda sin capa. El fail-quiet
es POR DISEÑO — lo que hay que confirmar es cuál de los tres fue.

**Rastro:** al estar en release NO hay flowLog. Queda `emb_debug.txt`,
`emb_server.log` e `inflate_topics.json` en AppData: si el hallazgo
fresco no aparece como clave en el caché, es que la pasada no lo vio.

**Arreglo propuesto:** ninguno todavía — primero diagnóstico. Si resulta
ser "el hallazgo nuevo llegó tarde", la pasada de temas debería
re-lanzarse cuando aparece un `inflate` sin `topics`.

### R4 · Dos `inflate` con exactamente 61k tok — 2026-08-19 (verificar)

**Qué se vio:** las dos tarjetas de R3 marcan `61k tok` idénticos, con
turnos y costos distintos (14 turnos/$0.49 y 20 turnos/$0.63), y la más
NUEVA tiene MENOS turnos.

**Qué comprobar:** que sean dos sesiones de verdad distintas y no el
mismo hallazgo partido, y que el `value` de `inflate` sea el crecimiento
de esa sesión y no un tope que las está aplanando a las dos.

**Rastro:** `get_findings` crudo (sesión de cada tarjeta) y
`scan_local_findings`.

