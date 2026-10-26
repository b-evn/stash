# stash

A tiny clipboard history for Windows. Press a hotkey and a small list pops up
right at your text caret — pick a clip with the keyboard and it's pasted.
Text, links, code, images and files all go in. Text inside copied images is
recognized locally, so screenshots are searchable too. Everything stays on
your PC.

<p align="center"><img src="docs/in-use-light.png" width="520" alt="stash in use"></p>

## Paste stack

<img src="docs/stack-dark.png" align="right" width="300" alt="paste stack">

Collect clips in the order you need them, then paste them one after another
with plain `Ctrl+V`, in any app. Made for forms: email, Tab, Ctrl+V, IBAN,
Tab, Ctrl+V…

1. In the popup, `Shift+Enter` (or the row's stack button) adds a clip. It
   gets a number, and the footer becomes the stack bar.
2. `Enter` pastes clip 1 and closes the popup. Each `Ctrl+V` after that pastes
   the next clip.
3. It ends after the last clip, when you copy something yourself, or with the
   × in the stack bar (open the popup to see what's next).

`Ctrl+Shift+Enter` instead pastes all text clips at once, one per line.

stash never intercepts `Ctrl+V`: while a stack runs it only notices that you
pasted and then puts the next clip on the clipboard.

<br clear="right">

## Use

| key | does |
| --- | --- |
| `Ctrl+Alt+V` | open / close the popup (configurable) |
| type | search |
| `↑` `↓` · `Ctrl+J/K` | move |
| `Enter` | paste |
| `Ctrl+1`…`9` | paste the numbered row |
| `Ctrl+Enter` | copy only, don't paste |
| `Shift+Enter` | add to / remove from the paste stack |
| `Ctrl+Shift+Enter` | paste the stack's text clips at once |
| `Ctrl+P` | pin / unpin (pinned clips are never pruned) |
| `Ctrl+D` · `Del` (empty search) | delete |
| `Tab` / `Shift+Tab` | cycle tabs: all · text · links · code · images · files |
| `Ctrl+S` | cycle sort: recent · frequent · a–z · largest |
| `Ctrl+T` | theme: auto · dark · light |
| `Backspace` (empty search) | remove the last filter chip |
| `Esc` | clear search, then the stack, then close |

### Search

Plain words are fuzzy (`ghb` finds *GitHub*), and every word must match.

| syntax | meaning |
| --- | --- |
| `'word` | exact substring |
| `^word` | starts with |
| `!word` | exclude clips containing it |
| `@link` `@code` `@path` `@color` `@number` `@email` `@text` `@image` `@file` | only that kind |
| `@pin` | only pinned |
| `@hour` `@today` `@week` | only recent |

A word with a capital letter matches case-sensitively. Filter words turn into
chips as you finish them (type a space); click a chip's × to drop it.
Combine freely: `@code !test fetch`.

Text that stash recognized inside a copied image is searched along with the
clip, and the first line is shown in the image's row. OCR runs on your machine
through the Windows OCR engine; set `ocr_images` to `false` in the config to
turn it off.

<p align="center"><img src="docs/showcase.png" width="660" alt="filter chips, dark and light"></p>

The pin button next to the tabs shows pinned clips only. Hover a row for its
quick actions: open a link in your browser, show a file in Explorer, copy code
as a fenced block, stack, pin or delete. The tray icon has: pause capturing,
start with Windows, clear history (keeps pinned), quit.

## Config

`%APPDATA%\app.stash.clipboard\config.json` is created on first run:

```json
{
  "hotkey": "Ctrl+Alt+V",
  "max_items": 2000,
  "max_clip_chars": 100000,
  "ignore_sensitive": true,
  "capture_images": true,
  "capture_files": true,
  "ocr_images": true,
  "max_image_megapixels": 40
}
```

Edits apply as soon as you save the file, no restart needed. If the file or
the hotkey is invalid, the previous settings stay active.

History lives next to it in `clips.db` (SQLite, not encrypted). Images are
stored in full as PNG, with a small preview for the list; recognized text is
stored next to them. For copied files stash keeps the paths, not the files:
pasting puts the same files back on the clipboard, so a moved or deleted file
won't paste. With
`ignore_sensitive`, copies that password managers mark as "exclude from
history" are never stored.

## Build

Needs Rust and, on Windows 10, the WebView2 runtime (already on Windows 11).

```sh
cargo install tauri-cli --version "^2" --locked
cd src-tauri
cargo tauri dev      # run
cargo tauri build    # exe + installer in target/release
```

`target/release/stash.exe` is a single portable file. Every push to `main` also
builds it in GitHub Actions (artifact `stash-windows`), so you don't need a
local toolchain.

Tests: `node ui/search.test.js` and `cd src-tauri && cargo test --lib`.
`ui/index.html` also opens in a plain browser with demo data for UI work;
`docs/showcase.html`, `docs/in-use.html` and `docs/stack.html` are the scenes for the screenshots.

## Known limits

- The popup appears at the caret in apps that expose a system caret (Notepad,
  Word, most native fields). Browsers and Electron apps draw their own caret, so
  there it appears at the mouse pointer instead.
- It can't paste into windows running as administrator unless stash does too,
  and a paste stack doesn't notice pastes made into them.
- A paste stack advances 120 ms after you let go of V. An app that reads the
  clipboard later than that would get the next clip instead.
- Image OCR uses the language packs installed in Windows. It handles ordinary
  screenshots and photos of text; very small or stylised text can be missed.
- Written and compile-checked against Windows targets from a Linux machine; it
  has not been run on a real Windows desktop yet, so expect to tweak placement
  and focus behaviour.

UI font: [Geist](https://github.com/vercel/geist-font) (SIL OFL, `ui/fonts`). Icons: [Lucide](https://lucide.dev) (ISC).
