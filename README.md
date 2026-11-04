# Penumbra

A notes app where notes live on an infinite canvas instead of in a list, and drift toward the things they're about.

The idea was basically to treat a pile of notes like an astronomer keeps an almanac (not a typo): Precisely recorded, always moving toward where they belong. You're supposed to write in plain Markdown, and then Penumbra reads the meaning, and related notes stay together on the map.
Pin the important ones and they become fixed stars everything else orbits! Pretty cool vision right.

It runs natively (macOS, Linux, Windows) and in the browser (eventually as a PWA), from one Rust + [Slint](https://slint.dev) codebase!!!!! Write once, run basically anywhere.

## Screenshots

The map is your whole vault at once. Glass cards sit over a ruled grid, each with a few colored dots for its tags, and faint edges connect notes that ended up near each other.

![The Penumbra map view](Docs/screenshots/map.png)

Opening a note zooms you into the editor. It's WYSIWYG, but it doesn't hide your syntax: the `#` on a heading is still there, just dimmed, because the thing you're editing really is the Markdown underneath. There's no separate rendered copy that can drift out of sync with the source.

![The Penumbra note editor](Docs/screenshots/editor.png)

## Running it

You need Rust 1.85 or newer. Slint is vendored as a submodule, so clone with `--recursive`:

```sh
git clone --recursive https://github.com/Nisoku/Penumbra.git
cd Penumbra
cargo run -p penumbra-ui
```

The `Justfile` has the usual shortcuts: `just build`, `just check`, `just test`, `just format`, `just clippy`, and `just all` to run everything.

For the browser build you'll want the Wasm target and the `candle-wasm` feature, which bundles the semantic model for the web:

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p penumbra-ui --features candle-wasm
```

After that you run `wasm-bindgen` over the output and serve `index.html` (the CI workflow in `.github/workflows/` has the exact commands). Browsers only load `.wasm` from a trusted context, so serve through a local web server instead of opening the file directly.

## Using it

You open the app to a vault folder and a map full of (one, at first) cards. Each card is a note!

To make a note, click New note in the bottom-right corner. It shows on the map as Untitled without opening itself, so double-click it to click into the editor, then click the title at the top and type what you want to call it. :D That title is also the filename in your vault folder, so renaming a note renames its file too. Any `[[wikilinks]]` that pointed at the old title are rewritten automatically when you save, isn't that cool?

To open an existing note, double-click its card on the map. Or use the Quick finder in the top bar: type a phrase, press Enter, and the camera goes "wheeee!" to the closest match and selects it. Search ranks by meaning when the semantic model is enabled/working, otherwise it falls back to matching characters like a primitive. /hj

Move a card by left-dragging it. When you let go it drifts back toward where the layout thinks it belongs, because the map sorts itself by what the notes are similar to. Pin an important note by right-dragging it; it stays exactly where you put it while the notes nearby settle around it. Right-click it again to unpin and let it drift away.

Panning is left-drag on empty space or scroll, and holding Cmd (Ctrl on Windows and Linux) while scrolling zooms in and out. Drag from one card onto another while holding Alt to link them; that appends a `[[target title]]` wikilink to the first note and pulls the two cards near each other.

Inside a note, it's plain Markdown with the markers dimmed or emphasized, depending on what "looked right" to me.

### Commands

- Cmd+B bolds
- Cmd+I italicizes
- Cmd+C/X/V copy, cut, and paste
- Cmd+A selects all
- Cmd+Z and Cmd+Shift+Z undo and redo
- Alt+Left and Alt+Right move by word
- Tab and Shift+Tab indent and outdent (real word btw)
- Esc closes the note and yeets you back out to the map

Tags live inline as `#tag`. Letters, digits, `-`, `_`, and `/` are all fine in a tag name, so `#projects/penumbra` works. Tags also show up as colored dots on the card. Anything in a `tags:` frontmatter field is merged with the inline tags, deduplicated.

A tag that is only digits is ignored!

## Embeddings

Out of the box Penumbra uses a small hash-based embedder, so it works with nothing to download and ranks on character-trigram overlap. That's fine for a quick start but it isn't semantic. The real model lives behind feature flags: `candle` pulls in the Candle embedder, `candle-load` fetches the weights at runtime on native, and `candle-wasm` does the same for the web build. With it on, search and auto-linking rank on meaning rather than matching characters.

## What's here

The engine is split into small crates under `Build/crates/` by what they own: the note model and storage, the layout solver, the embedder and vector index, search, Markdown parsing, the auto-linker that places notes near their neighbors, and the app layer that ties them together. The Slint front end is in `Build/ui/`, and the tests are in `Build/tests/`. Slint itself lives in `vendor/slint/` as a submodule.

A few things worth knowing about how it behaves. The map culls and drops detail off-screen so it stays responsive with a lot of notes. Editing a note only re-settles its local neighborhood rather than recomputing the whole layout. Cards ease into their new positions with a slight overshoot when you drag, drop, pin, or unpin them, driven by a plain timer in Rust. Layout positions are cached and saved on a throttle, so startup shows the map right away instead of solving it from scratch. The UI is only a light theme at the moment, with a dark one coming *soon*.

Plenty is still unfinished. See `TODO.md` for what's planned, including cloud sync, running inference in a web worker, and an encrypted vault mode.

## License

Penumbra is [Apache-2.0](LICENSE).

The editor bundles two fonts under the SIL Open Font License 1.1, [Inter](https://github.com/rsms/inter) and [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono); their license texts sit next to the font files in `Build/ui/penumbra-ui/assets/fonts/`.
