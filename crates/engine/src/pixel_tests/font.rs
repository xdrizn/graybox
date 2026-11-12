//! Pixel art (G16.6): pixel fonts drawn on a glyph sheet, and `pixel.paint` (a sprite from rows of
//! characters, how Claude draws).

use figcraft_doc::NodeId;
use serde_json::{Value, json};

use super::{ex, picture, refused, sprite};
use crate::Session;

fn id_of(v: &Value) -> u64 {
    v["id"].as_u64().unwrap()
}

fn width(s: &Session, id: u64) -> f64 {
    s.doc().unwrap().node(NodeId(id)).unwrap().width
}

/// Fonts are loaded for the whole process, so each test names its own.
#[test]
fn a_glyph_sheet_is_a_font_that_text_uses_and_follows_as_it_is_drawn() {
    let mut s = Session::new();
    let sheet = ex(&mut s, "pixel.font.sheet", json!({"name": "Sheet Test Mono", "cellWidth": 4, "cellHeight": 6, "chars": "AB", "scale": 1}));
    let id = id_of(&sheet);
    assert_eq!(sheet["name"], "Sheet Test Mono");
    // A and B, 3 pixels wide each, in cells 0 and 1 (x 0..4 and 4..8).
    ex(
        &mut s,
        "pixel.paint",
        json!({"id": id, "palette": {"#": "#ffffff"}, "rows": [
            ".#..##..",
            "#.#.#.#.",
            "###.##..",
            "#.#.#.#.",
            "#.#.##..",
        ]}),
    );
    let t = id_of(&ex(&mut s, "text.create", json!({"text": "AB", "x": 0, "y": 100})));
    ex(&mut s, "pixel.text", json!({"ids": [t], "font": "sheet test mono", "scale": 2}));
    let n = s.doc().unwrap().node(NodeId(t)).unwrap().clone();
    let figcraft_doc::NodeKind::Text(td) = &n.kind else { panic!("not text") };
    assert_eq!(td.style.font_family, "Sheet Test Mono", "the name as the font has it");
    assert_eq!(td.style.font_size, 16.0, "a 6-pixel cell is an 8-pixel em, at scale 2");
    // Each glyph is its painted columns plus 1 pixel of spacing: (3 + 1) + (3 + 1) = 8 art pixels.
    assert_eq!(width(&s, t), 16.0, "8 art pixels at scale 2");
    // Drawing on the sheet widens B (one more column): the text hugs the new glyph.
    ex(&mut s, "pixel.paint", json!({"id": id, "palette": {"#": "#ffffff"}, "rows": ["#"], "x": 7, "y": 2}));
    assert_eq!(width(&s, t), 18.0, "B is 4 wide now");
    assert!(refused(&mut s, "pixel.text", json!({"ids": [t], "font": "No Such Pixel Font"})).contains("No Such Pixel Font"));
}

#[test]
fn font_settings_change_how_the_sheet_is_read_and_names_stay_unique() {
    let mut s = Session::new();
    let a = id_of(&ex(&mut s, "pixel.font.sheet", json!({"name": "Unique Test Font"})));
    let b = ex(&mut s, "pixel.font.sheet", json!({"name": "unique test font"}));
    assert_eq!(b["name"], "unique test font 2", "a second sheet with the same name gets a number");
    let r = ex(&mut s, "pixel.font.set", json!({"id": a, "spacing": 2, "chars": "AB"}));
    assert_eq!((r["name"].as_str(), r["glyphs"].as_u64()), (Some("Unique Test Font"), Some(0)), "an empty sheet has no glyphs yet");
    // Any pixel layer can become a font, and stop being one.
    let (mut s2, plain) = sprite();
    ex(&mut s2, "pixel.paint", json!({"id": plain, "palette": {"x": "#000000"}, "rows": ["xx", "xx"]}));
    let r = ex(&mut s2, "pixel.font.set", json!({"id": plain, "name": "Plain Test Font", "cellWidth": 4, "cellHeight": 4, "chars": "ab"}));
    assert_eq!(r["glyphs"], 1, "only the first cell has pixels");
    assert_eq!(ex(&mut s2, "pixel.font.set", json!({"id": plain, "off": true}))["name"], Value::Null);
    assert!(refused(&mut s2, "pixel.font.set", json!({"id": plain, "name": "  "})).contains("name"));
    assert!(refused(&mut s2, "pixel.font.sheet", json!({"cellWidth": 128, "columns": 256})).contains("fit"));
}

#[test]
fn paint_draws_rows_in_one_undo_step_and_a_typo_changes_nothing() {
    let (mut s, id) = sprite();
    let r =
        ex(&mut s, "pixel.paint", json!({"id": id, "palette": {"r": "#c83447", "k": "#101010"}, "rows": [".rr.", "rkkr", " rr"], "x": 1, "y": 1}));
    assert_eq!(r["changed"], 8);
    assert_eq!(
        picture(&mut s, id),
        ["........", "..##....", ".####...", "..##....", "........", "........", "........", "........"],
        "`.` and space leave pixels alone"
    );
    let err = refused(&mut s, "pixel.paint", json!({"id": id, "palette": {"r": "#c83447"}, "rows": ["rq"]}));
    assert!(err.contains('q'), "names the unknown character: {err}");
    assert!(refused(&mut s, "pixel.paint", json!({"id": id, "palette": {"rr": "#c83447"}, "rows": ["r"]})).contains("palette"));
    // Clear empties the cel first; off-sprite pixels are skipped.
    ex(&mut s, "pixel.paint", json!({"id": id, "palette": {"r": "#c83447"}, "rows": ["r"], "x": 7, "y": 7, "clear": true}));
    assert_eq!(picture(&mut s, id)[7], ".......#");
    assert_eq!(picture(&mut s, id)[2], "........");
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(picture(&mut s, id)[2], ".####...", "one undo brings the rows back");
}

#[test]
fn a_shade_stroke_steps_colours_along_the_sprite_palette() {
    let (mut s, id) = sprite();
    ex(&mut s, "pixel.palette.set", json!({"id": id, "colors": ["#1e143c", "#5a3c8c", "#c8aaf0"]}));
    ex(&mut s, "pixel.paint", json!({"id": id, "palette": {"d": "#1e143c", "m": "#5a3c8c"}, "rows": ["dm"]}));
    let r = ex(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0], [2, 0]], "shade": 1}));
    assert_eq!(r["changed"], 2, "the empty pixel at x 2 stays empty");
    let px = ex(&mut s, "pixel.get", json!({"id": id, "pixels": true}));
    let row = &px["pictures"][0]["rows"][0];
    assert_eq!((row[0].as_str(), row[1].as_str(), row[2].as_str()), (Some("#5a3c8c"), Some("#c8aaf0"), Some(".")));
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "shade": "lighter"})).contains("shade"));
}

/// Review fix: two sheets of one font (a duplicated layer) used to take turns reloading it every
/// frame; and a pixel font must never take an installed family's name.
#[test]
fn a_copied_sheet_does_not_reload_the_font_every_frame_and_names_avoid_installed_fonts() {
    let mut s = Session::new();
    let a = id_of(&ex(&mut s, "pixel.font.sheet", json!({"name": "Copied Test Font", "cellWidth": 4, "cellHeight": 4, "chars": "A"})));
    let copy = ex(&mut s, "edit.duplicate", json!({"ids": [a]}));
    let b = copy["ids"][0].as_u64().unwrap();
    ex(&mut s, "pixel.paint", json!({"id": b, "palette": {"#": "#000000"}, "rows": ["##"]}));
    crate::cmd::pixel::font::refresh(s.doc().unwrap());
    assert_eq!(crate::cmd::pixel::font::refresh(s.doc().unwrap()), 0, "nothing changed: nothing reloads");
    // Renaming one sheet to the other's name keeps names unique.
    let r = ex(&mut s, "pixel.font.set", json!({"id": a, "name": "Renamed Test Font"}));
    let r2 = ex(&mut s, "pixel.font.set", json!({"id": b, "name": "renamed test font"}));
    assert_eq!((r["name"].as_str(), r2["name"].as_str()), (Some("Renamed Test Font"), Some("renamed test font 2")));
    if let Some(installed) = figcraft_text::family_names().into_iter().find(|f| !f.to_lowercase().contains("test")) {
        let r = ex(&mut s, "pixel.font.set", json!({"id": a, "name": installed}));
        assert_ne!(r["name"].as_str(), Some(installed.as_str()), "an installed family's name gets a number");
    }
    let (mut s, id) = sprite();
    assert!(refused(&mut s, "pixel.stroke", json!({"id": id, "points": [[0, 0]], "shade": 1, "erase": true})).contains("erase"));
}
