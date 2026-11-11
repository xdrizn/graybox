# Unity prefab round trip (G12) — design

Owner's need (2026-11-11): his first studio contract. The studio's game is in Unity and they will
hand him the **HUD prefab** as a test. Today Graybox only goes one way (Graybox → Unity:
`game.unity.export` + the Graybox Live Link package, G9). He needs the way back:

> open their prefab in Graybox, design on it as usual, and send the changes back **into the same
> prefab** without breaking anything their programmers wired (scripts, references, events), with
> a list of what changed to hand in.

Open questions for the studio (they change scope):
1. Is the HUD **uGUI (Canvas)** or **UI Toolkit (UXML/USS)**? This plan covers uGUI. UI Toolkit
   would be another importer (G12.x).
2. Unity version, and do they use **TextMeshPro**?
3. Do they use nested prefabs or prefab variants for the HUD pieces?

## Rules
- **Confidential.** The studio's prefabs, sprites, fonts and screenshots never enter this repo:
  no fixtures, no docs images, no commit. Tests use synthetic prefabs built by our own batchmode
  scripts in a scratch Unity project.
- Unity's format and API come from Unity's public documentation. No Unity code is copied.
- Same Graybox rules: no panics, wasm keeps building (the importer is desktop-only where it needs
  files; the web build shows a note), upstream touched only at listed hooks.

## Architecture: Unity does the Unity side
Graybox never edits `.prefab` YAML by hand. The **Graybox Live Link package** (C#, already in
`integrations/unity/`) reads the prefab through Unity's own API and applies changes through it
(`PrefabUtility.LoadPrefabContents` / `SerializedObject` / `SaveAsPrefabAsset`, Undo-aware). Unity
keeps doing the hard parts: GUIDs, fileIDs, nested prefabs, variants, script references and
serialization. Graybox only reads and writes JSON and PNG files in a shared folder. The designer
has the studio's project open in Unity anyway.

```
Unity (Live Link)                    shared folder                       Graybox
"Send prefab to Graybox"  ──►  <Prefab>/source.json + sprites/  ──►  game.unity.import → linked frame
                                                                      (design, Play, iterate)
"Review and apply"        ◄──  <Prefab>/changes.json + new sprites ◄──  save (or "Send changes")
   └─ review window, conflicts, Undo        + report.md (change list for the client)
```

## G12.1 Unity → Graybox (export from Unity, import into Graybox)
- **C#: Send prefab to Graybox** (Project window context menu and the Live Link window). It walks the
  prefab's `RectTransform` tree and writes `source.json` (schema `grayboxUnity: 1`):
  - identity per object: `GlobalObjectId` string plus the hierarchy path (fallback);
  - the RectTransform: anchors min/max, pivot, anchored position, size delta, local rotation and
    scale, sibling index, active state;
  - components it understands:
    - `Image` / `RawImage`: sprite (exported as PNG through a readable copy, with its border), colour,
      type (Simple, Sliced, Tiled, Filled + fill method/amount), preserve aspect;
    - `TextMeshProUGUI` / `Text`: text, font asset name, size, colour, alignment, wrapping, auto size,
      character and line spacing;
    - `CanvasGroup` alpha;
    - `HorizontalLayoutGroup`, `VerticalLayoutGroup`, `GridLayoutGroup`: padding, spacing, child
      alignment, control/expand flags;
    - `ContentSizeFitter`, `LayoutElement`;
    - `Mask`, `RectMask2D`;
    - the `Canvas` / `CanvasScaler` reference resolution and match;
  - **other components** (their scripts): type name and the fields that reference other objects,
    listed read-only so the designer sees "HealthBar (script) → fill: Fill".
- **Rust: `game.unity.import`** (`dir`): one top-level frame per prefab, at the reference resolution:
  - each object becomes a layer;
  - images become image fills, with Sliced as 9-slice;
  - texts become text, with a font mapped to an installed family or kept by name;
  - layout groups become auto layout, and masks become clip content;
  - **anchors become constraints**: the inverse of G9's export math. Anchor min = max on an axis
    means a fixed side or centre; min < max means stretch. Exact round trip, tested both ways.
  - The frame keeps the link: the prefab path, each layer's Unity identity (plugin data), and a
    **baseline** (the imported values per layer) so later changes are a diff against it.
  - Unknown components show in a read-only "Unity" section on the layer.
- **Tests**: import → export the same frame → `changes.json` is empty. Anchors, pivots, rotation,
  9-slice, layout groups and text alignment survive the trip. Synthetic prefabs only.

## G12.2 Graybox → changes
- On save (if the frame is linked) and with a "Send changes to Unity" button, write
  `changes.json`. It holds only what differs from the baseline, by Unity identity:
  - moved, resized or re-anchored;
  - colour, sprite (new PNG with its 9-slice border), text and text style;
  - order, active state;
  - **new layers**, with parent identity, as new GameObjects with Image/TMP;
  - **removed layers**, proposed as *deactivate*, never delete by default.
- `report.md`: a readable change list for the client, e.g. "Health bar: moved 12 px right; new
  sprite `hp_fill.png`; text size 18 → 20". English by default, Portuguese as an option.
- A layer created in Graybox and placed under a scripted object is flagged ("this object has a
  script; check with the programmer").

## G12.3 Apply in Unity
- **C#: Review and apply** window. It lists every change with a checkbox, grouped by object, with
  before → after. "Apply selected" edits the prefab through Unity's API (Undo-recorded), imports
  new sprites into a chosen folder with their borders (Sprite mode, Point filter if "Pixel art"),
  and saves the prefab. Nothing outside the listed properties is touched.
- **Conflicts**: if the prefab changed in Unity since it was sent (a programmer edited it), each
  changed property is compared with the baseline. A property changed on both sides is shown as a
  conflict, and the designer picks one; it is never silently overwritten.
- Tests (batchmode, scratch project): send → change one property in Graybox → apply → the
  prefab's text diff touches only that property's lines; references and scripts are intact;
  conflicts are detected.

## G12.4 Resolution check (smaller tool)
A Graybox view of a linked HUD side by side at 16:9, 21:9, 16:10, Steam Deck (16:10, 1280×800)
and 4:3, with safe-area guides, using the same anchor rules Unity uses. It catches anchoring
mistakes before the client does.

## G12.5 Compare with the game (smaller tool)
Paste a screenshot of the running game. Graybox overlays it on the linked frame (opacity, difference
blend) and points at layers off by more than N px, colours off by more than a threshold, and font
size differences.

## How to build it
- **G12.1 and G12.3 with Fable**: the hard reasoning is there (Unity's object model, anchor math
  both ways, a safe apply, conflicts).
- **G12.2, G12.4 and G12.5 with Sonnet sessions**, once the format is set.
- The `unity-livelink` agent (`.claude/agents/unity-livelink.md`) knows the package and how to
  compile and run it in batchmode (Unity 2022.3.39f1 is installed and licensed here).
