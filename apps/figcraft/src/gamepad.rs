//! Native controller input in the browser's 17-button standard layout.
//!
//! Linux/macOS use gilrs (Linux builds need pkg-config and libudev); Windows keeps
//! Windows.Gaming.Input. FreeBSD and other unsupported desktop targets return an empty sample
//! without linking a controller backend. Backends live on the UI thread, initialized only once.
//! Axes are left X/Y, right X/Y, positive Y down, with a rescaled 0.15 deadzone.

use figcraft_ui_egui::GamepadState;

/// The standard layout's buttons held on any controller (bit i: index i).
pub fn held() -> u32 {
    sample().buttons
}

/// Sample connected controllers, combining buttons but never mixing controllers' stick axes.
pub fn sample() -> GamepadState {
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        backend::sample()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        GamepadState::default()
    }
}

/// Reject bad driver values before clamping, and keep full stick travel after the deadzone.
fn axis(value: f32) -> f32 {
    const DEADZONE: f32 = 0.15;
    if !value.is_finite() || value.abs() <= DEADZONE {
        return 0.0;
    }
    value.signum() * ((value.abs().min(1.0) - DEADZONE) / (1.0 - DEADZONE))
}

fn playstation(name: &str) -> bool {
    // No lowercase String per frame: device names are short ASCII identifiers on these backends.
    ["playstation", "dualshock", "dualsense", "sony", "ps3", "ps4", "ps5"]
        .iter()
        .any(|needle| name.as_bytes().windows(needle.len()).any(|part| part.eq_ignore_ascii_case(needle.as_bytes())))
}

#[derive(Default)]
struct Samples {
    state: GamepadState,
    axis_activity: f32,
    selected_buttons: bool,
}

impl Samples {
    fn include(&mut self, buttons: u32, axes: [f32; 4], playstation: bool) {
        let activity = axes.iter().map(|value| value * value).sum::<f32>();
        // An idle first controller must not hide another controller's sticks. Prefer actual
        // stick activity, then button activity; ties keep the same enumeration order.
        if !self.state.connected || activity > self.axis_activity || (activity == self.axis_activity && buttons != 0 && !self.selected_buttons) {
            self.state.axes = axes;
            self.state.playstation = playstation;
            self.axis_activity = activity;
            self.selected_buttons = buttons != 0;
        }
        self.state.buttons |= buttons;
        self.state.connected = true;
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod backend {
    use std::cell::RefCell;

    use gilrs::{Axis, Button, Gilrs, GilrsBuilder};

    use super::{GamepadState, Samples, axis, playstation};

    // Order is the browser standard layout, NOT gilrs's enum discriminants. In gilrs the first
    // trigger is the shoulder, and Trigger2 is the analog trigger.
    const BUTTONS: [Button; 17] = [
        Button::South,
        Button::East,
        Button::West,
        Button::North,
        Button::LeftTrigger,
        Button::RightTrigger,
        Button::LeftTrigger2,
        Button::RightTrigger2,
        Button::Select,
        Button::Start,
        Button::LeftThumb,
        Button::RightThumb,
        Button::DPadUp,
        Button::DPadDown,
        Button::DPadLeft,
        Button::DPadRight,
        Button::Mode,
    ];

    thread_local! {
        // Cache failure too: missing input permissions/backend must not cause initialization
        // retries (or a log flood) every frame. A successfully initialized empty backend still
        // receives hotplug events when a controller is connected later.
        static INPUT: RefCell<Option<Gilrs>> = RefCell::new(
            GilrsBuilder::new()
                // Driver-dependent deadzones and jitter filtering would compound our 0.15
                // deadzone and can consume unbounded dropped events inside next_event(). Read
                // normalized state directly; d-pad axes are mapped explicitly below.
                .with_default_filters(false)
                .build()
                .inspect_err(|error| log::info!("controllers unavailable: {error}"))
                .ok()
        );
    }

    pub(super) fn sample() -> GamepadState {
        INPUT.with(|input| {
            let Ok(mut input) = input.try_borrow_mut() else { return GamepadState::default() };
            let Some(input) = input.as_mut() else { return GamepadState::default() };
            // Prevent a noisy device/event backlog from monopolizing the UI thread. gilrs updates
            // cached pressed/released/connected state on each event; remaining events wait for
            // the next frame. No independent held-state cache can retain a disconnected button.
            for _ in 0..256 {
                if input.next_event().is_none() {
                    break;
                }
            }
            let mut samples = Samples::default();
            for (_, pad) in input.gamepads() {
                let mut buttons = 0;
                for (index, button) in BUTTONS.into_iter().enumerate() {
                    let down = if index == 6 || index == 7 {
                        pad.button_data(button).is_some_and(|data| data.value() > 0.5)
                    } else {
                        pad.is_pressed(button)
                    };
                    if down {
                        buttons |= 1 << index;
                    }
                }
                buttons |= dpad(pad.value(Axis::DPadX), pad.value(Axis::DPadY));
                // gilrs normalizes both backends to positive Y up, unlike browser Gamepad axes.
                let axes = [
                    axis(pad.value(Axis::LeftStickX)),
                    axis(-pad.value(Axis::LeftStickY)),
                    axis(pad.value(Axis::RightStickX)),
                    axis(-pad.value(Axis::RightStickY)),
                ];
                samples.include(buttons, axes, playstation(pad.name()) || playstation(pad.os_name()));
            }
            input.inc();
            samples.state
        })
    }

    fn dpad(x: f32, y: f32) -> u32 {
        let mut buttons = 0;
        for (down, index) in [(y > 0.5, 12), (y < -0.5, 13), (x < -0.5, 14), (x > 0.5, 15)] {
            if down {
                buttons |= 1 << index;
            }
        }
        buttons
    }

    #[cfg(test)]
    mod tests {
        use super::dpad;

        #[test]
        fn maps_hat_diagonals_and_neutral_without_opposites() {
            assert_eq!(dpad(-1.0, 1.0), (1 << 12) | (1 << 14));
            assert_eq!(dpad(1.0, -1.0), (1 << 13) | (1 << 15));
            assert_eq!(dpad(0.0, 0.0), 0);
            assert_eq!(dpad(f32::NAN, f32::NAN), 0);
        }
    }
}

#[cfg(target_os = "windows")]
mod backend {
    use std::cell::RefCell;

    use windows::Gaming::Input::{Gamepad, GamepadButtons as B, RawGameController};

    use super::{GamepadState, Samples, axis, playstation};

    const BUTTONS: [(B, u32); 14] = [
        (B::A, 0),
        (B::B, 1),
        (B::X, 2),
        (B::Y, 3),
        (B::LeftShoulder, 4),
        (B::RightShoulder, 5),
        (B::View, 8),
        (B::Menu, 9),
        (B::LeftThumbstick, 10),
        (B::RightThumbstick, 11),
        (B::DPadUp, 12),
        (B::DPadDown, 13),
        (B::DPadLeft, 14),
        (B::DPadRight, 15),
    ];

    struct CachedPad {
        pad: Gamepad,
        playstation: bool,
        seen: bool,
    }

    thread_local! {
        // Only allocate/read the device name on hotplug, not every UI frame.
        static PADS: RefCell<Vec<CachedPad>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn sample() -> GamepadState {
        PADS.with(|cache| {
            let Ok(mut cache) = cache.try_borrow_mut() else { return GamepadState::default() };
            let Ok(pads) = Gamepad::Gamepads() else {
                cache.clear();
                return GamepadState::default();
            };
            for entry in cache.iter_mut() {
                entry.seen = false;
            }
            let mut samples = Samples::default();
            for pad in pads {
                let is_playstation = if let Some(entry) = cache.iter_mut().find(|entry| entry.pad == pad) {
                    entry.seen = true;
                    entry.playstation
                } else {
                    let is_playstation = RawGameController::FromGameController(&pad)
                        .and_then(|raw| raw.DisplayName())
                        .is_ok_and(|name| playstation(&name.to_string_lossy()));
                    cache.push(CachedPad { pad: pad.clone(), playstation: is_playstation, seen: true });
                    is_playstation
                };
                let Ok(reading) = pad.GetCurrentReading() else {
                    // The controller is connected even if this particular reading failed. Never
                    // retain previous buttons or axes when Windows cannot supply a fresh sample.
                    samples.include(0, [0.0; 4], is_playstation);
                    continue;
                };
                let mut buttons = 0;
                for (button, index) in BUTTONS {
                    if reading.Buttons.contains(button) {
                        buttons |= 1 << index;
                    }
                }
                if reading.LeftTrigger > 0.5 {
                    buttons |= 1 << 6;
                }
                if reading.RightTrigger > 0.5 {
                    buttons |= 1 << 7;
                }
                // Gaming.Input reserves Home for the OS and does not expose it; never assign an
                // unrelated button to standard bit 16. It remains available on gilrs/web.
                samples.include(
                    buttons,
                    [
                        axis(reading.LeftThumbstickX as f32),
                        axis(-reading.LeftThumbstickY as f32),
                        axis(reading.RightThumbstickX as f32),
                        axis(-reading.RightThumbstickY as f32),
                    ],
                    is_playstation,
                );
            }
            cache.retain(|entry| entry.seen);
            samples.state
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Samples, axis, playstation};

    #[test]
    fn deadzone_rescales_travel_and_rejects_invalid_values() {
        for value in [-0.15, -0.01, 0.0, 0.14, 0.15, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(axis(value), 0.0);
        }
        assert!((axis(0.575) - 0.5).abs() < 0.00001);
        assert!((axis(-0.575) + 0.5).abs() < 0.00001);
        assert_eq!(axis(1.0), 1.0);
        assert_eq!(axis(-2.0), -1.0);
        assert!(axis(0.151) > 0.0);
    }

    #[test]
    fn combines_buttons_but_takes_all_axes_and_layout_from_one_active_pad() {
        let mut samples = Samples::default();
        assert!(!samples.state.connected);
        samples.include(0, [0.0; 4], false);
        samples.include(1 << 0, [0.0, 0.0, 0.8, -0.5], true);
        samples.include(1 << 5, [-0.2, 0.3, -0.4, 0.0], false);
        assert!(samples.state.connected);
        assert_eq!(samples.state.buttons, (1 << 0) | (1 << 5));
        assert_eq!(samples.state.axes, [0.0, 0.0, 0.8, -0.5]);
        assert!(samples.state.playstation);
    }

    #[test]
    fn buttons_select_layout_when_all_sticks_are_idle() {
        let mut samples = Samples::default();
        samples.include(0, [0.0; 4], false);
        samples.include(1 << 9, [0.0; 4], true);
        assert!(samples.state.playstation);
        assert_eq!(samples.state.buttons, 1 << 9);
        assert_eq!(samples.state.axes, [0.0; 4]);
    }

    #[test]
    fn detects_known_playstation_names_without_calling_generic_wireless_pads_sony() {
        for name in [
            "Sony Computer Entertainment Wireless Controller",
            "PS4 Controller",
            "PLAYSTATION(R)3 Controller",
            "DualSense Wireless Controller",
            "DualShock 4",
        ] {
            assert!(playstation(name), "{name}");
        }
        for name in ["Xbox Wireless Controller", "Nintendo Switch Pro Controller", "Wireless Controller", ""] {
            assert!(!playstation(name), "{name}");
        }
    }
}
