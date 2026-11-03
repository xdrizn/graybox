//! Navigable menus in Play mode: focus, State variants, confirm, back and stale data.

use std::sync::Arc;

use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_engine::Session;
use figcraft_engine::components::variant_props;
use figcraft_engine::params::id_of;
use figcraft_game::menu::Dir;
use serde_json::{Value, json};

use super::{MenuInputs, menu_pointer, menu_step};
use crate::game::menu::{active_menu, layer_takes_input, read_menu};
use crate::present::Presenter;

fn ex(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, &params).unwrap()
}

fn idv(v: &Value) -> NodeId {
    id_of(&v["id"]).unwrap()
}

struct Scene {
    doc: Document,
    screen: NodeId,
    list: NodeId,
    items: Vec<NodeId>,
    popup: NodeId,
    popup_item: NodeId,
}

/// Screen with a vertical list of three Button instances (State set); the second opens Popup,
/// whose own one-item menu closes on its own Escape interaction.
fn scene() -> Scene {
    let mut s = Session::new();
    let variants: Vec<NodeId> = ["Default", "Focused", "Pressed", "Disabled"]
        .iter()
        .enumerate()
        .map(|(k, state)| {
            let f =
                idv(&ex(&mut s, "frame.create", json!({"x": 2000 + k * 150, "y": 0, "width": 100, "height": 40, "name": format!("State={state}")})));
            idv(&ex(&mut s, "component.create", json!({"ids": [f]})))
        })
        .collect();
    let set = idv(&ex(&mut s, "component.combineVariants", json!({"ids": variants})));
    let screen = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 0, "width": 1280, "height": 720, "name": "Screen"})));
    let list = idv(&ex(&mut s, "frame.create", json!({"parent": screen, "width": 200, "height": 200, "props": {"layout": "vertical", "gap": 8}})));
    let items: Vec<NodeId> = (0..3).map(|_| idv(&ex(&mut s, "instance.create", json!({"component": set, "parent": list})))).collect();
    let popup = idv(&ex(&mut s, "frame.create", json!({"x": 0, "y": 1000, "width": 400, "height": 300, "name": "Popup"})));
    let popup_list = idv(&ex(&mut s, "frame.create", json!({"parent": popup, "width": 200, "height": 100, "props": {"layout": "vertical"}})));
    let popup_item = idv(&ex(&mut s, "instance.create", json!({"component": set, "parent": popup_list})));
    ex(&mut s, "proto.interaction.add", json!({"id": items[1], "trigger": {"type": "onClick"}, "actions": [{"type": "openOverlay", "dest": popup}]}));
    ex(
        &mut s,
        "proto.interaction.add",
        json!({"id": popup, "trigger": {"type": "keyPress", "key": "Escape"}, "actions": [{"type": "closeOverlay"}]}),
    );
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": true}));
    ex(&mut s, "game.menu.set", json!({"node": popup_list, "enabled": true}));
    Scene { doc: s.doc().unwrap().clone(), screen, list, items, popup, popup_item }
}

fn play(doc: &Document, screen: NodeId) -> Presenter {
    let mut p = Presenter::new(screen);
    p.game = Some(crate::game::Play::new());
    p.sync(&Arc::new(doc.clone()), 0.0);
    p
}

fn idle() -> MenuInputs {
    MenuInputs::default()
}

fn held(dir: Dir) -> MenuInputs {
    MenuInputs { held: Some(dir), ..MenuInputs::default() }
}

fn confirm() -> MenuInputs {
    MenuInputs { confirm: true, confirm_held: true, ..MenuInputs::default() }
}

fn focused(p: &Presenter) -> Option<NodeId> {
    p.game.as_ref().and_then(|g| g.menu.item)
}

fn state(p: &Presenter, item: NodeId) -> String {
    let d = p.design.as_ref().unwrap();
    let NodeKind::Instance { component, .. } = &d.node(item).unwrap().kind else { panic!("not an instance") };
    variant_props(&d.node(*component).unwrap().name).get("State").cloned().unwrap_or_default()
}

#[test]
fn down_then_enter_focuses_second_item_and_runs_its_click() {
    let sc = scene();
    let mut p = play(&sc.doc, sc.screen);
    assert!(menu_step(&mut p, &idle(), 0.0));
    assert_eq!(focused(&p), Some(sc.items[0]));
    menu_step(&mut p, &held(Dir::Down), 0.1);
    menu_step(&mut p, &idle(), 0.2);
    assert_eq!(focused(&p), Some(sc.items[1]));
    menu_step(&mut p, &confirm(), 0.3);
    assert_eq!(p.overlays.iter().map(|o| o.id).collect::<Vec<_>>(), [sc.popup]);
}

#[test]
fn focus_swaps_state_variant_and_back_to_default() {
    let sc = scene();
    let mut p = play(&sc.doc, sc.screen);
    menu_step(&mut p, &idle(), 0.0);
    assert_eq!(state(&p, sc.items[0]), "Focused");
    assert_eq!(state(&p, sc.items[1]), "Default");
    menu_step(&mut p, &confirm(), 0.1);
    assert_eq!(state(&p, sc.items[0]), "Pressed");
    menu_step(&mut p, &idle(), 0.2);
    assert_eq!(state(&p, sc.items[0]), "Focused");
    menu_step(&mut p, &held(Dir::Down), 0.3);
    assert_eq!(state(&p, sc.items[0]), "Default");
    assert_eq!(state(&p, sc.items[1]), "Focused");
}

#[test]
fn overlay_close_returns_focus_to_previous_item() {
    let sc = scene();
    let mut p = play(&sc.doc, sc.screen);
    menu_step(&mut p, &idle(), 0.0);
    menu_step(&mut p, &held(Dir::Down), 0.1);
    menu_step(&mut p, &confirm(), 0.2);
    menu_step(&mut p, &idle(), 0.3);
    assert_eq!(focused(&p), Some(sc.popup_item));
    menu_step(&mut p, &MenuInputs { back: true, ..MenuInputs::default() }, 0.4);
    assert!(p.overlays.is_empty());
    menu_step(&mut p, &idle(), 0.5);
    assert_eq!(focused(&p), Some(sc.items[1]));
    assert_eq!(state(&p, sc.items[1]), "Focused");
}

#[test]
fn escape_with_own_close_interaction_closes_only_one_overlay() {
    let sc = scene();
    let mut p = play(&sc.doc, sc.screen);
    let doc = p.doc.clone().unwrap();
    assert!(layer_takes_input(&doc, sc.popup, "Escape"));
    assert!(!layer_takes_input(&doc, sc.popup, "Gamepad B"));
    assert!(!layer_takes_input(&doc, sc.screen, "Escape"));
    // The popup's Escape interaction handles Escape, so the menu's Back ignores it.
    let settings = &doc.game;
    let input = egui::InputState::default();
    let b = 1 << figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|n| *n == "Gamepad B").unwrap();
    let read = read_menu(&input, settings, b, b, [0.0; 2], &|name| layer_takes_input(&doc, sc.popup, name));
    assert!(read.back, "B is not used by the popup");
    let read = read_menu(&input, settings, b, b, [0.0; 2], &|name| name == "Gamepad B");
    assert!(!read.back, "an interaction on the same button wins");
    menu_step(&mut p, &idle(), 0.0);
}

#[test]
fn stick_counts_as_a_held_direction() {
    let sc = scene();
    let input = egui::InputState::default();
    let read = read_menu(&input, &sc.doc.game, 0, 0, [0.0, 0.8], &|_| false);
    assert_eq!(read.held, Some(Dir::Down));
    let read = read_menu(&input, &sc.doc.game, 0, 0, [-0.9, 0.1], &|_| false);
    assert_eq!(read.held, Some(Dir::Left));
    let read = read_menu(&input, &sc.doc.game, 0, 0, [0.2, f32::NAN], &|_| false);
    assert_eq!(read.held, None);
}

#[test]
fn removed_children_while_playing_do_not_panic() {
    let sc = scene();
    let mut p = play(&sc.doc, sc.screen);
    menu_step(&mut p, &idle(), 0.0);
    menu_step(&mut p, &held(Dir::Down), 0.1);
    menu_step(&mut p, &idle(), 0.15);
    menu_step(&mut p, &held(Dir::Down), 0.2);
    assert_eq!(focused(&p), Some(sc.items[2]));
    let mut edited = sc.doc.clone();
    for id in &sc.items[1..] {
        edited.remove(*id).unwrap();
    }
    p.sync(&Arc::new(edited.clone()), 1.0);
    menu_step(&mut p, &held(Dir::Up), 1.1);
    assert_eq!(focused(&p), Some(sc.items[0]));
    edited.node_mut(sc.list).unwrap().layout = None;
    p.sync(&Arc::new(edited), 2.0);
    assert!(!menu_step(&mut p, &held(Dir::Down), 2.1));
    assert_eq!(focused(&p), None);
}

#[test]
fn resting_pointer_does_not_steal_focus_from_keys() {
    let sc = scene();
    let mut p = play(&sc.doc, sc.screen);
    menu_step(&mut p, &idle(), 0.0);
    // The pointer marks an item; the next menu frame focuses it.
    menu_pointer(&mut p, Some(sc.items[2]), Some([10.0, 10.0]));
    menu_step(&mut p, &idle(), 0.05);
    assert_eq!(focused(&p), Some(sc.items[2]));
    menu_step(&mut p, &held(Dir::Up), 0.1);
    assert_eq!(focused(&p), Some(sc.items[1]));
    assert_eq!(state(&p, sc.items[2]), "Default");
    menu_pointer(&mut p, Some(sc.items[2]), Some([10.0, 10.0]));
    menu_step(&mut p, &idle(), 0.15);
    assert_eq!(focused(&p), Some(sc.items[1]), "the pointer did not move");
    // A layer inside the item counts as the item.
    menu_pointer(&mut p, Some(sc.items[0]), Some([10.0, 60.0]));
    menu_step(&mut p, &idle(), 0.2);
    assert_eq!(focused(&p), Some(sc.items[0]));
}

#[test]
fn item_without_state_gets_outline_and_still_confirms() {
    let mut s = Session::new();
    let screen = idv(&ex(&mut s, "frame.create", json!({"width": 1280, "height": 720})));
    let list = idv(&ex(&mut s, "frame.create", json!({"parent": screen, "width": 200, "height": 200, "props": {"layout": "vertical"}})));
    let item = idv(&ex(&mut s, "shape.rectangle", json!({"parent": list, "width": 200, "height": 40})));
    let popup = idv(&ex(&mut s, "frame.create", json!({"y": 1000, "width": 100, "height": 100})));
    ex(&mut s, "proto.interaction.add", json!({"id": item, "trigger": {"type": "onClick"}, "actions": [{"type": "openOverlay", "dest": popup}]}));
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": true}));
    let mut p = play(s.doc().unwrap(), screen);
    menu_step(&mut p, &idle(), 0.0);
    assert_eq!(focused(&p), Some(item));
    assert!(p.game.as_ref().unwrap().menu.outline);
    menu_step(&mut p, &confirm(), 0.1);
    assert_eq!(p.overlays.len(), 1);
}

#[test]
fn disabled_item_focuses_but_does_not_confirm() {
    let sc = scene();
    let mut doc = sc.doc.clone();
    // Item 1 (the one that opens the popup) becomes the Disabled variant.
    let NodeKind::Instance { component, .. } = &doc.node(sc.items[1]).unwrap().kind else { panic!() };
    let set = doc.node(*component).unwrap().parent.unwrap();
    let disabled = *doc.node(set).unwrap().children.iter().find(|c| doc.node(**c).unwrap().name == "State=Disabled").unwrap();
    if let NodeKind::Instance { component, .. } = &mut doc.node_mut(sc.items[1]).unwrap().kind {
        *component = disabled;
    }
    let mut p = play(&doc, sc.screen);
    menu_step(&mut p, &idle(), 0.0);
    menu_step(&mut p, &held(Dir::Down), 0.1);
    assert_eq!(focused(&p), Some(sc.items[1]));
    assert_eq!(state(&p, sc.items[1]), "Disabled", "a disabled item keeps its look");
    menu_step(&mut p, &confirm(), 0.2);
    assert!(p.overlays.is_empty());
}

#[test]
fn first_menu_in_layer_order_wins() {
    let mut s = Session::new();
    let screen = idv(&ex(&mut s, "frame.create", json!({"width": 1280, "height": 720})));
    let first = idv(&ex(&mut s, "frame.create", json!({"parent": screen, "width": 100, "height": 100, "props": {"layout": "vertical"}})));
    let second = idv(&ex(&mut s, "frame.create", json!({"parent": screen, "width": 100, "height": 100, "props": {"layout": "grid"}})));
    ex(&mut s, "game.menu.set", json!({"node": second, "enabled": true}));
    ex(&mut s, "game.menu.set", json!({"node": first, "enabled": true}));
    assert_eq!(active_menu(s.doc().unwrap(), screen), Some(first));
}

#[test]
fn no_menu_means_no_menu_input() {
    let mut s = Session::new();
    let screen = idv(&ex(&mut s, "frame.create", json!({"width": 1280, "height": 720})));
    let mut p = play(s.doc().unwrap(), screen);
    assert!(!menu_step(&mut p, &held(Dir::Down), 0.0));
    assert_eq!(focused(&p), None);
}

#[test]
fn a_tap_pressed_and_released_in_one_frame_still_moves() {
    let sc = scene();
    let ctx = egui::Context::default();
    let key = egui::Key::ArrowDown;
    let events = [true, false]
        .map(|pressed| egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: egui::Modifiers::NONE })
        .to_vec();
    ctx.begin_pass(egui::RawInput { events, ..Default::default() });
    let read = ctx.input(|i| read_menu(i, &sc.doc.game, 0, 0, [0.0; 2], &|_| false));
    ctx.end_pass().textures_delta.clear();
    assert_eq!(read.held, Some(Dir::Down));
}

#[test]
fn confirm_runs_on_press_when_the_item_has_no_click() {
    let mut s = Session::new();
    let screen = idv(&ex(&mut s, "frame.create", json!({"width": 1280, "height": 720})));
    let list = idv(&ex(&mut s, "frame.create", json!({"parent": screen, "width": 200, "height": 200, "props": {"layout": "vertical"}})));
    let item = idv(&ex(&mut s, "shape.rectangle", json!({"parent": list, "width": 200, "height": 40})));
    let popup = idv(&ex(&mut s, "frame.create", json!({"y": 1000, "width": 100, "height": 100})));
    ex(&mut s, "proto.interaction.add", json!({"id": item, "trigger": {"type": "onPress"}, "actions": [{"type": "openOverlay", "dest": popup}]}));
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": true}));
    let mut p = play(s.doc().unwrap(), screen);
    menu_step(&mut p, &idle(), 0.0);
    menu_step(&mut p, &confirm(), 0.1);
    assert_eq!(p.overlays.len(), 1);
}

/// A vertical layout that wraps into columns navigates like a grid: ↑↓ inside a column, ←→ across.
#[test]
fn wrapping_vertical_layout_navigates_like_a_grid() {
    let mut s = Session::new();
    let screen = idv(&ex(&mut s, "frame.create", json!({"width": 1280, "height": 720})));
    let list = idv(&ex(
        &mut s,
        "frame.create",
        json!({"parent": screen, "width": 300, "height": 100, "props": {"layout": "vertical", "wrap": true, "gap": 0, "padding": 0}}),
    ));
    let items: Vec<NodeId> = (0..4).map(|_| idv(&ex(&mut s, "shape.rectangle", json!({"parent": list, "width": 100, "height": 40})))).collect();
    ex(&mut s, "game.menu.set", json!({"node": list, "enabled": true}));
    let mut p = play(s.doc().unwrap(), screen);
    menu_step(&mut p, &idle(), 0.0);
    let press = |p: &mut Presenter, dir: Dir, t: f64| {
        menu_step(p, &held(dir), t);
        menu_step(p, &idle(), t + 0.05);
    };
    press(&mut p, Dir::Down, 0.1);
    assert_eq!(focused(&p), Some(items[1]));
    press(&mut p, Dir::Down, 0.2);
    assert_eq!(focused(&p), Some(items[1]), "the column ends here");
    press(&mut p, Dir::Right, 0.3);
    assert_eq!(focused(&p), Some(items[3]));
    press(&mut p, Dir::Up, 0.4);
    assert_eq!(focused(&p), Some(items[2]));
    press(&mut p, Dir::Left, 0.5);
    assert_eq!(focused(&p), Some(items[0]));
}

#[test]
fn an_open_menu_stops_the_player() {
    let mut play = crate::game::Play::new();
    play.captured = true;
    play.game.input.forward = 1.0;
    play.game.input.strafe = -1.0;
    play.game.input.jump = true;
    super::stop_moving(&mut play, false);
    assert_eq!(play.game.input.forward, 1.0, "no menu, captured: the player moves");
    super::stop_moving(&mut play, true);
    assert_eq!((play.game.input.forward, play.game.input.strafe, play.game.input.jump), (0.0, 0.0, false));
}
