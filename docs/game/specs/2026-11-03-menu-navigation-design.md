# Navigable menus — design

Status: approved in conversation with the owner (2026-11-03). Milestone: G4.

## Goal
A game UI designer builds a list, a row or a grid with ordinary auto layout, switches on
**Navigable menu**, presses Play, and the menu works with mouse, keyboard and controller. No
item-to-item wiring by hand.

Success: in Play mode, focus moves through the menu with arrows / WASD / D-pad / left stick and
mouse hover, Confirm (Enter, A/✕, click) runs the focused item's normal prototype interactions,
Back (Esc, B/○) closes the overlay the menu lives in. Works on desktop and in the browser build.

## Key decision: the "base component" is auto layout
Upstream auto layout already has `LayoutMode::Horizontal | Vertical | Grid` (with `grid_columns`)
in `crates/doc/src/props.rs`. A navigable menu is a **behaviour switched on for an auto layout
frame**, not a new node or component type:

| Auto layout | Menu kind | Directions |
|---|---|---|
| Vertical | list | ↑ ↓ |
| Horizontal | row | ← → |
| Grid | grid | ↑ ↓ ← → (by column count) |

A wrapping Horizontal/Vertical layout navigates like a grid, using its laid-out lines.

Rejected:
- Dedicated List / Grid component types: rigid, duplicates auto layout.
- Nearest-element ("spatial") navigation as the main model: works on any drawing but is less
  predictable. Kept for phase 2 as a fallback for loose layers without auto layout.

## Phase 1 (this task)

### Editor
- Prototype tab, with an auto layout frame selected: a **Navigable menu** toggle (off by default).
- Options when on:
  - `wrap`: going past the last item comes back to the first (default: off).
  - `start`: first item (default) or a chosen child.
  - `remember`: when the menu gets focus again, return to the last focused item (default: on)
    or to `start`.
  - `disabled`: items with `State=Disabled` get focus but do not confirm (default), or are skipped.
- Items are the frame's **direct, visible children**, in layout order. Hidden children are skipped.

### Focus look
- If the item is an instance whose component set has a `State` variant property, Play switches it
  among `Default`, `Focused`, `Pressed` (while Confirm is held) and `Disabled`. Missing values fall
  back to `Default`.
- Otherwise Play draws a simple orange focus outline (theme focus colour) around the item, so a
  menu can be tried before its variants exist.
- The starter kit's buttons, grid cells and tabs gain a `State` property with those values.

### Play
- Input actions `menu-up`, `menu-down`, `menu-left`, `menu-right`, `menu-confirm`, `menu-back`
  with default bindings (arrows + WASD + D-pad + left stick; Enter / Space / A; Esc / B), shown and
  rebindable in the existing controls panel.
- Holding a direction repeats after a delay (≈ 400 ms, then every ≈ 120 ms; sim constants).
- Mouse hover focuses an item; click = Confirm. Keyboard/controller and mouse share one focus.
- Confirm runs the item's own interactions (On click / On press) through the existing prototype
  player, so Open overlay, Play sound, Play animation etc. work unchanged.
- Back closes the topmost overlay that holds the focused menu (same path as Close overlay).
- Focus scope: the topmost open overlay that contains a navigable menu owns input; when it opens,
  focus goes to its menu's `start` (or remembered item); when it closes, focus returns to the
  item that had it before.
- While a menu owns input, menu bindings that collide with gameplay bindings (WASD) drive the menu,
  not the player (Play mode usually has the game paused under a menu anyway).

### Storage
- Stored per file in `GameSettings` (`crates/doc/src/game.rs`) as
  `menus: BTreeMap<NodeId, MenuSettings>`, not on the upstream node, so upstream's document format
  stays untouched. A menu whose frame was deleted or lost auto layout is ignored (and dropped the next time
  `game.menu.set` runs).
- Older files without `menus` load with none. Bounded: at most a few thousand entries, validated on load.

### Code layout
- `crates/game` (pure, L2): `menu` module. Input: item count, layout kind, column count (or laid-out
  line breaks), current index, direction, options, which items are disabled/hidden. Output: the next
  index or "stay". Also the hold-to-repeat timer on the fixed clock. No egui.
- `crates/engine/src/cmd/game.rs`: command `game.menu.set`
  `{node, enabled?, wrap?, start?, remember?, disabled? ("focus"|"skip")}`, undoable; refuses nodes
  that are not auto layout frames.
- `crates/ui-egui/src/game/` + small hook in the prototype player (`present/`): per-frame focus
  state, reading menu actions, calling the `menu` module, swapping the `State` variant or drawing
  the outline, dispatching Confirm/Back through the player's existing actions. Every hook listed in
  `STATUS.md` "Upstream hooks".
- Prototype tab panel: the toggle and options, acting through `game.menu.set`.

### Never crash
- Empty menu, deleted item, item index out of range after an edit, zero columns, non-finite rects:
  no navigation, no error. Indexes read with `.get()`.

### Tests
- `menu` module: list / row / grid moves, incomplete last grid row, wrap on/off, skipping hidden and
  (option) disabled items, start and remember, hold-to-repeat timing.
- `game.menu.set`: set, clear, undo, refusal on non-auto-layout nodes; extend the engine's
  junk-parameter sweep; settings round-trip and legacy files without `menus`.
- Play: simulated ↓ then Enter on a starter-kit list switches the `State` variant and runs the
  item's interaction; overlay close returns focus.
- Screenshot of Play mode with the starter kit showing focus moving. `cargo xtask ci` incl. wasm.

## Phase 2 (later, separate task)
- Nested menus: a tab bar (LB/RB, `menu-prev-tab` / `menu-next-tab`) that switches the menu below,
  each keeping its own focus.
- Spatial fallback for layers without auto layout.
