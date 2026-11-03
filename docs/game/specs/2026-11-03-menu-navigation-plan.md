# Navigable menus (phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Any auto layout frame (list, row, grid) can be switched to a navigable menu that, in Play
mode, moves focus with mouse, keyboard and controller, shows focus through the `State` variant (or an
orange outline), confirms with the item's own interactions and goes back by closing the overlay.

**Architecture:** Pure navigation math in `crates/game/src/menu.rs` (L2, no egui). Per-file menu
settings and menu bindings in `GameSettings` (`crates/doc/src/game.rs`). One undoable engine command
`game.menu.set`. Runtime focus in `crates/ui-egui/src/game/menu.rs`, applied by small glue in
`crates/ui-egui/src/present/game.rs` and one hook in `present/player.rs`. Prototype tab section for the
toggle and options. Starter kit rebuilt with auto layout and `State` variants.

**Tech Stack:** Rust, egui (UI crate only), serde/serde_json, `cargo xtask ci`.

**Spec:** [`docs/game/specs/2026-11-03-menu-navigation-design.md`](2026-11-03-menu-navigation-design.md)

## Global Constraints
- No `unwrap/expect/panic!/unreachable!/todo!` in shipped code; indexes from data via `.get()`.
- `crates/game` stays free of egui/eframe/wgpu/winit (L2). Never break wasm (`cargo xtask wasm`).
- Upstream files are touched only at small named hooks, each listed in `docs/game/STATUS.md` "Upstream hooks".
- Repo text in English; commits `G4.n: …` ending with the Co-Authored-By line.
- Repeat timing: first repeat after **0.4 s**, then every **0.12 s**.
- Defaults: `wrap` off, `start` first item, `remember` on, disabled items get focus but do not confirm.
- Focus look: `State` values `Default`, `Focused`, `Pressed`, `Disabled`; fallback outline in the theme's
  focus/accent colour.
- Before Task 6: the working tree has another session's uncommitted G4 work in
  `crates/ui-egui/src/game/mod.rs`, `present/game.rs`, `present/chrome.rs` and
  `examples/game-starter.figcraft`. Do not overwrite it; wait until it is committed, then build on it.

## Review Focus
1. **Esc closes two overlays**: the designer already has a Key `Escape` → Close overlay interaction
   and `menu-back` is also Escape. Expected: one overlay closes. Rule: a key/gamepad trigger on the
   active layer for the same input wins, and the menu ignores that press (Task 4 test).
2. **Menu edited while playing** (children removed, auto layout turned off): the focused index is
   out of range. Expected: focus clamps to the last item, or the menu goes inactive; no panic (Task 4 test).
3. **A static pointer steals focus**: the mouse rests on an item while the player presses ↓. Expected:
   focus moves; hover refocuses only when the pointer moves (Task 4 test).
4. **Item without a `State` property** (plain frame, or instance of a lone component): expected:
   outline shown, Confirm still fires its On click (Task 4 test).
5. **Two menus in one overlay**: expected: the first one in layer order takes focus (phase 1 rule;
   tab switching is phase 2) (Task 4 test).

---

### Task 1: Pure navigation math

**Files:**
- Create: `crates/game/src/menu.rs`; Modify: `crates/game/src/lib.rs` (`pub mod menu;`)
- Test: `crates/game/src/menu_tests.rs` (`#[cfg(test)] mod menu_tests;` in lib.rs)

**Interfaces — Produces:**
```rust
pub enum Dir { Up, Down, Left, Right }
#[derive(Clone, Copy)] pub struct NavOptions { pub wrap: bool, pub skip_disabled: bool }
/// Items in row-major order (hidden children are not passed). `columns` ≥ 1: list = 1,
/// row = items.len(), grid = grid_columns, wrapping flow = items on its first line.
pub struct MenuGrid { pub columns: usize, pub enabled: Vec<bool> }
pub fn step(grid: &MenuGrid, from: usize, dir: Dir, opts: NavOptions) -> usize;
pub fn first_focusable(grid: &MenuGrid, start: usize, opts: NavOptions) -> Option<usize>;
pub const REPEAT_DELAY: f64 = 0.4;
pub const REPEAT_EVERY: f64 = 0.12;
#[derive(Default)] pub struct Repeat { /* held dir + next due time */ }
impl Repeat { pub fn update(&mut self, held: Option<Dir>, now: f64) -> Option<Dir>; }
```
Semantics: Left/Right move ±1 inside the row; Up/Down move ±columns. At an edge: `wrap` → the other
end of the same row/column, else stay. Moving down into an incomplete last row lands on its last item.
`skip_disabled` keeps going the same way past disabled items; if none is focusable, stay. Empty grid or
`from` out of range → return `min(from, len-1)` (0 when empty). `Repeat::update` returns the direction
on a new press, again at `REPEAT_DELAY`, then every `REPEAT_EVERY`; `None` when released; non-finite
`now` → `None`.

- [ ] **Step 1: Write failing tests** `list_moves_up_and_down_and_ignores_left_right`,
  `row_moves_left_right_only`, `grid_3x2_moves_in_four_directions`,
  `down_into_incomplete_last_row_lands_on_its_last_item` (7 items, 3 columns: from 4 Down → 6),
  `wrap_off_stays_at_edges`, `wrap_on_goes_to_the_other_end_of_the_row_and_column`,
  `skip_disabled_jumps_over_and_stays_when_none_left`, `empty_and_out_of_range_never_panic`,
  `repeat_fires_on_press_then_after_0_4_then_every_0_12` (presses at t=0 → Some, 0.39 → None,
  0.40 → Some, 0.52 → Some, release → None).
- [ ] **Step 2:** `cargo test -p figcraft-game menu` → FAIL (module missing).
- [ ] **Step 3:** Implement `menu.rs` per the interface.
- [ ] **Step 4:** `cargo test -p figcraft-game menu` → PASS.
- [ ] **Step 5:** Commit `G4.1: menu navigation math (list, row, grid, repeat)`.

### Task 2: Menu settings and menu bindings in the file

**Files:**
- Modify: `crates/doc/src/game.rs`
- Test: `crates/doc/src/game_menu_tests.rs` (declared next to existing doc tests)

**Interfaces — Produces:**
```rust
pub const MENU_ACTIONS: [&str; 6] = ["menu-up", "menu-down", "menu-left", "menu-right", "menu-confirm", "menu-back"];
pub const MAX_MENUS: usize = 4096;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] pub enum DisabledItems { #[default] Focus, Skip }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] #[serde(default)]
pub struct MenuSettings { pub wrap: bool /*false*/, pub start: Option<NodeId>, pub remember: bool /*true*/, pub disabled: DisabledItems }
// New GameSettings fields (and in StoredGameSettings, defaulted):
pub menus: BTreeMap<NodeId, MenuSettings>,
pub menu_bindings: BTreeMap<String, Vec<String>>,
```
Default `menu_bindings`: up `ArrowUp, W, Gamepad Up`; down `ArrowDown, S, Gamepad Down`; left
`ArrowLeft, A, Gamepad Left`; right `ArrowRight, D, Gamepad Right`; confirm `Enter, Space, Gamepad A`;
back `Escape, Gamepad B`. `validate()` checks `menu_bindings` like `bindings` (known action, valid
keys, no key twice **within menu_bindings**; sharing a key with gameplay bindings is allowed) and
`menus.len() <= MAX_MENUS`.

- [ ] **Step 1: Failing tests** `defaults_validate_and_menu_keys_may_overlap_gameplay_keys`,
  `menu_binding_conflict_within_menu_actions_is_rejected`, `settings_round_trip_with_menus`
  (serialize → deserialize equal, with a `NodeId` key), `file_without_menus_loads_with_defaults`
  (JSON of today's `GameSettings` → `menus` empty, `menu_bindings` default), `too_many_menus_is_rejected`.
- [ ] **Step 2:** `cargo test -p figcraft-doc game_menu` → FAIL.
- [ ] **Step 3:** Add types, fields, defaults, validation.
- [ ] **Step 4:** `cargo test -p figcraft-doc` → PASS (existing game tests too).
- [ ] **Step 5:** Commit `G4.2: menu settings and menu bindings in the file`.

### Task 3: `game.menu.set` command and menu actions in `game.bind`

**Files:**
- Modify: `crates/engine/src/cmd/game.rs`
- Test: `crates/engine/src/game_config_tests.rs`

**Interfaces — Consumes:** Task 2 types. **Produces:** command
`spec!("game.menu.set", "Set navigable menu", "{node, enabled? (bool), wrap?, start? (child id|null), remember?, disabled? (\"focus\"|\"skip\")}", true, menu_set)`.
`game.bind` also accepts `MENU_ACTIONS` (writes `menu_bindings`); `reset: true` restores both maps.

Rules: `node` must be a frame-like node with `layout: Some(_)`, else error "Navigable menus need an
auto layout frame". `enabled: false` removes the entry. `start` must be a direct child of `node`.
Other fields update an existing entry or create one with defaults. All changes validated, atomic.
Every run also drops entries whose node no longer exists (stale menus are ignored at runtime anyway).

- [ ] **Step 1: Failing tests** `menu_set_enables_updates_disables_and_undoes`,
  `menu_set_refuses_frames_without_auto_layout_and_foreign_start`,
  `bind_accepts_menu_actions_and_reset_restores_them`.
- [ ] **Step 2:** `cargo test -p figcraft-engine game_config` → FAIL.
- [ ] **Step 3:** Implement `menu_set`; extend `bind`.
- [ ] **Step 4:** `cargo test -p figcraft-engine game_config every_command_survives_junk_params` → PASS
  (the sweep picks the new command up automatically).
- [ ] **Step 5:** Commit `G4.3: game.menu.set command`.

### Task 4: Focus in Play mode

**Files:**
- Create: `crates/ui-egui/src/game/menu.rs` (+ `pub mod menu;` in `game/mod.rs`)
- Modify: `crates/ui-egui/src/game/mod.rs` (`Play` gains `pub menu: menu::Focus`)
- Modify: `crates/ui-egui/src/present/game.rs` (glue, before `player()`)
- Modify (hook): `crates/ui-egui/src/present/player.rs` right after `draw(p, &painter, &ctx, &l, now);`:
  `super::game::menu_overlay(p, &painter, &ctx, &l);`
- Test: `crates/ui-egui/src/game/menu_tests.rs`

**Interfaces — Consumes:** `figcraft_game::menu::{step, first_focusable, Repeat, Dir, MenuGrid, NavOptions}`,
`GameSettings::{menus, menu_bindings}`, `game::input::action` for bindings,
`figcraft_engine` `find_variant` / `variant_props` (`crates/engine/src/assets/components/variants.rs`; make
them reachable from ui-egui if they are not already).
**Produces:**
```rust
pub struct Focus { pub menu: Option<NodeId>, pub item: Option<NodeId>, remembered: HashMap<NodeId, usize>, repeat: Repeat, last_pointer: Option<egui::Pos2> }
/// Which menu the active layer owns: first `menus` key inside `root`'s subtree (layer order) that is still an auto layout frame.
pub fn active_menu(doc: &Document, root: NodeId) -> Option<NodeId>;
/// Visible direct children in layout order, and the MenuGrid for them.
pub fn items(doc: &Document, menu: NodeId) -> (Vec<NodeId>, MenuGrid);
/// `State=Disabled` on the instance's variant name.
pub fn is_disabled(doc: &Document, item: NodeId) -> bool;
pub enum MenuInput { Move(Dir), Confirm, Back }
```
Glue behaviour (in `present/game.rs`, each frame, when `p.game` is `Some`):
- Active layer root = top overlay, else `p.current`. On a change of active menu: focus `remembered`
  (if `remember`) else `start` else `first_focusable`.
- Read `menu_bindings` with held/pressed pads; the left stick (`pad.axes[0..2]`, |v| ≥ 0.5) counts as
  a held direction. Feed `Repeat`. A press whose input is also a key/gamepad trigger on the active
  layer is left to that interaction (Review Focus 1). Consumed keys are removed from egui's input and
  consumed pad names from `p.pads`, so they do not reach other triggers.
- While a menu is active, set `play.game.input` forward/strafe/jump to 0.
- Focus look: swap the focused instance to `find_variant(set, props + State=Focused)` (and `Pressed`
  while Confirm is held, back to `Default` when focus leaves), the same way `Action::ChangeTo` edits
  `p.design` and calls `settle`. No `State` → no swap.
- Confirm: unless disabled, fire the item's first `On click` interaction (item, else first descendant
  that has one) via `p.fire(id, k, None, now)`. Back: `p.close_overlay(now)` when an overlay is open.
- `menu_overlay` hook: if the pointer moved since last frame and is over an item (`layer_at` on the
  active place), focus it; draw a 2 px outside outline in the accent colour around the focused item
  when it has no `State` swap.
- Stale data: missing menu or item, index ≥ len → clamp or deactivate, never panic.

- [ ] **Step 1: Failing tests** (build a `Presenter` in Play with a small doc, like
  `present/actions.rs` tests): `down_then_enter_focuses_second_item_and_runs_its_click`,
  `focus_swaps_state_variant_and_back_to_default`, `overlay_close_returns_focus_to_previous_item`,
  `escape_with_own_close_interaction_closes_only_one_overlay`, `removed_children_while_playing_do_not_panic`,
  `resting_pointer_does_not_steal_focus_from_keys`, `item_without_state_gets_outline_and_still_confirms`,
  `first_menu_in_layer_order_wins`, `menu_active_zeroes_movement`.
- [ ] **Step 2:** `cargo test -p figcraft-ui-egui menu` → FAIL.
- [ ] **Step 3:** Implement `game/menu.rs`, glue and hook.
- [ ] **Step 4:** `cargo test -p figcraft-ui-egui` → PASS.
- [ ] **Step 5:** Add the `player.rs` hook to STATUS.md "Upstream hooks". Commit `G4.4: menu focus in Play`.

### Task 5: Prototype tab and controls panel

**Files:**
- Modify: `crates/ui-egui/src/game/mod.rs` (new `pub(crate) fn menu_section(app, ui, doc, node)`)
- Modify (hook): `crates/ui-egui/src/panels/proto_panel.rs` `prototype()`, after `scroll_section(...)`:
  `crate::game::menu_section(app, ui, &doc, &node);`
- Modify: `crates/ui-egui/src/game/editor.rs` `controls` lists `MENU_ACTIONS` under a "Menus" label
  (labels: Menu up / down / left / right, Confirm, Back), bound through `game.bind`.
- Test: `crates/ui-egui/src/game/tests.rs`

Section shows only when the single selected node has auto layout: checkbox "Navigable menu"; when on,
"Loop at the end" (wrap), "Start at" dropdown (First item + direct children by name), "Remember last
item", "Disabled items: Can be focused / Skipped". Every change → `app.run("game.menu.set", …)`.

- [ ] **Step 1: Failing test** `menu_section_toggle_runs_game_menu_set` (drive the panel like other
  ui-egui panel tests; assert `doc.game.menus` contains the frame after the click).
- [ ] **Step 2:** run → FAIL. **Step 3:** implement. **Step 4:** run → PASS.
- [ ] **Step 5:** Hook in STATUS.md. Commit `G4.5: navigable menu settings in the Prototype tab`.

### Task 6: Starter kit with real menus

**Files:**
- Modify: `crates/engine/src/cmd/game.rs` (`starter_kit`), `crates/engine/src/game_config_tests.rs`
- Regenerate: `examples/game-starter.figcraft`

Button, Tab, Grid cell and list Row become component sets with `State=Default|Focused|Pressed|Disabled`
(Focused: orange inset, the editor theme's accent colour; Pressed: inset darker;
Disabled: 40 % opacity). List: Vertical auto layout, gap 12, padding 12. Grid: Grid mode, 3 columns,
gap 16. Inventory grid: Grid mode, 6 columns, of Grid cell instances; Pause buttons: Vertical list of
Button instances. `game.menu.set`-equivalent entries for those three frames. Keep example
interactions plain prototype interactions.

- [ ] **Step 1: Failing test** `starter_kit_menus_are_auto_layout_with_state_variants` (each menu
  entry's node has `layout`, its children are instances whose sets have all four `State` values).
- [ ] **Step 2:** run → FAIL. **Step 3:** implement; regenerate the example via
  `figcraft-cli run` (`game.starterKit`, save). **Step 4:** `cargo test -p figcraft-engine game_config` → PASS.
- [ ] **Step 5:** Commit `G4.6: starter kit menus with State variants`.

### Task 7: Look at it, record, CI

- [ ] **Step 1:** `cargo run -p figcraft -- --control <port> examples/game-starter.figcraft`; open
  Inventory in Play, send ↓ ↓ → via control channel, `ui.screenshot`; read the PNG: focus moved and
  shows the Focused variant. Same for Pause list with Enter.
- [ ] **Step 2:** `cargo xtask ci` → all green except the known failures in STATUS.md.
- [ ] **Step 3:** Tick the PLAN.md item, add Done entry + decisions (menu bindings separate from
  gameplay bindings; interaction wins over menu on shared input) to STATUS.md. Commit `G4.7: navigable menus verified`.
