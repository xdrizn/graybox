# Graybox UI fonts

The editor's UI uses **Space Grotesk** (text) and **JetBrains Mono** (numbers, code), both
SIL Open Font License 1.1. Font files never go in this repository (project rule): builds embed them
from a `craft-fonts` checkout beside the repo (`.cargo/config.toml` → `CRAFT_FONTS_DIR =
../craft-fonts`). Without it the app still runs, with whatever fonts the computer has, so the UI
looks different from one PC to another.

## Set up once per computer that builds Graybox

1. Next to this repo, clone the shared font set:
   `git clone --depth 1 https://github.com/storytold/craft-fonts.git ../craft-fonts`
2. Add the two Graybox faces (local addition to that checkout, not upstream):
   - `fonts/space-grotesk/`: `SpaceGrotesk-Regular.ttf`, `SpaceGrotesk-Medium.ttf` and `OFL.txt`
     from <https://github.com/floriankarsten/space-grotesk> (`fonts/ttf/static/`).
   - `fonts/jetbrains-mono/`: `JetBrainsMono-Regular.ttf` and `OFL.txt` from
     <https://github.com/JetBrains/JetBrainsMono> (`fonts/ttf/`).
3. Append one line per file to `../craft-fonts/fonts/latin-manifest.txt`
   (`family | style | file | Latn | OFL-1.1 | licence file | sha256 | source`), e.g.
   `Space Grotesk | Regular | fonts/space-grotesk/SpaceGrotesk-Regular.ttf | Latn | OFL-1.1 | fonts/space-grotesk/OFL.txt | <sha256> | <source>`.
4. Rebuild the text crate once so it notices the new folder: `cargo clean -p figcraft-text` (add
   `--release` for release builds), then build as usual.

A built `graybox.exe` carries the fonts inside it: people who only run the program need nothing.
