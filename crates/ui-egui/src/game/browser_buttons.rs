//! G11.6: in the browser, mouse 4 and mouse 5 are the back and forward buttons. While a game is
//! played they are game buttons (the third-person game swaps the shoulder with them), so the page
//! must not go back or forward. A listener on the window cancels the browser's own action for those
//! two buttons until half a second after the last game frame; the editor keeps the browser's usual
//! behaviour. It only cancels the default action: egui still sees the press.
//!
//! Limitation: a browser may navigate on its own before the page sees the button (some mice send
//! back and forward as keys through their driver software); nothing in a page can stop that.

/// Call on every frame a game is played.
pub(crate) fn playing() {
    #[cfg(target_arch = "wasm32")]
    web::playing();
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::Cell;
    use wasm_bindgen::{JsCast as _, closure::Closure};

    /// How long after the last game frame the buttons stay the game's (milliseconds).
    const HOLD_MS: f64 = 500.0;
    /// The browser's numbers for mouse 4 (back) and mouse 5 (forward).
    const BACK: i16 = 3;
    const FORWARD: i16 = 4;

    thread_local! {
        static LAST_FRAME: Cell<f64> = const { Cell::new(f64::NEG_INFINITY) };
        static INSTALLED: Cell<bool> = const { Cell::new(false) };
    }

    fn now() -> Option<f64> {
        web_sys::window()?.performance().map(|performance| performance.now()).filter(|now| now.is_finite())
    }

    pub(super) fn playing() {
        let Some(time) = now() else { return };
        LAST_FRAME.with(|last| last.set(time));
        if INSTALLED.with(|installed| installed.replace(true)) {
            return;
        }
        let Some(window) = web_sys::window() else { return };
        let cancel = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|event: web_sys::MouseEvent| {
            let game = now().is_some_and(|now| now - LAST_FRAME.with(Cell::get) < HOLD_MS);
            if game && matches!(event.button(), BACK | FORWARD) {
                event.prevent_default();
            }
        });
        let options = web_sys::AddEventListenerOptions::new();
        options.set_capture(true);
        options.set_passive(false);
        // Browsers go back or forward on the release (Chrome, Edge, Firefox); the press and the
        // auxclick are cancelled too, for the ones that act earlier or later.
        for name in ["pointerdown", "mousedown", "pointerup", "mouseup", "auxclick"] {
            // A failure here only leaves the browser's own behaviour: nothing to report in the game.
            let _ = window.add_event_listener_with_callback_and_add_event_listener_options(name, cancel.as_ref().unchecked_ref(), &options);
        }
        // Lives as long as the page (installed once).
        cancel.forget();
    }
}
