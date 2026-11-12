# Game layer — plan

What this fork adds to FigCraft: design game UI with everything FigCraft already offers (frames,
components, variants, auto layout, variables, prototype interactions), then press **Play** and play
that UI on top of a generic, low-poly game scene, with keyboard, mouse and controller, sound and
animations.

The owner is a game UI designer. Every feature must be **simple to use from the editor**, with good
defaults, and work the same on desktop and in the browser build.

## What already exists upstream (reuse, don't rebuild)
- Prototype player (`crates/ui-egui/src/present*`): all triggers incl. key/gamepad (17 standard
  buttons), all actions (navigate, overlays, change to variant, set variable, conditional), all
  transitions and easings, smart animate, interactive components.
- Variables and modes; components and variants; auto layout.
- Controller reading on Windows and in the browser (`services.rs: gamepad_held`).
- Auto-save of the file (3 s after a change, when on) and version-history checkpoints (30 min).

## Milestones

### G0 — Setup
- [x] `crates/game` (pure simulation, L2) and `crates/ui-egui/src/game/` skeleton; registered in
  `cargo xtask layers`; native and wasm checks pass (pre-existing test failures: see STATUS.md).
- [x] `docs/game/STATUS.md` kept up to date, with the list of upstream hook points.
- [~] Auto-save: on by default (done); checkpoint and recovery copy every **2 minutes** (not yet) (also for files
  not saved yet).

### G1 — Play mode and FPS test scene
Visual and gameplay spec: [`LOWPOLY-SCENE.md`](LOWPOLY-SCENE.md) (read its adaptation note first:
plain wgpu in egui, no Bevy).
- [x] **Play** button in the toolbar (and shortcut) opens the current frame in Play mode:
  **fullscreen**, the low-poly 3D scene drawn underneath, the frame's UI on top (frame background
  transparent). **F10** leaves Play mode (in the browser, Esc also leaves fullscreen: browser rule).
- [x] Scene: the arena of `LOWPOLY-SCENE.md` §3 (pillars, stairs, ramp, crates, portal), with its
  look (§1–§2: greys, orange only emissive, flat shading, 16² nearest textures, exp² fog, one
  shadowed key light, bloom on orange). The whole look comes from one theme struct, easy to change.
- [x] Sentinel enemies (§5) as targets that take damage and shoot back.
- [x] Rendering through egui-wgpu paint callback (WebGPU/WebGL2 on web). Keep it light: 60 fps on a
  laptop iGPU in the browser.
- [x] FPS controller: walk, run, jump, crouch, mouse/stick look (pointer locked), the low-poly pistol
  of §4 (pose, kick, reload dip, bob, muzzle flash) with shoot, reload, ammo; take damage, deal damage, heal, die and respawn; status effects as
  states (e.g. on fire), no VFX needed.

- [x] Editor branding: map [`tokens.json`](tokens.json) (black/orange, dark by default, light
  variant) onto FigCraft's `theme::Tokens`; orange only for selection, focus, primary button,
  active toggles. Space Grotesk and JetBrains Mono (OFL) loaded like FigCraft's other fonts, not
  committed as files. Keep this to a small, listed hook in the upstream theme.

### G2 — Input
- [x] Default bindings ready out of the box: keyboard + mouse and controller (Xbox and PlayStation
  layouts named correctly).
- [x] Rebinding panel; bindings saved in the `.figcraft` file.
- [~] Controllers on macOS and Linux too (gilrs code in, never run on Linux/macOS) (e.g. `gilrs`), keeping Windows and web working.

### G3 — Game ↔ prototype bridge
- [x] Game state published every frame as prototype **variables** in a "Game" collection, default
  names from `LOWPOLY-SCENE.md` §7: `vida, vidaMax, municao, reserva, atacante, ultimoDano, arma`,
  plus crouching, on fire…
  Bind them to text, fills, widths like any variable.
- [x] New trigger **Game event**: health changed, took damage, died, respawned, shot, reloaded,
  ammo empty, healed, status started/ended… usable on frames, layers, instances and variants.
- [x] Actions that reach the game: pause/resume, release/capture cursor.
- [x] Default event names, renamable.

### G4 — Game screens made simple
**No fixed screen types.** Any frame is a game screen; what it does is plain prototype
interactions in the Prototype tab (owner's decision, 2026-11-03). HUD, Inventory, Pause, Map and
Death are only *examples* in the starter kit, never required.
- [x] No screen roles in the file: opening a screen is a normal interaction (key/gamepad or game
  event trigger → Open overlay / Navigate), with its own transition, position and sound.
- [x] Pause game / cursor actions run together with Open overlay are undone when that overlay
  closes, however it closes (key, Esc, click outside, Close overlay).
- [x] Game event trigger picked from a list of the game's events (no typing).
- [x] File-level game settings (controls, sounds, share) live in the Prototype tab when nothing is
  selected; no separate floating window.
- [x] **Navigable menus**: switch on "Navigable menu" for any auto layout frame (list, row, grid);
  in Play, focus moves with mouse, keyboard and controller, the `State` variant shows focus, Confirm
  runs the item's interactions, Back closes the overlay. Spec:
  [`specs/2026-11-03-menu-navigation-design.md`](specs/2026-11-03-menu-navigation-design.md)
  (phase 1; nested tab menus are phase 2).
- [x] Starter kit file: INDIRECT-style wireframe components (rectangles with white border and white
  inside separated by transparent padding, colours editable) for HUD bars, lists, grids, tabs,
  buttons. Its example screens use ordinary interactions only.

### G5 — Sound
- [x] Sound library per file: import WAV/OGG/MP3 into the `.figcraft` file.
- [x] **Play sound** action on any trigger (click, hover, key/gamepad, game event, after delay),
  including interactions inside component variants (hover sound on a button variant, etc.).
- [x] **Master volume** and **per-sound volume** (plus optional groups: UI, SFX, music), adjustable
  in the editor and in Play mode.
- [x] Desktop and web (web audio starts on the Play click). Pure-Rust audio crate (rodio + symphonia).
  Desktop checked in the app; the web build compiles (sound not yet heard in a browser).

### G6 — Ready-made animations
- [x] Library of preset animations: fade in/out, slide in/out (4 directions), scale pop, pulse,
  shake, bounce, blink, with duration and easing (reuse upstream easings).
- [x] **Play animation** action on any trigger, applicable to any layer, instance or variant.
- [x] Components with variants accept animations, sounds and game events in their interactions,
  so a button or a health bar component carries its own behaviour everywhere it is used.

### G7 — Share
- [ ] Share a prototype as a link (web build + upstream server) that opens straight in Play mode.

### G8 — Figma-like editing and screen tools (owner's list, 2026-11-04)
- [x] Rename fields edit on the same line as the name (file, frame titles).
- [x] Component panel like Figma: compact Properties for a set, Current variant for a variant.
- [x] Prototype connections between variants of a set (Change to) from the canvas.
- [x] Only the selected screen's connections on the canvas.
- [x] Game behind each screen: running / paused / none.
- [x] Game bars: a layer's width follows health or ammo as a percentage of its drawn size.
- [x] Maps: pan and zoom with mouse and controller.
- [x] Same UI font on every PC: Space Grotesk and JetBrains Mono (OFL) embedded from the
  craft-fonts checkout beside the repo; setup in [`FONTS.md`](FONTS.md).
- [x] Menu navigation and game settings sections padded like the rest of the panel.
- [x] Named game inputs (T for a watch, E to pick up by default) as Game event triggers.
- [x] Hotbars: keys 1-9, wheel and LB/RB select a slot.
- [x] Wireframe hotbars (experimental): plain frames, no components; the first slot is drawn as
  the selected look and moves to the picked slot (size, scale, paints, corners, opacity).
- [x] Play fills any monitor: the frame takes the monitor's shape at its designed scale and the
  layers' constraints anchor them (corners, centre, stretched edges).
- [ ] "Selecting one selects all": not reproduced yet; needs a capture from the owner.

### G9 — Editing polish (owner's list, 2026-11-06)
- [ ] Auto layout padding typed as 0 applies at once (not reproduced yet; needs the owner's steps).
- [x] A click anywhere on a frame that shows its hover outline selects it.
- [x] Selection colors: near-equal colours are one row; folds past three colours (Figma-like).
- [x] Auto layout eases its other layers in and out when a layer is dropped in or taken out.
- [x] No light line around frames covered by a screenshot or background.
- [x] Alt and a drag anywhere on a number field changes its value (normal rate; Shift ×10).
- [x] Fonts installed while the app runs are found when its window comes back to the front.
- [x] No FigCraft update prompt or "Check for updates" (Graybox ships its own builds).
- [x] A boolean (union, subtract, intersect, exclude) has its result's box, not all its shapes'.
- [x] No padding handles on the canvas (they took clicks near frame edges).
- [x] Delete leaves Play, like F10.
- [x] Layers anchor by where they sit in their frame (9-slice thirds, stretch across) until set by
  hand; Figma-like Constraints box; "Auto (by position)"; "Anchor layers by position" on a frame.
- [x] CRT effect in Effects: a layer over the screen gives what lies behind it curvature, vignette,
  scanlines (size, thickness, opacity, bleed, direction), RGB split, phosphor, glow and brightness;
  static. Spec: [`specs/2026-11-06-crt-effect-design.md`](specs/2026-11-06-crt-effect-design.md).
- [x] Corner styles: the corner radius draws Round, Chamfer, Scoop, Notch or V-notch corners
  (per corner sizes, strokes, clipping, hit testing, SVG/PDF); a Corner style picker in Design.
- [x] Shift-click adds the layer at the selected layer's depth (frames in different auto layouts).
- [x] Frame selection on one rectangle turns it into the frame.
- [x] Right-click an image in a frame: Set as frame background.
- [x] Ctrl-drag crops images inside auto layout too.
- [x] A fill or stroke with a colour style shows only the style (swatch, name, detach).
- [x] Gradient Rotate 90° turns about the layer's middle.
- [x] FigCraft v0.17 merged (Developer Mode, pick select, Alt-scrub on number fields, web fonts...).
- [x] A whole ellipse edits with four points.
- [x] Nested pages: the Pages list is the Docs tab's tree (chevron, indent, drag in or before/after, row
  menu with Add subpage and Move out); the expanded state is saved in the file; the tree order is the page
  order everywhere.
- [x] The Pen shows no selection box while drawing; a click's wobble (8 px) makes no curve.
- [x] Join selection welds the meeting ends of several layers (an arrow of three lines).
- [x] Cmd/Ctrl-drag from a frame's background selects the layers inside it.
- [x] An upright line shows its length as H.
- [x] No Game settings button beside Play; game sections in the Prototype tab use the panel's headers.
- [ ] Line resized with Shift "behaves strangely": not reproduced (dragging an end with Shift is fine).
- [x] 9-slice: a section under Layout, cuts dragged on the canvas, corners kept while the middle stretches.
- [x] Pseudo-localization preview (Prototype tab switch, canvas only).
- [x] Unity export (layout.json + sprites per screen, again on save) and the Graybox Live Link
  package (run in Unity 2022.3.39 with a test screen: the Canvas matches Graybox's render).

### G10 — Claude inside Graybox (owner's request, 2026-11-10)
Spec: [`specs/2026-11-10-claude-chat-design.md`](specs/2026-11-10-claude-chat-design.md).
- [x] G10.1 Chat: a button beside Play opens a Claude panel; Claude edits the open file through the
  installed Claude Code CLI (his own plan, no API key) and the `graybox` MCP tools.
- [ ] G10.2 Icons from sketches: references, a brush sketch, and Claude turns it into a clean icon frame.

### G11 — More games under the UI (owner's request, 2026-11-11)
Spec: [`specs/2026-11-11-third-person-design.md`](specs/2026-11-11-third-person-design.md). Each file
chooses the game Play runs (new-file card and the Prototype tab); the first new game is a third-person
action game, original, made in code. One session per phase, each ends committed and checked in the app.
- [x] G11.1 Game kind plumbing: `GameKind`, `game.kind.set`, the new-file choice, the Game dropdown, Play
  dispatch, a placeholder third-person scene (flat ground, a capsule hero, an orbit camera).
- [x] G11.2 World: terrain, grass, props, lights, river, waterfall, sky, climbable tags, budgets test
  (parallel with G11.3).
- [x] G11.3 Hero and animation: rig, meshes, clips and crossfades, test pose viewer (parallel with G11.2).
- [x] G11.4 Movement and camera: controller (walk to climb), orbit camera with collision, bindings,
  interpolation. Needs G11.2 and G11.3. (Camera easing, bow over-the-shoulder and crowns in the camera's
  way moved to G11.6.)
- [ ] G11.5 Combat and the bridge: weapons, bow, dummies and husks, damage, game variables and events,
  third-person starter HUD. Needs G11.4. (Weapon switching and every strike done with G11.4; targets,
  damage, arrows, variables, events and the HUD to do.)
- [ ] G11.6 Polish and performance: browser at 60 fps, memory and size budgets, feel tuning, installed copy.

### G13 — Documents (owner's request, 2026-11-12)
Spec: [`specs/2026-11-12-documents-design.md`](specs/2026-11-12-documents-design.md) (decisions P1–P10).
Notion-like documents inside the project file, a Documents tab, versions closed by signature.
- [x] G13.1 Model, commands and the Documents tab (tree, open a document).
- [ ] G13.2 Block editor: left handle (turn into anything), `/` menu, formatting, embeds, tables.
- [ ] G13.3 Page layout, canvas card that follows the document, PDF and Markdown export.
- [ ] G13.4 Close version (fingerprint, read-only), new version, diff, decision block and log.
- [ ] G13.5 Templates, Claude reads and writes documents, auto-tables, per-section approval.

### G14 — Graybox online (owner's request, 2026-11-12)
- [ ] G14.1 Client links on our own `figcraft-server`: view and play, comment, read and download
  documents; password, expiry, revoke (tested on localhost).
- [ ] G14.2 Deploy on the owner's server and domain (waiting for his choice, P6).

### G15 — Signing through a link (owner's request, 2026-11-12)
- [ ] G15.1 Signers, email code, drawn or typed signature, audit trail signed by the server,
  certificate page, Verify. Needs G13.4 and G14.1.

### G16 — Pixel art (owner's request, 2026-11-12)
Spec: [`specs/2026-11-12-pixel-art-design.md`](specs/2026-11-12-pixel-art-design.md) (decisions D1–D10).
Pixel layers edited in place in a Pixel mode, Aseprite-style standard tools, smart resize with pixel
9-slice, pixel buttons that grow with their text, basic animation, Unity-first export.
- [x] G16.1 Core and drawing: `crates/pixel`, pixel layer, sharp render, Pixel mode, pencil to ellipse,
  brush, mirror with an axis, grid, undo.
- [x] G16.2 Selection, transform and colour: marquee, lasso, wand, move, outline, replace colour,
  palette tied to styles and variables, Selection colours, sprite layers.
- [ ] G16.3 Smart resize, pixel 9-slice, insets as padding, Pixel text, Pixel button starter, tile mode,
  components and palette-swap variants.
- [ ] G16.4 Animation: timeline, durations, onion skin, tags, playback in Prototype and Play.
- [ ] G16.5 Export and import: exporter trait, PNG, sheet and JSON, Unity with Live Link, PNG and sheet import.
- [ ] G16.6 Polish: pixel fonts, shade, Claude MCP tools, performance, guide, installed copy.

## Later
Third-person preset (now G11), 2D platformer and top-down presets, safe-area guides and resolution presets,
export package for engines (assets + spec sheet, 9-slice), Godot scene export, Unity plugin,
comparing a game screenshot with the design.
