# Building MichiClaude from source

For anyone who prefers to compile the app themselves instead of trusting the
release binaries. The whole build runs on your machine; nothing is downloaded
beyond the dependencies listed here.

## Requirements (Windows 10/11)

- [Node.js](https://nodejs.org) 20 or newer — only used for the Tauri CLI,
  there are no runtime npm dependencies.
- [Rust](https://rustup.rs) (stable toolchain) — installs `cargo` and the
  MSVC build tools it asks for.
- [Git](https://git-scm.com).

## Steps

```powershell
git clone https://github.com/oscarorozcos/michiclaude
cd michiclaude
npm install          # Tauri CLI (devDependency only)
npm run build        # release build
```

The installer comes out at:

```
src-tauri\target\release\bundle\nsis\MichiClaude_<version>_x64-setup.exe
```

Run that installer, or just launch the bare exe at
`src-tauri\target\release\michiclaude.exe`.

### About the signing-key error at the end

The build prints an error mentioning `TAURI_SIGNING_PRIVATE_KEY`. **It is
expected and harmless**: that key signs the auto-update artifacts and only
exists as a secret in the project's CI. Your locally built app works fully —
it just can't produce signed updates, which you don't need for your own build.

### Development mode

```powershell
npm run dev
```

Opens the app with hot reload for the frontend. Backend changes require a
rebuild (`cargo` recompiles automatically on the next `dev`/`build`).

---

# Compilar MichiClaude desde el código

Para quien prefiera compilar la app por su cuenta en vez de confiar en los
binarios del release. Todo corre en tu máquina; no se descarga nada más allá
de las dependencias listadas aquí.

## Requisitos (Windows 10/11)

- [Node.js](https://nodejs.org) 20 o más nuevo — solo se usa para el CLI de
  Tauri, no hay dependencias npm en la app.
- [Rust](https://rustup.rs) (toolchain estable) — instala `cargo` y las
  herramientas MSVC que pida.
- [Git](https://git-scm.com).

## Pasos

```powershell
git clone https://github.com/oscarorozcos/michiclaude
cd michiclaude
npm install          # CLI de Tauri (solo devDependency)
npm run build        # compilación de release
```

El instalador queda en:

```
src-tauri\target\release\bundle\nsis\MichiClaude_<versión>_x64-setup.exe
```

Ejecuta ese instalador, o directamente el exe suelto en
`src-tauri\target\release\michiclaude.exe`.

### Sobre el error de la llave de firma al final

La compilación imprime un error que menciona `TAURI_SIGNING_PRIVATE_KEY`.
**Es esperado y no afecta en nada**: esa llave firma los artefactos del
auto-actualizador y solo existe como secreto en el CI del proyecto. Tu app
compilada localmente funciona completa — solo no puede producir
actualizaciones firmadas, que para tu propia compilación no necesitas.

### Modo desarrollo

```powershell
npm run dev
```

Abre la app con recarga en caliente del frontend. Los cambios del backend
requieren recompilar (`cargo` lo hace solo en el siguiente `dev`/`build`).
