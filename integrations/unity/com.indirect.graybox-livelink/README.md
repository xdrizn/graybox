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

The `.meta` files are committed on purpose: scenes refer to `GrayboxNode` by its script's GUID.

Code: MIT OR Apache-2.0, as Graybox (C#: Unity plugins can't be written in Rust).
