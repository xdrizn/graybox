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

/// Rename fields (file name, frame titles) edit their text on the same line as the name they
/// replace: the edited text is centred in the row like `painter.text(.., LEFT_CENTER, ..)`.
#[test]
fn rename_text_stays_centred_on_its_row() {
    use egui::{Pos2, RawInput, Rect, vec2};
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx, true, 0);
    crate::theme::apply(&ctx, crate::theme::Tokens::dark());
    let font = crate::theme::semibold(13.0);
    let rect = Rect::from_min_size(Pos2::new(14.0, 55.0), vec2(180.0, 20.0));
    let id = egui::Id::unique("rename_centre_test");
    let mut centres = Vec::new();
    let mut editing = false;
    for frame in 0..3 {
        let mut out = ctx.run_ui(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))), ..Default::default() }, |ui| {
            crate::widgets::rename_field(ui, id, "game-starter", rect, font.clone(), frame == 0);
            editing |= ui.memory(|m| m.has_focus(id.with("te")));
        });
        out.textures_delta.clear();
        centres = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "game-starter" => Some(t.pos.y + t.galley.rect.center().y),
                _ => None,
            })
            .collect();
    }
    let static_galley = ctx.fonts_mut(|f| f.layout_no_wrap("game-starter".into(), font.clone(), egui::Color32::WHITE));
    let expected = rect.center().y - static_galley.size().y / 2.0 + static_galley.rect.center().y;
    assert!(editing, "the field is being edited");
    assert!(!centres.is_empty(), "the field draws its text");
    for c in centres {
        assert!((c - expected).abs() < 0.75, "edited text centre {c} vs name centre {expected}");
    }
}

/// A component set shows its properties compactly; a variant shows only its own values, and
/// picking another value moves the variant to it (Figma-like "Current variant").
#[test]
fn variant_shows_and_changes_only_its_own_value() {
    use crate::live_tests::{flat, has_text, text_center};
    use egui::{Event, Modifiers, PointerButton};
    use serde_json::json;
    crate::game::components::set_compact(true);
    let mut s = figcraft_engine::Session::new();
    let ex = |s: &mut figcraft_engine::Session, c: &str, p: serde_json::Value| s.execute(c, &p).unwrap();
    let id = |v: &serde_json::Value| figcraft_engine::params::id_of(&v["id"]).unwrap();
    let frame = id(&ex(&mut s, "frame.create", json!({"width": 100, "height": 40})));
    let main = id(&ex(&mut s, "component.create", json!({"ids": [frame]})));
    ex(&mut s, "component.addVariant", json!({"id": main}));
    let set = figcraft_engine::components::variant_set(s.doc().unwrap(), main).unwrap();
    let variants = s.doc().unwrap().node(set).unwrap().children.clone();
    let second = *variants.last().unwrap();
    s.set_selection(vec![set]);
    let mut u = crate::layers_drag_tests::Ui::new(s);
    u.app.ui.right_tab = crate::RightTab::Design;
    let shapes = flat(&u.frame_output(vec![]).shapes);
    assert!(has_text(&shapes, "Properties") && has_text(&shapes, "Add variant"));
    assert!(!has_text(&shapes, "Current variant"));
    u.app.session.set_selection(vec![second]);
    let shapes = flat(&u.frame_output(vec![]).shapes);
    assert!(has_text(&shapes, "Current variant"));
    assert!(!has_text(&shapes, "Add variant"), "a variant doesn't edit the set's definitions");
    let name = u.app.session.doc().unwrap().node(second).unwrap().name.clone();
    let mine = figcraft_engine::components::variant_props(&name);
    let (prop, value) = mine.iter().next().map(|(k, v)| (k.clone(), v.clone())).unwrap();
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
    // The dropdown shows this variant's value; the menu offers the other one.
    click(&mut u, &value);
    let first_name = u.app.session.doc().unwrap().node(variants[0]).unwrap().name.clone();
    let other = figcraft_engine::components::variant_props(&first_name).get(&prop).cloned().unwrap();
    let shapes = flat(&u.frame_output(vec![]).shapes);
    assert!(has_text(&shapes, &other));
    click(&mut u, &other);
    let renamed = u.app.session.doc().unwrap().node(second).unwrap().name.clone();
    assert_eq!(figcraft_engine::components::variant_props(&renamed).get(&prop), Some(&other));
}
