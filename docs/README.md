# docs/ — índice y guía de uso

Regla: `CLAUDE.md` (raíz) dice QUÉ ES VERDAD HOY y apunta aquí con
"LEERLO antes de tocar"; cada doc de diseño explica su área a fondo;
la bitácora dice CÓMO LLEGAMOS AQUÍ. Cuando un doc y CLAUDE.md
discrepan, manda CLAUDE.md y hay que corregir el doc.

## Qué doc abrir según lo que vayas a tocar

| Doc | Ábrelo cuando toques… |
|---|---|
| `bitacora.md` | Cualquier decisión vieja antes de rediscutirla; al cerrar una jornada (plantilla arriba del archivo). Desde 2026-08-05. |
| `bitacora-hasta-2026-08-04.md` | Historial de julio: el CLAUDE.md original de 118k, íntegro. Solo grep. |
| `analizador-fugas.md` | Pestaña Hallazgos: detectores, umbrales, qué NO se detecta, avisos. |
| `consejos-coach.md` | Pestaña Consejos: fichas, reglas del motor de sesión (`press`, `attach`, `shots`, `sum`…), anti-spam. |
| `analisis-local.md` | IA local: `ai_intent`, embeddings (etapa 2), descarga guiada, umbrales `EMB_*`; etapa 3 (temas sobre `inflate`, diseño). |
| `remediacion.md` | Relevo (`relevo/`), auto-/compact y auto-/clear, archivador y **purga**. §"REGLAS VIGENTES" es obligatorio. |
| `presion-y-rendimiento.md` | Pestaña Reporte, `uturns`, histórico de cuota, marcas de arreglo, `waste`. |
| `adr-multiharness-y-persistencia.md` | Integridad de las fuentes (las 4 piezas) y por qué NO hay SQLite ni otras herramientas. |
| `avisos-ntfy.md` | Pushes al celular: privacidad, topic, programados, cómo probarlo. |
| `hub-modo-equipo.md` | Modo HUB multi-máquina y el diseño (bloqueado) de rangos de fecha. |
| `ruteo-inteligente.md` | Ruteo de subagentes por modelo: etapa 0 hecha, plan 1-6 (bloqueado). |
| `prompts-diseno-*.md` | Los prompts con que se diseñaron remediación y desperdicio — para repetir el método, no para editar. |
| `img/` | Capturas y diagramas (convención en `img/README.md`). |

## Dónde mirar cuando algo falla ("no llegó X", "no se ve Y")

Todo lo de la app vive en `%APPDATA%\com.oscarorozco.michiclaude\`
(`app_data_dir()`); nada de esto sale del PC salvo lo marcado. Primero
el rastro, después el código.

| Síntoma / área | Rastro | Dónde |
|---|---|---|
| Cuota mal, 429, buckets raros | `quota_debug.json` (respuesta cruda del endpoint) | AppData |
| Histórico/Reporte "juntando datos" | `quota_history.json` (90 d, solo lecturas buenas) | AppData |
| Coach: no salió ficha/aviso/recibo | `coach_debug.json` y luego `flowLog` (📜 en dev; localStorage, 300 líneas) | AppData / panel |
| Coach en un servidor SSH | `~/.cache/michiclaude/coach_state.json` (reconstruible: borrarlo reinicia) | VPS |
| Análisis local: veredicto raro | `ai_debug.txt` (2B), `emb_debug.txt` + `emb_server.log` (embeddings; el 2B pisa `ai_debug`) | AppData |
| Relevo / auto-compact / auto-clear | `rem_debug.json` (app) y `wrap_debug.txt` (michi.exe, el que envuelve la sesión) | AppData |
| Pastilla / gatito no se ven bien | no hay rastro propio: DevTools de la ventana (dev) y `flowLog`; si dev bien y build mal → CSP (invariante #3) | panel |
| Panel dice "bajó el consumo" | `integrity.json` (archivos que encogieron o desaparecieron) | AppData |
| Escaneo lento o cifras viejas | `scan_cache.json` (borrarlo fuerza re-parseo, nunca cambia el coste) | AppData |
| Precio/techo con "~" | `prices_cache.json` (cascada LiteLLM→models.dev→OpenRouter, 24 h) | AppData |
| Push no llegó | `ntfy_debug.json`; recordar: SOLO %, horas y conteos viajan | AppData |
| Hub no fusiona | `hub_debug.json`; en el servidor `~/.michiclaude/hosts/<id>.json` | AppData / VPS |
| Hallazgos no "nacen" | re-armar: borrar `fndSeen` y `fndAutoLast` (localStorage) | panel |

Regla de oro al depurar: si con `npm run dev` se ve bien y con
`npm run build` mal, sospechar de la CSP antes que del código.

## Convenciones

- Un doc por área; nombre en kebab-case y español; empieza con qué es y
  para qué sirve, y tiene una sección "REGLAS VIGENTES" o equivalente si
  hay reglas duras (CLAUDE.md las resume, el doc las explica).
- Fechas absolutas (`2026-08-16`), nunca "ayer"/"la semana pasada".
- Lo que se DESCARTÓ también se escribe, con el porqué: es lo que evita
  rediscutirlo.
- Imágenes: `img/AAAA-MM-DD-area-que-muestra.png`, referenciadas desde el
  doc con ruta relativa. Ver `img/README.md`.
