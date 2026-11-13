# Graybox Live Link (Unity)

Builds the screens Graybox exports as uGUI under a Canvas, and rebuilds them each time Graybox
exports again.

1. In Graybox: select a screen (top-level frame), Prototype tab → **Export to Unity**. With nothing
   selected, Prototype tab → **Unity export** → **Choose folder…** (outside the Unity project).
   Each save of the file exports the marked screens again (or press **Export now**).
2. In Unity (2022.3+): Window → Package Manager → + → **Add package from disk…** → this folder's
   `package.json`.
3. Tools → **Graybox Live Link**: pick the same folder, turn on **Watch**, press **Import now**.

What comes over: each layer's box and its constraints as RectTransform anchors, sprites (copied to
`Assets/Graybox/<screen>/`; 9-sliced layers as sliced sprites with their borders), a layer's opacity
(as a `CanvasGroup`, so the layers inside it inherit it), and texts (TextMeshPro when the project has
it, else UI Text) with their size, colour and alignment. Auto-width texts stay on one line; the others
wrap. Unity does not have Graybox's fonts: texts use TextMeshPro's default font (or Unity's built-in
one), so letters are a little wider or narrower than in Graybox. Change the font on the text object;
it stays on the next import. With TextMeshPro, import its essentials once per project
(Window → TextMeshPro → Import TMP Essential Resources), or texts have no font.

**3D tilt:** a layer tilted in Graybox carries an optional `"rotation3d": {"x": deg, "y": deg,
"perspective": n}` in `layout.json` (in `nodes`, and in `root`). Live Link sets the RectTransform's
`localEulerAngles` to `(x, y, 0)` (Unity's `Quaternion.Euler` convention: +X tips the top away, +Y
brings the right side nearer) about the rect's centre; the box (`x, y, width, height`) and the sprite
stay flat. Layers without it are untouched. Note: perspective only shows with a Canvas in Screen Space -
Camera or World Space; in Screen Space - Overlay the tilt shows without perspective. Live Link does
not change the Canvas mode; `perspective` is informational.

**Pixel art:** layers drawn in Graybox's Pixel mode always import sharp (Point filtering, no
compression, no mipmaps), whatever the window says. To get the same for every sprite, turn on "Pixel
art (no smoothing)" in the window (or set `GrayboxLiveLink.PixelArt`) and import again.

**Animated pixel layers** (more than one frame) come with their sprite sheet (`sprites/<id>_anim.png`
and `.json`). Live Link copies the PNG to `Assets/Graybox/<screen>/`, cuts it into one sprite per
picture (frames that look the same share a sprite), makes one `AnimationClip` per tag (one, `all`,
when there are none) that swaps the `Image`'s sprite, and gives the layer an `Animator` whose first
state plays the sprite's "play" tag. The `Image` keeps the still sprite while the game is not
running. Clips and `<id>_anim.controller` sit next to the PNG, and are updated in place on the next
import (a tag deleted in Graybox loses its clip and state; states you add stay).

## Sprite sheets dropped in Assets

Graybox's pixel export in the **unity** format (`pixel.export` with `format: "unity"`) writes
`<name>.png` and `<name>.json`. Put both anywhere in `Assets` (the export folder can be inside the
project): when a PNG has a Graybox JSON of the same name next to it, Unity imports it as a Sprite
(Multiple), Point filtering, no compression, no mipmaps, the JSON's pixels per unit, one sprite per
picture named `<name>_<frame number>` with its pivot and 9-slice border, and makes
`<name>_<tag>.anim` clips (animating a `SpriteRenderer`) and `<name>.controller` beside it. Export again
over the same files and the sprites, clips and controller update in place. Delete the JSON to set the
PNG up by hand. A JSON that is not Graybox's is left alone; a broken Graybox one logs a warning.

Clip details: frames play in the tag's direction (forward, reverse, ping-pong without repeating its
ends: 0, 1, 2, 1) for the time each frame lasts. A tag that repeats forever loops; one with a finite
repeat count plays once (Unity clips cannot count repeats). Each clip has one extra key at its end so
the last picture lasts as long as it should.

Objects keep a `GrayboxNode`, so an update changes them in place and components you add stay.
Layers deleted in Graybox are deleted in the scene; a layer that stops painting loses its Image.

## From the command line

`Unity -batchmode -projectPath <project> -executeMethod Indirect.Graybox.Editor.GrayboxLiveLink.ImportBatch -grayboxFolder <export folder> [-grayboxScene Assets/Screen.unity]`
imports every screen in the export folder into the scene (opened and saved when `-grayboxScene` is
given) and quits.

## Status

Run in Unity 2022.3.39f1 (Personal licence): a test screen (9-sliced panel, gradient shape, texts
aligned left, centre, right and middle, constraints left/right/centre/stretch/scale, nested frames,
translucent layer) imports with the layers, anchors, sprite borders, text and colours of its
`layout.json`, and the Canvas renders like Graybox's own render of the screen, also when the screen is
made larger. A second export updates the scene in place. Both text paths (TextMeshPro and UI Text) are
checked. Watch was checked headless (the window opens, a rewritten `layout.json` is imported).
Not checked by hand: clicking the window's buttons.

Pixel sheets (batch mode, 2022.3.39f1, UI Text path): a sheet in `Assets` gets the right sprite count
(one per unique rectangle), Point, uncompressed, pixels per unit, flipped rectangles, pivots, borders,
one clip per tag with the right keys (ping-pong 3 frames: 0, 1, 2, 1) and a controller playing the
`play` tag; editing the JSON updates them. A Live Link screen with an animated layer gets its
`Animator`, and in Play mode the `Image` swaps sprites on time (0.1 s, 0.3 s, 0.4 s, 0.6 s for
durations 100, 200, 100, 200 ms ping-pong). Not run: the TextMeshPro path of this feature (it does not
touch text; the code compiles with and without `GRAYBOX_TMP`), and Unity 6.

Unity 6 (6000.x): prepared, not run. The Canvas lookup uses `FindFirstObjectByType` there, and
`GRAYBOX_TMP` is also set for uGUI 2.0 (where TextMeshPro lives); the C# compiles in 2022.3 with
both text paths and with the Unity 2023+ branch. Text wrapping still uses TextMeshPro's
`enableWordWrapping`, which newer TextMeshPro marks obsolete (a warning, or a compile error if a
Unity 6 version made it one: then switch to `textWrappingMode`). Needs one run in Unity 6.3.

The `.meta` files are committed on purpose: scenes refer to `GrayboxNode` by its script's GUID.

Code: MIT OR Apache-2.0, as Graybox (C#: Unity plugins can't be written in Rust).
