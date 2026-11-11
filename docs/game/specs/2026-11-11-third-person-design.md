# Third-person game (G11) — design

Owner's request (2026-11-11): more games to play under the UI, chosen when a file is created and
in the Prototype tab. First: a **third-person action game** in the same look as the FPS scene
(dark, grey low poly, orange only emissive), with smooth animation and Breath-of-the-Wild-like
*mechanics*: attack, switch weapon, crouch, jump, roll on a high fall, climb ladders and onto
objects, take damage, fall, walk and run in 360°, a camera the player turns on its own, keyboard and
mouse and controller. Grass, light and shadow. The character is centred on screen. The game has no
UI of its own (the UI is what he designs in Graybox). It must be as light as possible.

Mood reference: a night hillside with grass, ruined arches, low-poly trees and rocks, a river
and a glowing waterfall, a far keep and a thin eclipse ring in the sky. It is a mood reference
only. **Everything is original**: our own hero, world and names. No character, emblem, item,
place or art from Zelda or any other game (`CLAUDE.md`, assets rule; Claude's rules forbid
recreating a specific game too).

## Principles
- **Same architecture as the FPS**: pure, deterministic simulation in `crates/game` (fixed step,
  no egui/wgpu), drawn by `crates/ui-egui/src/game/renderer` (wgpu paint callback, WebGL2 on web).
  The FPS keeps working unchanged.
- **Everything made in code** from primitives (boxes, wedges, 6-sided cylinders, low-poly
  spheres), flat shaded, the theme of `LOWPOLY-SCENE.md` §1–§2 (greys, fog, one shadowed key light,
  bloom only on orange/white emissive). No imported models, textures or sounds.
- **Budgets** (checked by tests where possible, and in the browser):
  - 60 fps on a laptop iGPU in the browser;
  - under 250k triangles drawn, under 150 draw calls;
  - under 40 MB of GPU memory for the scene (shadow map 2048² on desktop, 1024² on web);
  - simulation and world data under 8 MB of RAM;
  - the wasm build grows by under 400 KB.
- **Smooth**: every animation change crossfades (0.1–0.25 s), locomotion blends by speed, the
  camera and the character's facing are smoothed with critically damped springs, and the
  simulation is interpolated between fixed steps when drawn.

## Choosing the game
- `GameSettings::kind: GameKind { Fps (default), ThirdPerson }` saved in the `.figcraft` file (serde
  default, so old files stay FPS). Adding a third kind later is one enum entry.
- **New file**: the new-file flow asks which game to play underneath (FPS / Third person), small
  cards with a still render of each.
- **Prototype tab** (nothing selected, file game settings): a "Game" dropdown at the top.
  Changing it is undoable (a command, `game.kind.set`).
- Play runs the chosen game. Game variables, game events, bindings and the rebinding panel follow
  the chosen game. Shared names stay the same (`vida`, `vidaMax`…); new ones are added for third
  person (`arma` = weapon name, `stamina`, `staminaMax`, `escalando`, `agachado`…).

## The world (one hand-built valley, about 160×160 m)
- **Terrain**: a low-poly heightfield (flat-shaded triangles, about 2 m cells, about 13k triangles)
  shaped in code. A hill where the hero starts, a cliff edge with a drop, a river valley, raised
  plateaus with ruins. The same heightfield is the collision ground (one lookup, no physics
  engine).
- **Grass**: GPU-instanced blades (one tiny 3-triangle mesh). Instances are placed on a grid near
  the camera with deterministic jitter, fade with distance (about 35 m), and sway with wind in the
  vertex shader. Blades bend away from the hero. Zero per-blade CPU work after placement.
- **Props** (instanced where repeated): low-poly trees (trunk and faceted crown), rocks, ruined
  arches and pillars, a lit watchtower with a white/orange beacon, a river (dark flat strips with a
  faint emissive edge) and a waterfall (scrolling emissive strips), a far keep and mountain
  silhouettes as cheap cards in the fog, a thin eclipse ring in the sky (emissive, bloom).
- **Climbable things**: wooden ladders, crates and blocks to mantle onto, a rock wall with
  handholds. Climbable surfaces are tagged in the world data.
- **Targets**: training dummies and a few slow "husk" enemies (original, faceted, orange slit eyes
  like the FPS Sentinels) that walk to the hero, hit, take damage and break apart.

## The hero
- Original, faceted adventurer of about 1.7 m: hooded head, cloak, chest, two-part arms and legs.
  Built from primitives on a **rigid-part skeleton** (about 16 bones, each part a mesh moved by its
  bone; no skinning). One draw call with per-bone transforms.
- Weapons drawn as small meshes on the hand bone, the others on the back:
  - sword (fast combo of 3);
  - spear (longer reach, slower);
  - club (heavy, knockback);
  - bow (hold to aim, over-the-shoulder camera, arrows as simple projectiles).
- **Animation system**: poses as key rotations per bone, clips as keyframed curves, a state
  machine with crossfades. Locomotion blends idle, walk, jog and run by speed, crouch by a 0..1
  weight, lean into turns, and adds a little secondary motion to the cloak and head. Clips: idle,
  walk, run, crouch idle and walk, jump up, fall, land, land-roll, climb ladder, climb wall, mantle,
  three sword attacks, spear thrust, club swing, bow draw and shoot, hit react, knockdown and get
  up, die, weapon draw and sheathe.

## Movement and camera
- **360° movement relative to the camera**, with acceleration and turn smoothing. Walk with a light
  stick or Alt, jog by default, run with Shift or the stick button (uses stamina), crouch (slower,
  lower).
- **Jump** with coyote time and a jump buffer. Landing from more than about 4 m without rolling
  takes damage; pressing roll near landing (or a high fall onto grass) rolls and takes none.
  Falling more than about 12 m without rolling is a heavy hit. A **dodge roll** on the ground too.
- **Climbing**: walk into a ladder to climb it (up/down, top and bottom exits). Ledges at chest
  height are vaulted or mantled automatically when moving into them; a tagged wall climbs on a
  grid, using stamina.
- **Camera**: an orbit camera behind and above the shoulder, the hero centred on screen.
  - Mouse and right stick turn it on their own, with sensitivity and invert settings.
  - It eases back behind the hero only after a while without camera input.
  - It pulls in on collision with terrain and props, frames the bow aim over the shoulder, and
    can lock on to the nearest enemy (a held or toggled button, stretch goal).
- **Bindings**: defaults for keyboard and mouse and for Xbox/PlayStation controllers, in the same
  rebinding panel as the FPS:

  | Action | Keyboard and mouse | Controller |
  |---|---|---|
  | Move | WASD | Left stick |
  | Camera | Mouse | Right stick |
  | Jump / climb | Space | A |
  | Roll / dodge | Ctrl | B |
  | Attack | Left click | X / RB |
  | Bow aim | Right click | LT |
  | Next / previous weapon | Wheel, 1–4 | D-pad |
  | Crouch | C | L3 tap |
  | Run | Shift | L3 hold |
  | Interact | E | Y |
  | Lock on | Middle click | R3 |

## Delivery in phases (one session each, each ends committed and checked in the app)
1. **G11.1 Game kind plumbing**: `GameKind`, `game.kind.set`, the new-file choice, the Prototype
   tab dropdown, Play dispatch. A placeholder third-person scene (flat ground, a capsule moving in
   360° with an orbit camera) proves the path end to end. FPS untouched.
2. **G11.2 World**: terrain, grass, props, lights, river, waterfall, sky, climbable tags, collision
   ground, budgets test. Runs in parallel with 3.
3. **G11.3 Hero and animation**: the rig, the meshes, the clip and crossfade system, every clip,
   and a test pose viewer (debug only). Runs in parallel with 2.
4. **G11.4 Movement and camera**: controller (walk, jog, run, crouch, jump, fall, land-roll,
   dodge, ladders, mantle, wall climb, stamina), the orbit camera with collision, bindings,
   interpolation. Needs 2 and 3.
5. **G11.5 Combat and the bridge**: weapons and switching, attacks and combos, bow and arrows,
   dummies and husks, damage, knockback, death and respawn, game variables and game events,
   starter-kit HUD example for third person. Needs 4.
6. **G11.6 Polish and performance**: browser check at 60 fps, memory and size budgets, feel tuning
   (spring constants, crossfade times), update the installed copy.
