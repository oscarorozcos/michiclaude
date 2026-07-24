# MichiClaude 🐱

> 🚧 **En desarrollo activo** — la app ya es funcional y se usa a diario,
> pero siguen llegando mejoras (ver [Roadmap](#roadmap)). Issues y
> sugerencias son bienvenidos.

Widget de bandeja para **Windows 11** que muestra, en tiempo real, cuánto has
usado de tu suscripción de Claude y cuánto te queda:

- **Cuota real de tu plan** (sesión de 5 h y límites semanales, con barras por
  modelo) — la misma que ves en claude.ai → Configuración → Uso, porque los
  límites son compartidos entre claude.ai, Claude Code y los IDEs.
- **Marcador de ritmo**: una línea indica cuánto del periodo ha transcurrido;
  si tu consumo va más rápido que el reloj, los colores pasan a ámbar/rojo.
- **Proyección**: "a este ritmo llegas al 100% en X min, antes del reset".
- **Costo estimado por proyecto** (equivalente API) con desglose por modelo,
  periodos de 1/7/30 días y gráfica de tendencia diaria.
- **Varias máquinas en un solo tablero**: tu PC, WSL y servidores por SSH.
- **Alarmas configurables**, presupuesto semanal, exportación CSV/JSON,
  tema claro/oscuro y 8 idiomas.
- **Widget flotante opcional**: una pastilla minimalista… o un **gatito
  programador** 🐱 que teclea tranquilo cuando vas bien, arde 🔥 cuando cruzas
  tu alarma y duerme 😴 si agotas la semana.

Construido con [Tauri 2](https://tauri.app): binario nativo pequeño, frontend
HTML/CSS/JS sin frameworks, backend Rust mínimo.

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

## Primeros pasos: qué estás viendo

El panel tiene **tres pestañas**:

- **Principal** — todo el tablero:
  - *Cuánto te queda*: gauge de la sesión de 5 h + barras semanales (una por
    modelo, según reporte tu plan). La marquita vertical en cada barra es el
    **ritmo**: si tu barra de consumo la rebasa, vas más rápido que el reloj.
  - *A este ritmo*: proyección de burn rate — si sigues así, ¿chocas con el
    límite antes del reset?
  - *Gasto por proyecto*: costo estimado de cada proyecto (ver siguiente
    sección) en 1/7/30 días. Pasa el mouse sobre un proyecto para ver su
    desglose por modelo. La fila *claude.ai / otros* estima (en % de cuota)
    lo consumido fuera de esta máquina.
  - *Tendencia diaria*: gráfica de los últimos 30 días.
  - *Modelos*: qué modelo usas más.
- **Fuentes de datos** — de dónde salen los números y alta de servidores SSH.
- **Preferencias** — idioma, widget flotante, alarmas, presupuesto y export.

El pie (Hoy / periodo) siempre está visible.

## ¿De dónde salen los dólares? (costo estimado)

De dos ingredientes, **ambos en tu equipo**:

1. **Tus logs locales de Claude Code** (`~/.claude/projects/**/*.jsonl`):
   cada petición queda registrada con sus tokens (entrada, salida, caché) y
   el modelo usado. La app los parsea con deduplicación (los logs repiten
   entradas al reanudar sesiones) y excluye la lectura de caché del conteo
   de tokens "de trabajo" (la incluye solo en el costo, a su precio real).
2. **La lista de precios pública de la API de Anthropic** (USD por millón de
   tokens):

   | Modelo | Entrada | Salida | Escritura caché | Lectura caché |
   |---|---|---|---|---|
   | Opus / Fable / Mythos | $15 | $75 | $18.75 | $1.50 |
   | Sonnet (y no reconocidos) | $3 | $15 | $3.75 | $0.30 |
   | Haiku | $1 | $5 | $1.25 | $0.10 |

**Ejemplo**: si un proyecto usó 2M de tokens de entrada y 0.5M de salida con
Sonnet → 2×$3 + 0.5×$15 = **$13.50 equivalente API**.

> 💡 **Importante**: para suscriptores este costo es **nocional** ("equiv.
> API") — no es dinero que pagaste, sino lo que *habría costado* pagando por
> API. Sirve para saber qué proyecto consume más y cuánto te ahorra la
> suscripción. Solo es gasto real si usas API key.

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
`mi-web · vps`.

## Configuración (pestaña Preferencias)

| Opción | Qué hace |
|---|---|
| **Idioma** | 8 idiomas; se autodetecta la primera vez. |
| **Widget flotante** | Muestra la pastilla (o el gatito) siempre visible sobre la barra de tareas. También se activa desde el menú del icono de bandeja. |
| **Estilo del widget** | *Pastilla* (marca + % + barras S/W) o *Gatito con laptop* 🐱. |
| **Diseño de la pastilla** | *Clásico* o *Tarjeta coral* (otro esquema de color). |
| **Alarmas de sesión** | Los % a los que quieres aviso (por defecto 80 y 95). Al cruzar uno, la notificación se repite cada 5 min **hasta que abras el panel** ("enterado"). |
| **Presupuesto semanal en $** | Si el costo estimado de 7 días lo supera, recibes un aviso (uno por semana). 0 = sin aviso. |
| **Exportar datos** | Guarda CSV o JSON del desglose en la carpeta que elijas (vacía = Descargas). |

### El widget gatito 🐱

- **Cápsula "Sesión X%"** sobre su cabeza, siempre visible.
- **Pasa el mouse** → globo de cómic con sesión, semanal y los buckets por
  modelo que reporte tu plan; se pliega al salir.
- **Clic en el sticker** de su pantalla → abre el panel.
- **Arrástralo** a donde quieras (funciona con varios monitores); clic
  derecho lo oculta.
- Estados: normal → 🔥 cuando cruzas una alarma (en modo gatito las alarmas
  de % llegan como **globo de cómic con ✕** en vez de notificación de
  Windows) → 😴 cuando la semana llega al 100%, hasta el reset.

### Fuentes de datos (opcional, para ver varias máquinas)

- **Este PC**: automático (logs de Claude Code).
- **claude.ai**: automático vía la cuota de tu cuenta.
- **WSL**: automático — sus proyectos salen como "nombre · wsl".
- **Servidores** (VPS, etc.): en *Fuentes de datos*, agrega nombre + host SSH
  (el mismo alias/usuario@ip con el que ya te conectas; usa tu llave SSH) y
  pulsa *Probar y agregar*. Sus proyectos aparecen como "nombre · servidor".
  Si un servidor no responde, se ignora en silencio — tus datos locales
  nunca se bloquean.

## Privacidad y cómo se conecta

1. La app lee el token OAuth de `~/.claude/.credentials.json` (lo crea
   Claude Code al iniciar sesión).
2. Con ese token consulta `https://api.anthropic.com/api/oauth/usage` — el
   mismo servicio que usa la página de Uso de claude.ai.
3. Los costos por proyecto salen de parsear tus `.jsonl` locales; nada de
   eso sale de tu equipo.

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

Requisitos (Windows): [Rust](https://rustup.rs) stable, Node.js 18+,
VS Build Tools (C++), WebView2 (incluido en Windows 11) y Claude Code con
sesión iniciada.

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

- [ ] Modo HUB: un servidor consolida los datos de todas tus máquinas para
      que los totales cuadren en cualquier PC
- [ ] Auto-actualización desde GitHub Releases (`tauri-plugin-updater`)
- [ ] Lectura incremental de `.jsonl` por offset (hoy: escaneo completo por refresco)
- [ ] Precios de modelos configurables (JSON externo)

## Licencia

**Código: [GPL-3.0](LICENSE)** — úsalo, modifícalo y compártelo libremente;
si distribuyes una versión modificada, debe seguir siendo open source bajo
esta misma licencia y conservar los créditos. © 2026 Oscar Orozco.

**Excepción**: los gifs de la mascota y el sticker (`src/cat*.gif`,
`src/sticker.png`) son **fan-art derivado del meme Bongo Cat** (arte
original de [@StrayRogue](https://twitter.com/StrayRogue), meme de
[@DitzyFlama](https://twitter.com/DitzyFlama)). **No** están cubiertos por
la GPL; se incluyen solo como parte de la app y los derechos del personaje
pertenecen a sus autores. Ver el detalle al final de [LICENSE](LICENSE).
