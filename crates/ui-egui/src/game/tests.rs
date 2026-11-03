use super::Play;
use figcraft_game::STEP_SECS;

#[test]
fn the_first_frame_starts_the_clock_and_later_ones_advance_it() {
    let mut p = Play::new();
    assert_eq!(p.frame(100.0), 0, "no time has passed in the game yet");
    assert_eq!(p.frame(100.0 + STEP_SECS * 3.5), 3);
    assert_eq!(p.game().ticks(), 3);
}

#[test]
fn a_clock_going_backwards_or_junk_times_change_nothing() {
    let mut p = Play::new();
    p.frame(5.0);
    assert_eq!(p.frame(4.0), 0);
    assert_eq!(p.frame(f64::NAN), 0);
    // A junk time isn't kept: the next frame measures from the last good one.
    assert_eq!(p.frame(4.0 + STEP_SECS * 1.5), 1);
    assert_eq!(p.game().ticks(), 1);
}

/// Selecting an auto layout frame shows "Navigable menu" in the Prototype tab; its options run
/// `game.menu.set`.
#[test]
fn menu_section_toggle_runs_game_menu_set() {
    use crate::live_tests::{flat, has_text, text_center};
    use egui::{Event, Modifiers, PointerButton};
    let mut s = figcraft_engine::Session::new();
    let list = s.execute("frame.create", &serde_json::json!({"width": 200, "height": 200, "props": {"layout": "vertical"}})).unwrap();
    let list = figcraft_engine::params::id_of(&list["id"]).unwrap();
    s.execute("shape.rectangle", &serde_json::json!({"parent": list, "width": 200, "height": 40})).unwrap();
    s.set_selection(vec![list]);
    let mut u = crate::layers_drag_tests::Ui::new(s);
    u.app.ui.right_tab = crate::RightTab::Prototype;
    let click = |u: &mut crate::layers_drag_tests::Ui, text: &str| {
        let shapes = flat(&u.frame_output(vec![]).shapes);
        let p = text_center(&shapes, text);
        for pressed in [true, false] {
            u.frame(vec![
                Event::PointerMoved(p),
                Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
            ]);
        }
        u.frame(vec![]);
    };
    click(&mut u, "Navigable menu");
    assert!(u.app.session.doc().unwrap().game.menus.contains_key(&list));
    click(&mut u, "Loop at the end");
    assert!(u.app.session.doc().unwrap().game.menus[&list].wrap);
    click(&mut u, "Navigable menu");
    assert!(u.app.session.doc().unwrap().game.menus.is_empty());
    let shapes = flat(&u.frame_output(vec![]).shapes);
    assert!(!has_text(&shapes, "Loop at the end"), "options hide when the menu is off");
}
