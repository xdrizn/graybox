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
- [ ] Auto-save: on by default; checkpoint and recovery copy every **2 minutes** (also for files
  not saved yet).

### G1 — Play mode and FPS test scene
Visual and gameplay spec: [`LOWPOLY-SCENE.md`](LOWPOLY-SCENE.md) (read its adaptation note first:
plain wgpu in egui, no Bevy).
- [ ] **Play** button in the toolbar (and shortcut) opens the current frame in Play mode:
  **fullscreen**, the low-poly 3D scene drawn underneath, the frame's UI on top (frame background
  transparent). **F10** leaves Play mode (in the browser, Esc also leaves fullscreen: browser rule).
- [ ] Scene: the arena of `LOWPOLY-SCENE.md` §3 (pillars, stairs, ramp, crates, portal), with its
  look (§1–§2: greys, orange only emissive, flat shading, 16² nearest textures, exp² fog, one
  shadowed key light, bloom on orange). The whole look comes from one theme struct, easy to change.
- [ ] Sentinel enemies (§5) as targets that take damage and shoot back.
- [ ] Rendering through egui-wgpu paint callback (WebGPU/WebGL2 on web). Keep it light: 60 fps on a
  laptop iGPU in the browser.
- [ ] FPS controller: walk, run, jump, crouch, mouse/stick look (pointer locked), the low-poly pistol
  of §4 (pose, kick, reload dip, bob, muzzle flash) with shoot, reload, ammo; take damage, deal damage, heal, die and respawn; status effects as
  states (e.g. on fire), no VFX needed.

- [ ] Editor branding: map [`tokens.json`](tokens.json) (black/orange, dark by default, light
  variant) onto FigCraft's `theme::Tokens`; orange only for selection, focus, primary button,
  active toggles. Space Grotesk and JetBrains Mono (OFL) loaded like FigCraft's other fonts, not
  committed as files. Keep this to a small, listed hook in the upstream theme.

### G2 — Input
- [ ] Default bindings ready out of the box: keyboard + mouse and controller (Xbox and PlayStation
  layouts named correctly).
- [ ] Rebinding panel; bindings saved in the `.figcraft` file.
- [ ] Controllers on macOS and Linux too (e.g. `gilrs`), keeping Windows and web working.

### G3 — Game ↔ prototype bridge
- [ ] Game state published every frame as prototype **variables** in a "Game" collection, default
  names from `LOWPOLY-SCENE.md` §7: `vida, vidaMax, municao, reserva, atacante, ultimoDano, arma`,
  plus crouching, on fire…
  Bind them to text, fills, widths like any variable.
- [ ] New trigger **Game event**: health changed, took damage, died, respawned, shot, reloaded,
  ammo empty, healed, status started/ended… usable on frames, layers, instances and variants.
- [ ] Actions that reach the game: pause/resume, release/capture cursor.
- [ ] Default event names, renamable.

### G4 — Game screens made simple
- [ ] Mark a frame with a **screen role**: HUD (always on), Inventory, Pause menu, Map, Death
  screen, Custom. Each role comes with sensible defaults the designer can change: open key/button
  (Tab / View, Esc / Menu, on death event…), opens as overlay, pauses the game, frees the cursor,
  which layers of the HUD stay visible.
- [ ] Opening an inventory, menu or death screen must take one or two clicks.
- [ ] Starter kit file: INDIRECT-style wireframe components (rectangles with white border and white
  inside separated by transparent padding, colours editable) for HUD bars, lists, grids, tabs,
  buttons.

### G5 — Sound
- [ ] Sound library per file: import WAV/OGG/MP3 into the `.figcraft` file.
- [ ] **Play sound** action on any trigger (click, hover, key/gamepad, game event, after delay),
  including interactions inside component variants (hover sound on a button variant, etc.).
- [ ] **Master volume** and **per-sound volume** (plus optional groups: UI, SFX, music), adjustable
  in the editor and in Play mode.
- [ ] Desktop and web (web audio starts on the Play click). Pure-Rust audio crate (e.g. `kira`).

### G6 — Ready-made animations
- [ ] Library of preset animations: fade in/out, slide in/out (4 directions), scale pop, pulse,
  shake, bounce, blink, with duration and easing (reuse upstream easings).
- [ ] **Play animation** action on any trigger, applicable to any layer, instance or variant.
- [ ] Components with variants accept animations, sounds and game events in their interactions,
  so a button or a health bar component carries its own behaviour everywhere it is used.

### G7 — Share
- [ ] Share a prototype as a link (web build + upstream server) that opens straight in Play mode.

## Later
Third-person preset, 2D platformer and top-down presets, safe-area guides and resolution presets,
export package for engines (assets + spec sheet, 9-slice), Godot scene export, Unity plugin,
comparing a game screenshot with the design.
