# Validación pasiva — checklist vivo: **dev vs. exe**

Qué es: la lista de todo lo que solo se puede dar por bueno **usando la
app**, en dos columnas para poder CASAR una cosa con la otra. CLAUDE.md
solo apunta aquí.

## Cómo se lee

| Columna | Qué significa |
|---|---|
| **Dev** | Probado durante el desarrollo con `npm run dev`. Fuente: `bitacora.md` (fecha entre paréntesis). |
| **Exe** | Probado con el **instalador de release**, usándolo como usuario. Ronda abierta el 2026-08-19. |

Marcas: `✅` visto funcionando con datos reales · `🧪` solo con el
**simulador** · `~` parcial o con un pero · `⬜` nunca · `?` sin registro
localizado (no se rastreó a fondo la bitácora; no quiere decir que falle).

**Por qué separarlas.** Dev valida el CÓDIGO. El exe valida además la CSP
de release (invariante #3, donde se han roto los estilos históricamente),
los permisos del build firmado, el autostart, el updater y —lo más
importante— el uso REAL, sin simulador. Por eso `✅ dev + ⬜ exe` no es
"ya está probado": es "está por confirmar donde importa". Y `🧪 dev + ✅
exe` es la pareja más valiosa de todas: lo que solo se sabía fingir,
ocurriendo solo.

**Cómo se está validando esta ronda (2026-08-19):** Oscar usa el
instalador, no `npm run dev`. Consecuencias:

- **No hay flowLog (📜) ni DevTools** — son de dev. Todo rastro sale de
  `%APPDATA%\com.oscarorozco.michiclaude\` (`coach_debug.json`,
  `rem_debug.json`, `emb_debug.txt`, `inflate_topics.json`,
  `quota_debug.json`…). No pedir el flowLog.
- Las sesiones observadas viven en el VPS y llegan por SSH; el panel
  corre en el Windows de Oscar.

**Marcador (2026-08-19):** 85 filas — en Exe van **25 ✅**, 4 `~` y 56
pendientes. En Dev hay ~40 ✅ y 15 🧪 (solo simulador): esas 15 son las
que más ganan al confirmarse en el exe.

**Interruptores de Oscar (2026-08-19):** TODOS encendidos salvo *borrado
automático* (purga) y *archivar logs*. Es decir: ruteo, guardián, escalar
solo, reenviar, modelo top, bajar solo, auto-/compact, auto-/clear y el
análisis local están puestos — si algo no dispara, la causa no es un
interruptor apagado.

Regla: nada se marca `✅` en la columna Exe por haberlo visto en el
simulador ni por "debería funcionar". La evidencia va con fecha y una
frase de qué se vio. Lo raro abre entrada en §Rarezas.

---

## 1. Cuota y alarmas

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Alarma de sesión por umbral (chip) | 🧪 | ✅ | **19/08**: globo "Sesión al 10% de tu límite de 5 h · Reset en 3 h 36 min", gatito en `cat-fire`. Primera alarma REAL de la historia del proyecto. |
| Se repite cada 5 min hasta abrir el panel | 🧪 | ⬜ | |
| Varios umbrales de golpe → solo el más alto | 🧪 | ⬜ | |
| Alarma semanal al 100% (una por ventana) | ⬜ | ⬜ | Hace falta llegar al 100% de verdad. |
| Restablecimiento de ventana con confirmación | 🧪 | ⬜ | |
| 429: el gauge conserva el último dato bueno 15 min | ✅ | ⬜ | En dev se provocó (arranques seguidos → 429 de 60 min); de ahí salió la cadencia de 3 min. |
| Tray con cuota en error: "–" gris | ? | ⬜ | |
| Con widget puesto, la alarma NO sale además como toast | ? | ⬜ | Regla dura: el toast es solo para quien no tiene widget. |

## 2. Avisos al celular (ntfy)

Bloque entero **nunca validado, ni en dev** (pendiente histórico).

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Push de umbral llega al celular | ⬜ | ⬜ | |
| 100%: aviso inmediato + "ya volvió" programado **con la PC apagada** | ⬜ | ⬜ | La pieza que de verdad prueba el diseño. |
| Un push por ventana (no se repite) | ⬜ | ⬜ | |
| Nombre de proyecto solo con la casilla `names` | ? | ⬜ | Privacidad: por ntfy solo van %, horas y conteos. |

## 3. Hallazgos (analizador de fugas)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Un hallazgo NACE natural y enciende post-it + contador | 🧪 | ✅ | **19/08**: post-it rojo `2` en la tapa y badge rojo `2` en la pestaña, solos. |
| "Leído" al clicar descuenta contador y post-it | 🧪 | ✅ | **19/08**: leídos los dos, se apagaron ambos; el turquesa del coach siguió con su `1`. |
| Ignorar persiste; restaurar revive las no leídas | ? | ⬜ | |
| La pasada ligera al nacer un recibo enciende el aviso | ? | ⬜ | |
| Temas de `inflate` (etapa 3): tramos y ahorro | ✅ (17/08 nº23) | ~ | **19/08**: de dos tarjetas, una trae "un solo tema" y el consejo bueno (/compact); la más fresca cae al genérico → **R3**. |
| Marcas de arreglo (`fndHist`) | ? | ⬜ | |

## 4. Coach (Consejos)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Ficha caliente en sesión **remota por SSH** | ? | ✅ | **19/08**: regla `cache` sobre `sparky-site · VPS-EU`, con proyecto resuelto y título de sesión. Valida el motor replicado en el exportador y la fusión con `origin`. |
| Ficha caliente sobre la sesión **local en curso** | ✅ | ✅ | **19/08**: `cache` sobre «Validación de funcionalidades». |
| Contador de la pestaña Consejos | ✅ | ✅ | |
| Botonera: chip, "Copiar comando", "Aplicar", "ver la copia" | ✅ (17/08 nº2) | ✅ | |
| "Copiar comando" copia de verdad ("Copiado ✓") | ? | ✅ | **19/08**: `clipboard-manager\|write_text` a pelo funciona en el build firmado. |
| Recibo `sum` al cerrar (título AI, hechos, `~$X`, ⚠) | ? | ✅ | **19/08**: "1 min · 4 comandos · … · ~$0.48" + "⚠ cerró con 37k tokens de contexto — el caché venció en la pausa". Ver **R2**. |
| Contraer una ficha y que deje de contar | ✅ | ✅ | **19/08**: el recibo se plegó a título + subtítulo. |
| Ficha caliente que se REFRESCA sin renacer | ✅ (17/08 nº3) | ~ | Falta verla cambiar el minutaje sin saltar de sitio. |
| Push `done` / `ask` al celular | ⬜ | ⬜ | Depende de ntfy (§2). |
| Tope diario de 10 fichas (`sum` exento) | ? | ⬜ | |
| Caducidad a 24 h | ? | ⬜ | |
| Post-it turquesa → panel abierto en Consejos | 🧪 | ⬜ | El post-it se ve; falta clicarlo. |

## 5. Presión de contexto (`press`)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Bombilla encendida en el gatito | ✅ (11/08) | ✅ | **19/08**: bombilla + cápsula desplazada (`body.hasidea`). |
| Ficha de contexto al hover, en la misma ventana | ✅ (11/08) | ✅ | **19/08**: "7% Presión de contexto · michiclaude · VPS-EU · relevo" — número, proyecto, origen y marca de relevo. |
| Coherencia: el mismo % en la bombilla y en Ajustes | — | ✅ | **19/08**: 7% en las dos, para el `pid 4020410`. |
| Arco en la pastilla y número en `pcard` | ? | ⬜ | Oscar usa el estilo gatito; requiere cambiar de estilo. |
| Techo por modelo correcto (200k vs 1M) | ? | ⬜ | |
| `compact_boundary` deja "sin medida" hasta el turno siguiente | ✅ | ⬜ | Bug con autopsia en la bitácora: el manómetro mentía 10 min. |

## 6. Intención (contexto ≥80%)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Tarjeta de intención aparece sola al 80% | ✅ (12/08) | ⬜ | |
| Insignia "Recomendado" solo con veredicto | ✅ | ⬜ | |
| "Copiar comando" pega en el portapapeles | ✅ | ✅ | Mismo botón validado en la ficha del coach (§4). |
| Advertencia si hay pendientes | ✅ | ⬜ | |

## 7. Análisis local (IA)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Primer `via:emb` en sesión REAL al 80% | ⬜ | ⬜ | Pendiente histórico: en dev decidió el 2B, no los embeddings. |
| Muestra natural antes de tocar `EMB_NEW`/`EMB_CROSS` | ⬜ | ⬜ | |
| `ai_intent` con veredicto unsure → insignia punteada | ✅ (12/08) | ⬜ | |
| Fail-quiet: sin GGUF se comporta como la v1 | ✅ | ⬜ | |
| llama-server arranca bajo demanda y **se mata** | ✅ | ⬜ | |
| Primer auto-`/clear` por `tema_nuevo` | ✅ (13/08 nº5) | ⬜ | El interruptor `relayClearAi` está ENCENDIDO desde el 19/08: la segunda razón ya está armada. |

## 8. Relevo y automáticos (remediación)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| El relevo se anuncia en la ventana de Claude Code | ✅ (17/08 nº8) | ✅ | **19/08**: "michi · relevo activo (sesión 4122038)…", en la extensión de VS Code sobre SSH. |
| Copia `/export` verificada y visible desde el panel | ✅ (13/08 nº8) | ✅ | **19/08**: "ver la copia" abre `HANDOFF-4122038-…JSONL · VPS-EU` con la conversación dentro. Pieza fail-closed del /clear. |
| Registro de acciones, una fila por aplicación | ✅ (17/08 nº6-7) | ✅ | **19/08**: filas del 13, 16 y 19 de agosto, `manual` y `auto`, en «VPS-EU» y «oscar». |
| Compuerta de aprendizaje (manuales antes del automático) | ✅ | ✅ | **19/08**: `/compact 2 de 2` y `/clear 8 de 3`, candados ya ocultos. Ver **R1**. |
| Lista de sesiones con relevo (pid, presión, `listo`) | ? | ✅ | **19/08**: `michiclaude` pid 4020410 · 7% y `sparky-site` pid 4122038 · 4%. |
| "Aplicar" inyecta el comando en la sesión | ✅ (17/08) | ~ | El registro anota 8 `/clear` manuales y las copias existen; falta ver el comando ATERRIZANDO en la ventana. |
| Auto-`/compact` con cuenta atrás de 15 s que DICE el comando | ✅ (13/08 nº3) | ⬜ | |
| Auto-`/clear` disparado solo | ✅ (13/08 nº5-7) | ~ | Hay filas `auto` del 13/08 en el registro; falta saber por qué razón y verlo en vivo. |
| Cualquier toque para la cuenta atrás | ✅ | ⬜ | |
| Archivador (mueve) y purga (solo lo archivado) | ✅ (15/08 nº2) | ⬜ | |
| El VPS **solo informa** (`--du`), nunca borra por SSH | ✅ | ✅ | **19/08**: "VPS-EU · 124 archivos · 245 MB" bajo SOLO INFORMACIÓN, sin botón de borrar. |

## 9. Ruteo inteligente

Etapas 0-5c cerradas en dev el 17/08; **nada** confirmado todavía en exe.

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Hook B rutea un subagente real | ✅ (17/08 nº9-10) | ⬜ | |
| Guardián frena un prompt pesado en haiku/sonnet | ✅ (17/08 nº11) | ⬜ | |
| Escalado por el relevo (`/model <alias>`) y reenvío (`then`) | ✅ (17/08 nº13-15) | ⬜ | |
| `/model` en **terminal ConPTY** (subir y bajar) | ✅ (17/08 nº17) | ⬜ | |
| Contexto inyectado (`ctx`): Claude sugiere bajar de modelo él solo | ✅ (17/08 nº11) | ✅ | **19/08**, chat de `sparky-site`: "Para implementar los pasos 1-4 ya no hace falta Opus… puedes bajar a Sonnet con /model y ahorrar cuota". Lo escribe el modelo del chat obedeciendo las dos líneas del hook — MichiClaude nunca escribe en la conversación. |
| Consejero `light` en vivo con cuota ≥70 | ⬜ | ⬜ | Distinto de la fila de arriba: `light` es la regla del coach que alimenta la bajada sola, no el texto inyectado. |
| Primer `think-top → fable` real con cuota <50 | ⬜ | ⬜ | |
| Primera BAJADA SOLA real (8 ligeros + cuota ≥70) | ⬜ | ⬜ | **19/08, revisado y NO es fallo**: con cuota al ~50% el hecho `light` se descarta en la compuerta (`LIGHT_QUOTA_PCT`=70 sobre el PEOR de sesión/semana, index.html:9359), así que no hay ni tarjeta ni cola. Además exige los CUATRO interruptores (ruteo + guardián + escalar solo + bajar solo, index.html:11594) y el último nace apagado — **confirmado el 19/08: los cuatro están puestos**. Se espera a que la semana suba del 70%. |
| Ruteo en **WSL** | ⬜ | ⬜ | |
| Medición `scan_ruteo`: lo que no casa no se factura | ✅ | ⬜ | |

## 10. Widget (pastilla, gatito, globos)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Cápsula "Sesión X%" con lectura real | ✅ | ✅ | **19/08**. |
| Globo de alarma anclado, con cola al widget | 🧪 | ✅ | **19/08**: bien pintado en release → la CSP no se comió `notif.html`. |
| Se queda hasta ✕ o abrir el panel, **y no vuelve** | 🧪 | ⬜ | |
| Hover lo esconde pero NO cuenta como leído | 🧪 | ⬜ | |
| Cerrar el globo NO cambia el dibujo del gatito | 🧪 | ⬜ | |
| Estado `cat-fire` (alarma por confirmar) | 🧪 | ✅ | **19/08**: llamas en la laptop con la alarma viva. |
| Estados `cat-zzz` (semana al tope) y `cat-break` (sesión al tope) | 🧪 | ⬜ | |
| Post-its rojo y turquesa con sus números | 🧪 | ✅ | **19/08**: `2` y `1` a la vez, iguales a los badges del panel. |
| Capa: el widget no se hunde tras otra app a pantalla completa | ✅ | ⬜ | |
| Globo como popover con la pastilla (`body.cap`) | 🧪 | ⬜ | Requiere estilo pastilla. |

## 11. Panel, Reporte y fuentes

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Reporte con ≥20 fotos de cuota (sale de "juntando datos") | ? | ⬜ | |
| "1M tok ≈ $X" con la tarifa real del periodo | ✅ | ⬜ | |
| Export CSV/JSON: una fila por hecho, BOM, sin totales | ✅ | ⬜ | |
| Presupuesto semanal contra los últimos 7 días | ✅ | ⬜ | |
| Integridad: un `.jsonl` que encoge → "no comparable" | ✅ (15/08) | ⬜ | |
| Multiidioma repinta TODO, incluido el menú del tray | ✅ | ⬜ | |
| Auto-updater: check al arrancar y globo de versión nueva | ✅ (12/08) | ⬜ | Se probó con un release REAL: es el único bloque que nació validado en exe. |

## 12. HUB (bloqueado)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Todo el bloque HUB + rangos de fecha | ⬜ | ⬜ | **NO sin una segunda máquina con MichiClaude** (`hub-modo-equipo.md`). |

---

## El vigía (VPS) — para no tener que estar mirando

`~/.michiclaude/vigia.py` — script propio de validación, **fuera del
repo** (es herramienta personal, no producto). Lee cada 60 s los rastros
que MichiClaude ya deja en el VPS y escribe en `~/.michiclaude/vigia.log`
**solo cuando algo cambia**, con una línea "→ en el panel:" que dice qué
debería haber pasado en la UI. No habla con la API, no toca los `.jsonl`
de Claude Code y no gasta cuota: es lectura pura.

| Comando | Para qué |
|---|---|
| `python3 ~/.michiclaude/vigia.py --now` | Foto del estado actual (panel, cuota, sesiones, relevos, hits). |
| `python3 ~/.michiclaude/vigia.py --watch` | Bucle de 60 s (así corre en fondo). |
| `cat ~/.michiclaude/vigia.log` | Lo que ha pasado desde que arrancó. |
| `pkill -f "vigia.py --watch"` | Pararlo. |

**Qué ve:** latido del panel y cuota (`router_state.json`), sesiones que
mide el coach con turnos/contexto/pausa/costo (`coach_debug.json`), hits
del motor, eventos del guardián (`ruteo_log.jsonl`), sesiones bajo relevo
y sus acciones aplicadas (`relevo/*.json`, con el mismo `RELAY_FRESH`=15 s
que la app) y las copias `/export` nuevas (`handoff/`).

**Qué NO ve, y por tanto sigue necesitando ojo humano:** la UI (si salió
la tarjeta, el globo, el post-it, qué dibujo tiene el gatito), las
sesiones de Windows y WSL, y los debug del panel en AppData.

**Reparto del trabajo (de las 60 filas abiertas el 2026-08-19):**

| Quién | Cuántas | Cuáles |
|---|:--:|---|
| El vigía, solo | 12 | Casi todo el **ruteo** (el hook corre en este servidor: Hook B, guardián, escalado con reenvío, `think-top`, bajada sola), el techo por modelo y `compact_boundary`. |
| El vigía avisa → Oscar confirma de un vistazo | 15 | Alarma semanal al 100%, restablecimiento de ventana, tarjeta de intención al 80%, recibos, ficha que se refresca. |
| Solo Oscar | 32 | Todo lo visual (globos, post-its, `cat-zzz`, tray, Reporte), lo de localStorage (Ignorar, tope diario, caducidad) y lo que pasa en Windows o WSL. |
| Bloqueado | 1 | HUB: necesita segunda máquina. |

**ntfy, si se quiere:** desde el VPS se puede `curl` el canal y ver los
pushes llegar — cerraría las 4 filas Y verificaría el invariante de
privacidad leyendo los payloads (que solo viajen % y horas). Cuesta
compartir el topic, que es la contraseña del canal; se regenera después.

**Prueba de aterrizaje:** el vigía no se conforma con que el relevo diga
`ok` (eso solo prueba que lo tecleó). Tras un `/clear` o `/compact`
comprueba que el contexto de esa sesión CAE; si en 10 min no bajó, lo
anota como sospecha de que el comando no llegó.

**Trampa que ya mordió:** los umbrales están COPIADOS de la app
(`CTX_INTENT`, `LIGHT_QUOTA`, `RELAY_FRESH`…). Si se cambian en el código
y no aquí, el vigía anuncia cosas que el panel ya no hace.

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

### R5 · El primer prompt de cada sesión escapa al guardián — 2026-08-19

**Qué se vio:** en `ruteo_log.jsonl`, 16 de 159 eventos van con
`model: null` — y **13 de ellos son el PRIMER prompt de su sesión**.
Coincide con la línea que inyecta el hook: "Session model: unknown".

**Por qué pasa:** el modelo se deduce del transcript, y en el primer
prompt todavía no hay respuesta del asistente de la que sacarlo.

**Impacto:** mientras el modelo es desconocido el guardián no puede
decidir — ni frenar un prompt pesado ni escalar. O sea que abrir una
sesión en haiku y pegar de entrada algo enorme pasa sin filtro. Es
justo el momento en que más valdría. Bajo riesgo, pero es un agujero
real y silencioso.

**Arreglo posible (sin hacer):** con modelo desconocido, leer el default
que la TUI guarda (el mismo que `type_model` restaura) en vez de
rendirse; o dejar constancia en el registro de que ese prompt no se pudo
evaluar.

