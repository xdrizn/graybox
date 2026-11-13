//! Play uses the ordinary prototype player: only its background and input policy change.
use super::{Presenter, layout::FULL};
use crate::{FigApp, game};
use egui::{Align2, Context, CursorGrab, Key, Rect, Ui, ViewportCommand, pos2, vec2};
use figcraft_doc::game::GameBackground;
use figcraft_doc::proto::Trigger;
use figcraft_doc::{Document, NodeId, NodeKind, VariableValue};

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
    // F10 or Delete leaves Play.
    if leave_key(&ctx) {
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
    // The navigable menu of the open screen, if any: it owns the menu keys and stops the player.
    // Menu keys are read only while the open screen has a menu (reading looks through its layers).
    let menu_input = p.doc.as_ref().map(|doc| {
        let root = p.overlays.last().map_or(p.current, |o| o.id);
        if game::menu::active_menu(doc, root).is_none() {
            return game::menu::MenuInputs::default();
        }
        let taken = |name: &str| game::menu::layer_takes_input(doc, root, name);
        ctx.input(|i| game::menu::read_menu(i, &doc.game, held | pressed, pressed, [pad.axes[0], pad.axes[1]], &taken))
    });
    let menu_active = menu_input.is_some_and(|input| menu_step(&mut p, &input, now));
    map_step(&mut p, &ctx, held, [pad.axes[0], pad.axes[1], pad.axes[3]]);
    named_inputs(&mut p, &ctx, pressed, now);
    hotbar_step(&mut p, &ctx, pressed);
    settle(&mut p);
    // Graybox: a screen can show the game paused or no game behind it; then the mouse stays free.
    let behind = background(&p);
    let running = behind == GameBackground::Running;
    app.services.game_capture_allowed(running && p.game.as_ref().is_some_and(|g| g.captured || p.overlays.is_empty()));
    if let (Some(play), Some(doc)) = (&mut p.game, &p.design) {
        // Graybox: Play runs the game the file chose.
        game::tps::sync(play, doc.game.kind);
        game::tps::debug_keys(play, &ctx);
        if !running {
            play.captured = false;
        } else if p.overlays.is_empty() && ctx.input(|i| i.pointer.primary_pressed()) {
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
        stop_moving(play, menu_active);
        if play.game.player.dead && play.game.input.respawn {
            play.game.respawn();
            play.game.input.respawn = false;
        }
        game::tps::respawn_on_request(play);
        if running {
            play.frame(now);
        } else {
            play.hold_clock(now);
        }
    }
    publish(&mut p);
    let events = p.game.as_mut().map(game::tps::drain_events).unwrap_or_default();
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
        if behind == GameBackground::Hidden {
            // No game: the screen's own background colour (black when it has none).
            let top = p.overlays.last().map_or(p.current, |o| o.id);
            let fill = p.design.as_ref().and_then(|d| d.node(top)).and_then(|n| n.fills.iter().find_map(|f| f.solid_color())).map_or(
                egui::Color32::BLACK,
                |c| {
                    let [r, g, b, a] = c.to_rgba8();
                    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
                },
            );
            ui.painter().rect_filled(rect, 0, fill);
        } else if let Some(play) = &p.game {
            game::tps::paint(ui, rect, play, &game::renderer::SceneTheme::default(), p.design.as_ref().is_none_or(|d| !d.game.hide_crosshair));
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

/// Whether F10 or Delete was pressed this frame (taken, so nothing else acts on it).
fn leave_key(ctx: &Context) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F10) || i.consume_key(egui::Modifiers::NONE, Key::Delete))
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

/// Game bars: each layer's width in Play, its designed width times the share of the value it
/// follows (health of the maximum, the magazine, the starting reserve). Bars empty from the right.
fn bar_widths(p: &Presenter) -> Vec<(NodeId, f64)> {
    let (Some(play), Some(original)) = (&p.game, &p.original) else { return Vec::new() };
    let player = &play.game.player;
    let share = |name: &str| -> f64 {
        if let Some(share) = game::tps::bar_share(play, name) {
            return share;
        }
        let (value, max) = match name {
            "vida" => (f64::from(player.health), f64::from(player.max_health)),
            "municao" => (f64::from(player.ammo), f64::from(figcraft_game::MAGAZINE_SIZE)),
            "reserva" => (f64::from(player.reserve), f64::from(figcraft_game::START_RESERVE)),
            // Graybox: no stamina in the first-person game, the bar stays full.
            _ => (1.0, 1.0),
        };
        if max > 0.0 && value.is_finite() { (value / max).clamp(0.0, 1.0) } else { 0.0 }
    };
    original.game.bars.iter().filter_map(|(id, name)| original.node(*id).map(|n| (*id, (n.width * share(name)).max(0.001)))).collect()
}

/// Hotbars on the screen: keys 1-9, the wheel (unless a map is open) and LB / RB select a slot; the
/// selected one shows its Focused variant. The player keeps moving meanwhile.
fn hotbar_step(p: &mut Presenter, ctx: &Context, pads: u32) {
    let (Some(doc), Some(original)) = (p.doc.as_ref(), p.original.as_ref()) else { return };
    let mut roots = vec![p.current];
    roots.extend(p.overlays.iter().map(|o| o.id));
    let bars = game::hotbar::in_view(doc, &roots);
    if bars.is_empty() {
        return;
    }
    let top = p.overlays.last().map_or(p.current, |o| o.id);
    let map_open = game::map::active(doc, top).is_some();
    let bit = |name: &str| figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|n| *n == name).is_some_and(|i| pads & (1 << i) != 0);
    let input = ctx.input(|i| {
        let wheel: f32 = i.events.iter().filter_map(|e| if let egui::Event::MouseWheel { delta, .. } = e { Some(delta.y) } else { None }).sum();
        let mut step = 0;
        if !map_open && wheel.abs() > 0.01 {
            step += if wheel < 0.0 { 1 } else { -1 };
        }
        step += i32::from(bit("Gamepad RB")) - i32::from(bit("Gamepad LB"));
        game::hotbar::HotbarInput { slot: game::hotbar::digit(i), step }
    });
    let Some(play) = p.game.as_mut() else { return };
    let mut looks = Vec::new();
    let mut plain_looks = Vec::new();
    play.hotbar_outline.clear();
    for (bar, settings) in &bars {
        let count = game::menu::items(original, *bar).len();
        let selected = play.hotbars.get(bar).copied().unwrap_or(0);
        let next = game::hotbar::step(selected, count, settings, &input);
        play.hotbars.insert(*bar, next);
        // Wireframe hotbars: the first slot's look moves to the selected one, no outline.
        if let Some(base) = game::hotbar::base_looks(original, *bar, next) {
            plain_looks.extend(base);
            continue;
        }
        let (bar_looks, outline) = game::hotbar::looks(original, *bar, next, settings);
        looks.extend(bar_looks);
        play.hotbar_outline.extend(outline);
    }
    let Some(design) = p.design.as_mut() else { return };
    let stale: Vec<(NodeId, NodeId)> = looks
        .into_iter()
        .filter(|(slot, variant)| matches!(design.node(*slot).map(|n| &n.kind), Some(NodeKind::Instance { component, .. }) if component != variant))
        .collect();
    let plain_stale: Vec<(NodeId, game::hotbar::Look)> =
        plain_looks.into_iter().filter(|(slot, look)| game::hotbar::Look::of(design, *slot).as_ref() != Some(look)).collect();
    if stale.is_empty() && plain_stale.is_empty() {
        return;
    }
    let before = design.clone();
    for (slot, look) in plain_stale {
        look.apply(design, slot);
    }
    for (slot, variant) in stale {
        figcraft_engine::cmd::game::keep_instance_size(design, slot);
        if let Some(n) = design.node_mut(slot)
            && let NodeKind::Instance { component, .. } = &mut n.kind
        {
            *component = variant;
        }
        figcraft_engine::components::sync_instance(design, slot);
    }
    p.settle(&before);
}

/// The designer's named inputs (T for a watch, E to pick up): a press fires the game event of that
/// name on the screen on top only, so the same key can open a screen and, on it, close it.
fn named_inputs(p: &mut Presenter, ctx: &Context, pads: u32, now: f64) {
    let Some(doc) = p.doc.as_ref() else { return };
    let fired: Vec<String> = ctx.input(|i| {
        doc.game.inputs.iter().filter(|(_, keys)| keys.iter().any(|k| game::input::binding(i, k, pads, true))).map(|(name, _)| name.clone()).collect()
    });
    if fired.is_empty() {
        return;
    }
    let top = p.overlays.last().map_or(p.current, |o| o.id);
    let mut hits = Vec::new();
    for id in doc.subtree(top) {
        let Some(node) = doc.node(id) else { continue };
        for (k, it) in node.interactions.iter().enumerate() {
            if matches!(&it.trigger, Trigger::GameEvent { event } if fired.contains(event)) {
                hits.push((id, k));
            }
        }
    }
    for (id, k) in hits {
        p.fire(id, k, None, now);
    }
}

/// The map on the screen on top, if any, moves and zooms with the mouse and the controller while the
/// mouse is free (the game isn't aiming).
fn map_step(p: &mut Presenter, ctx: &Context, held: u32, axes: [f32; 3]) {
    let Some(doc) = p.doc.as_ref() else { return };
    let root = p.overlays.last().map_or(p.current, |o| o.id);
    let Some((map, settings)) = game::map::active(doc, root) else { return };
    if p.game.as_ref().is_none_or(|g| g.captured) {
        return;
    }
    let scale = p.shown.as_ref().map_or(1.0, |s| s.1.scale).max(1e-6);
    let bit = |name: &str| figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|n| *n == name).is_some_and(|i| held & (1 << i) != 0);
    let input = ctx.input(|i| {
        let drag = if i.pointer.primary_down() { i.pointer.delta() } else { egui::Vec2::ZERO };
        game::map::MapInput {
            drag: [f64::from(drag.x) / scale, f64::from(drag.y) / scale],
            wheel: f64::from(i.smooth_scroll_delta.y),
            stick: [axes[0], axes[1]],
            zoom_stick: axes[2],
            zoom_in: bit("Gamepad RT"),
            zoom_out: bit("Gamepad RB"),
            dt: f64::from(i.stable_dt),
        }
    });
    let Some(play) = p.game.as_mut() else { return };
    let view = play.maps.entry(map).or_default();
    let before = *view;
    game::map::step(view, &settings, &input);
    if *view != before {
        p.rev += 1;
    }
}

/// What the screen on top (an overlay, else the presented frame) shows behind it.
fn background(p: &Presenter) -> GameBackground {
    let Some(doc) = p.doc.as_ref() else { return GameBackground::Running };
    let top = p.overlays.last().map_or(p.current, |o| o.id);
    doc.game.backgrounds.get(&top).or_else(|| doc.game.backgrounds.get(&p.current)).copied().unwrap_or_default()
}

/// Apply the Pause game / cursor actions the prototype ran; see [`game::Play::settle`].
fn settle(p: &mut Presenter) {
    let open: Vec<NodeId> = p.overlays.iter().map(|o| o.id).collect();
    if let Some(play) = &mut p.game {
        play.settle(&open);
    }
}

/// Escape frees the mouse while no screen is open, unless the prototype uses Escape itself
/// (a pause menu, say: its interaction decides). F10 and Delete always leave Play.
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
    let bars = bar_widths(p);
    let (Some(play), Some(doc)) = (&p.game, &mut p.design) else { return };
    let player = &play.game.player;
    let ids = &play.variable_ids;
    // Graybox: the third-person game publishes its own values (G11.5).
    let values = game::tps::variables(play).unwrap_or(game::tps::Variables {
        numbers: [
            ("vida", player.health as f64),
            ("vidaMax", player.max_health as f64),
            ("municao", player.ammo as f64),
            ("reserva", player.reserve as f64),
            ("ultimoDano", player.last_damage as f64),
        ],
        flags: [("agachado", player.crouching), ("emChamas", player.on_fire), ("escalando", false), ("mirando", false)],
        texts: [("atacante", player.attacker.as_str()), ("arma", "Pistola")],
    });
    let mut before = None;
    for (name, value) in values.numbers {
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
    for (name, value) in values.flags {
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
    for (id, width) in bars {
        let Some(node) = doc.node(id) else { continue };
        if (node.width - width).abs() < 0.01 {
            continue;
        }
        if before.is_none() {
            before = Some(doc.clone());
        }
        if let Some(node) = doc.node_mut(id) {
            node.width = width;
        }
    }
    for (name, value) in values.texts {
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
    let text = if play.game.player.dead || game::tps::dead(play) {
        "Enter: respawn  ·  F10 / Del: editor"
    } else if play.game.paused {
        "Paused  ·  close the menu to resume  ·  F10 / Del: editor"
    } else if !play.captured {
        "Click to capture cursor  ·  F10 / Del: editor"
    } else if play.tps.is_some() {
        game::tps::help(play)
    } else {
        "WASD: move  ·  mouse: look/shoot  ·  R: reload  ·  H: heal  ·  F10 / Del: editor"
    };
    // Graybox: low and see-through, so it covers as little of the HUD as it can.
    let r = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 16.0), vec2(rect.width().min(650.0) - 24.0, 24.0));
    ui.painter().rect_filled(r, 4, t.panel.gamma_multiply(0.45));
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, crate::theme::regular(12.0), t.text.gamma_multiply(0.75));
}

#[cfg(test)]
#[path = "game_menu_tests.rs"]
mod menu_tests;

#[cfg(test)]
#[path = "game_frames_tests.rs"]
mod frames_tests;

pub(super) use crate::game::menu::MenuInputs;

/// One frame of the navigable menu the open screen (top overlay, else the frame) owns: move focus,
/// show it, confirm and go back. True when a menu is active (it owns the menu keys).
pub(super) fn menu_step(p: &mut Presenter, input: &MenuInputs, now: f64) -> bool {
    let root = p.overlays.last().map_or(p.current, |o| o.id);
    let plan = match (p.doc.as_ref(), p.game.as_mut()) {
        (Some(doc), Some(play)) => play.menu.plan(doc, root, input, now),
        _ => return false,
    };
    if !plan.looks.is_empty()
        && let Some(d) = p.design.as_mut()
    {
        let before = d.clone();
        for (item, variant) in &plan.looks {
            figcraft_engine::cmd::game::keep_instance_size(d, *item);
            if let Some(n) = d.node_mut(*item)
                && let NodeKind::Instance { component, .. } = &mut n.kind
            {
                *component = *variant;
            }
            figcraft_engine::components::sync_instance(d, *item);
        }
        p.settle(&before);
    }
    if let Some(item) = plan.confirm {
        // The item's own On click or On press (or the first layer inside it that has one).
        let found = p.doc.as_ref().and_then(|doc| {
            doc.subtree(item)
                .into_iter()
                .find_map(|id| super::actions::find_trigger(doc, id, |t| matches!(t, Trigger::OnClick | Trigger::OnPress)).map(|k| (id, k)))
        });
        if let Some((id, k)) = found {
            p.fire(id, k, None, now);
        }
    }
    if plan.back && !p.overlays.is_empty() {
        p.close_overlay(now);
    }
    plan.active
}

/// The pointer at `pointer` is over layer `hit`; see [`game::menu::Focus::point`].
pub(super) fn menu_pointer(p: &mut Presenter, hit: Option<NodeId>, pointer: Option<[f32; 2]>) {
    if let (Some(doc), Some(play)) = (p.doc.as_ref(), p.game.as_mut()) {
        play.menu.point(doc, hit, pointer);
    }
}

/// The layer of `menu` under the pointer at `q`. Only the menu's own layers count, so a
/// decoration drawn over it (a monitor bezel, a vignette) does not hide its items from the mouse.
pub(super) fn menu_hit(doc: &Document, menu: NodeId, place: &super::layout::Place, q: egui::Pos2) -> Option<NodeId> {
    super::chrome::layer_at(doc, menu, place, q)
}

/// After the prototype is drawn: the pointer marks menu items and an item without a State variant
/// gets a focus outline.
pub(super) fn menu_overlay(p: &mut Presenter, painter: &egui::Painter, ctx: &Context, l: &super::layout::Layout) {
    // Graybox: hotbar slots without a selected variant get the same outline.
    if let Some(play) = p.game.as_ref() {
        let (_, place, clip) = l.active(p.current);
        for slot in &play.hotbar_outline {
            if let Some(b) = l.disp.world_bounds(*slot) {
                painter.with_clip_rect(clip).rect_stroke(
                    place.rect(b),
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, crate::theme::ACCENT),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }
    if p.game.as_ref().is_none_or(|g| g.menu.menu.is_none()) {
        return;
    }
    let (_, place, clip) = l.active(p.current);
    let menu = p.game.as_ref().and_then(|g| g.menu.menu);
    let pos = ctx.pointer_hover_pos().filter(|q| clip.contains(*q));
    let hit = pos.zip(menu).and_then(|(q, menu)| menu_hit(&l.disp, menu, &place, q));
    menu_pointer(p, hit, pos.map(|q| [q.x, q.y]));
    let Some(focus) = p.game.as_ref().map(|g| &g.menu) else { return };
    if let (true, Some(item)) = (focus.outline, focus.item)
        && let Some(b) = l.disp.world_bounds(item)
    {
        painter.with_clip_rect(clip).rect_stroke(
            place.rect(b),
            egui::CornerRadius::ZERO,
            egui::Stroke::new(2.0, crate::theme::ACCENT),
            egui::StrokeKind::Outside,
        );
    }
}

/// The player stands still while the mouse is free or a navigable menu owns the keys.
pub(super) fn stop_moving(play: &mut game::Play, menu_active: bool) {
    if !play.captured || menu_active {
        play.game.input.forward = 0.0;
        play.game.input.strafe = 0.0;
        play.game.input.jump = false;
    }
}

#[cfg(test)]
mod screen_tests {
    use super::*;
    use std::sync::Arc;

    /// A HUD (300 wide health bar) and an inventory, the game playing.
    fn play() -> (Presenter, NodeId, NodeId, NodeId) {
        let mut doc = figcraft_doc::Document::new("Screens");
        let page = doc.first_page().unwrap();
        let hud = doc.create(page, figcraft_doc::NodeKind::Frame, "HUD", 0.0, 0.0, 1280.0, 720.0).unwrap();
        let bar = doc.create(hud, figcraft_doc::NodeKind::Rectangle, "Health", 40.0, 600.0, 300.0, 20.0).unwrap();
        let inventory = doc.create(page, figcraft_doc::NodeKind::Frame, "Inventory", 1400.0, 0.0, 1280.0, 720.0).unwrap();
        doc.game.bars.insert(bar, "vida".into());
        doc.game.backgrounds.insert(inventory, GameBackground::Hidden);
        let mut p = Presenter::new(hud);
        p.game = Some(crate::game::Play::new());
        p.sync(&Arc::new(doc), 0.0);
        (p, hud, bar, inventory)
    }

    /// Play takes the monitor's shape: layers anchored by their constraints reach the monitor's
    /// corners, centre and edges whatever its size, and the saved design keeps its size.
    /// F10 and Delete leave Play; other keys don't.
    #[test]
    fn f10_and_delete_leave_play() {
        let press = |key: Key| {
            let ctx = egui::Context::default();
            let mut left = false;
            let ev = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
            let mut out = ctx.run_ui(egui::RawInput { events: vec![ev(true), ev(false)], ..Default::default() }, |ui| left = leave_key(ui.ctx()));
            out.textures_delta.clear();
            left
        };
        assert!(press(Key::F10));
        assert!(press(Key::Delete));
        assert!(!press(Key::Escape) && !press(Key::E));
    }

    #[test]
    fn constraints_anchor_layers_to_any_monitor() {
        use figcraft_doc::{Constraints, HConstraint, VConstraint};
        let mut doc = figcraft_doc::Document::new("Anchors");
        let page = doc.first_page().unwrap();
        let hud = doc.create(page, NodeKind::Frame, "HUD", 0.0, 0.0, 1280.0, 720.0).unwrap();
        let map = doc.create(hud, NodeKind::Rectangle, "Minimap", 1160.0, 20.0, 100.0, 100.0).unwrap();
        let aim = doc.create(hud, NodeKind::Rectangle, "Crosshair", 630.0, 350.0, 20.0, 20.0).unwrap();
        let strip = doc.create(hud, NodeKind::Rectangle, "Bottom strip", 0.0, 700.0, 1280.0, 20.0).unwrap();
        let health = doc.create(hud, NodeKind::Rectangle, "Health", 20.0, 660.0, 200.0, 20.0).unwrap();
        let anchor =
            |doc: &mut figcraft_doc::Document, id, horizontal, vertical| doc.node_mut(id).unwrap().constraints = Constraints { horizontal, vertical };
        anchor(&mut doc, map, HConstraint::Right, VConstraint::Top);
        anchor(&mut doc, aim, HConstraint::Center, VConstraint::Center);
        anchor(&mut doc, strip, HConstraint::LeftRight, VConstraint::Bottom);
        anchor(&mut doc, health, HConstraint::Left, VConstraint::Bottom);
        let mut p = Presenter::new(hud);
        p.game = Some(crate::game::Play::new());
        p.sync(&Arc::new(doc), 0.0);

        for (w, h) in [(1280.0, 720.0), (3440.0, 1440.0), (1920.0, 1200.0), (1024.0, 768.0), (3840.0, 2160.0)] {
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
            let l = super::super::layout::layout(&mut p, area, super::super::layout::FULL).unwrap();
            let on_screen = |id| l.place.rect(l.disp.world_bounds(id).unwrap());
            let k = l.scale as f32;
            let near = |a: f32, b: f32| (a - b).abs() < 0.5;
            assert!(near(l.screen.width(), w) && near(l.screen.height(), h), "{w}×{h}: the frame fills the monitor");
            let m = on_screen(map);
            assert!(near(m.max.x, w - 20.0 * k) && near(m.min.y, 20.0 * k), "{w}×{h}: the minimap keeps its corner");
            assert!(near(on_screen(aim).center().x, w / 2.0) && near(on_screen(aim).center().y, h / 2.0), "{w}×{h}: the crosshair stays centred");
            let s = on_screen(strip);
            assert!(near(s.min.x, 0.0) && near(s.max.x, w) && near(s.max.y, h), "{w}×{h}: the strip spans the bottom edge");
            let hb = on_screen(health);
            assert!(near(hb.min.x, 20.0 * k) && near(hb.max.y, h - 40.0 * k), "{w}×{h}: health keeps the bottom-left corner");
        }
        let saved = p.design.as_ref().unwrap().node(hud).unwrap();
        assert_eq!((saved.width, saved.height), (1280.0, 720.0), "the design keeps its size");
    }

    #[test]
    fn a_game_bar_keeps_its_drawn_size_as_the_full_mark() {
        let (mut p, _, bar, _) = play();
        publish(&mut p);
        assert_eq!(p.design.as_ref().unwrap().node(bar).unwrap().width, 300.0, "full health: the width it was drawn at");
        if let Some(play) = p.game.as_mut() {
            play.game.player.health = 25.0;
        }
        publish(&mut p);
        assert_eq!(p.design.as_ref().unwrap().node(bar).unwrap().width, 75.0, "a quarter of the health, a quarter of the bar");
        assert_eq!(p.original.as_ref().unwrap().node(bar).unwrap().width, 300.0, "the design is untouched");
    }

    #[test]
    fn the_screen_on_top_decides_what_is_behind_it() {
        let (mut p, hud, _, inventory) = play();
        assert_eq!(background(&p), GameBackground::Running, "a screen without a setting shows the game");
        let open = figcraft_doc::proto::Action::OpenOverlay {
            dest: inventory,
            position: figcraft_doc::proto::OverlayPosition::Center,
            close_on_outside: true,
            background: None,
            offset: None,
            anchor: figcraft_doc::proto::OverlayAnchor::Screen,
        };
        p.perform(&open, hud, figcraft_doc::proto::Transition::Instant, 0.0, figcraft_doc::proto::Easing::Linear, None, 0.0);
        assert_eq!(background(&p), GameBackground::Hidden, "the overlay on top has No game");
        p.close_overlay(0.1);
        assert_eq!(background(&p), GameBackground::Running);
    }

    #[test]
    fn number_keys_and_shoulders_select_a_hotbar_slot() {
        use serde_json::json;
        let mut s = figcraft_engine::Session::new();
        let ex = |s: &mut figcraft_engine::Session, c: &str, p: serde_json::Value| s.execute(c, &p).unwrap();
        let id = |v: serde_json::Value| figcraft_engine::params::id_of(&v["id"]).unwrap();
        let hud = id(ex(&mut s, "frame.create", json!({"width": 1280, "height": 720})));
        let slot = id(ex(&mut s, "frame.create", json!({"x": 2000, "width": 60, "height": 60, "name": "Slot"})));
        let main = id(ex(&mut s, "component.create", json!({"ids": [slot]})));
        ex(&mut s, "node.set", json!({"id": main, "props": {"name": "State=Default"}}));
        ex(&mut s, "component.addVariant", json!({"id": main}));
        let set = figcraft_engine::components::variant_set(s.doc().unwrap(), main).unwrap();
        let focused = *s.doc().unwrap().node(set).unwrap().children.iter().find(|v| **v != main).unwrap();
        ex(&mut s, "node.set", json!({"id": focused, "props": {"name": "State=Focused"}}));
        let bar = id(ex(&mut s, "frame.create", json!({"parent": hud, "width": 300, "height": 60, "props": {"layout": "horizontal"}})));
        let slots: Vec<NodeId> = (0..3).map(|_| id(ex(&mut s, "instance.create", json!({"component": main, "parent": bar})))).collect();
        ex(&mut s, "game.hotbar.set", json!({"node": bar}));
        let mut p = Presenter::new(hud);
        p.game = Some(crate::game::Play::new());
        p.sync(&Arc::new(s.doc().unwrap().clone()), 0.0);
        let ctx = egui::Context::default();
        let shown = |p: &Presenter| -> Vec<NodeId> {
            slots
                .iter()
                .map(|s| match p.design.as_ref().unwrap().node(*s).unwrap().kind {
                    NodeKind::Instance { component, .. } => component,
                    _ => NodeId(0),
                })
                .collect()
        };
        let frame = |p: &mut Presenter, events: Vec<egui::Event>, pads: u32| {
            let mut out = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| hotbar_step(p, ui.ctx(), pads));
            out.textures_delta.clear();
        };
        frame(&mut p, vec![], 0);
        assert_eq!(shown(&p), [focused, main, main], "the first slot starts selected");
        let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        frame(&mut p, vec![key(egui::Key::Num3)], 0);
        assert_eq!(shown(&p), [main, main, focused], "key 3 selects the third slot");
        let rb = figcraft_doc::proto::GAMEPAD_BUTTONS.iter().position(|n| *n == "Gamepad RB").unwrap();
        frame(&mut p, vec![], 1 << rb);
        assert_eq!(shown(&p), [focused, main, main], "RB after the last slot loops to the first");
    }

    #[test]
    fn a_named_input_opens_a_screen_and_on_it_closes_it() {
        use figcraft_doc::proto::{Action, Easing, Interaction, OverlayAnchor, OverlayPosition, Transition};
        let mut doc = figcraft_doc::Document::new("Watch");
        let page = doc.first_page().unwrap();
        let hud = doc.create(page, figcraft_doc::NodeKind::Frame, "HUD", 0.0, 0.0, 1280.0, 720.0).unwrap();
        let watch = doc.create(page, figcraft_doc::NodeKind::Frame, "Watch", 1400.0, 0.0, 400.0, 400.0).unwrap();
        doc.game.inputs.insert("relogio".into(), vec!["T".into()]);
        let on = |actions: Vec<Action>| Interaction {
            trigger: Trigger::GameEvent { event: "relogio".into() },
            actions,
            transition: Transition::Instant,
            duration_ms: 0,
            easing: Easing::Linear,
            delay_ms: 0,
        };
        let open = Action::OpenOverlay {
            dest: watch,
            position: OverlayPosition::Center,
            close_on_outside: true,
            background: None,
            offset: None,
            anchor: OverlayAnchor::Screen,
        };
        doc.node_mut(hud).unwrap().interactions.push(on(vec![open]));
        doc.node_mut(watch).unwrap().interactions.push(on(vec![Action::CloseOverlay]));
        let mut p = Presenter::new(hud);
        p.game = Some(crate::game::Play::new());
        p.sync(&Arc::new(doc), 0.0);
        let ctx = egui::Context::default();
        let press = |p: &mut Presenter, key: egui::Key| {
            let events = vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
            let mut out = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| named_inputs(p, ui.ctx(), 0, 0.0));
            out.textures_delta.clear();
        };
        press(&mut p, egui::Key::Y);
        assert!(p.overlays.is_empty(), "other keys do nothing");
        press(&mut p, egui::Key::T);
        assert_eq!(p.overlays.iter().map(|o| o.id).collect::<Vec<_>>(), [watch], "T opens the watch");
        press(&mut p, egui::Key::T);
        assert!(p.overlays.is_empty(), "on the watch, T closes it (only the screen on top reacts)");
    }
}
