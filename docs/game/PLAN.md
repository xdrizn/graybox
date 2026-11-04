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
- [ ] "Selecting one selects all": not reproduced yet; needs a capture from the owner.

## Later
Third-person preset, 2D platformer and top-down presets, safe-area guides and resolution presets,
export package for engines (assets + spec sheet, 9-slice), Godot scene export, Unity plugin,
comparing a game screenshot with the design.
