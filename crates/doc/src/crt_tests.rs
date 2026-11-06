//! The CRT effect's values: defaults, names in files, sanitising, its type name and backdrop reach.

use crate::*;
use serde_json::json;

#[test]
fn defaults_match_the_spec() {
    let c = Crt::default();
    assert_eq!((c.curvature, c.vignette, c.line_size, c.thickness), (0.3, 0.4, 4.0, 0.5));
    assert_eq!((c.line_opacity, c.bleed, c.direction), (0.5, 0.5, CrtDirection::Horizontal));
    assert_eq!((c.rgb_split, c.phosphor, c.glow, c.brightness), (0.2, 0.2, 0.3, 1.2));
}

#[test]
fn round_trips_and_reads_camel_case() {
    let e: Effect = serde_json::from_value(json!({"type": "crt", "lineSize": 8, "rgbSplit": 0.5, "direction": "vertical"})).unwrap();
    let EffectKind::Crt(c) = &e.kind else { panic!("{e:?}") };
    assert_eq!((c.line_size, c.rgb_split, c.direction), (8.0, 0.5, CrtDirection::Vertical));
    assert_eq!(c.curvature, Crt::default().curvature);
    let back: Effect = serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
    assert_eq!(back, e);
}

#[test]
fn sanitized_clamps_junk() {
    let c = Crt { curvature: f64::NAN, line_size: -3.0, brightness: 9.0, glow: f64::INFINITY, ..Crt::default() }.sanitized();
    assert_eq!((c.curvature, c.line_size, c.brightness, c.glow), (0.3, 1.0, 2.0, 0.3));
    let c = Crt { line_size: 500.0, vignette: -1.0, brightness: 0.0, ..Crt::default() }.sanitized();
    assert_eq!((c.line_size, c.vignette, c.brightness), (64.0, 0.0, 0.5));
}

#[test]
fn of_type_and_label() {
    for name in ["CRT", "crt"] {
        let e = Effect::of_type(name).unwrap();
        assert!(matches!(e.kind, EffectKind::Crt(_)));
        assert_eq!(e.label(), "CRT");
    }
    assert_eq!(Effect::TYPES.last(), Some(&("CRT", "crt")));
    assert_eq!(Effect::crt(), Effect::of_type("crt").unwrap());
}

#[test]
fn hidden_crt_reads_no_backdrop() {
    let mut e = Effect::crt();
    assert!(e.backdrop_radius() > 0.0);
    e.visible = false;
    assert_eq!(e.backdrop_radius(), 0.0);
}

#[test]
fn junk_json_is_refused_not_fatal() {
    assert!(serde_json::from_value::<Effect>(json!({"type": "crt", "lineSize": "wide"})).is_err());
    assert!(serde_json::from_value::<Effect>(json!({"type": "crt", "direction": 3})).is_err());
}
