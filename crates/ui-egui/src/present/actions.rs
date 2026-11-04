use super::{
    Action, Document, Easing, FrameTex, NodeId, NodeKind, OverlayAnchor, OverlayPosition, OverlayState, Presenter, Rect, ScrollAnim, Temp,
    Transition, Trigger, Waiting,
};

#[derive(Clone, Copy)]
struct ActionTiming {
    transition: Transition,
    duration: f64,
    easing: Easing,
    rect: Option<Rect>,
    now: f64,
    depth: usize,
}

impl Presenter {
    /// A mouse enter, leave, down or up on `node` (shown on `root`) met its interaction `k`: its
    /// actions run now, or after its delay while `root` is still what shows (again once only
    /// while it waits).
    pub(super) fn gesture(&mut self, node: NodeId, k: usize, rect: Option<Rect>, now: f64, root: NodeId) -> bool {
        let delay = self.doc.as_ref().and_then(|d| d.node(node)).and_then(|n| n.interactions.get(k)).map_or(0.0, |it| it.delay_secs());
        if delay <= 0.0 {
            return self.fire(node, k, rect, now);
        }
        if !self.waiting.iter().any(|w| (w.node, w.k) == (node, k)) {
            self.waiting.push(Waiting { node, k, rect, root, due: now + delay });
        }
        false
    }

    /// Run the delayed mouse triggers that are due (those whose screen or overlay went away are
    /// dropped). True when one changed what shows.
    pub(super) fn fire_waiting(&mut self, root: NodeId, now: f64) -> bool {
        let (due, later): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut self.waiting).into_iter().partition(|w| w.due <= now);
        self.waiting = later;
        self.waiting.retain(|w| w.root == root);
        let mut did = false;
        for w in due.into_iter().filter(|w| w.root == root) {
            // An earlier one may have left the screen: the rest are dropped with it. The gesture
            // is long over: what these open is not the viewer's doing (#1096).
            did = did || self.fire_from(w.node, w.k, w.rect, now, false);
        }
        did
    }

    /// Run the actions of interaction `k` on `node`. `rect` is the layer's screen rect.
    pub(crate) fn fire(&mut self, node: NodeId, k: usize, rect: Option<Rect>, now: f64) -> bool {
        self.fire_from(node, k, rect, now, true)
    }

    /// [`fire`](Self::fire); `live`: the viewer's input fired it just now (not a delay it waited
    /// out). The links it opens are the viewer's own only when live from a click, press, drag or
    /// key: a delay, the pointer passing over a layer and a timed mouse trigger fire without the
    /// viewer asking for anything ([`Presenter::urls`], #1096).
    fn fire_from(&mut self, node: NodeId, k: usize, rect: Option<Rect>, now: f64, live: bool) -> bool {
        let Some(it) = self.doc.as_ref().and_then(|d| d.node(node)).and_then(|n| n.interactions.get(k)).cloned() else { return false };
        let dur = it.easing.seconds(it.duration_ms);
        let temp = match it.trigger {
            Trigger::OnHover => Some(false),
            Trigger::OnPress => Some(true),
            _ => None,
        };
        let snap = if temp.is_some() && self.temp.is_none() { self.snapshot() } else { None };
        let mut did = false;
        let first_link = self.urls.len();
        for a in &it.actions {
            did |= self.perform(a, node, it.transition, dur, it.easing, rect, now);
        }
        let asked = live
            && matches!(
                it.trigger,
                Trigger::OnClick | Trigger::OnPress | Trigger::OnDrag | Trigger::MouseDown | Trigger::MouseUp | Trigger::KeyPress { .. }
            );
        for link in self.urls.iter_mut().skip(first_link) {
            link.1 = asked;
        }
        if did && let (Some(press), Some(snap), Some(rect)) = (temp, snap, rect) {
            self.temp = Some(Temp { rect, press, snap, transition: it.transition, dur, easing: it.easing });
        }
        did
    }

    /// Run the widget clicks the player took: each calls its plugin back on the player's copy (the
    /// prototype's own state of the widget), as on the canvas but without touching the file.
    /// Returns what went wrong, for a toast.
    pub fn click_widgets(&mut self, s: &figcraft_engine::Session) -> Option<String> {
        let mut error = None;
        for (id, target) in std::mem::take(&mut self.widget_clicks) {
            let Some(doc) = self.design.clone() else { break };
            match figcraft_engine::widgets::click_on(s, doc, id, &target) {
                Ok((d, _)) => {
                    self.design = Some(d);
                    self.relayout();
                }
                Err(e) => error = Some(e.to_string()),
            }
        }
        error
    }

    pub(crate) fn perform(&mut self, a: &Action, node: NodeId, tr: Transition, dur: f64, easing: Easing, rect: Option<Rect>, now: f64) -> bool {
        self.perform_at(a, node, ActionTiming { transition: tr, duration: dur, easing, rect, now, depth: 0 })
    }

    /// One action; `depth` counts the conditional actions it is inside.
    fn perform_at(&mut self, a: &Action, node: NodeId, timing: ActionTiming) -> bool {
        let ActionTiming { transition: tr, duration: dur, easing, rect, now, depth } = timing;
        let Some(doc) = self.doc.clone() else { return false };
        match a {
            Action::Navigate { dest, preserve_scroll } => {
                let Some(top) = figcraft_engine::proto_run::navigate_target(&doc, *dest, &self.sections) else { return false };
                if !doc.contains(top) || (top == self.current && self.overlays.is_empty()) {
                    return false;
                }
                self.go(top, tr, dur, easing, now, true, *preserve_scroll);
                true
            }
            Action::Back => {
                if self.close_overlay(now) {
                    return true;
                }
                match self.history.pop() {
                    Some(v) => {
                        self.go(v.frame, v.transition.reversed(), v.dur, v.easing, now, false, false);
                        true
                    }
                    None => false,
                }
            }
            Action::ScrollTo { dest, offset } => {
                let frames = self.scroll_to_layer(&doc, *dest, *offset);
                if frames.is_empty() {
                    return false;
                }
                if matches!(tr, Transition::Instant) || dur <= 0.0 {
                    self.scroll.extend(frames.iter().map(|(f, _, to)| (*f, *to)));
                    self.rev += 1;
                } else {
                    self.scroll_anim = Some(ScrollAnim { frames, start: now, dur, easing });
                }
                true
            }
            Action::OpenOverlay { dest, position, close_on_outside, background, offset, anchor } => {
                let at = self.page_rect(rect);
                if !doc.contains(*dest) {
                    return false;
                }
                self.delays_fired.retain(|(id, _)| !doc.is_ancestor(*dest, *id));
                self.overlays.push(OverlayState {
                    id: *dest,
                    position: *position,
                    close_on_outside: *close_on_outside,
                    background: *background,
                    anchor: at,
                    offset: *offset,
                    anchor_to: *anchor,
                    opened: now,
                    transition: tr,
                    dur,
                    easing,
                });
                self.hovered.clear();
                self.rev += 1;
                true
            }
            Action::SwapOverlay { dest } => {
                let at = self.page_rect(rect);
                if !doc.contains(*dest) {
                    return false;
                }
                self.delays_fired.retain(|(id, _)| !doc.is_ancestor(*dest, *id));
                match self.overlays.last_mut() {
                    Some(o) => {
                        o.id = *dest;
                        o.opened = now;
                        o.transition = tr;
                        o.dur = dur;
                        o.easing = easing;
                    }
                    None => self.overlays.push(OverlayState {
                        id: *dest,
                        position: OverlayPosition::Center,
                        close_on_outside: true,
                        background: None,
                        anchor: at,
                        offset: None,
                        anchor_to: OverlayAnchor::Screen,
                        opened: now,
                        transition: tr,
                        dur,
                        easing,
                    }),
                }
                self.hovered.clear();
                self.rev += 1;
                true
            }
            Action::CloseOverlay => self.close_overlay(now),
            Action::OpenUrl { url, new_tab } => {
                // The URL comes from the document and this action fires from whatever trigger the
                // prototype sets, including a delay, so no click is implied: only a web or mail
                // link is kept, as not the viewer's doing until `fire_from` says otherwise.
                if figcraft_engine::doc_urls::openable(url) {
                    self.urls.push((url.clone(), false, *new_tab));
                }
                true
            }
            Action::ChangeTo { dest } => {
                let Some(inst) = figcraft_engine::proto_run::change_to_target(&doc, node, *dest) else { return false };
                if !doc.contains(*dest) {
                    return false;
                }
                self.begin(tr, dur, easing, now);
                let Some(d) = self.design.as_mut() else { return false };
                let before = d.clone();
                // Graybox: a resized instance keeps its size in the new variant.
                figcraft_engine::cmd::game::keep_instance_size(d, inst);
                if let Some(n) = d.node_mut(inst)
                    && let NodeKind::Instance { component, .. } = &mut n.kind
                {
                    *component = *dest;
                }
                figcraft_engine::components::sync_instance(d, inst);
                self.settle(&before);
                true
            }
            Action::SetVariable { variable, value, expr } => {
                let Some(d) = self.design.as_mut() else { return false };
                let before = d.clone();
                let set = figcraft_engine::proto_run::set_variable(d, node, variable, value, expr.as_ref());
                if set {
                    self.settle(&before);
                }
                set
            }
            Action::SetVariableMode { collection, mode } => {
                let Some(d) = self.design.as_mut() else { return false };
                let before = d.clone();
                let set = figcraft_engine::proto_run::set_mode_by_id(d, self.current, collection, mode);
                if set {
                    self.settle(&before);
                }
                set
            }
            Action::GamePause { paused } => {
                let Some(play) = self.game.as_mut() else { return false };
                play.pending.push(crate::game::GameSet::Pause(*paused));
                true
            }
            Action::GameCursor { captured } => {
                let Some(play) = self.game.as_mut() else { return false };
                play.pending.push(crate::game::GameSet::Cursor(*captured));
                true
            }
            Action::PlaySound { sound, volume } => {
                if !doc.audio.sounds.contains_key(sound) || !volume.is_finite() || !(0.0..=1.0).contains(volume) {
                    return false;
                }
                self.sound_requests.push((sound.clone(), *volume));
                true
            }
            Action::PlayAnimation { target, preset, duration_ms, easing } => {
                let target = crate::game::animation::target_for(&doc, node, *target);
                if !self.animations.start(&doc, target, *preset, *duration_ms, *easing, now) {
                    return false;
                }
                self.rev += 1;
                true
            }
            Action::Conditional { blocks } => {
                if depth >= figcraft_doc::proto::MAX_ACTION_DEPTH {
                    return false;
                }
                // Evaluated now: earlier actions of the interaction may have changed variables.
                let mut did = false;
                for inner in figcraft_engine::proto_run::branch(&doc, node, blocks) {
                    did |= self.perform_at(&inner, node, ActionTiming { depth: depth + 1, ..timing });
                }
                did
            }
        }
    }

    pub(super) fn texture(&mut self, ctx: &egui::Context, doc: &Document, id: NodeId, px_scale: f64, now: f64) -> Option<FrameTex> {
        // Animated GIFs play from when the frame arrived: a new texture when one of them moves on,
        // and a wake-up for that moment (nothing else draws frames while it shows). egui wakes a
        // display frame before the time asked for, a little earlier still at times: ask that much
        // later, and count a frame due within a few milliseconds as due.
        let t = (now - self.arrived).max(0.0) + 0.005;
        let clock = figcraft_render::images::animation_clock(doc, id, t);
        if let Some((_, next)) = clock {
            crate::repaint_in(ctx, next + f64::from(ctx.input(|i| i.predicted_dt)));
        }
        let key = (self.rev, px_scale.to_bits(), clock.map_or(0, |c| c.0));
        if let Some(t) = self.textures.get(&id)
            && t.key == key
        {
            return Some(t.clone());
        }
        let bounds = figcraft_render::export_bounds(doc, id)?;
        let pix = figcraft_render::render_node_at(doc, id, texture_scale(ctx, bounds, px_scale), t)?;
        let img = crate::render_worker::screen_image(&pix, doc.color_profile);
        let tex = ctx.load_texture(format!("present-{}", id.0), img, egui::TextureOptions::LINEAR);
        let ft = FrameTex { key, tex, bounds };
        self.textures.insert(id, ft.clone());
        Some(ft)
    }
}

/// The most pixels a presented frame's texture holds (bigger prototypes, at 100% or filling the
/// window, are drawn from a smaller texture scaled up).
const MAX_TEXTURE_PIXELS: f64 = 16_777_216.0;

/// The pixel scale, at most `px`, at which `bounds` makes a texture the GPU takes (no side past
/// its limit) of at most [`MAX_TEXTURE_PIXELS`].
pub(super) fn texture_scale(ctx: &egui::Context, bounds: figcraft_geom::Rect, px: f64) -> f64 {
    let side = ctx.input(|i| i.max_texture_side) as f64 - 2.0;
    let (w, h) = (bounds.width().max(1.0), bounds.height().max(1.0));
    let s = px.min(side / w.max(h)).min((MAX_TEXTURE_PIXELS / (w * h)).sqrt());
    if s.is_finite() && s > 0.0 { s } else { 1e-3 }
}

/// The first interaction index on `id` whose trigger satisfies `want`.
pub(super) fn find_trigger(doc: &Document, id: NodeId, want: impl Fn(&Trigger) -> bool) -> Option<usize> {
    doc.node(id)?.interactions.iter().position(|it| want(&it.trigger))
}

#[cfg(test)]
mod layer_animation_tests {
    use super::*;
    use figcraft_doc::proto::{AnimationPreset, Interaction};
    use std::sync::Arc;

    fn scene() -> (Arc<Document>, NodeId, NodeId) {
        let mut doc = Document::new("Layer animation");
        let page = doc.first_page().unwrap();
        let frame = doc.create(page, NodeKind::Frame, "Screen", 50.0, 50.0, 200.0, 100.0).unwrap();
        let child = doc.create(frame, NodeKind::Rectangle, "Child", 20.0, 20.0, 20.0, 20.0).unwrap();
        doc.node_mut(child).unwrap().opacity = 0.8;
        doc.node_mut(frame).unwrap().interactions.push(Interaction {
            trigger: Trigger::GameEvent { event: "tomou-dano".into() },
            actions: vec![Action::PlayAnimation { target: child, preset: AnimationPreset::FadeOut, duration_ms: 1000, easing: Easing::Linear }],
            transition: Transition::Instant,
            duration_ms: 300,
            easing: Easing::EaseOut,
            delay_ms: 0,
        });
        (Arc::new(doc), frame, child)
    }

    #[test]
    fn fired_animation_changes_display_only_and_restores_latest_runtime_state() {
        let (source, frame, child) = scene();
        let mut player = Presenter::new(frame);
        player.sync(&source, 0.0);
        assert!(player.fire(frame, 0, None, 0.0));
        let ctx = egui::Context::default();
        player.animate_layers(&ctx, 0.5, false);
        assert!((player.display().unwrap().node(child).unwrap().opacity - 0.4).abs() < 1e-6);
        assert_eq!(source.node(child).unwrap().opacity, 0.8);
        assert_eq!(player.design.as_ref().unwrap().node(child).unwrap().opacity, 0.8);
        player.design.as_mut().unwrap().node_mut(child).unwrap().opacity = 0.6;
        player.relayout();
        assert!((player.display().unwrap().node(child).unwrap().opacity - 0.3).abs() < 1e-6);
        assert!(player.fire(frame, 0, None, 0.5));
        player.animate_layers(&ctx, 0.5, false);
        assert_eq!(player.display().unwrap().node(child).unwrap().opacity, 0.6, "retrigger starts afresh, not from the prior faded snapshot");
        let active_revision = player.rev;
        player.animate_layers(&ctx, 1.6, false);
        assert!(player.rev > active_revision, "expiration invalidates the final animated texture");
        assert_eq!(player.display().unwrap().node(child).unwrap().opacity, 0.6);
        let restored_revision = player.rev;
        player.animate_layers(&ctx, 2.0, false);
        assert_eq!(player.rev, restored_revision, "idle effects stop invalidating textures");
    }

    #[test]
    fn frame_animation_moves_content_without_recentering_the_viewport() {
        use crate::present::layout::{FULL, layout};
        let (source, frame, _) = scene();
        let mut player = Presenter::new(frame);
        player.sync(&source, 0.0);
        let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let resting = layout(&mut player, area, FULL).unwrap();
        let action = Action::PlayAnimation { target: frame, preset: AnimationPreset::SlideInLeft, duration_ms: 1000, easing: Easing::Linear };
        assert!(player.perform(&action, frame, Transition::Instant, 0.0, Easing::Linear, None, 0.0));
        let ctx = egui::Context::default();
        player.animate_layers(&ctx, 0.5, false);
        let moving = layout(&mut player, area, FULL).unwrap();
        assert_eq!(resting.screen, moving.screen);
        assert_eq!(resting.scale, moving.scale);
        let resting_bounds = resting.disp.world_bounds(frame).unwrap();
        let moving_bounds = moving.disp.world_bounds(frame).unwrap();
        assert!((resting.place.rect(resting_bounds).min.x - moving.place.rect(moving_bounds).min.x - 100.0 * moving.scale as f32).abs() < 1e-3);
    }

    #[test]
    fn overlay_effect_keeps_anchor_and_game_transparency_is_temporary() {
        use crate::present::layout::{FULL, layout};
        let (source, frame, _) = scene();
        let mut doc = (*source).clone();
        let page = doc.first_page().unwrap();
        let overlay = doc.create(page, NodeKind::Frame, "Overlay", 400.0, 0.0, 100.0, 80.0).unwrap();
        for target in [frame, overlay] {
            doc.node_mut(target).unwrap().fills = vec![figcraft_color::Paint::solid(figcraft_color::Color::WHITE)];
        }
        let source = Arc::new(doc);
        let mut player = Presenter::new(frame);
        player.game = Some(crate::game::Play::new());
        player.sync(&source, 0.0);
        let open = Action::OpenOverlay {
            dest: overlay,
            position: OverlayPosition::TopRight,
            close_on_outside: true,
            background: None,
            offset: None,
            anchor: OverlayAnchor::Screen,
        };
        player.perform(&open, frame, Transition::Instant, 0.0, Easing::Linear, None, 0.0);
        let area = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let resting = layout(&mut player, area, FULL).unwrap();
        assert!(resting.disp.node(frame).unwrap().fills.is_empty());
        assert!(!resting.disp.node(overlay).unwrap().fills.is_empty());
        assert!(!source.node(frame).unwrap().fills.is_empty());
        let action = Action::PlayAnimation { target: overlay, preset: AnimationPreset::ScalePop, duration_ms: 1000, easing: Easing::Linear };
        player.perform(&action, overlay, Transition::Instant, 0.0, Easing::Linear, None, 0.0);
        player.animate_layers(&egui::Context::default(), 0.0, false);
        let moving = layout(&mut player, area, FULL).unwrap();
        assert_eq!(resting.overlays[0].2, moving.overlays[0].2, "effects do not resize/reanchor their overlay");
        assert!(moving.disp.world_bounds(overlay).unwrap().width() < resting.disp.world_bounds(overlay).unwrap().width());
    }

    #[test]
    fn game_actions_pause_and_release_without_mutating_the_document() {
        let (source, frame, _) = scene();
        let mut player = Presenter::new(frame);
        player.game = Some(crate::game::Play::new());
        player.sync(&source, 0.0);
        for paused in [true, false] {
            assert!(player.perform(&Action::GamePause { paused }, frame, Transition::Instant, 0.0, Easing::Linear, None, 0.0));
            player.game.as_mut().unwrap().settle(&[]);
            assert_eq!(player.game.as_ref().unwrap().game.paused, paused);
        }
        for captured in [false, true] {
            assert!(player.perform(&Action::GameCursor { captured }, frame, Transition::Instant, 0.0, Easing::Linear, None, 0.0));
            player.game.as_mut().unwrap().settle(&[]);
            assert_eq!(player.game.as_ref().unwrap().captured, captured);
        }
        assert_eq!(player.design.as_ref().unwrap().node(frame), source.node(frame));
    }

    #[test]
    fn pause_and_cursor_set_with_an_overlay_come_back_when_it_closes() {
        let (source, frame, _) = scene();
        let mut doc = (*source).clone();
        let page = doc.first_page().unwrap();
        let menu = doc.create(page, NodeKind::Frame, "Any screen", 400.0, 0.0, 100.0, 80.0).unwrap();
        let inner = doc.create(page, NodeKind::Frame, "Nested", 600.0, 0.0, 100.0, 80.0).unwrap();
        let source = Arc::new(doc);
        let mut player = Presenter::new(frame);
        player.game = Some(crate::game::Play::new());
        player.sync(&source, 0.0);
        let open = |dest| Action::OpenOverlay {
            dest,
            position: OverlayPosition::Center,
            close_on_outside: true,
            background: None,
            offset: None,
            anchor: OverlayAnchor::Screen,
        };
        let run = |player: &mut Presenter, actions: &[Action]| {
            for a in actions {
                player.perform(a, frame, Transition::Instant, 0.0, Easing::Linear, None, 0.0);
            }
            let open: Vec<_> = player.overlays.iter().map(|o| o.id).collect();
            player.game.as_mut().unwrap().settle(&open);
        };
        let state = |player: &Presenter| {
            let play = player.game.as_ref().unwrap();
            (play.game.paused, play.captured)
        };
        // Actions before the overlay in the list still belong to it.
        run(&mut player, &[Action::GamePause { paused: true }, Action::GameCursor { captured: false }, open(menu)]);
        assert_eq!(state(&player), (true, false));
        run(&mut player, &[open(inner), Action::GameCursor { captured: true }]);
        assert_eq!(state(&player), (true, true));
        run(&mut player, &[Action::CloseOverlay]);
        assert_eq!(state(&player), (true, false), "closing the nested overlay undoes only its own change");
        run(&mut player, &[Action::CloseOverlay]);
        assert_eq!(state(&player), (false, true), "closing the menu, however it closes, resumes the game");
        // Without an overlay the change stays.
        run(&mut player, &[Action::GamePause { paused: true }]);
        assert_eq!(state(&player), (true, true));
    }
}
