//! Persistent game configuration and an original editable wireframe starter kit.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_color::{Color, Paint};
use figcraft_doc::game::{ACTIONS, DisabledItems, EVENT_NAMES, GameSettings, MENU_ACTIONS, MenuSettings, default_menu_bindings, validate_keys};
use figcraft_doc::proto::{Action, Easing, Interaction, OverlayAnchor, OverlayPosition, Transition, Trigger};
use figcraft_doc::{AutoLayout, Collection, Document, LayoutMode, Mode, Node, NodeId, NodeKind, TextData, Variable, VariableValue};
use figcraft_geom::Affine;
use serde_json::{Value, json};

use super::{CommandSpec, ok};
use crate::params::{opt_id, req_id, req_string};
use crate::{Result, Session, bad, spec};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!("game.setup", "Set up game variables", "{} → {variables}", true, setup),
        spec!("game.bind", "Edit game bindings", "{action?, keys? ([input names]), reset? (restore every default binding)}", true, bind),
        spec!(
            "game.menu.set",
            "Set navigable menu",
            "{node, enabled? (bool), wrap? (bool), start? (child id | null), remember? (bool), disabled? (focus|skip)}",
            true,
            menu_set
        ),
        spec!("game.event.rename", "Rename game event", "{event (canonical name), name (event alias)}", true, rename_event),
        spec!("game.settings", "Game settings", "{}", false, |s, _| {
            s.doc()?;
            s.request_ui("game.settings", json!({}));
            ok()
        }),
        spec!("game.starterKit", "Create game UI starter kit", "{} → {hud, inventory, pause, map, death, components, variables}", true, starter_kit),
    ]
}

fn defaults() -> Vec<(&'static str, VariableValue)> {
    vec![
        ("vida", VariableValue::Number(100.0)),
        ("vidaMax", VariableValue::Number(100.0)),
        ("municao", VariableValue::Number(12.0)),
        ("reserva", VariableValue::Number(72.0)),
        ("atacante", VariableValue::String(String::new())),
        ("ultimoDano", VariableValue::Number(0.0)),
        ("arma", VariableValue::String("Pistol".into())),
        ("agachado", VariableValue::Boolean(false)),
        ("emChamas", VariableValue::Boolean(false)),
    ]
}

/// Idempotent setup; existing same-name values (even incompatible user values) are never replaced.
/// Commands additionally report incompatible types before setup; runtime callers can use `variable_ids`
/// to publish only into correctly typed Game variables.
pub fn ensure_variables(doc: &mut Document) {
    let ci = match doc.variables.collections.iter().position(|c| c.name == "Game") {
        Some(index) => index,
        None => {
            let id = doc.new_collection_id();
            doc.variables.collections.push(Collection {
                modes: vec![Mode { id: format!("{id}/mode1"), name: "Default".into() }],
                id,
                name: "Game".into(),
                extends: None,
            });
            doc.variables.collections.len().saturating_sub(1)
        }
    };
    let Some(collection) = doc.variables.collections.get(ci).cloned() else { return };
    let modes = if collection.modes.is_empty() {
        let mode = Mode { id: format!("{}/mode1", collection.id), name: "Default".into() };
        if let Some(c) = doc.variables.collections.get_mut(ci) {
            c.modes.push(mode.clone());
        }
        vec![mode]
    } else {
        collection.modes
    };
    for (name, value) in defaults() {
        if doc.variables.variables.iter().any(|v| v.collection == collection.id && v.name == name) {
            continue;
        }
        let Some(kind) = value.type_of() else { continue };
        let id = doc.new_variable_id();
        doc.variables.variables.push(Variable {
            id,
            name: name.into(),
            collection: collection.id.clone(),
            kind,
            values: modes.iter().map(|m| (m.id.clone(), value.clone())).collect(),
            description: "Live game state, published in Play mode".into(),
            scopes: None,
            hidden_from_publishing: false,
            extends: None,
            overrides: BTreeSet::new(),
            code_syntax: BTreeMap::new(),
        });
    }
}

/// Canonical name → variable id, scoped to the Game collection and checked for the expected type.
pub fn variable_ids(doc: &Document) -> BTreeMap<String, String> {
    let Some(collection) = doc.variables.collections.iter().find(|c| c.name == "Game") else { return BTreeMap::new() };
    defaults()
        .into_iter()
        .filter_map(|(name, value)| {
            let kind = value.type_of()?;
            let mut matching = doc.variables.variables.iter().filter(|v| v.collection == collection.id && v.name == name);
            let v = matching.next()?;
            (v.kind == kind && matching.next().is_none()).then(|| (name.into(), v.id.clone()))
        })
        .collect()
}

fn check_variables(doc: &Document, cmd: &str) -> Result<()> {
    let mut collections = doc.variables.collections.iter().filter(|c| c.name == "Game");
    let Some(collection) = collections.next() else { return Ok(()) };
    if collections.next().is_some() || collection.extends.is_some() {
        return Err(bad(cmd, "Game must be one local variable collection"));
    }
    for (name, value) in defaults() {
        let mut vars = doc.variables.variables.iter().filter(|v| v.collection == collection.id && v.name == name);
        if let Some(v) = vars.next()
            && (Some(v.kind) != value.type_of() || vars.next().is_some())
        {
            return Err(bad(cmd, format!("Game/{name} has an incompatible type or duplicate name; rename it before setup")));
        }
    }
    Ok(())
}

fn setup(s: &mut Session, _: &Value) -> Result<Value> {
    check_variables(s.doc()?, "game.setup")?;
    ensure_variables(s.doc_mut()?);
    Ok(json!({"variables": variable_ids(s.doc()?)}))
}

fn keys(cmd: &str, value: &Value) -> Result<Vec<String>> {
    let values = value
        .as_array()
        .filter(|v| v.len() <= figcraft_doc::game::MAX_KEYS)
        .ok_or_else(|| bad(cmd, "keys must be an array of at most 8 input names"))?;
    let keys: Vec<String> = values
        .iter()
        .map(|v| v.as_str().filter(|s| s.len() <= 32).map(str::to_string).ok_or_else(|| bad(cmd, "keys must be short input names")))
        .collect::<Result<_>>()?;
    validate_keys(&keys).map_err(|e| bad(cmd, e))?;
    Ok(keys)
}

fn bind(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "game.bind";
    let mut settings = s.doc()?.game.clone();
    if let Some(reset) = p.get("reset") {
        let reset = reset.as_bool().ok_or_else(|| bad(cmd, "reset must be true or false"))?;
        if reset {
            if p.get("action").is_some() || p.get("keys").is_some() {
                return Err(bad(cmd, "reset cannot be combined with action or keys"));
            }
            settings.bindings = GameSettings::default().bindings;
            settings.menu_bindings = default_menu_bindings();
        } else {
            return Err(bad(cmd, "Use action and keys, or reset: true"));
        }
    } else {
        let action = req_string(cmd, p, "action")?;
        let value = p.get("keys").ok_or_else(|| bad(cmd, "keys is required; [] unbinds an action"))?;
        if ACTIONS.contains(&action.as_str()) {
            settings.bindings.insert(action, keys(cmd, value)?);
        } else if MENU_ACTIONS.contains(&action.as_str()) {
            settings.menu_bindings.insert(action, keys(cmd, value)?);
        } else {
            return Err(bad(cmd, "Unknown game action"));
        }
    }
    settings.validate().map_err(|e| bad(cmd, e))?;
    s.doc_mut()?.game = settings;
    ok()
}

fn opt_bool(cmd: &str, p: &Value, key: &str) -> Result<Option<bool>> {
    match p.get(key) {
        None => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| bad(cmd, format!("{key} must be true or false"))),
    }
}

fn menu_set(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "game.menu.set";
    let node = req_id(cmd, p, "node")?;
    let doc = s.doc()?;
    let frame = doc.node(node).ok_or_else(|| bad(cmd, "No such layer"))?;
    let mut settings = doc.game.clone();
    // Menus whose frame is gone are dropped on the way.
    settings.menus.retain(|id, _| doc.contains(*id));
    let enabled = opt_bool(cmd, p, "enabled")?;
    let wrap = opt_bool(cmd, p, "wrap")?;
    let remember = opt_bool(cmd, p, "remember")?;
    let disabled = match p.get("disabled") {
        None => None,
        Some(v) => Some(match v.as_str() {
            Some("focus") => DisabledItems::Focus,
            Some("skip") => DisabledItems::Skip,
            _ => return Err(bad(cmd, "disabled must be \"focus\" or \"skip\"")),
        }),
    };
    let start = match p.get("start") {
        None => None,
        Some(_) => Some(opt_id(cmd, p, "start")?),
    };
    if enabled == Some(false) {
        settings.menus.remove(&node);
    } else {
        if frame.layout.is_none() || !frame.kind.is_frame_like() {
            return Err(bad(cmd, "Navigable menus need an auto layout frame"));
        }
        if let Some(Some(child)) = start
            && !frame.children.contains(&child)
        {
            return Err(bad(cmd, "start must be a direct child of the menu"));
        }
        let menu = settings.menus.entry(node).or_insert_with(MenuSettings::default);
        if let Some(wrap) = wrap {
            menu.wrap = wrap;
        }
        if let Some(remember) = remember {
            menu.remember = remember;
        }
        if let Some(disabled) = disabled {
            menu.disabled = disabled;
        }
        if let Some(start) = start {
            menu.start = start;
        }
    }
    settings.validate().map_err(|e| bad(cmd, e))?;
    s.doc_mut()?.game = settings;
    ok()
}

fn rename_event(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "game.event.rename";
    let event = req_string(cmd, p, "event")?;
    if !EVENT_NAMES.contains(&event.as_str()) {
        return Err(bad(cmd, "Unknown canonical game event"));
    }
    let name = req_string(cmd, p, "name")?;
    let mut settings = s.doc()?.game.clone();
    settings.event_names.insert(event, name);
    settings.validate().map_err(|e| bad(cmd, e))?;
    s.doc_mut()?.game = settings;
    ok()
}

fn component_kind() -> NodeKind {
    NodeKind::Component {
        description: "Original white wireframe; edit border, padding and inset independently".into(),
        props: BTreeMap::new(),
        prop_order: Vec::new(),
    }
}

fn add(doc: &mut Document, parent: NodeId, kind: NodeKind, name: &str, box_: [f64; 4], fill: bool, border: bool) -> Result<NodeId> {
    let [x, y, width, height] = box_;
    let mut node = Node::new(doc.alloc_id(), kind, name, width, height);
    node.transform = Affine::translate((x, y));
    node.fills = if fill { vec![Paint::solid(Color::WHITE)] } else { Vec::new() };
    if border {
        node.strokes = vec![Paint::solid(Color::WHITE)];
        node.stroke.weight = 2.0;
    }
    Ok(doc.insert(node, parent, None)?)
}

fn text(doc: &mut Document, parent: NodeId, label: &str, x: f64, y: f64, size: f64) -> Result<NodeId> {
    let mut data = TextData::new(label);
    data.style.font_size = size;
    add(doc, parent, NodeKind::Text(data), label, [x, y, 440.0, size * 1.5], true, false)
}

fn wire(doc: &mut Document, parent: NodeId, name: &str, box_: [f64; 4]) -> Result<NodeId> {
    let id = add(doc, parent, component_kind(), name, box_, false, true)?;
    add(doc, id, NodeKind::Rectangle, "Editable white inset", [6.0, 6.0, (box_[2] - 12.0).max(1.0), (box_[3] - 12.0).max(1.0)], true, false)?;
    Ok(id)
}

/// A wireframe component set with `State=Default|Focused|Pressed|Disabled`: the inset turns
/// orange on focus, darker while pressed, and the item fades when disabled. Returns (set, Default).
fn state_set(doc: &mut Document, parent: NodeId, name: &str, at: (f64, f64), size: (f64, f64)) -> Result<(NodeId, NodeId)> {
    let (w, h) = size;
    let gap = 16.0;
    let kind = NodeKind::ComponentSet { description: "Navigable menu item: the State variant shows focus in Play".into() };
    let set = add(doc, parent, kind, name, [at.0, at.1, w * 4.0 + gap * 5.0, h + gap * 2.0], false, false)?;
    let focus = Color::from_hex("#ff5a00").unwrap_or(Color::WHITE);
    let pressed = Color::from_hex("#b83f00").unwrap_or(Color::WHITE);
    let mut default = None;
    for (k, (state, inset, opacity)) in
        [("Default", Color::WHITE, 1.0), ("Focused", focus, 1.0), ("Pressed", pressed, 1.0), ("Disabled", Color::WHITE, 0.4)].into_iter().enumerate()
    {
        let variant = wire(doc, set, &format!("State={state}"), [gap + k as f64 * (w + gap), gap, w, h])?;
        if let Some(inner) = doc.get(variant)?.children.first().copied() {
            doc.get_mut(inner)?.fills = vec![Paint::solid(inset)];
        }
        doc.get_mut(variant)?.opacity = opacity;
        default.get_or_insert(variant);
    }
    Ok((set, default.ok_or_else(|| bad("game.starterKit", "Missing default variant"))?))
}

/// A frame with `layout`, for a navigable menu.
fn menu_frame(doc: &mut Document, parent: NodeId, name: &str, box_: [f64; 4], layout: AutoLayout) -> Result<NodeId> {
    let id = add(doc, parent, NodeKind::Frame, name, box_, false, false)?;
    doc.get_mut(id)?.layout = Some(layout);
    Ok(id)
}

fn close_button(doc: &mut Document, parent: NodeId, label: &str, y: f64) -> Result<NodeId> {
    let id = wire(doc, parent, label, [520.0, y, 240.0, 48.0])?;
    let label = text(doc, id, label, 18.0, 12.0, 16.0)?;
    let node = doc.get_mut(label)?;
    node.width = 204.0;
    node.fills = vec![Paint::solid(Color::BLACK)];
    doc.get_mut(id)?.interactions.push(Interaction {
        trigger: Trigger::OnClick,
        actions: vec![Action::CloseOverlay],
        transition: Default::default(),
        duration_ms: 0,
        easing: Default::default(),
        delay_ms: 0,
    });
    Ok(id)
}

fn interaction(trigger: Trigger, actions: Vec<Action>, transition: Transition, duration_ms: u32) -> Interaction {
    Interaction { trigger, actions, transition, duration_ms, easing: Easing::EaseOut, delay_ms: 0 }
}

fn open_menu(dest: NodeId, close_on_outside: bool) -> Vec<Action> {
    vec![
        Action::OpenOverlay {
            dest,
            position: OverlayPosition::Center,
            close_on_outside,
            background: None,
            offset: None,
            anchor: OverlayAnchor::Screen,
        },
        Action::GamePause { paused: true },
        Action::GameCursor { captured: false },
    ]
}

/// The kit's screens are examples of what plain interactions can do: keys and controller buttons
/// open and close them, game events show and hide the death screen. Nothing here is special to
/// these frames; every interaction is editable in the Prototype tab.
fn example_interactions(doc: &mut Document, hud: NodeId, inventory: NodeId, pause: NodeId, map: NodeId, death: NodeId) -> Result<()> {
    let key = |key: &str| Trigger::KeyPress { key: key.into() };
    let event = |event: &str| Trigger::GameEvent { event: event.into() };
    let fade = Transition::Dissolve;
    let mut add_to = |id: NodeId, list: Vec<Interaction>| -> Result<()> {
        doc.get_mut(id)?.interactions.extend(list);
        Ok(())
    };
    let mut on_hud = Vec::new();
    for (screen, keys) in [(inventory, ["Tab", "Gamepad View"]), (pause, ["Escape", "Gamepad Menu"]), (map, ["M", "Gamepad Up"])] {
        for k in keys {
            on_hud.push(interaction(key(k), open_menu(screen, true), fade, 150));
        }
        add_to(screen, keys.iter().map(|k| interaction(key(k), vec![Action::CloseOverlay], fade, 150)).collect())?;
    }
    on_hud.push(interaction(event("vida-zero"), open_menu(death, false), fade, 400));
    add_to(hud, on_hud)?;
    add_to(death, vec![interaction(event("renasceu"), vec![Action::CloseOverlay], fade, 200)])
}

fn starter_kit(s: &mut Session, _: &Value) -> Result<Value> {
    let cmd = "game.starterKit";
    check_variables(s.doc()?, cmd)?;
    let page = s.page()?;
    let doc = s.doc_mut()?;
    ensure_variables(doc);
    let vars = variable_ids(doc);
    let mut frames = BTreeMap::new();
    for (index, (name, label)) in
        [("hud", "HUD"), ("inventory", "Inventory"), ("pause", "Pause"), ("map", "Map"), ("death", "Death")].into_iter().enumerate()
    {
        let frame = add(
            doc,
            page,
            NodeKind::Frame,
            &format!("Game / {label}"),
            [(index % 3) as f64 * 1360.0, (index / 3) as f64 * 800.0, 1280.0, 720.0],
            false,
            false,
        )?;
        frames.insert(name, frame);
    }
    let frame_id = |name: &str| frames.get(name).copied().ok_or_else(|| bad(cmd, "Missing starter frame"));
    let hud = frame_id("hud")?;
    let health = wire(doc, hud, "HUD / Health bar", [48.0, 620.0, 252.0, 32.0])?;
    if let Some(inner) = doc.get(health)?.children.first().copied() {
        let n = doc.get_mut(inner)?;
        n.width = 100.0;
        n.transform = Affine::new([2.4, 0.0, 0.0, 1.0, 6.0, 6.0]);
        if let Some(id) = vars.get("vida") {
            n.bound.insert("width".into(), id.clone());
        }
    }
    text(doc, hud, "HEALTH", 48.0, 582.0, 16.0)?;
    let health_text = text(doc, hud, "100", 320.0, 620.0, 24.0)?;
    if let Some(id) = vars.get("vida") {
        doc.get_mut(health_text)?.bound.insert("characters".into(), id.clone());
    }
    let ammo = wire(doc, hud, "HUD / Ammo bar", [1036.0, 620.0, 156.0, 32.0])?;
    if let Some(inner) = doc.get(ammo)?.children.first().copied() {
        let n = doc.get_mut(inner)?;
        n.width = 12.0;
        n.transform = Affine::new([12.0, 0.0, 0.0, 1.0, 6.0, 6.0]);
        if let Some(id) = vars.get("municao") {
            n.bound.insert("width".into(), id.clone());
        }
    }
    text(doc, hud, "AMMO", 1036.0, 582.0, 16.0)?;
    let ammo_text = text(doc, hud, "12", 1036.0, 660.0, 24.0)?;
    if let Some(id) = vars.get("municao") {
        doc.get_mut(ammo_text)?.bound.insert("characters".into(), id.clone());
    }
    let reserve = text(doc, hud, "72", 1136.0, 660.0, 24.0)?;
    if let Some(id) = vars.get("reserva") {
        doc.get_mut(reserve)?.bound.insert("characters".into(), id.clone());
    }
    add(doc, hud, NodeKind::Rectangle, "Reticle horizontal", [630.0, 359.0, 20.0, 2.0], true, false)?;
    add(doc, hud, NodeKind::Rectangle, "Reticle vertical", [639.0, 350.0, 2.0, 20.0], true, false)?;
    // The component library first: menus below are built from its State sets.
    let library = add(doc, page, NodeKind::Frame, "Wireframe / Components", [2720.0, 800.0, 1280.0, 800.0], false, false)?;
    text(doc, library, "WIREFRAME COMPONENTS", 48.0, 40.0, 32.0)?;
    text(doc, library, "White border / transparent padding / editable white inset. State shows focus in Play.", 48.0, 100.0, 18.0)?;
    let (button_set, button) = state_set(doc, library, "Button", (48.0, 150.0), (240.0, 48.0))?;
    let (tab_set, _) = state_set(doc, library, "Tab", (48.0, 250.0), (160.0, 36.0))?;
    let (cell_set, cell) = state_set(doc, library, "Grid cell", (48.0, 340.0), (112.0, 64.0))?;
    let mut menus = Vec::new();
    let inventory = frame_id("inventory")?;
    text(doc, inventory, "INVENTORY", 220.0, 112.0, 32.0)?;
    for (i, label) in ["Equipment", "Supplies", "Key items"].into_iter().enumerate() {
        wire(doc, inventory, &format!("Tabs / {label}"), [220.0 + i as f64 * 280.0, 180.0, 248.0, 36.0])?;
        text(doc, inventory, label, 220.0 + i as f64 * 280.0, 230.0, 16.0)?;
    }
    let grid_layout = |columns: u32, rows: u32, gap: f64, cross_gap: f64| AutoLayout {
        mode: LayoutMode::Grid,
        grid_columns: columns,
        grid_rows: rows,
        gap,
        cross_gap,
        padding: [0.0; 4],
        ..AutoLayout::default()
    };
    let slots = menu_frame(doc, inventory, "Grid / Slots", [220.0, 280.0, 812.0, 240.0], grid_layout(6, 3, 28.0, 24.0))?;
    for i in 0..18 {
        let slot = crate::components::create_instance(doc, cell, slots, 0.0, 0.0)?;
        doc.get_mut(slot)?.name = format!("Grid / Slot {}", i + 1);
    }
    menus.push(slots);
    close_button(doc, inventory, "Close inventory · Tab", 600.0)?;
    let pause = frame_id("pause")?;
    text(doc, pause, "PAUSED", 520.0, 160.0, 40.0)?;
    let vertical = |gap: f64, padding: f64| AutoLayout { mode: LayoutMode::Vertical, gap, padding: [padding; 4], ..AutoLayout::default() };
    let pause_menu = menu_frame(doc, pause, "Pause / Menu", [520.0, 280.0, 240.0, 120.0], vertical(24.0, 0.0))?;
    close_button(doc, pause_menu, "Resume", 0.0)?;
    close_button(doc, pause_menu, "Return to game", 0.0)?;
    menus.push(pause_menu);
    text(doc, pause, "Escape / controller Menu to resume", 520.0, 450.0, 16.0)?;
    let map = frame_id("map")?;
    text(doc, map, "ARENA MAP", 220.0, 96.0, 32.0)?;
    add(doc, map, NodeKind::Rectangle, "Map / Arena outline", [220.0, 176.0, 840.0, 380.0], false, true)?;
    for (i, [x, y, w, h]) in
        [[360.0, 256.0, 80.0, 80.0], [820.0, 256.0, 80.0, 80.0], [560.0, 336.0, 160.0, 48.0], [600.0, 480.0, 80.0, 12.0]].into_iter().enumerate()
    {
        wire(doc, map, &format!("Map / Landmark {}", i + 1), [x, y, w, h])?;
    }
    close_button(doc, map, "Close map · M", 600.0)?;
    let death = frame_id("death")?;
    text(doc, death, "SIGNAL LOST", 440.0, 220.0, 48.0)?;
    text(doc, death, "Enter / controller Y to respawn", 440.0, 336.0, 20.0)?;
    let attacker = text(doc, death, "Sentinel", 440.0, 400.0, 18.0)?;
    if let Some(id) = vars.get("atacante") {
        doc.get_mut(attacker)?.bound.insert("characters".into(), id.clone());
    }
    let mut components = vec![health, ammo];
    let list = add(doc, library, component_kind(), "List", [48.0, 460.0, 264.0, 252.0], false, true)?;
    doc.get_mut(list)?.layout = Some(vertical(12.0, 12.0));
    for i in 0..4 {
        let row = crate::components::create_instance(doc, button, list, 0.0, 0.0)?;
        doc.get_mut(row)?.name = format!("Row {}", i + 1);
    }
    components.push(list);
    let grid = add(doc, library, component_kind(), "Grid", [400.0, 460.0, 368.0, 144.0], false, false)?;
    doc.get_mut(grid)?.layout = Some(grid_layout(3, 2, 16.0, 16.0));
    for _ in 0..6 {
        crate::components::create_instance(doc, cell, grid, 0.0, 0.0)?;
    }
    components.push(grid);
    menus.extend([list, grid]);
    for menu in &menus {
        doc.game.menus.insert(*menu, MenuSettings::default());
    }
    example_interactions(doc, hud, inventory, pause, map, death)?;
    let sets = [button_set, tab_set, cell_set];
    let result = json!({
        "hud": hud, "inventory": inventory, "pause": pause, "map": map, "death": death,
        "components": components, "sets": sets, "menus": menus, "variables": vars
    });
    s.set_selection(vec![hud]);
    Ok(result)
}

/// In a prototype, "Play animation" actions an instance plays (its main's, or a layer's inside it)
/// aim at the instance's own copy of the layer, not at the main component's: a button component
/// that pulses its glow pulses the glow of the button that was hovered. Called on the play copy by
/// [`crate::proto_run::play_interactions`].
pub fn aim_animations_at_instances(doc: &mut Document) {
    let instances: Vec<(NodeId, NodeId)> =
        doc.nodes().filter_map(|n| if let NodeKind::Instance { component, .. } = &n.kind { Some((n.id, *component)) } else { None }).collect();
    for (instance, component) in instances {
        let layers = doc.subtree(instance);
        let mut map: BTreeMap<NodeId, NodeId> = layers.iter().filter_map(|id| Some((doc.node(*id)?.source?, *id))).collect();
        map.insert(component, instance);
        for id in layers {
            let Some(node) = doc.node_mut(id) else { continue };
            for it in &mut node.interactions {
                figcraft_doc::proto::walk_mut(&mut it.actions, &mut |action| {
                    if let Action::PlayAnimation { target, .. } = action
                        && let Some(own) = map.get(target)
                    {
                        *target = *own;
                    }
                });
            }
        }
    }
}
