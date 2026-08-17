# Contributing to MichiClaude · Contribuir a MichiClaude

> 🇬🇧 English below · 🇪🇸 [Español más abajo](#-español)

---

# 🇬🇧 English

Thanks for your interest. MichiClaude is a single-author project, so before
you invest time writing code, **please open an issue** and let's discuss the
idea first. It's the fastest way to avoid writing something that doesn't fit
the design or that's already in progress.

## Before opening a Pull Request

- **Talk first.** An issue describing your proposal saves us both work.
  Large unannounced PRs tend to go unmerged.
- **One PR, one topic.** Don't mix a bug fix with a redesign.
- **Make sure it builds.** `cd src-tauri && cargo check` clean, and if you
  touched the frontend, `npm run build` must finish without errors.
- **Commit messages are in Spanish**, [Conventional Commits] format
  (`fix(coach): ...`, `feat(panel): ...`, `docs: ...`).

## Non-negotiable project rules

They're explained in full in `CLAUDE.md` (section "INVARIANTES"), but these
are the ones most often broken by accident:

- **Vanilla frontend.** Hand-written HTML + CSS + JS. No frameworks, no
  bundlers, no runtime npm dependencies.
- **Zero telemetry.** The app sends no data about you anywhere. The user's
  OAuth token is never logged, never displayed, and never sent to any domain
  other than `api.anthropic.com`.
- **Never invent numbers.** If a value can't be computed, don't render it.
  We'd rather show a gap than a pretty, wrong figure.
- **All visible text goes through `t()`.** The UI ships in 8 languages.
- **Don't touch the mascot gifs.** They're Bongo Cat fan-art under a separate
  license (see below) and are cropped via CSS, not by editing the files.
- **New Rust dependencies:** only if strictly necessary, with minimal
  features.

## Contribution agreement (important)

MichiClaude is released under **GPL-3.0**, and I want to keep the option of
offering it in the future under other terms as well (for example, a
commercial license for anyone wanting to integrate it into a closed product).
For that to remain possible without having to track down every person who
ever contributed a line, I need your explicit permission.

**By opening a Pull Request in this repository, you represent and agree to
the following:**

1. **You keep your authorship.** You do not assign your copyright to me: you
   remain the author of what you wrote and may reuse it anywhere.

2. **Your contribution is licensed under GPL-3.0**, the same license as the
   rest of the project.

3. **You grant me the right to relicense.** You grant Oscar Orozco a
   perpetual, worldwide, irrevocable, non-exclusive, royalty-free license to
   use, reproduce, modify, distribute and **sublicense your contribution
   under any terms**, including proprietary or commercial licenses. In plain
   words: I can sell a commercial license of MichiClaude without having to
   ask you again.

4. **You also grant any patent rights** necessary to use your contribution,
   under the same terms.

5. **You warrant the origin of what you submit.** That it is your original
   work or that you have the right to contribute it; that it contains no
   third-party code, images, sounds, typefaces or other material without
   clearly stating its source and license in the PR; and that your employer
   or client holds no rights over that work that would prevent it (if unsure,
   check before submitting).

6. **Your contribution is provided "as is"**, without warranties of any kind.

Nothing to sign and no bot to install: **opening the Pull Request is your
acceptance**. If you don't agree with these terms, please don't open a PR —
you're very welcome to report the problem in an issue and describe the fix in
words instead.

### About third-party material

If your contribution includes anything you didn't write, **say so in the PR
description** along with its license. This matters especially for images and
sounds: the mascot gifs are fan-art derived from the *Bongo Cat* meme and sit
outside the GPL (see the ASSETS EXCEPTION at the end of [LICENSE](LICENSE)).
Don't add new artwork without discussing it in an issue first.

## Reporting a problem

When opening an issue it helps a lot to include: MichiClaude version, Windows
version, what you expected and what actually happened. **Never paste your
OAuth token, the contents of `~/.claude/.credentials.json`, or paths
containing private data** — if you attach a `quota_debug.json`, scrub
anything sensitive from it first.

---

# 🇪🇸 Español

Gracias por el interés. MichiClaude es un proyecto de un solo autor, así que
antes de invertir tu tiempo en código, **abre un issue** y comentemos la idea.
Es la forma más rápida de evitar que escribas algo que no encaje con el diseño
o que ya esté en camino.

## Antes de abrir un Pull Request

- **Habla primero.** Un issue con la propuesta ahorra trabajo a los dos. Los
  PR grandes que llegan sin aviso suelen quedarse sin mergear.
- **Un PR, un tema.** Nada de mezclar un arreglo de bug con un rediseño.
- **Que compile.** `cd src-tauri && cargo check` limpio, y si tocaste el
  frontend, que `npm run build` termine sin errores.
- **Commits en español**, formato [Conventional Commits]
  (`fix(coach): ...`, `feat(panel): ...`, `docs: ...`).

## Reglas del proyecto que no se negocian

Están explicadas a fondo en `CLAUDE.md` (sección "INVARIANTES"), pero las
que más se rompen sin querer:

- **Frontend vanilla.** HTML + CSS + JS a mano. Sin frameworks, sin
  bundlers, sin dependencias npm de runtime.
- **Cero telemetría.** La app no manda a ningún lado datos sobre ti. El token
  OAuth del usuario nunca se registra en logs, nunca se muestra en pantalla y
  nunca viaja a otro dominio que no sea `api.anthropic.com`.
- **Nunca inventar cifras.** Si un dato no se puede calcular, no se pinta.
  Preferimos un hueco a un número bonito y falso.
- **Todo texto visible pasa por `t()`.** La interfaz está en 8 idiomas.
- **No toques los gifs de la mascota.** Son fan-art de Bongo Cat con una
  licencia aparte (ver más abajo) y se recortan por CSS, no editando los
  archivos.
- **Dependencias de Rust nuevas:** solo si son imprescindibles, y con las
  features mínimas.

## Acuerdo de contribución (importante)

MichiClaude se publica bajo **GPL-3.0**, y quiero conservar la posibilidad de
ofrecerlo en el futuro también bajo otras condiciones (por ejemplo, una
licencia comercial para quien quiera integrarlo en un producto cerrado). Para
que eso siga siendo posible sin tener que localizar a cada persona que alguna
vez aportó una línea, necesito un permiso explícito por tu parte.

**Al abrir un Pull Request en este repositorio, declaras y aceptas lo
siguiente:**

1. **Conservas tu autoría.** No me cedes tu copyright: sigues siendo autor de
   lo que escribiste y puedes reutilizarlo donde quieras.

2. **Tu aporte entra bajo GPL-3.0**, la misma licencia del resto del proyecto.

3. **Me concedes permiso para relicenciar.** Le otorgas a Oscar Orozco una
   licencia perpetua, mundial, irrevocable, no exclusiva y libre de regalías
   para usar, reproducir, modificar, distribuir y **sublicenciar tu aporte
   bajo cualquier condición**, incluidas licencias propietarias o comerciales.
   En cristiano: puedo vender una licencia comercial de MichiClaude sin tener
   que pedirte permiso otra vez.

4. **Concedes también los derechos de patente** que fueran necesarios para
   usar tu aporte, en los mismos términos.

5. **Garantizas el origen de lo que envías.** Que es obra tuya original o que
   tienes derecho a aportarlo; que no incluye código, imágenes, sonidos,
   fuentes tipográficas ni ningún otro material de terceros sin declarar
   claramente en el PR su procedencia y su licencia; y que tu empleador o
   cliente no tiene derechos sobre ese trabajo que lo impidan (si tienes
   dudas, consúltalo antes de enviarlo).

6. **Tu aporte se entrega "tal cual"**, sin garantías de ningún tipo.

No hay que firmar nada ni instalar ningún bot: **abrir el Pull Request es la
aceptación**. Si no estás de acuerdo con estas condiciones, no abras el PR —
puedes reportar el problema en un issue y describir la solución con palabras,
que también ayuda mucho.

### Sobre material de terceros

Si tu aporte incluye algo que no escribiste tú, **dilo en la descripción del
PR** con su licencia. Es especialmente delicado con imágenes y sonidos: los
gifs de la mascota son fan-art derivado del meme *Bongo Cat* y están fuera de
la GPL (ver la EXCEPCIÓN DE ASSETS al final de [LICENSE](LICENSE)). No
agregues arte nuevo sin haber hablado antes en un issue.

## Reportar un problema

Al abrir un issue ayuda muchísimo incluir: versión de MichiClaude, versión de
Windows, qué esperabas que pasara y qué pasó. **Nunca pegues tu token OAuth,
el contenido de `~/.claude/.credentials.json` ni rutas con datos privados** —
si adjuntas un `quota_debug.json`, bórrale antes cualquier dato sensible.

[Conventional Commits]: https://www.conventionalcommits.org/
