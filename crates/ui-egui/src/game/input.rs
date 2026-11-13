//! Bindings read the same egui input on desktop and web; simulation receives device-independent input.
use egui::{InputState, Key, PointerButton};
use figcraft_doc::game::{AIM_KEYS, GameSettings, SHOULDER_KEYS};
use figcraft_game::Input;

fn key(i: &InputState, name: &str, pressed: bool) -> bool {
    let button = match name {
        "MouseLeft" => Some(PointerButton::Primary),
        "MouseRight" => Some(PointerButton::Secondary),
        "MouseMiddle" => Some(PointerButton::Middle),
        "MouseExtra1" => Some(PointerButton::Extra1),
        "MouseExtra2" => Some(PointerButton::Extra2),
        _ => None,
    };
    if let Some(button) = button {
        return if pressed { i.pointer.button_pressed(button) } else { i.pointer.button_down(button) };
    }
    match name {
        "Shift" | "Control" | "Ctrl" | "Alt" => {
            let held = |m: egui::Modifiers| match name {
                "Shift" => m.shift,
                "Alt" => m.alt,
                _ => m.ctrl,
            };
            if pressed { i.events.iter().any(|e| matches!(e, egui::Event::ModifiersChanged(m) if held(*m))) } else { held(i.modifiers) }
        }
        _ => Key::from_name(name).is_some_and(|k| if pressed { i.key_pressed(k) } else { i.key_down(k) }),
    }
}
pub(crate) fn binding(i: &InputState, binding: &str, buttons: u32, pressed: bool) -> bool {
    if let Some(index) = figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|name| *name == binding) {
        buttons & (1 << index) != 0
    } else {
        key(i, binding, pressed)
    }
}

pub(crate) fn action(i: &InputState, settings: &GameSettings, action: &str, buttons: u32, pressed: bool) -> bool {
    settings.bindings.get(action).is_some_and(|bindings| bindings.iter().any(|name| binding(i, name, buttons, pressed)))
}

pub(crate) fn read(i: &InputState, settings: &GameSettings, held: u32, pressed: u32, captured: bool) -> Input {
    let down = |name| action(i, settings, name, held, false);
    let edge = |name| action(i, settings, name, pressed, true);
    let mut look = egui::Vec2::ZERO;
    let mut raw = false;
    for e in &i.events {
        if let egui::Event::MouseMoved(delta) = e {
            look += *delta;
            raw = true;
        }
    }
    if !raw {
        look = i.pointer.delta();
    }
    Input {
        forward: down("forward") as u8 as f32 - down("backward") as u8 as f32,
        strafe: down("right") as u8 as f32 - down("left") as u8 as f32,
        look: if captured { [look.x * 0.002, -look.y * 0.002] } else { [0.0, 0.0] },
        run: down("run"),
        crouch: down("crouch"),
        jump: edge("jump"),
        shoot: captured && down("shoot"),
        reload: edge("reload"),
        heal: edge("heal"),
        respawn: edge("respawn"),
        roll: edge_or(i, settings, "roll", ROLL_KEYS, pressed),
        weapon: WEAPON_KEYS.iter().position(|key| i.key_pressed(*key)).map_or(0, |index| index as u8 + 1),
        aim: captured && bound_or(i, settings, "aim", &AIM_KEYS, held, false),
        shoulder: captured && bound_or(i, settings, "shoulder", &SHOULDER_KEYS, pressed, true),
    }
}

/// Third person: roll with Ctrl or the controller's X when the file binds no "roll" of its own.
const ROLL_KEYS: &[&str] = &["Control", "Gamepad X"];
/// Third person: 1 sword, 2 spear, 3 club, 4 bow.
const WEAPON_KEYS: [Key; 4] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4];

/// A press of `name` as the file binds it, or of `defaults` for files made before the action existed.
fn edge_or(i: &InputState, settings: &GameSettings, name: &str, defaults: &[&str], buttons: u32) -> bool {
    if settings.bindings.contains_key(name) {
        action(i, settings, name, buttons, true)
    } else {
        defaults.iter().any(|key| binding(i, key, buttons, true))
    }
}

/// `name` held (or pressed) as the file binds it, or `defaults` for files made before the action
/// existed, less any key the file already gives another action or one of its own inputs.
fn bound_or(i: &InputState, settings: &GameSettings, name: &str, defaults: &[&str], buttons: u32, pressed: bool) -> bool {
    if settings.bindings.contains_key(name) {
        return action(i, settings, name, buttons, pressed);
    }
    free_defaults(settings, defaults).any(|key| binding(i, key, buttons, pressed))
}

/// The keys of `defaults` that the file does not already use for an action or an input.
fn free_defaults<'a>(settings: &'a GameSettings, defaults: &'a [&'a str]) -> impl Iterator<Item = &'a str> + 'a {
    let taken = move |key: &str| settings.bindings.values().chain(settings.inputs.values()).any(|keys| keys.iter().any(|k| k == key));
    defaults.iter().copied().filter(move |key| !taken(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_files_aim_on_the_default_keys_they_do_not_use_themselves() {
        let mut settings = GameSettings::default();
        settings.bindings.remove("aim");
        settings.bindings.remove("shoulder");
        assert_eq!(free_defaults(&settings, &AIM_KEYS).collect::<Vec<_>>(), AIM_KEYS);
        settings.inputs.insert("zoom".into(), vec!["MouseRight".into()]);
        settings.bindings.insert("heal".into(), vec!["MouseExtra1".into()]);
        assert_eq!(free_defaults(&settings, &AIM_KEYS).collect::<Vec<_>>(), ["Gamepad LT"]);
        assert_eq!(free_defaults(&settings, &SHOULDER_KEYS).collect::<Vec<_>>(), ["MouseExtra2", "Gamepad RS"]);
    }
}
