# MichiClaude 🐱

[🇬🇧 English](README.md) · **🇪🇸 Español**

<p align="center">
  <a href="https://github.com/oscarorozcos/michiclaude/releases/latest"><img alt="Descargar para Windows" src="https://img.shields.io/badge/Descargar-Windows%2010%20%2F%2011-E08B63?style=for-the-badge&labelColor=151F3A&logo=windows&logoColor=white"></a>
  <img alt="Estado: beta" src="https://img.shields.io/badge/estado-beta%20%C2%B7%20en%20desarrollo%20activo-E08B63?style=for-the-badge&labelColor=151F3A">
  <a href="https://github.com/oscarorozcos/michiclaude/releases/latest"><img alt="Última versión" src="https://img.shields.io/github/v/release/oscarorozcos/michiclaude?style=for-the-badge&label=versi%C3%B3n&color=E08B63&labelColor=151F3A"></a>
  <a href="LICENSE"><img alt="Licencia GPL-3.0" src="https://img.shields.io/badge/licencia-GPL--3.0-E08B63?style=for-the-badge&labelColor=151F3A"></a>
  <a href="https://github.com/oscarorozcos/michiclaude/discussions"><img alt="Discusiones" src="https://img.shields.io/badge/Discusiones-preg%C3%BAntanos-E08B63?style=for-the-badge&labelColor=151F3A&logo=github&logoColor=white"></a>
  <a href="https://www.linkedin.com/in/oscar-os/"><img alt="LinkedIn" src="https://img.shields.io/badge/LinkedIn-Oscar%20Orozco-E08B63?style=for-the-badge&labelColor=151F3A&logo=linkedin&logoColor=white"></a>
</p>

<p align="center">
  <img src="docs/img/michiclaude.gif" alt="MichiClaude en el escritorio: el gatito, el icono de bandeja y el panel" width="900">
</p>

> 🚧 **En desarrollo activo** — la app ya es funcional y se usa a diario,
> pero siguen llegando mejoras (ver [Roadmap](#roadmap)). Issues y
> sugerencias son bienvenidos.

Widget de bandeja para **Windows 10 y 11** que muestra, en tiempo real,
cuánto has usado de tu suscripción de Claude y cuánto te queda:

- **Cuota real de tu plan** (sesión de 5 h y límites semanales, con barras por
  modelo) — la misma que ves en claude.ai → Configuración → Uso, porque los
  límites son compartidos entre claude.ai, Claude Code y los IDEs.
- **Marcador de ritmo**: una línea indica cuánto del periodo ha transcurrido;
  si tu consumo va más rápido que el reloj, los colores pasan a ámbar/rojo.
- **Proyección**: "a este ritmo llegas al 100% en X min, antes del reset".
- **Costo estimado por proyecto** (equivalente API) con desglose por modelo,
  periodos de 1/7/30 días y gráfica de tendencia diaria.
- **Analizador de fugas**: encuentra dónde se te están yendo los tokens —
  archivos leídos una y otra vez, cachés rotos, servidores MCP que instalaste
  y nunca usas.
- **Coach**: vigila tu sesión viva y te dice cuándo un `/compact` o un
  `/clear` valen la pena — y puede escribirlo por ti si se lo permites.
- **Varias máquinas en un solo tablero**: tu PC, WSL y servidores por SSH.
- **Alarmas configurables**, presupuesto semanal, exportación CSV/JSON,
  tema claro/oscuro y 8 idiomas.
- **Widget flotante opcional**: una pastilla minimalista… o un **gatito
  programador** 🐱 que teclea tranquilo cuando vas bien, arde 🔥 cuando cruzas
  tu alarma y duerme 😴 si agotas la semana.

Construido con [Tauri 2](https://tauri.app): binario nativo pequeño, frontend
HTML/CSS/JS sin frameworks, backend Rust mínimo.

> **Qué abre esta primera versión pública (0.2.0):** Principal, Fuentes de
> datos, Hallazgos y Consejos. La pestaña **Reporte** y todo lo que escribe en
> tus sesiones —aplicar `/compact` y `/clear` por ti, el ruteo de modelos, el
> análisis local con IA— existe en el código pero viene **apagado**; se irá
> abriendo en próximas versiones, cuando esté validado con uso real. Algunas
> secciones de más abajo describen esas piezas para que sepas qué viene.
> **Esta versión solo mira: nunca toca tus sesiones.**

---

## Instalación (usuario final)

1. Ten instalado [Claude Code](https://claude.com/claude-code) y una sesión
   iniciada (`claude` en la terminal e inicia sesión con tu cuenta Pro/Max).
   La app usa esa misma sesión — **no necesitas API key ni crear cuentas**.
2. Descarga el instalador (`.exe`) desde
   [Releases](../../releases) y ejecútalo.
3. Al abrir, aparece el **icono en la bandeja** (junto al reloj) con tu % de
   sesión dibujado. Clic izquierdo = abrir el panel; clic derecho = menú.

> No hay pasos de configuración obligatorios: si usas Claude Code en esa PC,
> la cuota y los costos por proyecto aparecen solos.

### ¿Qué versión de Windows necesito?

**Windows 10 o Windows 11.** Hasta hoy solo se ha probado en Windows 11 (es
donde se desarrolla), así que en Windows 10 *debería* funcionar sin cambios
pero no está verificado — si lo pruebas ahí, cuéntanos en un issue.

Lo que marca ese mínimo no es la app, sino las piezas sobre las que corre:

| pieza | mínimo | por qué |
| --- | --- | --- |
| Tauri 2 (el motor) | Windows 10 | la versión 2 dejó de soportar Windows 7 y 8 |
| WebView2 | Windows 10 | viene preinstalado en 11; en 10 llega con el Edge moderno, y si falta, el instalador lo descarga |
| Notificaciones | Windows 10 | usan la API moderna de avisos, que no existe antes |
| Cliente SSH (opcional) | Windows 10 | solo si conectas un servidor; incluido desde entonces |
| Ventanas del widget | Windows XP | `SetWindowPos` y compañía son APIs antiguas: no limitan nada |

En **Windows 7 y 8 no funciona**, y no es algo que se pueda arreglar del lado
de MichiClaude.

No hay versión de macOS ni de Linux: el icono de bandeja dinámico y el widget
flotante están escritos contra las APIs de Windows.

### ¿Cuánto ocupa y cuánta memoria usa?

Cifras **medidas**, no estimadas (Windows 11, versión compilada, con el panel
abierto y el gatito en pantalla):

| | |
| --- | --- |
| Instalador | **5.8 MB** |
| Instalada en disco | ~22 MB |
| Tus datos (cachés y ajustes) | **menos de 1 MB** |
| Memoria en uso | **~276 MB** |

**El instalador es pequeño porque la app no lleva un navegador dentro.** La
interfaz está hecha con HTML, pero usa el WebView2 que Windows ya trae de
fábrica en lugar de empaquetar el suyo. Una app equivalente hecha con Electron
ronda los 90-150 MB de descarga.

La memoria es la otra cara de esa misma decisión: cada ventana del programa
—el panel, el widget, sus globos— es una vista web y cuesta lo suyo. Para que
esos 276 MB signifiquen algo, aquí están medidos con la misma vara y en el
mismo momento, en la máquina de desarrollo:

| | memoria |
| --- | --- |
| Visual Studio Code | 799 MB |
| Navegador (Brave) | 730 MB |
| Explorador de Windows | 360 MB |
| **MichiClaude** | **276 MB** |

O sea: **un tercio de lo que gasta tu editor**. En una máquina de 8 GB no vas
a notarlo; en una de 4 GB con el navegador abierto, sí.

Lo decimos claro porque nadie más publica este dato: si lo que buscas es la
huella mínima absoluta, una herramienta de terminal siempre va a ganar —
la ejecutas, te da el número y desaparece. MichiClaude está siempre encendido
a cambio de avisarte antes de quedarte sin cuota.

<details>
<summary>Cómo medirlo tú (PowerShell)</summary>

Cuidado con el método: sumar la memoria "de trabajo" de cada proceso
**cuenta varias veces** lo que comparten entre ellos, e infla el resultado
más del doble. Esto suma la memoria **privada**, que es la real:

```powershell
$w=Get-CimInstance Win32_Process
$ids=@($w|? Name -eq 'michiclaude.exe'|% ProcessId)
do{$n=@($w|?{$ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId}|% ProcessId);$ids+=$n}while($n.Count)
$pf=Get-CimInstance Win32_PerfRawData_PerfProc_Process
"{0:N0} MB" -f ((($pf|?{$ids -contains $_.IDProcess}|measure WorkingSetPrivate -Sum).Sum)/1MB)
```

</details>

## Primeros pasos: qué estás viendo

El panel tiene **seis pestañas**:

- **Principal** — todo el tablero:
  - *Cuánto te queda*: gauge de la sesión de 5 h + barras semanales (una por
    modelo, según reporte tu plan). La marquita vertical en cada barra es el
    **ritmo**: si tu barra de consumo la rebasa, vas más rápido que el reloj.
  - *A este ritmo*: proyección de burn rate — si sigues así, ¿chocas con el
    límite antes del reset?
  - *Gasto por proyecto*: costo estimado de cada proyecto (ver siguiente
    sección) en 1/7/30 días. Pasa el mouse sobre un proyecto para ver su
    desglose por modelo. Ojo: esos dólares son **solo de Claude Code**. Lo que
    uses en claude.ai también gasta tu límite semanal, pero no se puede medir
    en dólares — el endpoint no dice cuánto vale el tope en dinero.
  - *Tendencia diaria*: gráfica de los últimos 30 días.
  - *Modelos*: qué modelo usas más.
- **Fuentes de datos** — de dónde salen los números y alta de servidores SSH.
- **Hallazgos** — el [analizador de fugas](#hallazgos-dónde-se-te-mueren-los-tokens).
- **Consejos** — el [coach](#consejos-el-coach).
- **Reporte** — [métricas de uso](#reporte-esto-está-mejorando) más allá del
  dólar a secas.
- **Ajustes** — idioma, widget flotante, alarmas, presupuesto y export.

El pie (Hoy / periodo) siempre está visible en la pestaña Principal.

## ¿Dónde usas Claude Code? (Windows nativo, WSL o servidor)

Explicación sin tecnicismos, por si estos términos no te suenan. La idea de
fondo es simple:

> Claude Code siempre deja un "recibo" de lo que gastas en una carpeta de la
> computadora donde lo usaste. **MichiClaude es como un lector de esos
> recibos.** Lo único que cambia entre los tres casos es **en qué computadora
> están los recibos.**

Piensa que usar Claude Code es **cocinar**, y los recibos son los tickets de
lo que gastaste:

### 🪟 Windows nativo — cocinas en TU cocina normal

Instalas Claude Code **directamente en Windows** (como cualquier programa) y lo
usas en la terminal de Windows (PowerShell). Los recibos quedan en tu PC.

- *Ejemplo:* María abre PowerShell, escribe `claude` y le pide "arréglame este
  error". Todo pasó en su PC.
- *En MichiClaude:* **nada, automático.** Vive en el mismo Windows, encuentra
  los recibos solo. Aparece como **"Este PC"**.

### 🐧 WSL — una "mini-computadora Linux" DENTRO de tu Windows

WSL (Windows Subsystem for Linux) es una herramienta de Windows que crea una
especie de **segunda computadora con Linux, escondida dentro de tu Windows**.
Mucha gente la usa porque antes Claude Code solo funcionaba en Linux. Es como
tener en tu casa una cocina aparte, de otro estilo, y cocinar ahí: sigue siendo
tu casa, pero es otro "cuartito".

- *Ejemplo:* Juan abre su "Ubuntu" (WSL se ve como una app más), escribe
  `claude` ahí dentro y trabaja. Los recibos quedan en ese cuartito Linux.
- *En MichiClaude:* **también automático.** Se asoma a ese Linux dentro de tu
  Windows y los lee. Aparecen con el sufijo de su distro (ej: *mi-proyecto · wsl-Ubuntu*), así
  que si tienes varias sabes cuál es cuál.

> 💡 **Nativo vs WSL en una frase:** *nativo* = Claude Code corre en Windows a
> secas; *WSL* = corre en un Linux que vive dentro de tu Windows. Para ti,
> ambos están en la **misma PC física** y MichiClaude los lee **solos**.

### 🌐 SSH — cocinas en la casa de OTRA persona (un servidor), a distancia

SSH es una forma de **conectarte a otra computadora que está en otro lado** (un
servidor en internet, un "VPS") y usarla como si estuvieras ahí. Tú tecleas en
tu PC, pero todo pasa **en esa otra máquina**, y los recibos quedan **allá**.

- *Ejemplo:* Lucía, desde su Windows, se conecta a su servidor y usa `claude`
  allá. Su PC es solo el control remoto; el trabajo y los recibos viven en el
  servidor. (En VS Code lo reconoces porque abajo a la izquierda dice
  `SSH: <algo>`.)
- *En MichiClaude:* aquí **sí hay un paso manual, una sola vez.** Como los
  recibos están en otra máquina, le dices a MichiClaude que también la revise:
  pestaña **Fuentes de datos → agregar servidor** (nombre + dirección SSH, la
  misma con la que ya te conectas). Después aparecen con **el nombre corto
  que tú le pusiste** — por ejemplo `· servidor-trabajo`.

### Resumen

| Caso | ¿Dónde corre Claude Code? | ¿Qué haces en MichiClaude? |
|---|---|---|
| 🪟 Windows nativo | En tu PC, directo | Nada — automático ("Este PC") |
| 🐧 WSL | En un Linux dentro de tu PC | Nada — automático ("· wsl-Ubuntu") |
| 🌐 SSH | En otra computadora (servidor) | Agregarla una vez en Fuentes de datos (sale con el nombre que le pongas) |

**¿Cómo sé cuál tengo yo?** Mira qué abres para usar Claude Code:

- Abres **PowerShell** (el azul de Windows) y escribes `claude` → **nativo**.
- Abres una app que dice **"Ubuntu"** o una ventana de Linux → **WSL**.
- Primero te **conectas a un servidor** (o VS Code muestra `SSH: …` abajo a la
  izquierda) → **SSH**.

En 2 de los 3 casos no haces nada: instalas MichiClaude y ya. Solo el servidor
pide un pasito de configuración.

### ¿Y si uso VS Code, Cursor u otro editor? (no un terminal)

**Da igual el editor — funciona igual.** Uses lo que uses (VS Code, Cursor,
JetBrains o la terminal a secas), por dentro todos ejecutan el **mismo Claude
Code**, que deja sus "recibos" en la misma carpeta. MichiClaude no mira *con
qué* lo usas, sino *en qué máquina* corre. Así que:

- **VS Code / Cursor local en tu PC** (sin conectarte a nada) → automático,
  sale como **"Este PC"**, igual que si usaras la terminal.
- **VS Code trabajando dentro de WSL** (abajo a la izquierda dice `WSL: …`) →
  automático, sale como **"· wsl-Ubuntu"**.
- **VS Code Remote-SSH** (abajo a la izquierda dice `SSH: …`) → es el caso del
  servidor: agrégalo una vez en Fuentes de datos.

> 🔑 **Regla de oro:** no importa **con qué** uses Claude Code, sino **dónde
> corre**. Misma PC → local automático; WSL → "· wsl-Ubuntu" automático; servidor por
> SSH → agregarlo una vez.

## ¿De dónde salen los dólares? (costo estimado)

De dos ingredientes, **ambos en tu equipo**:

1. **Tus logs locales de Claude Code** (`~/.claude/projects/**/*.jsonl`):
   cada petición queda registrada con sus tokens (entrada, salida, caché) y
   el modelo usado. La app los parsea con deduplicación (los logs repiten
   entradas al reanudar sesiones) y excluye la lectura de caché del conteo
   de tokens "de trabajo" (la incluye solo en el costo, a su precio real).
2. **Los precios públicos de la API de Anthropic**, que la app **descarga y
   mantiene al día sola** (ver *La descarga de precios* más abajo). Si no hay
   red, usa el último caché y, en último término, esta tabla incluida (USD por
   millón de tokens):

   | Modelo | Entrada | Salida | Escritura caché | Lectura caché |
   |---|---|---|---|---|
   | Fable 5 | $10 | $50 | $12.50 | $1.00 |
   | Opus 4.5 y posteriores | $5 | $25 | $6.25 | $0.50 |
   | Opus 3 / 4.0 / 4.1 | $15 | $75 | $18.75 | $1.50 |
   | Sonnet (y no reconocidos) | $3 | $15 | $3.75 | $0.30 |
   | Haiku | $1 | $5 | $1.25 | $0.10 |

**Ejemplo**: si un proyecto usó 2M de tokens de entrada y 0.5M de salida con
Sonnet → 2×$3 + 0.5×$15 = **$13.50 equivalente API**.

> 💡 **Importante**: para suscriptores este costo es **nocional** ("equiv.
> API") — no es dinero que pagaste, sino lo que *habría costado* pagando por
> API. Sirve para saber qué proyecto consume más y cuánto te ahorra la
> suscripción. Solo es gasto real si usas API key.

> ⚠️ **Con subagentes el costo puede quedarse corto.** Cuando Claude Code
> delega trabajo a subagentes, parte de ese consumo no siempre queda reflejado
> en los registros locales que la app puede leer, así que el costo mostrado
> puede ser **menor que el real**. Es una limitación de los propios registros,
> compartida con otras herramientas del estilo (`ccusage` incluida), no un
> error de cálculo. **Tu cuota no se ve afectada**: los gauges de sesión y
> semanales vienen de tu cuenta y siempre son exactos.

### ¿Qué cuenta como "proyecto"?

Un proyecto **no** es cada terminal que abres: es **la carpeta desde donde
ejecutas `claude`** (el directorio de trabajo). Así agrupa Claude Code sus
registros, y la app hereda esa agrupación:

- 5 terminales abiertas en la misma carpeta, todo el día → **un solo
  proyecto** que acumula todo ese gasto.
- Corres `claude` en otra carpeta → aparece **otro proyecto** en la lista.
- Corres `claude` parado en tu carpeta de usuario "solo para una pregunta
  rápida" → eso también crea su proyecto (con nombres raros tipo `oscar` o
  `Downloads`). Si ves proyectos extraños en la lista, vienen de ahí.

> ✅ **Tip**: ejecuta `claude` siempre **dentro de la carpeta del proyecto**
> en el que trabajas — así el desglose de costos queda limpio y con sentido.

**¿De dónde salen los nombres?** De la ruta real de trabajo registrada en
los logs: la app toma el último segmento (`/opt/projects/mi-web` → `mi-web`).
Cuando el gasto viene de otra máquina, se añade el origen: `mi-web · wsl`,
`mi-web · servidor-trabajo` (el sufijo es el nombre corto que le pusiste al
servidor, así que lo eliges tú).

## Hallazgos: dónde se te mueren los tokens

La pestaña **Hallazgos** es un analizador de fugas. Lee esos mismos logs
locales y busca patrones que queman tokens sin darte nada a cambio. Cada
hallazgo es una tarjeta con qué pasó, cuánto te costó aproximadamente y qué
hacer al respecto. La severidad va por costo: rojo desde $10, ámbar desde $1.

Qué busca, en llano:

| Hallazgo | Qué significa |
|---|---|
| **Relecturas** | El mismo archivo *y el mismo rango de líneas* leído 3+ veces en una sesión. Adjuntarlo una vez sale más barato. |
| **Inflado de contexto** | Una sesión cuyo contexto crece más de +50k a lo largo de 10+ turnos sin compactar. |
| **Caché roto** | 300k+ tokens reescritos porque cambió el principio de la conversación y hubo que rehacer el caché. |
| **Comandos mecánicos** | 5+ ejecuciones de `git` / `pytest` / `cargo` / `npm` delegadas al modelo. Esas las corres tú gratis. |
| **Subagentes pesados** | 50k+ tokens gastados en sesiones delegadas. |
| **Auto-compactaciones repetidas** | Un mismo proyecto tocando 3+ veces la compactación automática de Claude Code — señal de sesiones demasiado largas. |
| **Pegados gigantes** | 3+ mensajes de 5k+ caracteres. Casi siempre un archivo que debió adjuntarse. |
| **Hooks ruidosos** | Hooks que se disparan 15+ veces e inyectan 10k+ tokens. |
| **MCP sin usar** | Configurados, cargados en cada sesión, jamás llamados. |
| **Skills sin usar** | La misma idea. |
| **CLAUDE.md ignorado** | Su contenido se carga en cada sesión pero nada de él se menciona nunca. |
| **CLAUDE.md demasiado grande** | Pasados los 40k caracteres Claude Code deja de cargar el resto — la cola sencillamente no se lee. |

Cada tarjeta se puede **ignorar** (no vuelve) y se marca como leída al hacerle
clic. El contador de la pestaña y el post-it rojo del gatito solo bajan a
medida que las lees de verdad, una por una.

**Estos nunca van a tu celular.** Los hallazgos mencionan nombres de
proyectos, así que se quedan en la máquina (ver
[Privacidad](#privacidad-y-a-dónde-se-conecta)).

## Consejos: el coach

La pestaña **Consejos** tiene dos mitades.

**Fichas curadas** — ocho tarjetas fijas sobre cómo gastar menos: cuándo
compactar, cómo mantener el caché caliente, por qué adjuntar gana a releer, y
demás.

**Coach de sesión viva** — MichiClaude vigila tus sesiones abiertas y levanta
una ficha cuando pasa algo concreto:

- Contexto por encima del 60% de la ventana del modelo → un `/compact`
  compensa.
- Llevas 6+ minutos en pausa con el contexto caliente → el caché se está
  enfriando.
- El mismo archivo leído 3 veces → mejor adjuntarlo.
- 10+ imágenes en una sesión.
- **Recibos de sesión**: cuando una sesión se queda quieta 10 minutos, recibes
  un resumen con cuánto duró, cuántos comandos y archivos tocó, más o menos
  cuánto costó, y los desperdicios que detectó por el camino.

Hay un tope diario de 10 fichas para que nunca se vuelva ruido y, como en
Hallazgos, una ficha está "leída" solo cuando le haces clic.

### El manómetro de contexto y los comandos automáticos

El widget muestra un **manómetro de presión de contexto** — un arco en la
pastilla, una bombilla sobre el gatito — con cuán llena está la ventana de
contexto de tu sesión. Divide entre el *techo real del modelo de esa sesión*,
no entre un número fijo.

Pasado el 80%, el coach te ofrece la opción que cree que encaja, con el
comando al lado: `/compact` si tu siguiente mensaje continúa esta sesión,
`/clear` si vas a empezar otra cosa. Puedes copiar el comando, o dejar que
MichiClaude lo escriba por ti:

- **`/compact` automático** (apagado por defecto) — una cuenta atrás de 15
  segundos que dice en voz alta qué comando va a mandar, con el widget a la
  vista. Cualquier tecla la cancela. Una vez por sesión.
- **`/clear` automático** (apagado por defecto) — igual, pero *solo* se
  ejecuta después de que una copia `/export` de tu conversación quede
  verificada en disco. Si la copia no aparece, no se borra nada.

Los dos necesitan que la sesión se haya abierto con el relevo (`michi claude`
en vez de `claude`, o un ajuste que haga pasar `claude` por él). El relevo
solo manda **dos** comandos, `/compact` y `/clear`, y esa lista blanca se
comprueba en los dos lados.

### Análisis local con IA (opcional, apagado por defecto)

Para elegir entre `/compact` y `/clear`, el coach tiene que adivinar si sigues
con el mismo tema o vas a empezar uno nuevo. Casi siempre bastan las señales
sencillas (tareas pendientes abiertas, si sigues tocando los mismos archivos,
si acabas de hacer commit). Cuando no bastan, puedes dejar que decida un
**modelo local**.

- Corre **en tu máquina**, en `127.0.0.1`, arrancado solo cuando hace falta y
  apagado en cuanto responde. **Nada de tu conversación sale de la
  computadora.**
- Encenderlo descarga los modelos una vez: uno rápido de embeddings (~319 MB)
  y, opcionalmente, un modelo de 2B para la franja ambigua (~1.7 GB en total).
  Esa descarga es la **única** conexión que MichiClaude hace fuera de
  `api.anthropic.com` que tenga algo que ver con tu contenido — y aun así, es
  una descarga, no un envío.
- Si los modelos no están, todo se degrada en silencio al comportamiento sin
  IA.

## Reporte: ¿esto está mejorando?

La pestaña **Reporte** responde a algo que el dólar a secas no puede: ¿estás
gastando *bien*?

- **Turnos útiles**: cuántos de los mensajes de un periodo fueron realmente tú
  hablándole a Claude, y no resultados de herramientas, inyecciones y
  resúmenes de compactación. El costo por turno útil es un número mucho más
  honesto que el costo a solas.
- **% de desperdicio estructural**: cuánto de tu gasto se fue en los patrones
  que el analizador reconoce.
- **Histórico de cuota**: hasta 90 días de lecturas, para ver si tus semanas
  se están apretando.
- **"1M tokens ≈ $X"** calculado con la tarifa *real* de ese periodo, nunca
  con una fija.

Si todavía no hay datos suficientes, lo dice en vez de dibujar algo. La app
nunca pinta una cifra que no puede calcular de verdad.

## Ajustes

| Opción | Qué hace |
|---|---|
| **Idioma** | 8 idiomas; se autodetecta la primera vez. |
| **Widget flotante** | Muestra la pastilla (o el gatito) siempre visible sobre la barra de tareas. También se activa desde el menú del icono de bandeja. |
| **Estilo del widget** | *Pastilla* (marca + % + barras S/W) o *Gatito con laptop* 🐱. |
| **Alarmas de sesión** | Los % a los que quieres aviso (por defecto 80 y 95). Al cruzar uno, la notificación se repite cada 5 min **hasta que abras el panel** ("enterado"). |
| **Presupuesto semanal en $** | Si el costo estimado de 7 días lo supera, recibes un aviso (uno por semana). 0 = sin aviso. |
| **Avisos en el celular** | Manda los avisos de cuota a tu teléfono con la app [ntfy](https://ntfy.sh). Apagado por defecto; ver abajo. |
| **`/compact` y `/clear` automáticos** | Apagados por defecto. Ver [los comandos automáticos](#el-manómetro-de-contexto-y-los-comandos-automáticos). |
| **Análisis local (IA)** | Apagado por defecto. Ver [más arriba](#análisis-local-con-ia-opcional-apagado-por-defecto). |
| **Exportar datos** | Guarda CSV o JSON del desglose en la carpeta que elijas (vacía = Descargas). |

### Avisos en el celular 📱 (opcional)

Sirve para lo mismo que un temporizador de cocina: te vas y él te avisa. Es
especialmente útil si dejas a Claude trabajando solo y te levantas de la
computadora, o si te agotaste la cuota y **apagaste el equipo**.

**Cómo se activa** (unos 30 segundos):

1. Instala la app gratuita **ntfy** en tu teléfono (Android / iPhone).
2. En MichiClaude: *Ajustes → Avisos en el celular* → enciende la casilla.
3. Escanea el QR con la **cámara normal** de tu teléfono (la app ntfy no trae
   escáner propio) y acepta abrirlo en ntfy. En iPhone, o si el QR no abre la
   app, pulsa **Copiar** y agrega ese canal a mano en ntfy.
4. Pulsa **Enviar prueba**. Si el teléfono suena, ya está.

**Qué te llega:**

| Cuándo | Mensaje |
|---|---|
| Se agota tu sesión o tu semana | "Sin cuota de sesión. Vuelvo en 45 min. **Puedes apagar la compu: yo te aviso cuando vuelva** 🐱" |
| A la hora del reset | "Cuota de sesión restablecida" — **este llega aunque tengas la computadora apagada** |
| Al cruzar una alarma de % | Solo si activas "También mis alarmas de %". |
| Termina una sesión larga, o Claude se quedó esperando tu aprobación | Solo si activas "Avisarme cuando termine una sesión larga". Te llega cuánto duró, cuántos turnos fueron y un *conteo* de los desperdicios que detectó — nunca los dólares, ni los archivos, ni qué regla saltó. |

Por defecto esos mensajes no dicen **de qué** proyecto hablan. Hay una casilla
aparte, *"Incluir el nombre del proyecto (el canal es público)"*, que lo
añade — la advertencia en su propia etiqueta es justamente el punto: piénsalo
antes de activarla.

Lo del reset con la máquina apagada no es magia: al detectar el límite, la app
deja el segundo mensaje **encargado en el servidor de ntfy** con la hora exacta
de entrega, y ese servidor lo manda a tu teléfono llegado el momento. (Tiene un
tope de 3 días; si tu reset semanal cae más lejos, no se promete nada — el
primer mensaje simplemente te dice el día.)

#### ⏱️ ¿Te llega tarde? Actívale la entrega instantánea

Si el aviso llega minutos después (o mucho después), **no es MichiClaude ni el
servidor: es Firebase**, el sistema de notificaciones de Google que usa la
versión de Play Store para los canales de `ntfy.sh`. Su propia documentación
lo advierte: sin entrega instantánea los mensajes "pueden llegar con un
retraso significativo — a veces muchos minutos, o incluso horas".

Se arregla con un interruptor, en el teléfono:

1. Abre **ntfy** → menú **⋮** → **Settings** → activa **Instant delivery**.
2. Aparecerá una notificación permanente de ntfy ("Subscription service").
   **Es normal y es la que hace el trabajo**: mantiene una conexión abierta en
   vez de depender de Firebase.
3. Además, en Android: **Ajustes → Aplicaciones → ntfy → Batería → Sin
   restricciones**. Si queda en "Optimizada", el sistema la duerme igual.

Con eso los avisos llegan en segundos, incluso con la pantalla apagada. El
costo es algo de batería (una conexión en reposo, poca cosa).

Dos atajos que también la evitan: instalar ntfy desde **F-Droid** (esa versión
no lleva Firebase y siempre es instantánea) o **usar tu propio servidor ntfy**,
porque la app solo pasa por Firebase cuando el canal es de `ntfy.sh`.

Esto **no afecta** al aviso programado de "tu cuota volvió": ese lo entrega el
servidor a su hora y funciona igual con la computadora apagada.

#### Si usas MichiClaude en dos o tres computadoras

Cada instalación **crea su propio canal**, y eso es a propósito:

- En tu segunda PC repites los mismos pasos: enciendes la casilla y escaneas
  **el QR nuevo** con el mismo teléfono. Tu app ntfy acaba con dos canales.
- Los ajustes compartidos (*Fuentes de datos → Guardar en el servidor*) **no
  copian este canal**: esa pantalla promete no guardar contraseñas, y el canal
  **es** la contraseña. Se activa a mano en cada equipo, y son 30 segundos.
- **Ventaja**: en la app ntfy puedes silenciar un canal sin tocar el otro — que
  la PC de casa te avise siempre y la del trabajo se calle el fin de semana.
- **Ojo**: la cuota es de tu *cuenta*, no de la máquina. Si dos PCs están
  encendidas al agotarse, **cada una te avisa por su canal** (dos notificaciones
  del mismo hecho). Es lo esperado; se arregla silenciando un canal.

#### Trata el QR como una contraseña

En ntfy no hay cuentas: **el canal es el secreto**. Quien lo conozca recibe tus
avisos (y puede mandarte notificaciones falsas). Por eso el canal se genera
aleatorio en tu equipo, y por eso **no publiques capturas donde se vea tu QR o
tu canal**. Si se te escapó uno, el botón **Canal nuevo** genera otro y deja el
viejo muerto (tendrás que volver a escanear en tu teléfono).

Como salvaguarda, por ese canal viajan **solo porcentajes y horas de reset**:
nunca nombres de proyectos, ni rutas, ni cuánto gastas. Lo peor que vería un
intruso es "alguien va al 80% de su sesión".

#### ¿Cuánto cuesta? ¿Hay límite?

Es **gratis y sin cuenta**. El servidor público no impone un cupo diario de
mensajes: limita el *ritmo* de peticiones (unas 60 seguidas, luego una cada 5
segundos). MichiClaude manda un puñado de mensajes al día en el peor caso —
un par por límite alcanzado y, si las activas, tus alarmas de %. Ni te
acercas. Si prefieres no depender del servidor público, ntfy es open source y
puedes montar el tuyo: pon su dirección en `"server"` dentro de
`%APPDATA%\com.oscarorozco.michiclaude\ntfy_config.json`.

### El widget gatito 🐱

- **Cápsula "Sesión X%"** sobre su cabeza, siempre visible.
- **Una bombilla** encima con la presión de contexto de tu sesión viva; pasa
  el mouse por ella para ver el detalle.
- **Pasa el mouse por el gato** → globo de cómic con sesión, semanal y los
  buckets por modelo que reporte tu plan; se pliega al salir.
- **Post-its en la tapa de la laptop**: una pilita roja con los hallazgos sin
  leer y una turquesa con los consejos. Al hacer clic en uno se abre el panel
  directamente en esa pestaña.
- **Clic en el sticker** de su pantalla → abre el panel.
- **Arrástralo** a donde quieras (funciona con varios monitores); clic
  derecho lo oculta.
- Estados: normal → 🔥 cuando cruzas una alarma (en modo gatito las alarmas
  de % llegan como **globo de cómic con ✕** en vez de notificación de
  Windows) → 😴 cuando la semana llega al 100%, hasta el reset.

### Fuentes de datos (opcional, para ver varias máquinas)

- **Este PC**: automático (logs de Claude Code).
- **claude.ai**: automático vía la cuota de tu cuenta.
- **WSL**: automático — sus proyectos salen como "nombre · wsl-<distro>"
  (p. ej. "· wsl-Ubuntu"), así que con dos distribuciones instaladas se
  distinguen entre sí.
- **Servidores** (VPS, etc.): nombre + host SSH en *Fuentes de datos* y listo —
  **no tienes que copiar ni instalar nada**. Sus proyectos aparecen como
  "nombre · servidor"; si un servidor no responde, se ignora en silencio y tus
  datos locales nunca se bloquean. Paso a paso con ejemplo, justo abajo.
- **Modo HUB**: si usas MichiClaude en varias máquinas, un servidor puede
  consolidar la foto de todas para que los totales cuadren en cualquier PC.

#### Conectar un servidor, paso a paso

**El problema.** Si usas Claude en tu propia computadora, MichiClaude lo
detecta y lo cuenta de inmediato. Pero si te conectas a una computadora remota
(un servidor en la nube) para trabajar desde allá, MichiClaude no puede
adivinar lo que estás gastando en esa otra máquina.

**La solución.** Darle permiso para que se conecte a tu servidor, revise cuánto
has consumido ahí y te sume ese gasto a tu total.

##### Los 3 requisitos

1. **Poder conectarte al servidor sin escribir contraseña.** MichiClaude
   trabaja solo en segundo plano, así que no puede quedarse esperando a que
   teclees nada: necesita una *llave SSH* (un archivo de acceso automático).
2. **Tener Python en el servidor.** Casi todos los Linux ya lo traen. No hace
   falta que lo compruebes: MichiClaude lo busca y, si no lo encuentra, te
   avisa con las instrucciones.
3. **Haber usado Claude Code al menos una vez en ese servidor.** Si nunca has
   ejecutado `claude` allí, no hay historial de gasto que leer.

> En **Windows** necesitas además el cliente SSH, incluido desde Windows 10.
> Para comprobarlo escribe `ssh` en PowerShell: si te responde con la ayuda,
> ya lo tienes. Si no: Configuración → Aplicaciones → Características
> opcionales → *Cliente OpenSSH*.

##### Ejemplo real, paso a paso

Imagina a **Carlos**. Programa desde su laptop, pero tiene un servidor en la
nube (`carlos@203.0.113.10`) donde corre sus proyectos más pesados.

**Paso 1 — Comprueba el acceso sin contraseña.** Abre la terminal de su laptop:

```bash
ssh carlos@203.0.113.10
```

- *Le deja entrar directo:* listo, al paso 2.
- *Le pide contraseña:* lo resuelve ejecutando **una sola vez** en su laptop
  `ssh-copy-id carlos@203.0.113.10`, que copia su llave al servidor para que no
  se la vuelva a pedir.

**Paso 2 — Usa Claude en el servidor.** Ya conectado, entra a un proyecto y:

```bash
claude "explícame este código"
```

Con eso se crea el primer registro de gasto **dentro** del servidor.

**Paso 3 — Lo agrega en MichiClaude.** Abre el panel desde el icono de la
bandeja (junto al reloj) y va a **Fuentes de datos → Agregar servidor**:

- **Nombre corto**: `servidor-trabajo` (el que quiera, es solo para reconocerlo)
- **Host SSH**: `carlos@203.0.113.10` (exactamente lo mismo que escribe tras `ssh`)
- Pulsa **Probar y agregar**

**¿Qué ocurre después?** MichiClaude se conecta, deja un lector suyo de 16 KB
en `~/.michiclaude/` de ese servidor y empieza a leer **solo los números** de
consumo. Desde ese momento Carlos ve su gasto local sumado al del servidor,
etiquetado como `· servidor-trabajo`.

##### Preguntas rápidas

**¿Es seguro? ¿Va a leer mis conversaciones o mi código?**
No. El lector que instala solo suma contadores de tokens. No lee tus mensajes,
ni tus archivos, ni tu código, y nada de eso sale del servidor.

**¿Y el campo "Comando a ejecutar" de Opciones avanzadas?**
Déjalo en blanco. MichiClaude ya busca solo dónde está Python y usa su propio
lector, en una carpeta que él controla — **no supone nada sobre las rutas de tu
servidor**. Solo tendrías que rellenarlo si quisieras ejecutar tu propia
versión del lector; en ese caso no se escribe nada en tu servidor.

**¿Y si me sale un error?**

| Mensaje | Qué hacer |
|---|---|
| `Permission denied` | Falta la llave sin contraseña: `ssh-copy-id usuario@servidor` |
| `No encontré Python 3.7…` | En el servidor: `sudo apt install python3` |
| `No pude ejecutar ssh` | Instala el *Cliente OpenSSH* en Windows (ver arriba) |
| Se agregó pero no salen datos | Ejecuta `claude` al menos una vez en ese servidor |

## Privacidad y a dónde se conecta

La versión corta: **tu token, tus conversaciones y tu código nunca salen de tu
máquina.** La versión larga es la tabla, porque la app sí habla con unas
cuantas direcciones públicas y mereces saber cuáles.

| Cuándo | A dónde | Qué viaja |
|---|---|---|
| Siempre | `api.anthropic.com` | Tu token OAuth de Claude Code, para leer tu propia cuota — el mismo dominio con el que ya habla Claude Code. |
| Una vez al día (se puede apagar) | LiteLLM (vía `raw.githubusercontent.com`), `models.dev`, `openrouter.ai` | Nada. Un GET anónimo de una tabla pública de precios. |
| Al arrancar y cada 12 h | `github.com` (los Releases de este proyecto) | Nada más que la petición. Comprueba si hay versión nueva; el instalador firmado solo se descarga si aceptas. |
| Solo con los avisos al celular encendidos | `ntfy.sh` (o tu propio servidor) | Porcentajes, horas de reset y —si activas ese aviso— cuánto duró una sesión y cuántos turnos tuvo. Nunca rutas, nunca cifras de gasto, y nunca el nombre del proyecto salvo que actives la casilla que lo dice. |
| Solo si enciendes el análisis local con IA | `huggingface.co` y `github.com` | Nada. Una descarga única de los modelos (~319 MB o ~1.7 GB). |
| Mientras corre el análisis local | `127.0.0.1` | Tu conversación — a un modelo que corre en **tu propia computadora**, y que se apaga en cuanto responde. |

Todo lo que está debajo de la primera fila es opcional o se puede desactivar.
Lo que nunca se manda, con ninguna configuración: tu token (salvo a Anthropic),
tus rutas, el contenido de tus archivos, tus cifras de gasto, ni telemetría de
uso. **La app no recolecta estadísticas de nadie.** Los nombres de proyecto son
la única excepción, y solo si enciendes a propósito una casilla que lo advierte
— ver más abajo.

De dónde salen los números:

1. La app lee el token OAuth de `~/.claude/.credentials.json` (lo crea
   Claude Code al iniciar sesión).
2. Con ese token consulta `https://api.anthropic.com/api/oauth/usage` — el
   mismo servicio que usa la página de Uso de claude.ai.
3. Los costos por proyecto salen de parsear tus `.jsonl` locales; nada de
   eso sale de tu equipo.

### Los avisos al celular, en claro

Es la única función que **envía** algo, y por eso es opcional y viene apagada.

- Lo que sale: el mismo texto que verías en pantalla — "Sin cuota de sesión.
  Vuelvo en 45 min", "Sesión al 80%" — más la hora del reset. **Nunca** el
  token, ni nombres de proyecto, ni rutas, ni cifras de gasto.
- A dónde: al servidor ntfy configurado (`ntfy.sh` por defecto). Ese servidor
  ve tu IP, como cualquier petición HTTP, y **los canales de ntfy son públicos
  por diseño**: el nombre aleatorio del canal es lo que hace de contraseña.
  Por eso lo que se manda está limitado a propósito.
- Cómo apagarlo: la misma casilla de *Ajustes*. Y si quieres tu propio
  servidor ntfy (es open source), cambia `"server"` en
  `%APPDATA%\com.oscarorozco.michiclaude\ntfy_config.json`.

**Los hallazgos nunca se mandan al celular**, justamente porque están
construidos alrededor de nombres de proyectos.

Las fichas del coach tampoco, con una excepción a propósito: los dos avisos de
"puedes irte" — *terminó una sesión larga* y *Claude está esperando tu
aprobación* —, que son la razón misma de tener avisos en el celular. Van tras
su propia casilla, llevan solo duraciones y conteos, y el nombre del proyecto
solo se incluye si activas la segunda casilla.

### La descarga de precios, en claro

Anthropic no publica sus tarifas en ninguna API, así que MichiClaude las toma
de las tablas públicas que mantiene la comunidad, en cascada por fiabilidad:
[LiteLLM](https://github.com/BerriAI/litellm) (la que usa `ccusage`) →
[models.dev](https://models.dev) → [OpenRouter](https://openrouter.ai). Se
guarda el resultado en caché y solo se reintenta cada 24 h; si no hay red, se
usa el caché y, en último término, una tabla incluida en la app.

Qué implica exactamente:

- Es un **GET anónimo a un archivo JSON público**. No se envía tu token, ni
  tus proyectos, ni identificadores, ni estadísticas: solo se descarga.
- Como toda petición HTTP, ese servidor ve tu dirección IP.
- **Se puede apagar.** No hay interruptor en la interfaz a propósito (apagarlo
  solo deja precios viejos, y era fácil hacerlo sin querer), pero sí en la
  configuración: pon `"auto": false` en
  `%APPDATA%\com.oscarorozco.michiclaude\prices_config.json`. En ese mismo
  archivo puedes cambiar las URLs de las fuentes (`litellm_url`,
  `modelsdev_url`, `openrouter_url`) si prefieres un espejo propio.
- Dentro de la app, **Ajustes → Precios de modelos → ⓘ** muestra estas
  mismas fuentes, para que nadie tenga que leer el README para saberlo.
- Si un modelo no aparece en ninguna tabla, se marca con `~` en la app en vez
  de cobrarlo en silencio con una tarifa supuesta.

> ⚠️ **El endpoint de uso no es una API oficial**: Anthropic no lo documenta
> para terceros y puede cambiarlo o apagarlo sin aviso. Si eso pasa, los
> gauges de cuota dejarían de funcionar hasta adaptar la app (el resto —
> costos locales — seguiría funcionando). La app parsea la respuesta de
> forma dinámica precisamente para tolerar cambios.
>
> 🔒 **Tu token nunca sale de tu equipo salvo hacia `api.anthropic.com`**
> (el mismo dominio oficial al que ya se conecta Claude Code). Jamás se
> envía a servidores de terceros, no se loggea ni se muestra en pantalla.
> **Sin telemetría**: la app no recolecta ni envía estadísticas de nadie.
> Al ser open source, puedes verificarlo en el código.

## Actualizaciones

MichiClaude se actualiza solo. Comprueba al arrancar y cada 12 horas; cuando
hay versión nueva aparece una franja en la cabecera del panel. El instalador
se descarga de los Releases de este proyecto y se **verifica su firma** antes
de instalarlo — si la firma no cuadra, no se instala nada.

Si la instalación automática falla por lo que sea, la app ofrece un botón para
descargarla a mano desde Releases. Esa dirección es una constante compilada
dentro de la app: nunca sale de un archivo descargado.

## Compatibilidad por plan

| Plan | Cuota (gauges) | Costo por proyecto |
|---|---|---|
| Pro / Max 5x / Max 20x | ✅ buckets dinámicos (Sonnet/Opus/Fable/los que existan) | ✅ (nocional) |
| Team/Enterprise con Claude Code | ✅ | ✅ |
| Solo API key | ✗ (no hay ventanas de suscriptor) | ✅ (gasto real) |
| Solo claude.ai web, sin Claude Code | ✗ | ✗ |

### ¿Funciona con el plan gratuito de Claude?

**No.** MichiClaude mide el uso de **Claude Code**, y Claude Code requiere
suscripción Pro/Max (o una API key de pago) — no está disponible en el plan
gratuito de claude.ai. Sin Claude Code no existe ni el token de cuota ni los
logs locales: **no hay nada que medir** (la app mostraría "inicia sesión en
Claude Code" y $0.00).

La alternativa sin suscripción: usar Claude Code con una **API key** de
[console.anthropic.com](https://console.anthropic.com) (pago por consumo).
En ese caso verás los costos por proyecto — y serían **dólares reales**, no
estimados — aunque no los medidores de cuota, que son de suscriptores.

---

## Desarrollo

Requisitos (Windows 10 u 11): [Rust](https://rustup.rs) stable, Node.js 18+,
VS Build Tools (C++), WebView2 (preinstalado en Windows 11; en 10 llega con el
Edge moderno) y Claude Code con sesión iniciada.

```powershell
npm install
npm run icons     # genera src-tauri/icons/ desde app-icon.png (solo la primera vez)
npm run dev       # desarrollo
npm run build     # instalador NSIS en src-tauri/target/release/bundle/nsis/
```

**Releases automáticas**: al pushear un tag `v*`, GitHub Actions compila el
instalador y lo publica en Releases.

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## Roadmap

Hecho desde el primer release público: modo HUB, auto-actualización desde
Releases, escaneo incremental de logs, precios configurables, el analizador de
fugas, el coach con sus comandos automáticos y la pestaña Reporte.

Lo que se está mirando ahora:

- [ ] Rangos de fecha (no solo "los últimos N días") en todas las máquinas
- [ ] Temas sobre el analizador de fugas: partir una sesión larga en temas
      para enseñar cuánto habría ahorrado un `/clear` antes
- [ ] Capturas en este README
- [ ] Una tarjeta semanal compartible del gatito

No está planeado: rastrear herramientas que no sean Claude Code, una base de
datos de historial, ni un modo equipo multiusuario.

## Contribuir

Se aceptan aportes, pero **abre un issue antes de escribir código**: es un
proyecto de un solo autor con reglas de diseño bastante cerradas (frontend
vanilla sin dependencias, cero telemetría, nunca pintar una cifra que no se
pueda calcular) y sería una lástima que trabajaras en algo que no encaja.

Léete **[CONTRIBUTING.md](CONTRIBUTING.md)** antes del primer PR. Contiene el
acuerdo de contribución: al abrir un Pull Request conservas tu autoría y tu
aporte entra bajo GPL-3.0, pero das permiso para relicenciarlo — así el
proyecto puede ofrecerse en el futuro también bajo una licencia comercial sin
tener que localizar a cada persona que aportó una línea. También se te pide
declarar el origen de cualquier material de terceros que incluyas
(especialmente imágenes y sonidos, por lo que se explica abajo).

Reportar un fallo o proponer una idea en un issue no requiere nada de esto.

## Licencia

**Código: [GPL-3.0](LICENSE)** — úsalo, modifícalo y compártelo libremente;
si distribuyes una versión modificada, debe seguir siendo open source bajo
esta misma licencia y conservar los créditos. © 2026 Oscar Orozco.

**Excepción**: los gifs de la mascota y el sticker (`src/cat*.gif`,
`src/sticker*.png`) son **fan-art derivado del meme Bongo Cat** (arte
original de [@StrayRogue](https://twitter.com/StrayRogue), meme de
[@DitzyFlama](https://twitter.com/DitzyFlama)). **No** están cubiertos por
la GPL; se incluyen solo como parte de la app y los derechos del personaje
pertenecen a sus autores. Ver el detalle al final de [LICENSE](LICENSE).
