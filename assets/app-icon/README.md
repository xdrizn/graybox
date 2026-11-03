# Graybox app icon

A pixel-grid **G** inside an orange selection frame with corner handles, on a black tile with a
faint grid: a design tool's selection over a game-style glyph.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Black | `#000000` | the tile |
| White at 20 % | `#ffffff` | grid lines |
| Orange | `#ff5a00` | selection frame, handles, the G (frame fill at 15 %) |
| Light grey | `#d9d9d9` | the G's bar |

## Geometry

Drawn on a 1697-unit tile with `rx=274`. `figcraft.svg` wraps it in the 512-unit
`viewBox="0 0 512 512"` that `cargo xtask icon` expects (scale 0.301709). The macOS renders pad it
onto Apple's 824/1024 icon grid; Windows and Linux use the tile edge to edge.

## Provenance

The project owner's original design (Adriel, INDIRECT), supplied as SVG and used unchanged inside
the scale wrapper. License: see `LICENSE.txt`.

## Files

- `figcraft.svg`: canonical master. `figcraft-small.svg`: the same drawing (scalable theme icon).
- `figcraft-<size>.png`, `figcraft.ico`, `hicolor/`: rendered by `cargo xtask icon` (needs `resvg`).
