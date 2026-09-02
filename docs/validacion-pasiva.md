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

**Actualización 2026-08-24 (jornada 3, uso normal):** caen 3 filas más en
Exe —el `/clear` tecleado por ti detectado y contado, el globo `cleared`
con su visor, y el ✕ que despacha una ficha— y se abre **R8** (el contador
de la compuerta de aprendizaje volvió a empezar). La presión de contexto
midió 12-13% en dos sesiones largas: **R6** sigue tal cual.

**Interruptores de Oscar — VIGENTE (2026-08-25, capturas).** Compuerta de
aprendizaje **reganada** tras R8 (el marcador ya no se pinta = los dos cupos
cumplidos), y con ella se abre la **fase 4**:

- **Automáticos:** auto-`/compact` ENCENDIDO, auto-`/clear` ENCENDIDO,
  `/clear` por análisis local APAGADO (pide la IA local, que sigue apagada).
- **Relevo:** chat de VS Code ✓ y terminales Linux ✓; el atajo del PATH
  APAGADO **a propósito** — Oscar trabaja en el chat de VS Code contra el
  VPS y ese camino no pasa por ahí (ver la etiqueta nueva, R15).
- **Ruteo: TODO apagado** — guardián, escalar solo, reenviar, bajar solo,
  modelo top e inyectar contexto. Es la fase 5, aún no tocada: si algo del
  ruteo «no dispara», la causa es esta.

Antes de dar por «no dispara» nada, mirar el interruptor. Lo de abajo son
fotos viejas:

**Interruptores de Oscar — CADUCADO, ver R8.** La reinstalación del 21/08
devolvió a fábrica lo que vivía en localStorage: el 24/08 el auto-`/compact`,
el auto-`/clear` y el `/clear` por análisis local están APAGADOS, y la
compuerta de aprendizaje volvió a 0/2 y 1/3.

**Interruptores de Oscar (2026-08-19, ya no vale):** TODOS encendidos salvo *borrado
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
| El push de umbral **se ve en el celular** | ⬜ | ✅ | **21/08**: Oscar confirmó que los pushes llegan a su celular ("ya validé que sí funcionan"). Con esto ntfy entra VISIBLE en la v1 del lanzamiento. **24/08**: repetida la prueba tras reinstalar sobre el build nuevo — siguen llegando (el canal sobrevive a la reinstalación: el topic vive en `ntfy_config.json`, no en localStorage). |
| 100%: aviso inmediato + "ya volvió" programado **con la PC apagada** | ⬜ | ⬜ | La pieza que de verdad prueba el diseño. |
| Un push por ventana (no se repite) | ⬜ | ✅ | Bitácora: el globo se reemitió 3 veces a las 21:33-21:34 y hubo **un solo** `push ok`. La dedup aguanta aunque el globo insista (ver R7). |
| Nombre de proyecto solo con la casilla `names` | ? | ~ | Los textos de la bitácora SÍ llevan proyecto (`Terminó tu sesión en sparky-site · VPS-EU`), o sea que la casilla está encendida. Falta leer un payload real para confirmar que no viaja nada más. |

## 3. Hallazgos (analizador de fugas)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Un hallazgo NACE natural y enciende post-it + contador | 🧪 | ✅ | **19/08**: post-it rojo `2` en la tapa y badge rojo `2` en la pestaña, solos. |
| "Leído" al clicar descuenta contador y post-it | 🧪 | ✅ | **19/08**: leídos los dos, se apagaron ambos; el turquesa del coach siguió con su `1`. |
| Ignorar persiste; restaurar revive las no leídas | ? | ✅ | **21/08**: «Volver a mostrar 1 hallazgo que ocultaste» sobrevivió a la reinstalación. **24/08**: pulsado — la tarjeta revivió arriba del todo (`inflate` de 12 turnos, `$0.41 · 62k tok`, «hace 7 min») y el enlace desapareció. Ciclo cerrado: ocultar, persistir, restaurar. |
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
| El ✕ despacha la ficha y apaga el aviso | ✅ | ✅ | **24/08**: `17:51:01 tips: ✕ despacha sum\|1a6e69e2` → `tips: aviso apagado`, mismo segundo. El recibo había nacido a las 17:26 y encendido el aviso él solo. |
| Ficha caliente que se REFRESCA sin renacer | ✅ (17/08 nº3) | ~ | Falta verla cambiar el minutaje sin saltar de sitio. |
| Push `done` / `ask` publicados | ⬜ | ✅ | Bitácora: `push ok: Terminó tu sesión en michiclaude · VPS-EU · 7 min, 29 turnos` y `23:20:38 push ok: Claude espera tu aprobación en sparky-site · VPS-EU · 3 min`. Justo el `ask` de la herramienta colgada que se vio esa noche: el aviso SÍ salió. |
| Tope diario de 10 fichas (`sum` exento) | ? | ⬜ | |
| Caducidad a 24 h | ? | ⬜ | |
| Compás adaptativo del coach (3 min ↔ 60 s ↔ rampa de 10 s) | ✅ | ✅ | Bitácora: `compás 180 s`, `compás 60 s (presión 8%)` y `21:50:09 compás 10 s (presión 8%, rampa)` — la rampa entra por SALTO de tokens, no por presión, y por eso funciona aunque R6 tenga dormido lo demás. |
| Post-it turquesa → panel abierto en Consejos | 🧪 | ✅ | **24/08**: post-it turquesa `1` en la tapa del portátil → clic → panel en **Consejos** con el badge `1` y la ficha «El caché caduca en minutos» arriba (`6 min de pausa con contexto grande`, michiclaude · VPS-EU). El salto directo a la pestaña funciona. |

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
| Tarjeta de intención aparece sola (al umbral) | ✅ (12/08) | ✅ | **24/08 17:40:39**, primera vez en exe y a los pocos minutos de instalar R6: `nace tarjeta intent` junto a `nace tarjeta compact`, con **26% de presión** (~262k de un techo de 1M). Con la regla vieja hacía falta 80% = 800k: no habría nacido nunca. |
| Insignia "Recomendado" solo con veredicto | ✅ | ✅ | **24/08**: la tarjeta salió con «RECOMENDADO» en *Ya terminé, empiezo algo nuevo* (`/clear`) y sin insignia en la otra. El veredicto era boundary, y la línea de pruebas lo dice en llano: «Michi detectó: commit reciente sin cambios después · último mensaje hace 2 min». |
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
| Compuerta de aprendizaje (manuales antes del automático) | ✅ | ✅ | **19/08**: `/compact 2 de 2` y `/clear 8 de 3`, candados ya ocultos. Ver **R1**. **25/08, RE-GANADA sobre el archivo de disco** tras el borrón de R8: el marcador desapareció y el registro enseña los `/clear` tecleados en «oscar» y el `/compact` aplicado en «VPS-EU». Cuentan las DOS vías —tecleado por ti y botón «Aplicar»—, `relayMark` se llama en las dos (index.html:11463 y 11606). |
| Lista de sesiones con relevo (pid, presión, `listo`) | ? | ✅ | **19/08**: `michiclaude` pid 4020410 · 7% y `sparky-site` pid 4122038 · 4%. |
| El relevo ve un `/clear` **tecleado por ti** y lo cuenta | ✅ (17/08) | ✅ | **24/08 18:37**: `relevo: /clear tecleado por el usuario en pid 3695326` → `/clear aplicado a mano (1/3)`, sin tocar el panel. Ojo al contador: ver **R8**. |
| Globo `cleared` + clic → la conversación anterior (`read_cleared`) | ✅ (16-17/08) | ✅ | **24/08**: globo anclado al gatito «/clear en michiclaude · VPS-EU — la conversación anterior quedó guardada. Clic para verla» y el visor abriéndola entera. Es la vía SIN copia handoff (el `/clear` tuyo no la deja): lee el `.jsonl` de la sesión por SSH. Cierra además la trampa del *sid vivo* del 17/08 — la conversación era la correcta. |
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
| Hook B rutea un subagente real | ✅ (17/08 nº9-10) | ✅ | **24/08**, tarjeta de ruteo del Reporte (7 d): «1 subagente ruteado (1 → Haiku, 0 → Sonnet)», casado con su transcript. No se vio en vivo: se ve MEDIDO después, que es más fuerte. |
| Guardián frena un prompt pesado en haiku/sonnet | ✅ (17/08 nº11) | ✅ | **24/08**: «el guardián frenó 3 prompts pesados en un modelo barato antes de gastar un token · insististe 1». Los tres frenados y la insistencia, en uso normal. |
| Escalado por el relevo (`/model <alias>`) y reenvío (`then`) | ✅ (17/08 nº13-15) | ✅ | **24/08**, tarjeta de ruteo (30 d): «el guardián frenó **23** prompts pesados… insististe 9» y «**13 de esos se escalaron solos** (la sesión subió y tú solo reenviaste) · 5 reenviados por ti». Las dos vías, medidas contra los transcripts. |
| `/model` en **terminal ConPTY** (subir y bajar) | ✅ (17/08 nº17) | ⬜ | |
| Contexto inyectado (`ctx`): Claude sugiere bajar de modelo él solo | ✅ (17/08 nº11) | ✅ | **19/08**, chat de `sparky-site`: "Para implementar los pasos 1-4 ya no hace falta Opus… puedes bajar a Sonnet con /model y ahorrar cuota". Lo escribe el modelo del chat obedeciendo las dos líneas del hook — MichiClaude nunca escribe en la conversación. |
| Consejero `light` en vivo con cuota ≥70 | ⬜ | ⬜ | Distinto de la fila de arriba: `light` es la regla del coach que alimenta la bajada sola, no el texto inyectado. |
| Primer `think-top → fable` real con cuota <50 | ⬜ | ⬜ | |
| Primera BAJADA SOLA real (8 ligeros + cuota ≥70) | ⬜ | ⬜ | **19/08, revisado y NO es fallo**: con cuota al ~50% el hecho `light` se descarta en la compuerta (`LIGHT_QUOTA_PCT`=70 sobre el PEOR de sesión/semana, index.html:9359), así que no hay ni tarjeta ni cola. Además exige los CUATRO interruptores (ruteo + guardián + escalar solo + bajar solo, index.html:11594) y el último nace apagado — **confirmado el 19/08: los cuatro están puestos**. Se espera a que la semana suba del 70%. |
| Ruteo en **WSL** | ⬜ | ⬜ | |
| Medición `scan_ruteo`: lo que no casa no se factura | ✅ | ✅ | **24/08**: «1 casados con su transcript · 9783 tokens · costaron $0.01 · habrían costado $0.13» → ahorro $0.12, y aparte el autoconsumo «~4440 tokens (74 contextos inyectados)». Un ruteado, un casado: no se facturó nada sin transcript. |

## 10. Widget (pastilla, gatito, globos)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Cápsula "Sesión X%" con lectura real | ✅ | ✅ | **19/08**. |
| Globo de alarma anclado, con cola al widget | 🧪 | ✅ | **19/08**: bien pintado en release → la CSP no se comió `notif.html`. |
| Se queda hasta ✕ o abrir el panel, **y no vuelve** | 🧪 | ⬜ | |
| Hover lo esconde pero NO cuenta como leído | 🧪 | ✅ | **24/08**: confirmado por Oscar — el globo se esconde al pasar por encima y vuelve al salir. |
| Cerrar el globo NO cambia el dibujo del gatito | 🧪 | ⬜ | |
| Estado `cat-fire` (alarma por confirmar) | 🧪 | ✅ | **19/08**: llamas en la laptop con la alarma viva. |
| Estados `cat-zzz` (semana al tope) y `cat-break` (sesión al tope) | 🧪 | ⬜ | |
| Globo resumen al hover en `.head` (sesión + semanales) | ✅ | ✅ | **24/08**: tarjeta blanca sobre el gatito con cola al portátil — `Session 5%`, `Weekly 14%`, `Weekly · Fable 19%` y sus resets. Los buckets POR MODELO se pintan solos (invariante #6): «Fable» salió sin estar en ninguna lista. |
| Post-its rojo y turquesa con sus números | 🧪 | ✅ | **19/08**: `2` y `1` a la vez, iguales a los badges del panel. |
| Capa: el widget no se hunde tras otra app a pantalla completa | ✅ | ✅ | **24/08**: vídeo a pantalla completa, gatito encima; repetido con el estilo pastilla. |
| Globo como popover con la pastilla (`body.cap`) | 🧪 | ✅ | **24/08**: aviso de presupuesto como popover pegado a la cápsula, fondo opaco, ⚠ ámbar de severidad y cola pequeña — sobre un vídeo a pantalla completa. |

## 11. Panel, Reporte y fuentes

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Reporte con ≥20 fotos de cuota (sale de "juntando datos") | ? | ✅ | **24/08**: los TRES modos —Semana, Mes ($2420 en 30 d) y Personalizado— pintan enteros; desbloqueado con Mayús+clic — héroe, desperdicio, ruteo, «¿te duró más o menos?» (con las fotos de cuota reales: 0 topes de 5 h), gráfica de 4 semanas, proyectos, arreglos y tareas. Cuadra por dentro (ver **R10**/**R12** para los dos peros). |
| "1M tok ≈ $X" con la tarifa real del periodo | ✅ | ✅ | **24/08**: «1M tok ≈ $31» y todo el Reporte casa con esa tarifa — 36.577 tok/mensaje ≈ $1.12, 5.3M ≈ $164. Es la tarifa MEDIDA del periodo, no una de tabla. |
| Export CSV/JSON: una fila por hecho, BOM, sin totales | ✅ | ✅ | **24/08**: CSV de 30 d abierto en Excel — cabeceras con acentos correctos (BOM), columnas Fecha · Proyecto · Modelo · Origen · Costo estimado (USD) · Tokens, una fila por fecha×proyecto×modelo×origen, sin fila de totales, y `Local` / `VPS-EU` conviviendo. El aviso «CSV exportado» con su botón **Abrir** también va. |
| Presupuesto semanal contra los últimos 7 días | ✅ | ✅ | **24/08**: puesto en $100 → globo «El gasto semanal $382.30 superó tu presupuesto de $100 (equiv. API)»; cambiado a $200 → volvió a avisar con la cifra nueva. Se ve la pega de UX en **R13**. |
| Integridad: un `.jsonl` que encoge → "no comparable" | ✅ (15/08) | ✅ | **24/08**, Reporte en **Mes**: banda «Comparación no concluyente · 1 día(s) con trabajo ya no aparecen en los logs», insignia `no comparable` junto al número y la frase honesta («lo que parece un cambio podría ser solo lo que falta»). La pieza 2 del ADR, disparada sola por los logs reales. |
| Multiidioma repinta TODO, incluido el menú del tray | ✅ | ✅ | **24/08**: panel entero en inglés (Overview/Data sources/Findings/Tips/Report/Preferences) **y el globo resumen del gatito también** («Session / Weekly / Weekly · Fable / Resets in 19 min») — o sea que el idioma llega a las ventanas del widget, no solo al panel. El **menú del tray** era lo único que faltaba y tardó porque fallaba callado (**R14**): ya dice **Abrir panel · Widget flotante · Salir**, y el tooltip «Sesión 0% · Semanal 16%». |
| **Bitácora PRO**: botón visible en Ajustes que copia el flujo | — | ✅ | *2026-08-20*: "copiada · 300 renglones" y el contenido llegó entero. Con ella se cerró R3 en una sola pegada. |
| **Gating v1**: bloques escondidos, Reporte "Próximamente", Mayús+clic alterna, tooltip cambia de idioma | — | ✅ | **21/08**, capturas de Oscar tras instalar el build 4c26726: Ajustes sin IA/remediación/ruteo/HUB, Reporte gris ("Coming soon" en inglés), y Mayús+clic en Acerca de devolviendo todo (relevo con sesiones "listo" incluido). La desinstalación con "borrar datos locales" hizo de prueba en limpio real: AppData vacía, localStorage del panel sobrevive (vive en WebView2). |
| Auto-updater: check al arrancar y globo de versión nueva | ✅ (12/08) | ⬜ | Se probó con un release REAL: es el único bloque que nació validado en exe. |
| **Instalador con marca propia** (huella, español, icono) | — | ✅ | **21/08 (madrugada del 22)**: las tres pantallas con el lateral y la cabecera azules, textos en español por el idioma de Windows, icono de la huella en el setup y en el acceso directo. Oscar desinstaló y reinstaló: así se fue también la caché de iconos. |

## 12. HUB (bloqueado)

| Qué | Dev | Exe | Evidencia / nota |
|---|:--:|:--:|---|
| Rangos de fecha (chip **Personalizado** del Reporte) | ✅ | ✅ | **24/08**: calendario con atajos Hoy/7/15/30 d, «10 ago → 24 ago» aplicado y el Reporte entero recalculado (24.6M tok ≈ $991, día más pesado 17/Ago). La nota del hub sale sola. Lo que sigue bloqueado es el HUB, no el rango. |
| Todo el bloque HUB | ⬜ | ⬜ | **NO sin una segunda máquina con MichiClaude** (`hub-modo-equipo.md`). |

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

**ARREGLADO (2026-08-24), las dos cosas:** el respaldo de `settings.json`
ya existía (`modelo_sesion`), pero solo sirve si tienes un modelo por
defecto puesto — Oscar lo tuvo a partir del 19/08 y por eso los 16 nulos
son todos del 17 y el 19 (31 eventos seguidos con modelo desde entonces).
Ahora hay un último recurso, `modelo_ultimo_del_proyecto()`: el modelo que
ESA carpeta ya venía usando, según `projects[<cwd>].lastModelUsage` de
`~/.claude.json`, y **solo si hay uno** — con varios no se adivina, que un
guardián equivocado de modelo es peor que uno callado. Y si aun así no se
sabe, se apunta `ev:"noeval"` en el registro: un agujero contable se
puede medir, uno silencioso no.

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

**ARREGLADO (2026-08-24), con los números acordados con Oscar:** las
reglas entran por **lo que ocurra ANTES** — el % del techo *o* un suelo
absoluto. Cinco piezas, todas con su gemelo (invariante #1):

| Pieza | Dónde | Umbral nuevo |
|---|---|---|
| Ficha «compacta» y ⚠ `ctx` del recibo | `ctx_alto()` en lib.rs + réplica en `meter-export.py` | 60% del techo **o 150k** (`COACH_CTX_ABS`) |
| Tarjeta de intención y compuerta del AUTOMÁTICO (`relayAutoCheck`) | `pressHot()` | 80% **o 200k** (`CTX_ABS_INTENT`) |
| Color del manómetro y bombilla | `pressLevel()` → viaja como `lvl` dentro de `press` | 40/60/85% **o** 100k/150k/200k |
| Compás del coach (20 s / 10 s) | `coachSched` | 55/70% **o** 150k/200k |

El **DIBUJO no cambia**: el arco y el número siguen siendo % del techo,
que es lo honesto (te queda mucho depósito). Lo que cambia es CUÁNDO
avisa. Y el `lvl` se decide UNA vez, en el panel: que cada ventana del
widget recalculara su propio 60/85 era el mismo bug repetido tres veces.

**VALIDADA EN VIVO el mismo día (2026-08-24 17:40:39)**, a los minutos de
instalar el build:

```
17:40:39 · coach: nace tarjeta compact|362dfab7 (michiclaude)
17:40:39 · coach: nace tarjeta intent|362dfab7 (michiclaude)
17:40:39 · tips: AVISO ENCENDIDO (2 sin ver)
17:40:39 · coach: compás 10 s (presión 26%)
```

Las tres piezas a la vez y con **26%** de presión (~262k de un techo de
1M): la ficha de compactar (suelo 150k), la tarjeta de intención (suelo
200k) y el compás en 10 s (suelo 200k). Con la regla vieja habrían hecho
falta 600k y 800k. En pantalla: la tarjeta «Tu sesión ya pesa mucho ·
26%» con sus dos opciones y «RECOMENDADO» en el `/clear`, la bombilla del
gatito encendida y su ficha «26% Memoria de la conversación · michiclaude
· VPS-EU · relevo». **La regla que llevaba dormida desde que existen los
modelos de 1M funcionó a la primera.**

**Medida nueva (2026-08-24):** dos sesiones largas de trabajo real en
`michiclaude · VPS-EU` marcaron **12-13%** de presión durante toda la
tarde (bitácora PRO, 17:15 → 18:37). Ni se acercó al 60%. R6 intacta.

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

**Se repite con OTRO tipo de globo (24/08):** el del presupuesto, cuatro
renglones —`20:17:18`, `20:17:32` ×2, `20:20:23`— para un solo aviso. O
sea que no es cosa de las alarmas: es la vía de restauración del globo,
común a todos. Refuerza la sospecha de abajo.

**Cómo distinguirlo:** mirar si en pantalla aparece un globo o varios.
Si es uno solo, sobra el `flog` en la vía de restauración; si son
varios, es un fallo de verdad.

### R8 · El contador de la compuerta de aprendizaje volvió a empezar — 2026-08-24

**Qué se vio:** al teclear un `/clear` a mano, la bitácora PRO anota
`relevo: /clear aplicado a mano (1/3)`. El 19/08 ese mismo contador iba
por **8 de 3** (§8, fila de la compuerta). O sea que `relayDone` está en 1.

**Por qué importa:** `relayUnlocked()` lee ese contador, así que el
auto-`/clear` está **bloqueado otra vez** — pide 2 aplicaciones manuales
más. (En la práctica R6 ya lo tenía dormido: el automático exige presión
≥80%, que con techo de 1M no llega. Pero conviene saber cuál de los dos
candados está echado.)

**Sospechoso:** la desinstalación con «borrar datos locales» del 21/08.
Contradice la nota de esa fecha —«localStorage del panel sobrevive»,
deducida de que el hallazgo ignorado seguía ahí—, así que una de las dos
observaciones necesita repaso: puede que sobreviviera una parte
(WebView2 en `AppData\Local`) y se perdiera otra, o que el hallazgo
ignorado se volviera a ocultar después.

**CONFIRMADO (2026-08-24):** el marcador dice «Aplicado por ti: **/compact
0 de 2** · /clear 1 de 3». El `/compact` iba por 2 de 2 el 19/08, así que
la cuenta se borró ENTERA: fue la desinstalación con «borrar datos
locales», no un fallo del contador. La nota del 21/08 («localStorage del
panel sobrevive») queda **corregida**: no sobrevivió — lo que se vio
entonces fue un hallazgo ocultado DESPUÉS.

**Y no solo el contador:** en la misma captura, *Aplicar /compact en
automático*, *Aplicar /clear en automático* y *…cuando el análisis local
diga «tema nuevo»* están **APAGADOS**. La reinstalación devolvió los
interruptores a fábrica. Ver el aviso de interruptores al principio.

**ARREGLADO (2026-08-24):** la compuerta vive en DISCO —`relay_gate.json`
en AppData, comando `relay_gate(add, seed)`— y localStorage queda como
ESPEJO, nunca como jefe. La primera carga manda lo que haya en
localStorage como `seed` y Rust fusiona **por el máximo**, así la cuenta
que sobreviva sube al archivo y no puede bajar. `relayGate` es la copia en
memoria porque las compuertas se consultan en caliente y no pueden ser
async. Lo que ya se borró no vuelve: la cuenta empieza donde está hoy.

### R9 · La Bitácora PRO va en UTC y la interfaz en tu hora — 2026-08-24

**Qué se vio:** el mismo `/clear` sale como `18:37:03` en la bitácora PRO
y como `24/08 12:34 p.m.` en el registro de acciones. Seis horas de
diferencia (y tres minutos, que son el sondeo del relevo: a las 12:34 lo
tecleaste, a las 12:37 lo vio el panel).

**Por qué pasa:** `flog()` sella con `new Date().toISOString()`
(index.html:1977) — eso es **UTC**. El resto de la interfaz usa la hora
local (`fmtWhen`, `toLocaleTimeString`). Ninguno de los dos miente; es que
no hablan la misma hora.

**Impacto:** solo al depurar, pero es justo cuando más duele — casar «lo
que vi en pantalla» con «lo que dice la bitácora» obliga a sumar seis
horas a mano, y la trampa es silenciosa (las dos horas parecen válidas).

**ARREGLADO Y VISTO (2026-08-24):** `flog()` sella en hora local con el
mismo formato `MM-DD HH:MM:SS`. En la misma bitácora se ve el salto: los
renglones viejos llegan hasta `21:39` (UTC) y los nuevos empiezan en
`17:39` (local, seis horas menos) — el mismo instante, contado ya como lo
cuenta la interfaz. Los renglones ya escritos siguen en UTC (viven en
localStorage); a partir del próximo arranque, bitácora e interfaz cuentan
la misma hora. Sin `cargo check`: es solo frontend.

### R10 · «Esta semana» tiene dos cifras en la misma pestaña — 2026-08-24

**Qué se vio** en el Reporte con el chip *Semana*:

| Dónde | Trabajo | Coste | Tok/mensaje |
|---|---|---|---|
| Héroe, arriba | 5.3M tok | $164 | 36.577 |
| Gráfica de 4 semanas, punto «esta» | 3.9M tok | $117 | 33.463 |

**Por qué pasa:** son dos ventanas distintas con el mismo nombre. El héroe
—y la lista de proyectos, que suma $163.82 y cuadra con él— usa la ventana
del motor: `window_ago = end - Duration::days(7)` (lib.rs:2092), o sea
**7×24 h rodando desde este instante**, que incluye la COLA del octavo día
natural. La gráfica suma **7 fechas del calendario** de la serie diaria
(`repWeeks`, index.html:10205). Con el 17/08 (un día de ~4.5M tok) justo en
esa cola, la diferencia es 1.4M tok / $47.

**Impacto:** ninguna de las dos miente, pero conviven a dos pantallazos de
distancia y el usuario no tiene forma de saber por qué. Peor: el «↓71%
menos que el periodo anterior» y el «↓29% más barato que la anterior»
comparan cosas distintas.

**ARREGLADO (2026-08-24):** las ventanas del Reporte terminan al **cierre
del día**, no en «ahora» (`repDayEnd()`, y `repArgs`/`repEnd` lo pasan como
`end`). Con eso la ventana de 7 d son exactamente los 7 días naturales que
suma la gráfica, y lo mismo para el periodo ANTERIOR, el desperdicio y su
«antes». El ancla es **UTC** porque la serie diaria agrupa por fecha UTC
(en Rust los `ts` se pasan a Utc antes de formatear); anclarlo en local
habría descuadrado medio día. El motor no se tocó: sigue entendiendo ancho
+ final (invariante #1) — solo se le dice dónde termina.

**CONFIRMADO EN PANTALLA (2026-08-24, exe):** con el chip *Semana* el héroe
dice **4.4M tok ≈ $131** y el punto «esta» de la gráfica de 4 semanas dice
**4.4M tok ≈ $131**, con el mismo tok/mensaje (34.469) en los dos sitios.
Las dos cifras que estaban a dos pantallazos de distancia ya son la misma.

### R11 · El título de «Desperdicio estructural» sale cortado — 2026-08-24

**Qué se vio:** «DESPERDICIO ESTRUCTUR… es un piso — hay más que no se
puede medir». El título con puntos suspensivos; la coletilla, entera.

**Por qué pasa:** `.eyebrow>span:first-child{overflow:hidden;
text-overflow:ellipsis}` y `.q{flex:0 0 auto}` (index.html:373-374). La
regla se escribió para las cabeceras cuyo lado derecho es un CONTROL (el
selector de rango, el conmutador tokens/$): ahí el título cede a propósito.
Pero en las que llevan solo una coletilla de texto, el que cede es el dato
importante.

**Impacto:** cosmético y solo en español (el título es más largo que en
inglés), pero está en la tarjeta que da el número más delicado del Reporte.

**ARREGLADO (2026-08-24):** `.eyebrow` envuelve (`flex-wrap:wrap`) y el
título ya no baja de `min-width:min-content`, así que cuando no caben los
dos es la COLETILLA la que se va a la segunda línea. El selector de rango
conserva su `white-space:nowrap` propio: sigue sin partirse.

### R12 · El total del periodo sale $164 arriba y $163 en desperdicio — 2026-08-24

**Qué se vio:** el héroe dice «5.3M tokens ≈ $164 estimado» y la tarjeta de
desperdicio, justo debajo, «$25 de $163 del periodo». En **Mes** el hueco
era mucho mayor: **$2418 arriba y $2381 abajo**.

**CAUSA CONFIRMADA (2026-08-24) — es el HUB.** (Y no el desfase de
instantes, que fue mi primera sospecha y era falsa.) Prueba, medida en el
VPS con los mismos datos y la misma ventana:

```
días=7   cost_week=140.0820   waste.total_cost=140.0820   dif=0.0000
días=30  cost_week=2346.3421  waste.total_cost=2346.3421  dif=0.0000
```

Los dos caminos coinciden AL CÉNTIMO en una máquina sola: el motor está
bien. El hueco aparece al FUSIONAR: las máquinas que dejan su foto en el
hub mandan `waste` en **ceros** (se ve en el propio export:
`"waste": {"struct_cost": 0.0, "total_cost": 0.0, ...}`), así que su gasto
entra en el total del héroe y no en el denominador del desperdicio. La foto
de `OSCAR-HUAWEI` suma **$37.03** en 30 días y **$0.357** en 7 — que son
exactamente los dos huecos vistos.

**De paso, descartado un susto:** NO hay doble conteo. `oscar ·
OSCAR-HUAWEI` es la máquina de Oscar entrando por el hub, y su escaneo
local no la duplica (el panel no encontró ningún proyecto con etiqueta
local). El Reporte etiqueta el origen local con el nombre del equipo y el
CSV lo llama `Local`: misma fuente, dos nombres — ojo al comparar.

**ARREGLADO (2026-08-24):** el denominador pasa a ser el `cost_week` que YA
enseña el héroe, con respaldo al de siempre si no hay cifra. Dividir por el
total pequeño INFLABA el porcentaje; con el grande el número es más
conservador, que es el único lado seguro para un dato que se anuncia como
«al menos». La regla queda escrita en `presion-y-rendimiento.md` §Reglas de
cálculo, porque contradice el «mismos orígenes» de la línea de arriba y eso
tiene que estar dicho, no escondido.

**CONFIRMADO en pantalla (24/08, tras instalar):** Reporte en Mes,
«55.5M tokens ≈ **$2393**» arriba y «~$242 de **$2393** del periodo» en
desperdicio. El mismo número en las dos tarjetas.

### R13 · El presupuesto semanal se guarda a escondidas — 2026-08-24

**Qué se vio:** el aviso FUNCIONA (globo con «$382.30 superó tu presupuesto
de $100», y otra vez con $200 al cambiarlo). La pega es la casilla: justo
encima, las alarmas de sesión tienen **chips** (`80% ✕`, `95% ✕`) y un botón
**Agregar**; el presupuesto es un `<input>` pelado que guarda en el evento
`change` (index.html:12556) — sin botón, sin chip, sin «guardado ✓». Tecleas
100, te vas, y no sabes si quedó.

**Impacto:** UX. Dos controles hermanos, en la MISMA tarjeta, con dos
lenguajes distintos: uno te confirma y el otro no. En un ajuste que dispara
avisos, «no sé si se guardó» es peor que en cualquier otro sitio.

**ARREGLADO (2026-08-24):** misma forma que las alarmas — chip del valor
activo (`$100 ✕`, quitarlo = 0 = sin aviso) e input con botón. El botón
reusa `cal_apply` («Aplicar»/«Apply»), que ya está en los 8 idiomas: cero
texto nuevo. El `change` del input se conserva, así que quien teclea y se
va con Tab tampoco pierde el valor. **VISTO (24/08)**: chip `$100 ✕` sobre
el input y el botón «Aplicar» al lado, con la misma pinta que las alarmas
de arriba.

### R14 · El menú del tray y su globito siguen en inglés — 2026-08-24

**Qué se vio** (exe, con toda la app en español): clic derecho en el icono
de bandeja → *Open panel / Floating widget / Quit*. Y al pasar el ratón por
encima, *«Session 27% · Resets in 7 min · Weekly 16%»*.

**Lo que YA estaba puesto:** el menú lo construye Rust al arrancar en inglés
y el panel se lo manda traducido con `set_tray_menu` desde `applyI18n()`
(hecho el 2026-07-29, invariante #10). El diccionario tiene las tres
etiquetas y `tray_tip` en los 8 idiomas. O sea: la lógica está, lo que falla
es la EJECUCIÓN.

**Dos causas posibles, tapadas las dos (2026-08-24):**

1. **Hilo equivocado.** En Windows los menús nativos solo se pueden crear y
   asignar desde el hilo del bucle de eventos, y un comando de Tauri corre en
   el pool del runtime. Hecho desde ahí falla **en silencio** — no hay error
   que devolver al panel, así que el `.catch()` tampoco se enteraba.
   `set_tray_menu` ahora despacha la reconstrucción con `run_on_main_thread`.
2. **Llamada perdida al arrancar.** Si el icono de bandeja no está listo
   cuando el panel pinta por primera vez, la única llamada se pierde: el
   idioma no vuelve a cambiar en toda la sesión, así que nadie reintenta.
   La llamada sale a `sendTrayMenu()` y `updateTray` la repite **la primera
   vez que el tray responde**, que es cuando sabemos seguro que existe.

**CAUSA CONFIRMADA (2026-08-24, con el dato de Oscar).** El idioma que tenía
puesto era INGLÉS; al pasarlo a español el TOOLTIP cambió al instante
(«Sesión 0% · Semanal 16%») y el MENÚ se quedó en inglés. Mismo panel, mismo
`lang`, misma pasada de `applyI18n()`: uno obedeció y el otro no. Eso descarta
que el idioma llegara mal y señala exactamente a la causa 1 — la llamada sale,
Rust la recibe, y la reconstrucción del menú se pierde por hacerse fuera del
hilo principal. El tooltip nunca estuvo roto: seguía al idioma elegido.

**ARREGLADO Y VISTO (2026-08-24, exe):** clic derecho en el icono de bandeja
→ **Abrir panel · Widget flotante · Salir**. Con esto se cierra la ÚLTIMA
fila de idiomas: ya no queda un solo texto de la app en inglés teniéndola en
español. (El `use tauri::Manager` sobraba —`run_on_main_thread` es inherente
a `AppHandle`— y se quitó; el aviso de compilación no afectaba al arreglo.)

**HISTORIAL — lo que se sospechó antes de tener ese dato:** El TOOLTIP no pasa por el menú:
lo arma el panel con `t("tray_tip")` en cada ciclo y viaja en `update_tray`,
que sí funciona (el número del icono se actualiza). Si el panel está en
español, ese texto TIENE que salir en español. Que salga en inglés apunta a
que la variable `lang` del panel es `en` mientras la interfaz se ve en
español — y eso, con `applyI18n()` pintando todo desde el mismo diccionario,
no debería poder pasar. **Falta comprobar en la app**: qué idioma marca
Ajustes → Idioma, y si el globito sigue diciendo «Session» ahora mismo. Sin
ese dato no se toca el tooltip: no hay causa, hay solo un síntoma.

### R15 · «Hacer que «claude» pase por el relevo» APAGADO y el relevo funcionando — 2026-08-25

**Lo que vio Oscar:** el interruptor del atajo del PATH apagado, con su nota
diciendo «para tener relevo hay que escribir *michi claude*», y a la vez la
lista enseñando `michiclaude · VPS-EU · chat · pid 3994986 · listo` con su
botón «Aplicar /compact» y la bombilla del gatito midiendo «7% Memoria de la
conversación · michiclaude · VPS-EU · relevo». Lo dio por fallo.

**NO ERA FALLO — son tres puertas distintas.** El atajo (`set_relay_alias`)
escribe un shim en el PATH de USUARIO **de Windows**; su alcance ya estaba
escrito en `docs/remediacion.md` §"El atajo del PATH" («NO cubre WSL desde
dentro ni SSH»). Esa sesión es de tipo `chat`, o sea el wrapper del chat de
VS Code sobre SSH, que tiene interruptor propio y estaba encendido — igual
que el de las terminales Linux.

**ARREGLADO (2026-08-25, solo texto):** la etiqueta pasa a
«Hacer que «claude» pase por el relevo **(terminales de Windows)**» y las dos
notas se acotan igual, en los 8 idiomas. La aclaración va en la ETIQUETA y no
en la nota a propósito: sin servidores dados de alta las filas del chat y de
las terminales Linux están OCULTAS (`rlyChatRow`/`rlyTermRow`), así que una
nota del tipo «SSH y WSL tienen su propio interruptor» señalaría algo
invisible (invariante #8).

**Lección:** un alcance escrito solo en el doc no evita la confusión; si el
interruptor de al lado hace algo parecido, el alcance va EN la etiqueta.

### R16 · El auto-/clear interrumpe un proceso autónomo en marcha — 2026-09-02

**Lo que vio Oscar:** trabajando en polymarket-bot (chat de VS Code sobre
SSH, modelo de 1M), un análisis largo y autónomo pasó del 20% de contexto
(200k absolutos → intención). A las 12:39:56 arrancó la cuenta atrás del
auto-/clear "por hecho" y a las 12:40:14 se aplicó — con Claude EN PLENA
FAENA (spinner girando, tareas en segundo plano corriendo). La copia
/export se hizo y el globo salió, pero el proceso quedó decapitado: el
paso siguiente ya no tenía conversación. Antes de eso, el mismo día,
también confirmó lo BUENO: /compact y /clear automáticos disparando en
vivo con su cuenta atrás — lo que faltaba de la fase 4.

**La causa (dos capas):**
1. `intentVerdict` da Boundary con "lista de TODOs cerrada" o "commit sin
   ediciones después" — pero un proceso autónomo largo cierra su lista y
   commitea A MITAD del trabajo. La evidencia de "tarea terminada" no
   distingue el final del proceso de un hito intermedio.
2. El candado del relevo (`ready()`) solo ve el INSTANTE: 2 s de silencio
   de la PTY (terminal) o el turno en curso (chat). Entre dos pasos del
   proceso —o esperando una tarea en segundo plano— la sesión parece
   libre, y el /clear entra por ese hueco.

**ARREGLADO (2026-09-02):** compuerta de REPOSO en `relayAutoCheck`
(`AUTO_REST_MIN = 5`): el auto-/clear exige `quiet` ≥5 min del hit press
(mismo umbral que la regla `done` usa para "terminó tu sesión"). Sin
reposo el veredicto se degrada a /compact — comprime sin matar y conserva
la carrera ganada al auto-compact del ~94%. Solo panel (JS): ni el relevo
ni el exportador cambian. Rastro nuevo en la Bitácora PRO:
"[sin reposo: /clear degradado, quiet X min]".

**PENDIENTE DE VER:** un auto-/clear disparando con la sesión de verdad
en reposo (quiet ≥5 min y <10, la ventana en que press sigue saliendo), y
un Boundary a mitad de proceso degradándose a /compact con su rastro.
Ojo: el escenario SE DIO el mismo 02/09 a las ~15:48 y no disparó por un
motivo ajeno a R16 — ver R17, que lo desbloquea.

**Lección:** "tarea cerrada" y "sesión terminada" no son lo mismo; para
borrar hace falta el segundo, y eso lo dice el reposo, no el veredicto.

### R17 · El sello del automático era perpetuo y desarmaba la sesión — 2026-09-02

**Lo que vio Oscar:** la misma tarde de R16, revisando la Bitácora PRO:
"me llegaron los consejos pero no lo automático de compact y clear, ¿está
bien o no?". No había ni un rastro que lo explicara.

**Lo que se midió** (los `.jsonl` de polymarket-bot, sesión `dc321d80`,
Fable 5.1, techo 1M — el manómetro casó al 100% con el contexto real:
13:32 → 127.277 tok = 13%; 13:47 → 150.966 = 15%; 14:44 → 197.985 = 20%):

| hora | qué pasó |
|---|---|
| 12:59:54 | el automático aplica `/compact` (`relevo/3737148.json`, `id: app-…`, `ok:true`) |
| 13:01:41 | `compact_boundary`, preTokens **259.023** → el contexto cae a ~114k |
| 14:46 | vuelve a cruzar los 200k absolutos (201.143) |
| 15:43 | cierra con **344.265 tok** (34% del techo), su pico del día |
| ~15:48 | relevo `ready:true`, quieta 5-10 min, tarea cerrada: **el escenario exacto del /clear de R16** |

Y no disparó nada. La causa: `autoStamp(sid,"done")` sellaba la sesión
PARA SIEMPRE, y `/compact` no cambia el `sessionId` (a diferencia de
`/clear`). Una sesión larga que recibe un `/compact` temprano se queda sin
automático el resto de su vida — con la auto-compactación del ~94% de
Claude Code como única red. Y de regalo: el escenario que llevábamos
esperando para validar R16 se dio, y se lo comió el sello.

**ARREGLADO (2026-09-02):** el sello dura UN CICLO DE CONTEXTO. Guarda los
tokens que había al aplicarlo (`{done:<tok>}`) y `autoRearm()` lo levanta
cuando una lectura de `press` de esa sesión cae a la mitad o menos —
señal del motor, no corazonada: todo `compact_boundary`, de quien sea,
pone `last_ctx = 0` (lib.rs), y dentro de un ciclo el contexto solo crece.
Se mira en CADA sondeo y sobre TODAS las `press`, no sobre la reina: el
vaciado solo se ve en el tramo BAJO, y ese tramo no arde (a 344k del
259k → 114k ya no queda rastro). La cadena `"done"` a secas sigue siendo
perpetua: formato viejo y sello del "bajar solo". Solo panel (JS).

**PENDIENTE DE VER:** el rastro nuevo en vivo — "relevo auto: sello
levantado en <proyecto> — contexto vaciado (259k → 115k), vuelve a estar
armado" — y detrás de él un segundo automático en la misma sesión.

**Lección:** una compuerta "una vez por sesión" hay que fecharla contra
algo; sin fecha, "una vez" se convierte en "nunca más" en cuanto la
sesión dura más que el motivo que la selló. Y un candado que se cierra en
silencio cuesta una tarde de investigación: por eso el levantamiento deja
línea en la bitácora del flujo.
