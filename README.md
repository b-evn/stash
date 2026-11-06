<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/hero-light.webp">
  <img alt="stash: everything you copied, one keystroke away. The popup over a Baroque painting of the gods." src="docs/hero-dark.webp">
</picture>

<p align="center">
  <b>A tiny clipboard history for Windows and macOS.</b><br>
  Press a hotkey and a small list pops up right at your text caret. Pick a clip with the keyboard and it's pasted.
</p>

<p align="center">
  <img alt="Windows 10 and 11" src="https://img.shields.io/badge/Windows-10%20%7C%2011-27272a?style=flat-square&labelColor=09090b">
  <img alt="macOS 10.15 and later, universal" src="https://img.shields.io/badge/macOS-10.15%2B%20universal-27272a?style=flat-square&labelColor=09090b">
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-27272a?style=flat-square&labelColor=09090b">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-27272a?style=flat-square&labelColor=09090b">
  <img alt="Stays on your computer" src="https://img.shields.io/badge/data-stays%20local-fafafa?style=flat-square&labelColor=09090b">
</p>

<p align="center">
  <a href="#everything-goes-in">Everything goes in</a> ·
  <a href="#paste-stack">Paste stack</a> ·
  <a href="#keys">Keys</a> ·
  <a href="#find-anything">Find anything</a> ·
  <a href="#private-by-default">Private by default</a> ·
  <a href="#build">Build</a>
</p>

<br>

## Everything goes in

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/kinds-light.webp">
  <img alt="Six kinds of clip: text, links, code, images, files and colours, each with its search filter." src="docs/kinds-dark.webp">
</picture>

Text, links, code, images and files all go in, and
everything stays on your computer, encrypted.
Text keeps the formatting it was copied with, so bold, links and tables from a
browser or Word paste back as they were; `Alt+Enter` pastes plain text.
Text inside screenshots is recognized on your machine, so you can search
those too.

- **Previews** for what a row can't spell out: thumbnails for image files, SVG
  markup and image data URLs, an "Aa" in the font for font files, swatches for
  colours and CSS gradients, the language on code. Hold `Shift` to see an image
  larger or long text in full.
- **Quick actions** on hover: open a link, show a file in Explorer or Finder,
  copy code as a fenced block, copy a colour as HEX, RGB or HSL. Right-click for
  the same in a menu.
- **macOS screenshots** (`⌘⇧3`, `⌘⇧4`, `⌘⇧5`) are still saved as files and land
  in the history too, ready to paste.

## Paste stack

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/stack-light.webp">
  <img alt="A paste stack of four clips over a payment form: each Cmd+V pastes the next clip into the next field." src="docs/stack-dark.webp">
</picture>

Made for forms. `Ctrl`-click clips in the order you need them, close the popup,
and each plain `Ctrl+V` pastes the next one, in any app: email, Tab, `Ctrl+V`,
IBAN, Tab, `Ctrl+V`…

Stacked clips go in as plain text.
The stack ends after its last clip, when you copy something
yourself, or with the × in its bar. It never holds up your paste: it waits for
you to let go of V, then puts the next clip on the clipboard.

## Keys

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/keys-light.webp">
  <img alt="The main keys: Ctrl+Option+V opens, Enter pastes, Cmd+1 to 9 pastes a row, hold Shift to preview, Cmd-click stacks, Tab cycles kinds." src="docs/keys-dark.webp">
</picture>

On macOS read `Ctrl` as `⌘` and `Alt` as `⌥`, except in the hotkey, which is
`⌃⌥V` on both.

| key | does |
| --- | --- |
| `Ctrl+Alt+V` | open / close the popup (configurable) |
| `Enter` | paste |
| `Ctrl+1`…`9` | paste that row |
| `Alt+Enter` | paste as plain text |
| `Ctrl+Enter` | copy without pasting |
| hold `Shift` | preview |
| `Ctrl`+click | add to the paste stack |
| `Ctrl+P` | pin, so it's never pruned |
| `Ctrl+D` | delete |
| `Tab` | cycle kinds |
| `Ctrl+S` | cycle sort: recent · frequent · a–z · largest |
| `Ctrl+T` | dark or light |
| `Ctrl+,` | settings |
| `Esc` | clear the search, then close |

## Find anything

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/panels-light.webp">
  <img alt="Three popups: filter chips for files without logo, a code clip previewed with Shift held, and the settings page." src="docs/panels-dark.webp">
</picture>

Just type. Words match loosely (`ghb` finds *GitHub*), every word has to match,
and a capital letter makes a word case-sensitive.

| type | to find |
| --- | --- |
| `'word` · `^word` · `!word` | exactly it · starting with it · without it |
| `@code` `@link` `@image` `@file` `@color` `@email` `@number` `@path` `@text` | one kind |
| `@pin` · `@hour` `@today` `@week` | pinned · recent |

Filters turn into chips as you type them. Combine freely: `@code !test fetch`.

The gear in the footer opens the settings. They start with what stash is doing:
capturing or paused, how many clips, and whether they're encrypted on disk. Then
the shortcut, start at login, theme, a tile for each kind of clip that gets
captured, and how many clips to keep. They're saved to `config.json`, which you
can also edit by hand; it applies as soon as you save.

## Private by default

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/secrets-light.webp">
  <img alt="Blurred secrets in the list, each row naming its kind: an IBAN, a Visa card, a GitHub token, a password revealed under the pointer, and a database URL." src="docs/secrets-dark.webp">
</picture>

- **Secrets are blurred** in the list, so they stay hidden when you share your
  screen: IBANs, card numbers, API tokens, passwords, private keys and URLs with
  a password in them. Each row names what it holds, such as
  *Visa card* or *GitHub token*; hover to read it. Pasting gives you the clip as it was copied.
- **Encrypted on disk.** The history is a
  [SQLCipher](https://www.zetetic.net/sqlcipher/) database (AES-256), and its
  key lives in the macOS Keychain or Windows Credential Manager, never in a
  file. Copies and backups of it are unreadable. It guards the history at rest,
  not against software running as you.
- **Password managers are respected.** Copies they mark as "exclude from
  history" (1Password, KeePassXC…) are never stored.
- **Nothing leaves your computer.** Text in images is recognized by the OCR
  built into Windows and macOS. No account, no cloud.

## Build

Every push to `main` builds both in GitHub Actions (artifacts `stash-windows`
and `stash-macos`), so you don't need a local toolchain. To build it yourself
you need Rust, on Windows also Strawberry Perl (for the OpenSSL that SQLCipher
uses) and on Windows 10 the WebView2 runtime, on macOS the Xcode command line
tools.

```sh
cargo install tauri-cli --version "^2" --locked
# or prebuilt, in seconds: npm install -g @tauri-apps/cli@2, then write `tauri` for `cargo tauri`
cd src-tauri
cargo tauri dev           # run
cargo tauri build         # Windows: portable stash.exe + installer
../scripts/build-mac.sh   # macOS: stash.app + .dmg
```

On macOS drag `stash.app` to Applications. It isn't notarized, so allow the
first launch under System Settings → Privacy & Security → Open Anyway, and give
it **Accessibility** there so it can paste for you (without it, stash copies the
clip and you press `⌘V`). `build-mac.sh` signs every build as the same app, so
macOS keeps that permission and the Keychain access.

Tests: `node ui/search.test.js`, `node ui/secrets.test.js` and
`cd src-tauri && cargo test --lib`. `ui/index.html` opens in a plain browser
with demo data; the pictures here are that popup, redrawn by
`scripts/readme-art.sh`.

<br>

<p align="center">
  <img alt="stash" src="src-tauri/icons/128x128@2x.png" width="72"><br>
  <sub>UI font: <a href="https://github.com/vercel/geist-font">Geist</a> (SIL OFL, <code>ui/fonts</code>). Icons: drawn for stash.</sub><br>
  <sub>Backdrops, both public domain: Peter Paul Rubens, <i>The Fall of Phaeton</i> (c. 1604–08, National Gallery of Art, Washington) and Francisco Bayeu, <i>Olympus: The Battle of the Giants</i> (1764, Museo del Prado).</sub>
</p>
