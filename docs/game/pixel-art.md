# Pixel art in Graybox

## Resumo (pt-BR)

Graybox desenha pixel art direto no canvas, para UI de jogo:

- **Camada de pixel art**: um sprite (largura × altura em pixels de arte, cada um com `escala`
  unidades). Fica nítido em qualquer zoom, no canvas, no Prototype, no Play e nos exports.
- **Modo Pixel** (botão "Pixel" no seletor de modos, duplo clique na camada ou "Edit pixels"):
  lápis, borracha, balde, conta-gotas, linha, retângulo, elipse, seleções, mover, espelho, modo
  tile, grade, sombra (Shade) e duas cores (botão direito usa a segunda).
- **Cores do Graybox**: as duas cores abrem o seletor de cor padrão, com os estilos de cor e as
  variáveis do arquivo; a cor lembra de que estilo veio, e se o estilo mudar o painel oferece
  "Update from styles". Paletas `.gpl`/`.hex` viram estilos de cor (Palette > Import…).
- **UI de jogo**: 9-slice em pixels de arte, camada que cresce sem esticar a arte, Pixel text
  (texto nítido na grade), o componente pronto "Pixel button" (Default/Hover/Pressed) e troca de
  paleta como variante.
- **Animação**: timeline (botão na barra ou Alt+T) com frames, duração, tags (idle, hover…), onion skin; a tag toca no
  Prototype e no Play.
- **Export/Import**: PNG, PNG por frame, sprite sheet + JSON, GIF e Unity (com o Live Link criando
  sprites, clips e Animator). Importa PNG ou sheet cortada em frames.
- **Fontes pixel**: desenhe as letras numa "folha de glifos" e use como fonte em qualquer texto.
- **Claude**: peça no chat "desenha um ícone 16×16 de moeda" — ele usa `pixel.paint`.
- **Quadro (board)**: no modo Pixel o W/H do painel é o tamanho do quadro em pixels de arte;
  digite ou arraste as bordas para crescer ou cortar (Alt: pelo centro). Fora do modo Pixel a camada
  abraça só os pixels desenhados, e W/H são de camada normal.
- **9-slice com componentes**: aplique no componente principal ou na camada de pixel dentro dele;
  as instâncias redimensionam junto, com os cantos intactos.

The rest of this guide is in English, like the other docs in the repository.

## Pixel layers

A pixel layer is an ordinary layer (it can sit in frames, auto layout, components) whose picture is
a sprite: a grid of art pixels with its own layers, frames, tags and palette. Each art pixel is drawn
`scale` page units big, with no smoothing at any zoom.

- Make one: Pixel mode with nothing selected (drag a box, or Create with the size in the pill), or
  the "Pixel button" and "Pixel font" starters in the same pill.
- The **Pixel art** section of the Design panel: scale, size, what plays, how it grows when resized
  (Middle, Edges, Tile; with 9-slice: Repeat or Stretch, Cuts as padding), Pixel font, Export and
  Import. While a sprite is edited it also lists the sprite's layers and Palette (Import… makes
  colour styles from a `.gpl` or `.hex` file; Save… writes the sprite's colours as `.gpl`). The
  section comes last, after Effects, and folds away with its arrow.
- **The board and the layer.** In Pixel mode the layer is a board: the Design panel's W and H are
  its size in art pixels. Type a size, or drag the board's sides or corners, to add or cut art
  pixels on that side (Alt: on both sides, from the centre); the art stays where it is and the scale
  stays. Outside Pixel mode the layer hugs the pixels it uses (all frames, visible layers): its box
  is the box round them, where they are on the page, so leaving Pixel mode never moves the art. An
  empty board keeps the board's size. In Design its W and H are a layer's, and resizing it grows
  the trimmed art the way the section says (9-slice, Edges, Tile, Middle). The Pixel art section
  shows the board's size ("Board: 32 × 32 art pixels").
- The layer's image fill is the frame at rest, so files opened elsewhere still show the art.

## Pixel mode

Enter with the mode switcher's **Pixel**, a double-click on a pixel layer, or **Edit pixels**. Leave
with **Done**, `Esc` or `Enter`.

| Tool | Key | Notes |
|---|---|---|
| Pencil | `B` | Size `[` `]`, square or round, Pixel-perfect lines, **Shade** |
| Eraser | `E` | |
| Bucket | `G` | Contiguous or the whole layer, tolerance |
| Eyedropper | `I` | Or hold `Alt` with any tool |
| Replace colour | `R` | Paints only over the second colour |
| Line / Rectangle / Ellipse | `L` / `U` / `Shift+U` | Filled option, `Shift` for square or circle |
| Marquee / Lasso / Magic wand | `M` / `Q` / `W` | `Shift` adds, `Alt` takes away, both keep the overlap |
| Move | `V` | Arrows move the selected pixels (`Shift`: 10) |

- Two colours: the main button draws the first, the right button the second; `X` swaps them. A
  click on either opens Graybox's colour picker: a custom colour, the file's colour styles (by
  folder), colour variables and the colours in this sprite. A colour taken from a style or variable
  keeps that link in the sprite, so when the style changes **Update from styles** (Pixel colors)
  repaints those pixels, and Replace colour moves the link to the new colour.
- Mirror left/right and up/down, with pink handles past the top and left sides to drag each axis
  (the pointer turns into a resize arrow over them); with both on, the ring where they cross drags
  both at once. Tile mode wraps drawing round the edges and shows the sprite repeated around
  itself; the pixel grid shows when zoomed in.
- Shift-click draws a line from the last point. `Ctrl+A`, `Ctrl+D`, `Ctrl+Shift+I` select all,
  none, invert; `Ctrl+C`/`X`/`V` copy, cut and paste pixels; `Delete` clears.
- **Shade**: with the pencil's Shade option on, each pixel you pass over moves one colour along a
  ramp (main button: the next colour, right button: the one before), once per stroke. The ramp is
  the colour style folder the first colour came from (e.g. `Skin/1`…`Skin/4`), dark to light; with
  no such folder, the sprite's palette or its own colours from dark to light.
- The pill fits narrow windows by leaving out the zoom label and folding the mirror buttons into one.

## Game UI

- **9-slice** on a pixel layer cuts on whole art pixels; the corners stay and the edges and middle
  repeat (or stretch) by whole pixels, so a button frame grows without blurring. On a frame with
  auto layout the cuts can be its padding. The cuts are measured on the trimmed art. The 9-slice
  section sits just above Export.
- **9-slice with components**: set it on the main component, on the pixel layer inside it, or on the
  pixel layer before making it a component: instances resized keep the corners, and the section says
  so ("Applies to this layer; instances of the component resize with it").
- **Pixel text**: any text layer, drawn sharp on the art-pixel grid of the pixel art around it.
- **Pixel button**: a component set with Default, Hover and Pressed states that grows with its label.
- **Palette swap**: in Pixel colors, pick a colour, then **As variant** makes a new variant
  ("Palette=Red") with the colours swapped.

## Animation

In Pixel mode the timeline sits over the pill once turned on (the pill's timeline button, `Alt+T`,
or "Show pixel timeline" in Actions; Graybox remembers it; hidden, you draw on the frame shown): frames across (number and duration), the sprite's
layers down. Add, duplicate, delete and drag frames; `,` and `.` step; Play loops the current tag.
**New tag** names a run of frames (direction Forward, Reverse or Ping-pong, Repeat). The tag that
**plays** (panel: Plays) runs in Prototype and Play; a hover or pressed variant that plays another
tag switches animation with the prototype's own triggers. Onion skin shows the frame before (red) and
after (blue).

## Export and import

Pixel art panel → **Export pixel art**: a preset (Unity sheet, Sprite sheet + JSON, PNG, PNG per
frame, GIF), scale, padding, which frames (all or one tag), Packed / Grid / Row, Trim. Sheets come
with a JSON (frames, durations, tags, 9-slice borders, pivot). **Import pixel art** makes a new pixel
layer from a PNG, cut into frames by the frame size given.

Unity: the screen export (`game.unity.export`) marks pixel layers so they import sharp, and writes an
animated layer as a sheet; the Live Link package slices sheets, sets borders and pivots, and makes a
clip per tag and an Animator.

## Pixel fonts

A pixel layer can be a font: a **glyph sheet**, a grid of cells with one character drawn in each.

1. Pixel mode with nothing selected → **Pixel font** (or Pixel art panel → Pixel font `+` on any
   pixel layer). A new sheet has 6 × 8 cells for printable ASCII, a faint cell grid and baseline,
   and each cell's character in its corner while you draw.
2. Draw the letters on the first frame. Pixels at least half opaque are the glyph. Each glyph is as
   wide as its drawn columns plus the gap (1 pixel); an empty cell is a space.
3. Set any text in it: select a text layer → Pixel text → **Font**. One glyph pixel becomes one art
   pixel at the text's pixel scale. Text in a pixel font lays out, wraps and hugs like any font.
4. Drawing on the sheet updates every text that uses it. The Pixel font section sets the font's name,
   cell size, baseline, gap and the characters (cell by cell).

Names stay unique: a second font with the same name, or a name of a font installed on the computer,
gets a number. A copied sheet keeps the name but isn't the font until it's renamed.

## Claude

Claude reaches every pixel command through the `graybox` MCP tools (`run_command`). The useful ones:

- `pixel.create {width, height, scale, x, y, name?}` → `{id}`.
- `pixel.paint {id, rows, palette, x?, y?, clear?}`: a whole sprite in one step, one character a
  pixel, e.g. `{"rows": ["..kk..", ".kffk."], "palette": {"k": "#1b1b2a", "f": "#e8b830"}}`.
- `pixel.get {id, pixels: true}` reads the pixels back.
- `pixel.stroke`, `pixel.shape`, `pixel.fill`, `pixel.replaceColor`, `pixel.outline`,
  `pixel.frame.add`, `pixel.tag.add`, `pixel.font.sheet`, `pixel.font.set`, `pixel.text`,
  `pixel.export`, `pixel.import`. `list_commands` with `filter: "pixel"` lists them all with their
  parameters.

## Limits

1024 art pixels a side, 64 sprite layers, 1024 frames, 256 tags and palette colours; exports up to
8192 pixels a side. A pixel font is read from the sheet's first frame, up to 1024 characters and
128-pixel cells. Not done yet: Aseprite file import, a bitmap font export for Unity.
