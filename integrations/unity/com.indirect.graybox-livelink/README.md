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
`Assets/Graybox/<screen>/`; 9-sliced layers as sliced sprites with their borders), and texts
(TextMeshPro when the project has it, else UI Text). Objects keep a `GrayboxNode`, so an update
changes them in place and components you add stay.

Status: written against Unity 2022.3; not yet run in a Unity project.

Code: MIT OR Apache-2.0, as Graybox (C#: Unity plugins can't be written in Rust).
