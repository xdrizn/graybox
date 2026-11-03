//! Play uses the ordinary prototype player: only its background and input policy change.
use super::{Presenter, layout::FULL};
use crate::{FigApp, game};
use egui::{Align2, Context, CursorGrab, Key, Rect, Ui, ViewportCommand, pos2, vec2};
use figcraft_doc::proto::Trigger;
use figcraft_doc::{NodeId, VariableValue};

pub(super) fn show(app: &mut FigApp, ui: &mut Ui, mut p: Presenter, now: f64) {
    let ctx = ui.ctx().clone();
    if let Ok(ds) = app.session.ds() {
        let changed = p.source.as_ref().is_none_or(|source| !std::sync::Arc::ptr_eq(source, &ds.doc));
        p.sync(&ds.doc, now);
        if changed {
            if let Some(design) = &mut p.design {
                figcraft_engine::cmd::game::ensure_variables(design);
                if let Some(play) = &mut p.game {
                    play.variable_ids = figcraft_engine::cmd::game::variable_ids(design);
                }
            }
            p.relayout();
        }
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F10)) {
        leave(app, &ctx, p);
        return;
    }
    let pad = app.services.gamepad_state();
    let held = pad.buttons;
    let mouse = app.services.game_mouse_delta();
    let mut pressed = 0;
    for name in &p.pads {
        if let Some(index) = figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|n| n == name) {
            pressed |= 1 << index;
        }
    }
    if let Some(play) = &mut p.game {
        pressed |= held & !play.pads_held;
        play.pads_held = held;
        if !play.entered {
            play.entered = true;
            play.volume = ctx.data(|d| d.get_temp::<f32>(egui::Id::unique(VOLUME_KEY))).unwrap_or(1.0);
            play.was_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            if cfg!(target_arch = "wasm32") {
                play.captured = app.services.game_cursor_locked() == Some(true);
            } else {
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
            }
            if let Err(e) = app.services.game_enter() {
                app.toast(e);
            }
            if let Err(e) = game::audio::unlock(&ctx) {
                app.toast(e);
            }
        }
        if play.applied_capture == Some(true) && now - play.capture_requested_at > 0.5 && app.services.game_cursor_locked() == Some(false) {
            play.captured = false;
        }
    }
    settle(&mut p);
    escape_frees_cursor(&mut p, &ctx);
    app.services.game_capture_allowed(p.game.as_ref().is_some_and(|g| g.captured || p.overlays.is_empty()));
    if let (Some(play), Some(doc)) = (&mut p.game, &p.design) {
        if p.overlays.is_empty() && ctx.input(|i| i.pointer.primary_pressed()) {
            play.captured = true;
        }
        play.game.input = ctx.input(|i| game::input::read(i, &doc.game, held | pressed, pressed, play.captured));
        if play.captured {
            let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.05);
            play.game.input.forward = (play.game.input.forward - pad.axes[1]).clamp(-1.0, 1.0);
            play.game.input.strafe = (play.game.input.strafe + pad.axes[0]).clamp(-1.0, 1.0);
            if let Some(delta) = mouse {
                play.game.input.look = [delta[0] * 0.002, -delta[1] * 0.002];
            }
            play.game.input.look[0] += pad.axes[2] * dt * 2.5;
            play.game.input.look[1] -= pad.axes[3] * dt * 2.5;
        }
        if !play.captured {
            play.game.input.forward = 0.0;
            play.game.input.strafe = 0.0;
            play.game.input.jump = false;
        }
        if play.game.player.dead && play.game.input.respawn {
            play.game.respawn();
            play.game.input.respawn = false;
        }
        play.frame(now);
    }
    publish(&mut p);
    let events = p.game.as_mut().map(|play| play.game.drain_events()).unwrap_or_default();
    for event in events {
        dispatch(&mut p, event.name(), now);
    }
    settle(&mut p);
    let captured = p.game.as_ref().is_some_and(|g| g.captured);
    if p.game.as_ref().is_some_and(|g| g.applied_capture != Some(captured)) {
        set_cursor(app, &ctx, captured);
        if let Some(g) = &mut p.game {
            g.applied_capture = Some(captured);
            g.capture_requested_at = now;
        }
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
        let rect = ui.max_rect();
        if let Some(play) = &p.game {
            game::renderer::paint(ui, rect, &play.game, &game::renderer::SceneTheme::default());
        }
        super::player::player(&mut p, ui, rect, now, FULL);
        help(&p, ui, rect);
        volume_control(&mut p, ui, rect);
    });
    super::chrome::finish(app, &mut p, &ctx);
    app.ui.present = Some(p);
    ctx.request_repaint();
}

fn set_cursor(app: &mut FigApp, ctx: &Context, captured: bool) {
    if !cfg!(target_arch = "wasm32") {
        ctx.send_viewport_cmd(ViewportCommand::CursorVisible(!captured));
        ctx.send_viewport_cmd(ViewportCommand::CursorGrab(if captured { CursorGrab::Locked } else { CursorGrab::None }));
    }
    if let Err(e) = app.services.game_cursor(captured) {
        app.toast(e);
    }
}

fn leave(app: &mut FigApp, ctx: &Context, p: Presenter) {
    let fullscreen = p.game.as_ref().is_some_and(|g| g.was_fullscreen);
    if !cfg!(target_arch = "wasm32") {
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(fullscreen));
    }
    set_cursor(app, ctx, false);
    app.services.game_leave();
    app.ui.present = Some(p);
    if let Some(id) = app.ui.present_active {
        crate::tabs::close_present(app, id);
    } else {
        app.ui.present = None;
    }
}

/// Apply the Pause game / cursor actions the prototype ran; see [`game::Play::settle`].
fn settle(p: &mut Presenter) {
    let open: Vec<NodeId> = p.overlays.iter().map(|o| o.id).collect();
    if let Some(play) = &mut p.game {
        play.settle(&open);
    }
}

/// Escape frees the mouse while no screen is open, unless the prototype uses Escape itself
/// (a pause menu, say: its interaction decides). F10 always leaves Play.
fn escape_frees_cursor(p: &mut Presenter, ctx: &Context) {
    if !p.overlays.is_empty() || !ctx.input(|i| i.key_pressed(Key::Escape)) {
        return;
    }
    let uses_escape = p.doc.as_ref().is_some_and(|doc| {
        doc.subtree(p.current)
            .into_iter()
            .filter_map(|id| doc.node(id))
            .any(|n| n.interactions.iter().any(|it| matches!(&it.trigger, Trigger::KeyPress { key } if key == "Escape")))
    });
    if !uses_escape && let Some(play) = &mut p.game {
        play.captured = false;
    }
}

fn dispatch(p: &mut Presenter, canonical: &str, now: f64) {
    let Some(doc) = p.doc.as_ref() else { return };
    let alias = doc.game.event_name(canonical);
    let mut roots = vec![p.current];
    roots.extend(p.overlays.iter().map(|o| o.id));
    let mut hits = Vec::new();
    for root in roots {
        for id in doc.subtree(root) {
            if let Some(node) = doc.node(id) {
                for (k, it) in node.interactions.iter().enumerate() {
                    if matches!(&it.trigger, Trigger::GameEvent { event } if event == canonical || event == alias) {
                        hits.push((id, k));
                    }
                }
            }
        }
    }
    for (id, k) in hits {
        p.fire(id, k, None, now);
    }
}

fn publish(p: &mut Presenter) {
    let (Some(play), Some(doc)) = (&p.game, &mut p.design) else { return };
    let player = &play.game.player;
    let ids = &play.variable_ids;
    let numbers = [
        ("vida", player.health as f64),
        ("vidaMax", player.max_health as f64),
        ("municao", player.ammo as f64),
        ("reserva", player.reserve as f64),
        ("ultimoDano", player.last_damage as f64),
    ];
    let mut before = None;
    for (name, value) in numbers {
        let Some(id) = ids.get(name) else { continue };
        let Some(var) = doc.variables.get_by_id(id) else { continue };
        if var.values.values().all(|v| *v == VariableValue::Number(value)) {
            continue;
        }
        if before.is_none() {
            before = Some(doc.clone());
        }
        figcraft_engine::proto_run::set_variable(doc, p.current, id, &serde_json::json!(value), None);
    }
    for (name, value) in [("agachado", player.crouching), ("emChamas", player.on_fire)] {
        let Some(id) = ids.get(name) else { continue };
        let Some(var) = doc.variables.get_by_id(id) else { continue };
        if var.values.values().all(|v| *v == VariableValue::Boolean(value)) {
            continue;
        }
        if before.is_none() {
            before = Some(doc.clone());
        }
        figcraft_engine::proto_run::set_variable(doc, p.current, id, &serde_json::json!(value), None);
    }
    for (name, value) in [("atacante", player.attacker.as_str()), ("arma", "Pistola")] {
        let Some(id) = ids.get(name) else { continue };
        let Some(var) = doc.variables.get_by_id(id) else { continue };
        if var.values.values().all(|v| matches!(v,VariableValue::String(s) if s==value)) {
            continue;
        }
        if before.is_none() {
            before = Some(doc.clone());
        }
        figcraft_engine::proto_run::set_variable(doc, p.current, id, &serde_json::json!(value), None);
    }
    if let Some(before) = before {
        p.settle(&before);
    }
}

const VOLUME_KEY: &str = "greybox.play.volume";

/// The player's volume, bottom right, while the mouse is free (a menu is open or the cursor was
/// released). It scales every sound on top of the file's volumes and is kept for the app run.
fn volume_control(p: &mut Presenter, ui: &mut Ui, rect: Rect) {
    let Some(play) = &mut p.game else { return };
    if play.captured {
        return;
    }
    let t = crate::theme::Tokens::get(ui.ctx());
    let r = Rect::from_min_size(pos2(rect.max.x - 236.0, rect.max.y - 52.0), vec2(220.0, 32.0));
    ui.painter().rect_filled(r, 4, t.panel.gamma_multiply(0.9));
    let mut volume = play.volume;
    ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 6.0))), |ui| {
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("Volume").color(t.text).font(crate::theme::regular(12.0)));
            ui.spacing_mut().slider_width = 120.0;
            ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false));
        });
    });
    if (volume - play.volume).abs() > f32::EPSILON && volume.is_finite() {
        play.volume = volume.clamp(0.0, 1.0);
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(VOLUME_KEY), play.volume));
    }
}

fn help(p: &Presenter, ui: &Ui, rect: Rect) {
    let Some(play) = &p.game else { return };
    let t = crate::theme::Tokens::get(ui.ctx());
    let text = if play.game.player.dead {
        "Enter: respawn  ·  F10: editor"
    } else if play.game.paused {
        "Paused  ·  close the menu to resume  ·  F10: editor"
    } else if !play.captured {
        "Click to capture cursor  ·  F10: editor"
    } else {
        "WASD: move  ·  mouse: look/shoot  ·  R: reload  ·  H: heal  ·  F10: editor"
    };
    let r = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 24.0), vec2(rect.width().min(650.0) - 24.0, 28.0));
    ui.painter().rect_filled(r, 4, t.panel.gamma_multiply(0.9));
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, crate::theme::regular(12.0), t.text);
}
