//! Which game Play runs under the file's screens (`GameSettings::kind`).

use figcraft_doc::game::GameKind;
use serde_json::{Value, json};

use super::CommandSpec;
use crate::{Result, Session, bad, spec};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "game.kind.set",
            "Set game",
            "{kind: fps|thirdPerson}  the game Play runs under the screens (older files play the first-person one) → {kind}",
            true,
            set_kind
        ),
        spec!(
            "game.crosshair.set",
            "Show game crosshair",
            "{visible: bool}  whether the third-person game draws its own crosshair in aim mode (hide it when your UI draws one from `mirando`) → {visible}",
            true,
            set_crosshair
        ),
    ]
}

fn set_kind(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "game.kind.set";
    let kind = p.get("kind").and_then(Value::as_str).and_then(GameKind::parse).ok_or_else(|| bad(cmd, "kind must be fps or thirdPerson"))?;
    s.doc()?;
    s.doc_mut()?.game.kind = kind;
    Ok(json!({"kind": kind.id()}))
}

fn set_crosshair(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "game.crosshair.set";
    let visible = p.get("visible").and_then(Value::as_bool).ok_or_else(|| bad(cmd, "visible must be true or false"))?;
    s.doc()?;
    s.doc_mut()?.game.hide_crosshair = !visible;
    Ok(json!({"visible": visible}))
}
