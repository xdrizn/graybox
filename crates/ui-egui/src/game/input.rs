//! Bindings read the same egui input on desktop and web; simulation receives device-independent input.
use egui::{InputState, Key, PointerButton};
use figcraft_doc::game::GameSettings;
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
    }
}
