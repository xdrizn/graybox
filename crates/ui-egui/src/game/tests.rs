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
    // The field shows this variant's value; typing renames it, for this variant only.
    click(&mut u, &value);
    u.frame(vec![Event::Text("Hover".into())]);
    u.frame(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    u.frame(vec![]);
    let renamed = u.app.session.doc().unwrap().node(second).unwrap().name.clone();
    assert_eq!(figcraft_engine::components::variant_props(&renamed).get(&prop).map(String::as_str), Some("Hover"));
    let first_name = u.app.session.doc().unwrap().node(variants[0]).unwrap().name.clone();
    assert_ne!(
        figcraft_engine::components::variant_props(&first_name).get(&prop).map(String::as_str),
        Some("Hover"),
        "the other variant keeps its value"
    );
}

/// G9: a padding typed as 0 applies at once, whichever way the edit ends (Enter, or a click on
/// the canvas), on a frame and on an instance.
#[test]
fn padding_typed_as_zero_applies() {
    use egui::{Event, Key, Modifiers};
    use serde_json::json;
    let field = |u: &mut crate::layers_drag_tests::Ui, label: &str| {
        u.ctx.enable_accesskit();
        u.frame(vec![]);
        let out = u.frame_output(vec![]);
        let nodes = out.platform_output.accesskit_update.expect("an AccessKit update").nodes;
        let n = nodes.iter().map(|(_, n)| n).find(|n| n.role() != egui::accesskit::Role::Button && n.label() == Some(label)).expect(label);
        let b = n.bounds().expect("bounds");
        egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    };
    let enter = vec![
        Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE },
        Event::Key { key: Key::Enter, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE },
    ];
    for how in ["enter", "select-all", "backspace"] {
        let mut s = figcraft_engine::Session::new();
        let f = s
            .execute("frame.create", &json!({"x": 0, "y": 0, "width": 300, "height": 200, "props": {"layout": "vertical", "padding": 10}}))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        s.execute("select.set", &json!({"ids": [f]})).unwrap();
        let mut u = crate::layers_drag_tests::Ui::new(s);
        let pad =
            |u: &crate::layers_drag_tests::Ui| u.app.session.doc().unwrap().node(figcraft_doc::NodeId(f)).unwrap().layout.clone().unwrap().padding;
        let v = field(&mut u, "Vertical padding");
        u.button(v, true);
        u.button(v, false);
        u.frame(vec![]);
        match how {
            "backspace" => {
                let bs = Event::Key { key: Key::Backspace, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
                u.frame(vec![bs]);
                u.frame(vec![Event::Text("0".into())]);
            }
            _ => u.frame(vec![Event::Text("0".into())]),
        }
        u.frame(enter.clone());
        u.frame(vec![]);
        assert_eq!(pad(&u), [0.0, 10.0, 0.0, 10.0], "{how}");
    }
}

/// G9: Selection colors folds past three colours (a preview and "+N"), and the header opens it.
#[test]
fn selection_colors_fold_past_three_and_open_on_click() {
    use serde_json::json;
    let mut s = figcraft_engine::Session::new();
    let frame = s.execute("frame.create", &json!({"width": 300, "height": 100, "props": {"fill": "none"}})).unwrap()["id"].as_u64().unwrap();
    for c in ["#ff0000", "#00ff00", "#0000ff", "#ffff00", "#00ffff"] {
        s.execute("shape.rectangle", &json!({"parent": frame, "width": 10, "height": 10, "props": {"fill": c}})).unwrap();
    }
    s.execute("select.set", &json!({"ids": [frame]})).unwrap();
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx, false, 0);
    let mut app = crate::FigApp::new(s, Box::new(crate::NoServices));
    let mut time = 0.0;
    let mut frame_texts = |app: &mut crate::FigApp, events: Vec<egui::Event>| {
        time += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(280.0, 600.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let ds = app.session.ds().unwrap();
            let (doc, ids) = (ds.doc.clone(), ds.selection.clone());
            crate::selection_colors::section(app, ui, &doc, &ids);
        });
        out.textures_delta.clear();
        crate::live_tests::texts(&crate::live_tests::flat(&out.shapes)).into_iter().map(|(t, _, _)| t).collect::<Vec<_>>()
    };
    let folded = frame_texts(&mut app, vec![]);
    assert!(folded.iter().any(|t| t == "+2"), "five colours fold to three swatches and +2: {folded:?}");
    assert!(!folded.iter().any(|t| t == "FF0000"), "no rows while folded");
    let p = egui::pos2(60.0, 20.0);
    for pressed in [true, false] {
        frame_texts(
            &mut app,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() },
            ],
        );
    }
    let open = frame_texts(&mut app, vec![]);
    assert!(open.iter().any(|t| t == "FF0000") && !open.iter().any(|t| t == "+2"), "the header opens the list: {open:?}");
    assert!(super::color_fold::opens_by_default(3) && !super::color_fold::opens_by_default(4));
}

/// G9: Alt and a drag anywhere on a number field's value changes it at the normal rate (one a
/// point); without Alt the same drag changes nothing, and a plain click still starts typing.
#[test]
fn alt_drag_on_a_number_field_scrubs_it() {
    use egui::{Event, Modifiers, PointerButton};
    use serde_json::json;
    let mut s = figcraft_engine::Session::new();
    let f = s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 300, "height": 200, "props": {"layout": "vertical", "padding": 10}})).unwrap()
        ["id"]
        .as_u64()
        .unwrap();
    s.execute("select.set", &json!({"ids": [f]})).unwrap();
    let mut u = crate::layers_drag_tests::Ui::new(s);
    let pad =
        |u: &crate::layers_drag_tests::Ui| u.app.session.doc().unwrap().node(figcraft_doc::NodeId(f)).unwrap().layout.clone().unwrap().padding[0];
    u.ctx.enable_accesskit();
    u.frame(vec![]);
    let out = u.frame_output(vec![]);
    let nodes = out.platform_output.accesskit_update.expect("an AccessKit update").nodes;
    let n = nodes.iter().map(|(_, n)| n).find(|n| n.role() != egui::accesskit::Role::Button && n.label() == Some("Vertical padding")).unwrap();
    let b = n.bounds().unwrap();
    // The middle of the value, well right of the label icon.
    let at = egui::pos2(((b.x0 + b.x1) / 2.0) as f32 + 10.0, ((b.y0 + b.y1) / 2.0) as f32);
    let drag = |u: &mut crate::layers_drag_tests::Ui, mods: Modifiers| {
        let button = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: mods };
        u.frame_with(vec![Event::PointerMoved(at), button(at, true)], mods);
        for i in 1..=4 {
            u.frame_with(vec![Event::PointerMoved(at + egui::vec2(5.0 * i as f32, 0.0))], mods);
        }
        let end = at + egui::vec2(20.0, 0.0);
        u.frame_with(vec![button(end, false)], mods);
        u.frame(vec![]);
    };
    drag(&mut u, Modifiers::NONE);
    assert_eq!(pad(&u), 10.0, "no Alt: a drag on the value changes nothing");
    drag(&mut u, Modifiers::ALT);
    assert_eq!(pad(&u), 30.0, "Alt: 20 points right is +20");
}

/// The CRT effect's settings: every field shows, a typed line size and a picked direction land on
/// the effect.
#[test]
fn crt_effect_fields_edit_the_effect() {
    use crate::live_tests::{flat, text_center};
    use egui::accesskit::Role;
    use egui::{Event, Id, Key, Modifiers, PointerButton, pos2};
    use figcraft_doc::{CrtDirection, EffectKind};
    let mut s = figcraft_engine::Session::new();
    let r = s.execute("shape.rectangle", &serde_json::json!({"x": 0, "y": 0, "width": 200, "height": 120})).unwrap();
    let r = figcraft_engine::params::id_of(&r["id"]).unwrap();
    s.execute("effect.add", &serde_json::json!({"ids": [r], "type": "crt"})).unwrap();
    s.set_selection(vec![r]);
    let mut u = crate::layers_drag_tests::Ui::new(s);
    u.ctx.enable_accesskit();
    u.ctx.data_mut(|d| d.insert_temp(Id::unique(("fx_open", 0usize)), true));
    let press = |u: &mut crate::layers_drag_tests::Ui, p: egui::Pos2| {
        for pressed in [true, false] {
            u.frame(vec![
                Event::PointerMoved(p),
                Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
            ]);
        }
        u.frame(vec![]);
    };
    let field = |u: &mut crate::layers_drag_tests::Ui, name: &str| {
        u.frame(vec![]);
        let out = u.frame_output(vec![]);
        let nodes = out.platform_output.accesskit_update.unwrap().nodes;
        let n = nodes.iter().map(|(_, n)| n).find(|n| n.role() == Role::SpinButton && n.label() == Some(name)).map(|n| n.bounds().unwrap());
        n.map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
    };
    for name in ["Curvature", "Vignette", "Line size", "Thickness", "Line opacity", "Bleed", "RGB split", "Phosphor", "Glow", "Brightness"] {
        assert!(field(&mut u, name).is_some(), "no field {name:?}");
    }
    let p = field(&mut u, "Line size").unwrap();
    press(&mut u, p);
    u.frame(vec![Event::Text("8".into())]);
    u.frame(vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    u.frame(vec![]);
    let shapes = flat(&u.frame_output(vec![]).shapes);
    let p = text_center(&shapes, "Horizontal");
    press(&mut u, p);
    let shapes = flat(&u.frame_output(vec![]).shapes);
    let p = text_center(&shapes, "Vertical");
    press(&mut u, p);
    let doc = u.app.session.doc().unwrap();
    let Some(EffectKind::Crt(c)) = doc.node(r).and_then(|n| n.effects.first()).map(|e| &e.kind) else { panic!("no CRT") };
    assert_eq!((c.line_size, c.direction), (8.0, CrtDirection::Vertical));
}
