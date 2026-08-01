# Avisos en el celular (ntfy) — diseño y decisiones

> Leer antes de tocar código de esta función. Implementada el 2026-08-01 y
> VALIDADA EN VIVO ese mismo día en lo básico (QR escaneado, suscripción en
> la app y prueba llegando al teléfono de Oscar). Pendiente: alarma de %
> real, 100% real y el programado con la PC apagada.
> Sustituye a la propuesta de Telegram (descartada: fricción de BotFather,
> chat_id como dato personal, y sobre todo la imposibilidad de avisar con la
> PC apagada).

## Qué es

Push opcional al teléfono vía [ntfy](https://ntfy.sh) (open source). El
usuario instala la app gratuita, escanea un QR y recibe los avisos de cuota
en el celular. **Apagado por defecto.**

La pieza que lo justifica es la **entrega programada**: al llegar al 100%,
MichiClaude manda el aviso inmediato Y deja encargado en el servidor el
"tu cuota volvió" con el header `delay` al `resets_at`. Ese segundo mensaje
llega **con la PC apagada** — por eso el inmediato puede prometer "puedes
apagar la compu, yo te aviso 🐱". Ningún monitor de la competencia hace esto
(todos son fire-and-forget con la máquina prendida).

## Arquitectura — quién habla con quién

```
MichiClaude (PC del usuario) ──POST──► ntfy.sh ──► app ntfy (celular)
```

- **Nada corre en servidores nuestros.** Cada instalación habla directo con
  ntfy.sh (mismo patrón que la descarga de precios). Si ntfy.sh cae, solo
  falla el push remoto; globos y toasts locales siguen igual.
- El servidor es configurable en `ntfy_config.json` (campo `server`, sin UI
  a propósito — como las URLs de precios). Self-host gratis de diseño.

## Config — `ntfy_config.json` (carpeta de datos)

```json
{ "enabled": false, "topic": "michi-k3x9f8a2d7b1", "server": "https://ntfy.sh", "alarms": false }
```

- `topic`: se genera al activar por primera vez. En ntfy **el topic es la
  contraseña** (no hay cuentas): "michi-" + 12 símbolos [a-z0-9] del CSPRNG
  del sistema (`getrandom`, ~62 bits). NUNCA con SystemTime: sería adivinable.
- `alarms`: segunda casilla — las alarmas de % solo van al celular si el
  usuario lo pide aparte (frente a la PC ya tiene el globo).

## Reglas fijas (el porqué de cada una)

1. **Por este canal viajan SOLO porcentajes, horas de reset y frases del
   diccionario.** Nunca nombres de proyecto, rutas ni dólares: los topics de
   ntfy son públicos por diseño — quien conozca/adivine el topic lee el
   canal. Con solo porcentajes, lo peor que filtra es "alguien usa Claude".
2. **Rust no redacta avisos.** Los textos llegan del panel ya traducidos
   (regla del menú del tray, invariante #10). `ntfy_push(title, body,
   priority, at)` es genérico.
3. **Publicación JSON** (POST a la raíz del servidor, no headers `Title:`):
   los headers HTTP no aguantan UTF-8 y los avisos van en 8 idiomas —
   japonés/coreano/chino se romperían con headers.
4. **Fire-and-forget**: un fallo de red jamás bloquea nada local. El último
   error queda en `ntfy_debug.json` (código y hora; el topic jamás se
   escribe ahí). El botón "Enviar prueba" sí muestra el error traducido.
5. **El simulador nunca manda pushes**: `ntfyPush()` corta con `simRunning`,
   y además los globos simulados no pasan por `trackResets`.
6. **Un push por ventana**: heredado gratis de los banderines `notifS`/
   `notifW` que ya limitaban los globos.

## Los mensajes (reutilizan i18n existente)

| Evento | Cuerpo | Prioridad | Programado |
|---|---|---|---|
| Sesión al 100% | `breakBody(resets)` + `ntfy_promise` | 4 | `notif_back_session` con `delay` = resets+120 s |
| Semana al 100% | `weekBody(resets)` (+ promesa solo si cabe) | 4 | `notif_back_week`, solo si cabe |
| Alarma de % (opt-in) | `notif_lo`/`notif_hi` | 3 / 4 (≥95) | — |
| Prueba | `ntfy_test_body` | 3 | — |

- **Límite de 3 días** del servidor público (verificado 2026-08-01 en
  docs.ntfy.sh; mínimo 10 s): el reset de sesión (≤5 h) siempre cabe; el
  semanal puede no caber. Si no cabe: no se programa nada Y no se promete
  nada — `weekBody` ya dice el día ("Vuelvo el lunes"). Prometer "yo te
  aviso" sin poder cumplirlo sería mentir.
- **+120 s de colchón** sobre `resets_at`: el endpoint trae jitter y es
  mejor avisar "ya volvió" un minuto tarde que un minuto antes.
- Los mensajes programados **no se pueden cancelar** en la práctica (no
  diseñar nada que dependa de eso). Consecuencia asumida: si la PC está
  prendida a la hora del reset, el usuario recibe globo local Y push — no
  es un bug, es la función.

## Onboarding (matiz que costó investigación)

La app ntfy **NO trae escáner de QR** (el borrador de estrategia decía que
sí — falso). Lo que existe es el enlace profundo `ntfy://host/topic`: el QR
lo codifica, se escanea con la **cámara normal** del teléfono y Android
ofrece abrirlo en la app ntfy ya suscrito. En iPhone el camino es copiar el
canal (botón Copiar, URL completa) y agregarlo a mano en la app. Los 3 pasos
están numerados en la propia UI (`ntfy_steps`).

El QR se genera en Rust (`qrcode` sin features = sin dependencia de imagen)
y viaja como **matriz de módulos** que el panel pinta en un canvas — cero
PNG. Siempre negro sobre blanco en ambos temas: un QR invertido no lo leen
todas las cámaras.

## Comandos

- `get_ntfy()` / `save_ntfy(cfg)` — síncronos (archivo pequeño). `save_ntfy`
  inventa el topic al activar y devuelve la config final.
- `ntfy_push(title, body, priority, at)` — **async** (red, regla 10ter).
  Errores: `ERR_NTFY_OFF`, `ERR_NET`, `ERR_NTFY:<status>`.
- `ntfy_qr()` — síncrono, devuelve `{size, cells}`.

## Cómo probarlo

1. Preferencias → activar "Enviar avisos a mi celular" → aparecen QR/canal.
2. Instalar ntfy en el teléfono, escanear el QR con la cámara (Android) o
   copiar el canal y agregarlo en la app (iPhone).
3. "Enviar prueba" → el teléfono suena en segundos.
4. El camino completo (100% real + push programado llegando con la PC
   apagada) solo se prueba agotando la sesión de verdad — está en la lista
   de pruebas pendientes junto a las alarmas reales.

Curl de diagnóstico (sustituir el topic por el del `ntfy_config.json`):
`curl -d "hola" ntfy.sh/michi-xxxxxxxxxxxx`

## Canal nuevo (botón, 2026-08-01)

Junto a Copiar. Para cuando el canal se filtró — un QR visible en una
captura de pantalla basta para regalar la contraseña. En DOS PASOS (patrón
del bote de borrar servidor: primer clic arma, 8 s y se desarma): un clic
accidental dejaría al teléfono sordo sin aviso. Al regenerar, el canal
viejo queda muerto (quien lo tuviera deja de recibir) y hay que re-escanear
el QR en el teléfono propio. Comando `ntfy_regen`.

## ntfy y los ajustes compartidos del hub — NO viaja, a propósito

La pantalla de "Ajustes compartidos" promete: **"No guarda llaves ni
contraseñas."** El topic de ntfy ES una contraseña (es lo único que protege
el canal), así que meterlo en el `config.json` del servidor rompería esa
promesa literal. Además no hace falta: activar ntfy en otra PC son 30
segundos con su propio QR, y que cada máquina tenga su canal es una
VENTAJA — en la app ntfy se puede silenciar un canal sin tocar el otro
("la PC de la oficina no me avise en fin de semana"). Nota: la cuota es de
la CUENTA, así que dos PCs prendidas con ntfy activo pueden avisar lo mismo
por sus dos canales; es el comportamiento esperado, no un bug.

Si algún día se pidiera compartirlo, lo compartible serían los booleanos
(`enabled`/`alarms`), nunca el topic. Hoy: nada.

## Lo que NO se hizo, a propósito

- **Autohostear un ntfy "oficial de MichiClaude"**: nos volveríamos
  responsables del uptime de los avisos de todos. El campo `server` cubre a
  quien quiera el suyo.
- **Cancelar el programado si el usuario vuelve antes**: no se puede en el
  servidor público; asumido.
- **`GetLastInputInfo`** (no duplicar push si el usuario está activo en la
  PC): pulido de v2, no estructura.
- **Encadenar mensajes** para resets semanales a >3 días: complejidad por
  un caso que el texto del inmediato ya resuelve.
