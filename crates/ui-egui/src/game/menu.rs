//! Navigable menus in Play mode: which menu the open screen owns, its items, the menu keys, and
//! what focus should look like. The prototype player applies the result
//! (`present::game::menu_step`).

use std::collections::HashMap;

use egui::InputState;
use figcraft_doc::game::{DisabledItems, GameSettings};
use figcraft_doc::proto::KeyMods;
use figcraft_doc::{Document, LayoutMode, NodeId, NodeKind};
use figcraft_engine::components::{find_variant, variant_props};
use figcraft_game::menu::{Dir, MenuGrid, NavOptions, Repeat, first_focusable, step};

/// A stick pushed this far counts as a held direction.
const STICK_DEADZONE: f32 = 0.5;

/// Focus state of the menu the open screen owns.
#[derive(Clone, Debug, Default)]
pub struct Focus {
    pub menu: Option<NodeId>,
    pub item: Option<NodeId>,
    /// The focused item has no State variant to show focus: the player outlines it.
    pub outline: bool,
    remembered: HashMap<NodeId, NodeId>,
    repeat: Repeat,
    last_pointer: Option<[f32; 2]>,
    /// An item the pointer moved onto, focused on the next menu frame.
    pointed: Option<NodeId>,
    /// The focused instance shown as another variant: (item, its own variant, the shown one).
    swapped: Option<(NodeId, NodeId, NodeId)>,
    pressed: bool,
}

/// The menu actions of one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MenuInputs {
    pub held: Option<Dir>,
    pub confirm: bool,
    pub confirm_held: bool,
    pub back: bool,
}

/// What the player has to do this frame: instances to show as another variant, an item to
/// confirm, Back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuPlan {
    pub active: bool,
    /// (instance, variant it shows), applied in order.
    pub looks: Vec<(NodeId, NodeId)>,
    pub confirm: Option<NodeId>,
    pub back: bool,
}

/// The first navigable menu in `root`'s layers (layer order) that is still a visible auto layout frame.
pub fn active_menu(doc: &Document, root: NodeId) -> Option<NodeId> {
    if doc.game.menus.is_empty() {
        return None;
    }
    doc.subtree(root).into_iter().find(|id| doc.game.menus.contains_key(id) && doc.node(*id).is_some_and(|n| n.visible && n.layout.is_some()))
}

/// The menu's items: its visible direct children, in layout order.
pub fn items(doc: &Document, menu: NodeId) -> Vec<NodeId> {
    let Some(n) = doc.node(menu) else { return Vec::new() };
    n.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|c| c.visible)).collect()
}

/// How the items lie: items per row (a list has one, a row all of them, a grid its column count, a
/// wrapping row the items on its first line), and `true` for a wrapping column, whose items run
/// down each column first (its rows here are its columns, so the directions turn).
fn columns(doc: &Document, menu: NodeId, items: &[NodeId]) -> (usize, bool) {
    let Some(layout) = doc.node(menu).and_then(|n| n.layout.as_ref()) else { return (1, false) };
    // Items on the first line: same x (a column) or same y (a row) as the first item.
    let first_line = |axis: usize| {
        let at = |id: &NodeId| doc.node(*id).and_then(|n| n.transform.as_coeffs().get(axis).copied());
        let first = items.first().and_then(at);
        items.iter().take_while(|id| at(id).zip(first).is_some_and(|(a, b)| (a - b).abs() < 0.5)).count().max(1)
    };
    match layout.mode {
        LayoutMode::Vertical if !layout.wrap => (1, false),
        LayoutMode::Vertical => (first_line(4), true),
        LayoutMode::Grid => ((layout.grid_columns as usize).max(1), false),
        LayoutMode::Horizontal if !layout.wrap => (items.len().max(1), false),
        LayoutMode::Horizontal => (first_line(5), false),
    }
}

/// A direction in a wrapping column, read on its turned grid.
fn turned(dir: Dir) -> Dir {
    match dir {
        Dir::Up => Dir::Left,
        Dir::Down => Dir::Right,
        Dir::Left => Dir::Up,
        Dir::Right => Dir::Down,
    }
}

fn component_of(doc: &Document, item: NodeId) -> Option<NodeId> {
    match doc.node(item).map(|n| &n.kind) {
        Some(NodeKind::Instance { component, .. }) => Some(*component),
        _ => None,
    }
}

fn state_of(doc: &Document, component: NodeId) -> Option<String> {
    doc.node(component).and_then(|c| variant_props(&c.name).get("State").cloned())
}

/// The variant of `component`'s set with the same properties but `State=value`.
pub(crate) fn state_variant(doc: &Document, component: NodeId, value: &str) -> Option<NodeId> {
    let c = doc.node(component)?;
    let set = c.parent?;
    if !matches!(doc.node(set)?.kind, NodeKind::ComponentSet { .. }) {
        return None;
    }
    let mut wanted = variant_props(&c.name);
    wanted.get("State")?;
    wanted.insert("State".into(), value.into());
    let found = find_variant(doc, set, &wanted)?;
    (state_of(doc, found).as_deref() == Some(value)).then_some(found)
}

/// True when a key or controller trigger in `root`'s layers handles input `name`: that
/// interaction wins and the menu leaves the input alone.
pub fn layer_takes_input(doc: &Document, root: NodeId, name: &str) -> bool {
    doc.subtree(root)
        .into_iter()
        .filter_map(|id| doc.node(id))
        .any(|n| n.interactions.iter().any(|it| it.trigger.fires_on_key(name, KeyMods::default())))
}

/// The menu actions from the file's menu bindings, held/pressed controller buttons and the left
/// stick (`[x, y]`, y down). Inputs `taken` by an interaction are left out.
pub fn read_menu(i: &InputState, settings: &GameSettings, held: u32, pressed: u32, stick: [f32; 2], taken: &dyn Fn(&str) -> bool) -> MenuInputs {
    let any = |action: &str, buttons: u32, edge: bool| {
        settings
            .menu_bindings
            .get(action)
            .is_some_and(|names| names.iter().filter(|name| !taken(name)).any(|name| super::input::binding(i, name, buttons, edge)))
    };
    let dirs = [("menu-up", Dir::Up), ("menu-down", Dir::Down), ("menu-left", Dir::Left), ("menu-right", Dir::Right)];
    // Held, or tapped this frame (a key pressed and released between two frames is not down).
    let keyed = dirs.iter().find(|(action, _)| any(action, held, false) || any(action, pressed, true)).map(|(_, dir)| *dir);
    let [x, y] = stick;
    let stuck = (x.is_finite() && y.is_finite() && x.abs().max(y.abs()) >= STICK_DEADZONE).then(|| {
        if x.abs() > y.abs() {
            if x > 0.0 { Dir::Right } else { Dir::Left }
        } else if y > 0.0 {
            Dir::Down
        } else {
            Dir::Up
        }
    });
    MenuInputs {
        held: keyed.or(stuck),
        confirm: any("menu-confirm", pressed, true),
        confirm_held: any("menu-confirm", held, false),
        back: any("menu-back", pressed, true),
    }
}

impl Focus {
    /// The pointer is at `pointer` over layer `hit`: when it moved onto an item of the menu, that
    /// item takes focus on the next menu frame (a resting pointer never takes it back).
    pub fn point(&mut self, doc: &Document, hit: Option<NodeId>, pointer: Option<[f32; 2]>) {
        let moved = pointer != self.last_pointer;
        self.last_pointer = pointer;
        let (true, Some(mut id), Some(menu)) = (moved, hit, self.menu) else { return };
        // Walk up from the layer hit to the menu's direct child.
        for _ in 0..256 {
            let Some(parent) = doc.node(id).and_then(|n| n.parent) else { return };
            if parent == menu {
                if doc.node(id).is_some_and(|n| n.visible) {
                    self.pointed = Some(id);
                }
                return;
            }
            id = parent;
        }
    }

    /// The variant `item` really is (not the one shown for focus).
    fn own_variant(&self, doc: &Document, item: NodeId) -> Option<NodeId> {
        let shown = component_of(doc, item)?;
        match self.swapped {
            Some((i, own, as_shown)) if i == item && as_shown == shown => Some(own),
            _ => Some(shown),
        }
    }

    fn disabled(&self, doc: &Document, item: NodeId) -> bool {
        self.own_variant(doc, item).and_then(|v| state_of(doc, v)).as_deref() == Some("Disabled")
    }

    /// Put the swapped item back to its own variant.
    fn unswap(&mut self, doc: &Document, plan: &mut MenuPlan) {
        if let Some((item, own, shown)) = self.swapped.take()
            && component_of(doc, item) == Some(shown)
            && doc.contains(own)
        {
            plan.looks.push((item, own));
        }
    }

    /// One menu frame for the screen whose layers start at `root`.
    pub fn plan(&mut self, doc: &Document, root: NodeId, input: &MenuInputs, now: f64) -> MenuPlan {
        let mut plan = MenuPlan::default();
        let active = active_menu(doc, root);
        if active != self.menu {
            if let (Some(menu), Some(item)) = (self.menu, self.item) {
                self.remembered.insert(menu, item);
            }
            self.unswap(doc, &mut plan);
            self.menu = active;
            self.item = None;
            self.pointed = None;
            self.pressed = false;
        }
        let dir = self.repeat.update(input.held, now);
        let Some(menu) = active else {
            self.item = None;
            self.outline = false;
            return plan;
        };
        plan.active = true;
        let settings = doc.game.menus.get(&menu).cloned().unwrap_or_default();
        let items = items(doc, menu);
        let opts = NavOptions { wrap: settings.wrap, skip_disabled: settings.disabled == DisabledItems::Skip };
        let (columns, column_first) = columns(doc, menu, &items);
        let grid = MenuGrid { columns, enabled: items.iter().map(|i| !self.disabled(doc, *i)).collect() };
        let position = |id: Option<NodeId>| id.and_then(|id| items.iter().position(|i| *i == id));
        let mut index = position(self.pointed.take()).or_else(|| position(self.item));
        if index.is_none() {
            let remembered = if settings.remember { position(self.remembered.get(&menu).copied()) } else { None };
            let start = remembered.or_else(|| position(settings.start)).unwrap_or(0);
            index = first_focusable(&grid, start, opts);
        }
        if let (Some(dir), Some(i)) = (dir, index) {
            index = Some(step(&grid, i, if column_first { turned(dir) } else { dir }, opts));
        }
        let item = index.and_then(|i| items.get(i).copied());
        if item != self.item {
            self.unswap(doc, &mut plan);
            self.pressed = false;
        }
        self.item = item;
        let Some(item) = item else {
            self.outline = false;
            return plan;
        };
        let disabled = self.disabled(doc, item);
        if input.confirm && !disabled {
            plan.confirm = Some(item);
            self.pressed = true;
        }
        if !input.confirm_held {
            self.pressed = false;
        }
        plan.back = input.back;
        // Focus look: the State variant, else an outline (a disabled item keeps its own look).
        let own = self.own_variant(doc, item);
        let wanted = own.filter(|_| !disabled).and_then(|own| state_variant(doc, own, if self.pressed { "Pressed" } else { "Focused" }));
        self.outline = wanted.is_none();
        match (wanted, own) {
            (Some(shown), Some(own)) => {
                if component_of(doc, item) != Some(shown) {
                    plan.looks.push((item, shown));
                }
                self.swapped = Some((item, own, shown));
            }
            _ => self.unswap(doc, &mut plan),
        }
        plan
    }
}
