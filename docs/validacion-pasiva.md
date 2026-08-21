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

**Marcador (2026-08-20, cierre de la 1.ª jornada):** 88 filas — en Exe van
**32 ✅**, 4 `~` y 52 pendientes. En Dev hay ~40 ✅ y 15 🧪 (solo
simulador): esas 15 son las que más ganan al confirmarse en el exe, y en
esta ronda cayeron seis de ellas.

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
| Tray con cuota en error: "–" gris | ? | ✅ | **21/08**: captura de Oscar en la prueba en limpio del gating v1 — token vencido, tray con "–" y el panel con la guía "corre claude una vez", sin inventar datos. |
| Con widget puesto, la alarma NO sale además como toast | ? | ⬜ | Regla dura: el toast es solo para quien no tiene widget. |

## 2. Avisos al celular (ntfy)

**CORRECCIÓN 2026-08-20:** ntfy está ENCENDIDO y publicando — la bitácora
PRO lo demuestra. Lo que sigue sin confirmarse es el último tramo: que el
push **aparezca en el celular** (`push ok` solo dice que la publicación
salió bien).

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| El panel publica el push de umbral | ⬜ | ✅ | Bitácora: `21:33:57 push ok: Sesión al 10% de tu límite de 5 h.` y varios más los días 12, 13 y 19. |
| El push de umbral **se ve en el celular** | ⬜ | ✅ | **21/08**: Oscar confirmó que los pushes llegan a su celular ("ya validé que sí funcionan"). Con esto ntfy entra VISIBLE en la v1 del lanzamiento. |
| 100%: aviso inmediato + "ya volvió" programado **con la PC apagada** | ⬜ | ⬜ | La pieza que de verdad prueba el diseño. |
| Un push por ventana (no se repite) | ⬜ | ✅ | Bitácora: el globo se reemitió 3 veces a las 21:33-21:34 y hubo **un solo** `push ok`. La dedup aguanta aunque el globo insista (ver R7). |
| Nombre de proyecto solo con la casilla `names` | ? | ~ | Los textos de la bitácora SÍ llevan proyecto (`Terminó tu sesión en sparky-site · VPS-EU`), o sea que la casilla está encendida. Falta leer un payload real para confirmar que no viaja nada más. |

## 3. Hallazgos (analizador de fugas)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Un hallazgo NACE natural y enciende post-it + contador | 🧪 | ✅ | **19/08**: post-it rojo `2` en la tapa y badge rojo `2` en la pestaña, solos. |
| "Leído" al clicar descuenta contador y post-it | 🧪 | ✅ | **19/08**: leídos los dos, se apagaron ambos; el turquesa del coach siguió con su `1`. |
| Ignorar persiste; restaurar revive las no leídas | ? | ~ | **21/08**: el panel enseña «Volver a mostrar 1 hallazgo que ocultaste», así que Ignorar SÍ persistió a la reinstalación (vive en localStorage). Falta pulsar el enlace y ver revivir la tarjeta. |
| La pasada ligera al nacer un recibo enciende el aviso | ? | ✅ | **21/08 22:03**, en la bitácora PRO y sin provocarlo: `nace tarjeta sum` → 1 s después `fnd: pasada por cierre de sesión ok, 2 tarjetas (1d)` → `fnd: AVISO ENCENDIDO (1 sin ver, de 2)`. La cadena entera —recibo, pasada, aviso— en dos segundos. |
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
| Push `done` / `ask` publicados | ⬜ | ✅ | Bitácora: `push ok: Terminó tu sesión en michiclaude · VPS-EU · 7 min, 29 turnos` y `23:20:38 push ok: Claude espera tu aprobación en sparky-site · VPS-EU · 3 min`. Justo el `ask` de la herramienta colgada que se vio esa noche: el aviso SÍ salió. |
| Tope diario de 10 fichas (`sum` exento) | ? | ⬜ | |
| Caducidad a 24 h | ? | ⬜ | |
| Compás adaptativo del coach (3 min ↔ 60 s ↔ rampa de 10 s) | ✅ | ✅ | Bitácora: `compás 180 s`, `compás 60 s (presión 8%)` y `21:50:09 compás 10 s (presión 8%, rampa)` — la rampa entra por SALTO de tokens, no por presión, y por eso funciona aunque R6 tenga dormido lo demás. |
| Post-it turquesa → panel abierto en Consejos | 🧪 | ⬜ | El post-it se ve; falta clicarlo. |

## 5. Presión de contexto (`press`)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Bombilla encendida en el gatito | ✅ (11/08) | ✅ | **19/08**: bombilla + cápsula desplazada (`body.hasidea`). |
| Ficha de contexto al hover, en la misma ventana | ✅ (11/08) | ✅ | **19/08**: "7% Presión de contexto · michiclaude · VPS-EU · relevo" — número, proyecto, origen y marca de relevo. |
| Coherencia: el mismo % en la bombilla y en Ajustes | — | ✅ | **19/08**: 7% en las dos, para el `pid 4020410`. |
| Arco en la pastilla y número en `pcard` | ? | ⬜ | Oscar usa el estilo gatito; requiere cambiar de estilo. |
| Techo por modelo correcto (200k vs 1M) | ? | ✅ | **19/08**: con `claude-opus-5` a 193k de contexto, la bombilla marcó **19%** — o sea techo de 1M, la regla "Opus y Sonnet saltaron a 1M en la 4.6" de `ctx_table()`. Prueba empírica de que acierta: la auto-compactación de Claude Code (~94%) habría entrado a 188k si el techo fuera 200k, y la sesión pasó de 185k a 197k sin compactar. |
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
| "Aplicar" inyecta el comando en la sesión | ✅ (17/08) | ✅ | La bitácora distingue las dos vías y las dos se ven: `relevo: /clear tecleado por el usuario en pid 3236` frente a `21:14:06 relevo: aplicado /clear en pid 4122038` — esta última es el botón del panel, y a esa misma hora nació `handoff-4122038-1787174044.jsonl`. |
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
| **Bitácora PRO**: botón visible en Ajustes que copia el flujo | — | ✅ | *2026-08-20*: "copiada · 300 renglones" y el contenido llegó entero. Con ella se cerró R3 en una sola pegada. |
| **Gating v1**: bloques escondidos, Reporte "Próximamente", Mayús+clic alterna, tooltip cambia de idioma | — | ✅ | **21/08**, capturas de Oscar tras instalar el build 4c26726: Ajustes sin IA/remediación/ruteo/HUB, Reporte gris ("Coming soon" en inglés), y Mayús+clic en Acerca de devolviendo todo (relevo con sesiones "listo" incluido). La desinstalación con "borrar datos locales" hizo de prueba en limpio real: AppData vacía, localStorage del panel sobrevive (vive en WebView2). |
| Auto-updater: check al arrancar y globo de versión nueva | ✅ (12/08) | ⬜ | Se probó con un release REAL: es el único bloque que nació validado en exe. |

## 12. HUB (bloqueado)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Todo el bloque HUB + rangos de fecha | ⬜ | ⬜ | **NO sin una segunda máquina con MichiClaude** (`hub-modo-equipo.md`). |

---

## Plan por fases (propuesta de Oscar, 2026-08-19)

**Premisa que hay que tener clara:** dev y el exe son EL MISMO código —
`npm run dev` y `npm run build` compilan el mismo `index.html` y el mismo
`lib.rs`. No hay funcionalidades "en dev" pendientes de portar; el exe las
lleva todas desde el primer día. Lo que hacía parecer dev más completo era
el **simulador** (finge globos, contexto e intención con un clic) y **R6**
(umbrales inalcanzables con techo de 1M).

Lo que SÍ tiene sentido escalonar son los **interruptores**: con todo
encendido a la vez, cuando algo sale raro no se sabe quién lo hizo. Orden
propuesto, cada fase se cierra antes de abrir la siguiente:

| Fase | Qué se enciende | Qué se valida | Requisito |
|---|---|---|---|
| 0 (hecha) | Lo que dispara solo | Ficha de caché, recibo, hallazgos, alarmas de cuota, consejero del ruteo | — |
| 1 | **TODO apagado**, y de ahí uno a uno | Que cada interruptor haga solo lo suyo: con todo encendido no se sabe quién actuó | Ninguno |
| 2 | Lo MANUAL de cada área | Aplicar `/compact` y `/clear` desde el panel, copiar comando, ver la copia, leer hallazgos | No necesita R6 |
| 3 | *(arreglo, no interruptor)* | **R6**: umbrales absolutos → desbloquea presión, manómetro, intención y el ⚠ de fugas | Tanda de arreglos |
| 4 | Los AUTOMÁTICOS | Cuenta atrás que dice el comando, toque que la para, copia verificada, aterrizaje | Fase 3: sin presión no hay disparo |
| 5 | Ruteo completo | Guardián que frena, escalado, reenvío, `/model` en ConPTY, bajada sola | Trabajar A PROPÓSITO en sonnet: en opus el guardián no tiene nada que frenar |
| 6 | ntfy | Los 4 pushes y el invariante de privacidad | Canal nuevo si se comparte el topic |
| 7 | Archivador y purga | Mover ≥365 d y borrar solo lo archivado | Hoy apagados a propósito |
| 8 | HUB | Bloqueado | Segunda máquina |

**Manual antes que automático** (idea de Oscar, y encaja con el diseño: la
propia app exige aplicaciones manuales antes de desbloquear el automático).
Lo manual NO necesita R6: se aplica desde el panel a mano. R6 sí bloquea el
automático — mientras siga en pie, esas reglas no llegan a dispararse.

**AVISO al apagar el ruteo:** `save_router_state` no escribe la nota con el
ruteo apagado, así que el vigía del VPS pierde el LATIDO del panel y la
CUOTA (seguirá viendo sesiones, relevos y copias). Mientras dure esa fase,
esos dos datos hay que mirarlos en el panel.

**Bitácora PRO** (2026-08-19, commit 287b559): Ajustes tiene ya su botón
propio para copiar la bitácora del flujo desde el exe — `flog()` grababa
siempre, solo faltaba poder sacarla. Es la forma de que un "no salió X"
llegue con datos. Exige recompilar en Windows.

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

**Trampa que YA mordió (2026-08-19):** los umbrales y tablas están
COPIADOS de la app (`CTX_INTENT`, `LIGHT_QUOTA`, `RELAY_FRESH`,
`ctx_table`…). El vigía nació con el techo de contexto clavado en 200k
mientras la app daba 1M a `claude-opus-5`, y anunció un "93% de contexto,
urgente" que era **falsa alarma**: eran 19%. Se arregló leyendo el modelo
del transcript y replicando `ctx_table()`. Moraleja: cuando el vigía y el
panel discrepen, **el sospechoso es el vigía** — la app tiene el dato de
primera mano.

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

**ARREGLADO (2026-08-19):** cumplidos los dos cupos, el marcador se
esconde. Se descartó capar el número y añadir una frase de "desbloqueado":
el marcador existe para VER que se acumula, y la señal de desbloqueo ya la
dan los candados al desaparecer. Cero texto nuevo en 8 idiomas.

### R2 · El recibo no distingue singular de plural — 2026-08-19

**Qué se vio:** "1 min · 4 comandos · **1 archivos editados**".

**Por qué pasa:** `tip_sum_line` es una plantilla plana sin concordancia.
Afecta a 5 idiomas (ES/EN/PT/FR/DE: "1 commands", "1 files edited",
"1 comandos"…). Los tres asiáticos usan contadores y están bien.

**Impacto:** cosmético, pero es la tarjeta más visible del coach.

**ARREGLADO (2026-08-19):** concordancia por cantidad en las cinco
plantillas (EN/ES/PT/FR/DE); JA/KO/ZH usan contadores y ya estaban bien.

### R3 · Un `inflate` fresco se quedó sin capa de temas — CERRADA 2026-08-20

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

**RESUELTO (2026-08-20) con la bitácora PRO.** La pasada corre en CADA
escaneo y va llenando lo que le cabe: `19:24:52 temas listos en 1
sesión(es)`, `21:29:33 en 1` con dos tarjetas en pantalla, y `00:39:16 en
2 sesión(es)` con tres. O sea que la cobertura CRECE sola pasada a pasada
— coherente con el presupuesto duro de 25 s y con el caché
`inflate_topics.json`, que va acumulando. No es un fallo: es el
presupuesto haciendo su trabajo, y el fail-quiet dejando el consejo
genérico mientras tanto.

**Lo único mejorable (no urgente):** mientras a una tarjeta le falta la
capa, enseña el consejo genérico "un /clear al cambiar de tema", que
puede ser el consejo MALO para esa sesión. Se podría callar el consejo
hasta que haya veredicto, en vez de dar uno que quizá no toca.

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

### R6 · Con techo de 1M, media app se queda dormida — 2026-08-19 (GORDA)

**Qué se vio:** Oscar, usando el exe con normalidad: "veo a michi muy
tranquilo, en dev veía más movimiento". Cierto, y no es cosa suya.

**Los números.** Las reglas de presión miden PORCENTAJE DEL TECHO
(`COACH_CTX_PCT`=60, `INTENT_PCT`=80, manómetro 60/85). Con
`claude-opus-5` el techo es 1M, así que hacen falta:

| Regla | Umbral | Contexto necesario |
|---|---|---|
| Ficha "compacta" y ⚠ `ctx` de `coach_leaks` | 60% | 600k |
| Tarjeta de intención, análisis local, auto-`/clear` | 80% | 800k |
| Manómetro ámbar / rojo | 60 / 85 | 600k / 850k |

Una sesión real de 197k va por ~$20. Llegar a 600k son horas y varias
veces ese gasto: **en la práctica no se alcanza nunca**. Toda la capa de
presión, intención y automáticos está apagada de hecho en los modelos de
1M — que son los que Oscar usa a diario.

**Lo que sí sigue vivo** (y por eso el coach no está mudo del todo): la
ficha de caché (pausa ≥6 min con ctx ≥30k, umbral ABSOLUTO), `done`,
`ask`, el recibo `sum`, los hallazgos, el consejero del ruteo y las
alarmas de cuota. Justo las que se han visto disparar estos días.

**Por qué se rompió:** medir en % del techo era un atajo válido cuando
todos los modelos tenían 200k. Pero **el daño es ABSOLUTO, no relativo**:
releer 200k de contexto cuesta lo mismo tenga el modelo 200k o 1M de
techo. El dinero no sabe de porcentajes.

**Arreglo propuesto (sin hacer, para la tanda):** que las reglas entren
por **lo que ocurra ANTES** — el 60/80% del techo *o* un umbral absoluto
(orden de 150k para compactar, 200k para intención). El techo sigue
mandando en el DIBUJO del manómetro (es honesto: te queda mucho), pero
no en CUÁNDO se avisa. Ojo: toca `press`, `coach_leaks`, la tarjeta de
intención y las compuertas del automático — hay que revisarlas juntas, y
el auto-`/clear` con especial cuidado.

**Nota para la bitácora:** este es el hallazgo más valioso de la ronda de
validación pasiva, y solo aparece USANDO la app. En dev estaba tapado por
el simulador, que finge justo esos estados.

### R7 · Una alarma, tres o cuatro globos — 2026-08-20 (verificar)

**Qué se vio** en la bitácora PRO:

```
21:33:56 · globo alarm: Sesión al 10% de tu límite de 5 h.
21:33:57 · push ok:     Sesión al 10% de tu límite de 5 h.
21:34:09 · globo alarm: Sesión al 10% de tu límite de 5 h.
21:34:12 · globo alarm: Sesión al 10% de tu límite de 5 h.
```

Tres renglones de globo en 16 segundos para UNA alarma. Se repite los
días 12, 13 y 19 (dos o tres cada vez).

**Lo que NO es:** no es la repetición cada 5 min (los intervalos son de
segundos), y **no es spam al celular**: el `push ok` sale una sola vez,
o sea que la dedup por ventana aguanta.

**Sospecha:** cada vez que la ventana del globo se recarga emite
`notif:ready` y el panel le reenvía el aviso — que es el comportamiento
QUERIDO (esconder con hover no cuenta como leído), y `flog()` lo apunta
otra vez. Si es eso, el ruido está en la bitácora, no en la pantalla.

**Cómo distinguirlo:** mirar si en pantalla aparece un globo o varios.
Si es uno solo, sobra el `flog` en la vía de restauración; si son
varios, es un fallo de verdad.

