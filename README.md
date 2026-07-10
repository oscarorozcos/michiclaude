# Claude Code Meter

Widget de bandeja para Windows que muestra, en tiempo real:

- **Cuota real** de tu plan de Claude (sesión de 5 h y límites semanales, con buckets por modelo) — la misma que ves en claude.ai → Configuración → Uso, porque los límites son compartidos entre claude.ai, Claude Code e IDEs.
- **Marcador de ritmo**: una línea que indica cuánto del periodo ha transcurrido; si tu consumo la adelanta, se pinta de ámbar/rojo.
- **Proyección**: a tu ritmo actual, ¿agotas la sesión antes del reset?
- **Gasto por proyecto** (equivalente API) y **modelo más usado**, parseando los logs locales de `~/.claude/projects` con deduplicación por `message.id + requestId`.

Construido con [Tauri 2](https://tauri.app): binario nativo pequeño, frontend HTML/CSS/JS, backend Rust mínimo.

## Cómo se conecta (sin API key, sin login propio)

1. Lee el token OAuth de `~/.claude/.credentials.json` (creado al iniciar sesión en Claude Code).
2. Consulta `https://api.anthropic.com/api/oauth/usage` con ese token.
3. Parsea los `.jsonl` locales para el desglose por proyecto/modelo.

> ⚠️ El endpoint de uso **no es una API oficial** y puede cambiar sin aviso.
> 🔒 El token nunca sale de tu equipo salvo hacia `api.anthropic.com`. Sin telemetría.

## Requisitos de desarrollo (Windows)

- [Rust](https://rustup.rs) (stable)
- Node.js 18+
- Microsoft Visual Studio Build Tools (C++), y WebView2 (incluido en Windows 11)
- Claude Code instalado y con sesión iniciada

## Correr en desarrollo

```powershell
npm install
npm run icons     # genera src-tauri/icons/ desde app-icon.png (solo la primera vez)
npm run dev
```

Aparece el icono en la bandeja; clic izquierdo abre/cierra el panel.

## Compilar el EXE

```powershell
npm run build
# instalador en src-tauri/target/release/bundle/nsis/
```

## Releases automáticas

Al pushear un tag `v*`, GitHub Actions (`.github/workflows/release.yml`) compila el instalador de Windows y lo publica en Releases.

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## Compatibilidad por plan

| Plan | Cuota (gauges) | Coste local |
|---|---|---|
| Pro / Max 5x / Max 20x | ✅ buckets dinámicos (Sonnet/Opus/Fable/los que existan) | ✅ (nocional) |
| Team/Enterprise con Claude Code | ✅ | ✅ |
| Solo API key | ✗ (no hay ventanas de suscriptor) | ✅ (gasto real) |
| Solo claude.ai web, sin Claude Code | ✗ | ✗ |

## Roadmap

- [ ] Notificaciones de Windows al cruzar 80/90% (`tauri-plugin-notification`)
- [ ] Auto-actualización desde GitHub Releases (`tauri-plugin-updater`)
- [ ] Lectura incremental de `.jsonl` por offset (hoy: escaneo completo por refresco)
- [ ] Autoarranque con Windows (`tauri-plugin-autostart`)
- [ ] Tema claro
- [ ] Precios de modelos configurables (JSON)

## Licencia

MIT
