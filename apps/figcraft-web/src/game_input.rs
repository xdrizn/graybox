//! Browser-only game input. Restricted APIs run synchronously in the trusted DOM gesture,
//! not in egui's later animation frame. Menus never recapture a pointer on their UI clicks.

use figcraft_ui_egui::game;
use std::cell::Cell;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CAPTURE_ALLOWED: Cell<bool> = const { Cell::new(false) };
    static MOUSE: Cell<[f32; 2]> = const { Cell::new([0.0; 2]) };
}

pub fn enter() {
    ACTIVE.set(true);
    CAPTURE_ALLOWED.set(true);
}

pub fn leave() {
    ACTIVE.set(false);
    CAPTURE_ALLOWED.set(false);
    MOUSE.set([0.0; 2]);
    if let Some(document) = document() {
        document.exit_pointer_lock();
        document.exit_fullscreen();
    }
}

pub fn capture_allowed(allowed: bool) {
    CAPTURE_ALLOWED.set(allowed);
}

pub fn cursor(captured: bool) {
    if !captured && let Some(document) = document() {
        document.exit_pointer_lock();
    }
    // Capture is fulfilled by the next eligible trusted gesture, or was already fulfilled by
    // the Play gesture. Calling requestPointerLock here rejects in browser implementations.
}

pub fn locked() -> bool {
    document().and_then(|d| d.pointer_lock_element()).is_some()
}

pub fn mouse_delta() -> [f32; 2] {
    MOUSE.replace([0.0; 2])
}

fn document() -> Option<web_sys::Document> {
    web_sys::window().and_then(|w| w.document())
}

fn report(ctx: &egui::Context, error: String) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(game::BROWSER_ERROR), error));
    ctx.request_repaint();
}

fn request(ctx: &egui::Context, canvas: &web_sys::Element, method: &str) {
    let result = js_sys::Reflect::get(canvas, &JsValue::from_str(method))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
        .and_then(|function| function.call0(canvas));
    match result {
        Ok(value) => {
            if let Ok(promise) = value.dyn_into::<js_sys::Promise>() {
                let ctx = ctx.clone();
                let method = method.to_owned();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        report(&ctx, format!("{method}: {e:?}"));
                    }
                });
            }
        }
        Err(e) => report(ctx, format!("{method}: {e:?}")),
    }
}

fn capture(ctx: &egui::Context) {
    let Some(document) = document() else { return };
    let Some(canvas) = document.get_element_by_id("figcraft_canvas") else { return };
    if document.pointer_lock_element().is_none() {
        request(ctx, &canvas, "requestPointerLock");
    }
    if document.fullscreen_element().is_none() {
        request(ctx, &canvas, "requestFullscreen");
    }
    if let Err(e) = game::audio::unlock(ctx) {
        report(ctx, e);
    }
}

pub fn install(ctx: &egui::Context) -> Result<(), JsValue> {
    let Some(document) = document() else { return Ok(()) };
    let mouse_ctx = ctx.clone();
    let mouse = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
        if locked() {
            let before = MOUSE.get();
            MOUSE.set([
                (before[0] + event.movement_x() as f32).clamp(-10000.0, 10000.0),
                (before[1] + event.movement_y() as f32).clamp(-10000.0, 10000.0),
            ]);
            mouse_ctx.request_repaint();
        }
    });
    document.add_event_listener_with_callback("mousemove", mouse.as_ref().unchecked_ref())?;
    mouse.forget();
    let click_ctx = ctx.clone();
    let click = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
        if event.button() != 0 {
            return;
        }
        let point = egui::pos2(event.client_x() as f32, event.client_y() as f32) / click_ctx.zoom_factor();
        let play = click_ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::unique(game::PLAY_BUTTON_RECT))).is_some_and(|r| r.contains(point));
        if play || (ACTIVE.get() && CAPTURE_ALLOWED.get()) {
            capture(&click_ctx);
        }
    });
    document.add_event_listener_with_callback("pointerdown", click.as_ref().unchecked_ref())?;
    click.forget();
    let key_ctx = ctx.clone();
    let key = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
        if event.code() == "F9"
            && !event.repeat()
            && !key_ctx.egui_wants_keyboard_input()
            && key_ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(game::PLAY_KEY_ENABLED)).unwrap_or(false))
        {
            capture(&key_ctx);
        }
    });
    document.add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())?;
    key.forget();
    Ok(())
}
