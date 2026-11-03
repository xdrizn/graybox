//! Temporary presenter-only effects, sampled onto the existing copy-on-write display document.
//! A target owns one effect: retriggering replaces its clock, never its designed transform.
use std::collections::BTreeMap;

use figcraft_doc::proto::{AnimationPreset, Easing, MAX_ANIMATION_MS, valid_easing};
use figcraft_doc::{Document, NodeId, NodeKind};
use figcraft_geom::Affine;

#[derive(Default)]
pub(crate) struct Animations {
    tracks: BTreeMap<NodeId, Track>,
    reduced: bool,
}

struct Track {
    preset: AnimationPreset,
    start: f64,
    seconds: f64,
    easing: Easing,
}

#[derive(Debug)]
struct Effect {
    x: f64,
    y: f64,
    scale: f64,
    opacity: f32,
}

impl Animations {
    pub(crate) fn clear(&mut self) {
        self.tracks.clear();
    }

    pub(crate) fn active(&self) -> bool {
        !self.tracks.is_empty()
    }

    pub(crate) fn start(&mut self, doc: &Document, target: NodeId, preset: AnimationPreset, duration_ms: u32, easing: Easing, now: f64) -> bool {
        let Some(node) = doc.node(target) else { return false };
        if !now.is_finite()
            || duration_ms == 0
            || duration_ms > MAX_ANIMATION_MS
            || !valid_easing(easing)
            || matches!(node.kind, NodeKind::Page { .. })
            || ![node.width, node.height].iter().all(|v| v.is_finite())
            || !node.transform.as_coeffs().iter().all(|v| v.is_finite())
        {
            return false;
        }
        self.tracks.insert(target, Track { preset, start: now, seconds: easing.seconds(duration_ms), easing });
        true
    }

    /// True while moving, and once after the last effect expires so its texture is restored.
    pub(crate) fn advance(&mut self, now: f64, reduced: bool) -> bool {
        self.reduced = reduced;
        let changed = self.active();
        self.tracks.retain(|_, track| now.is_finite() && now <= track.start + track.seconds);
        changed
    }

    pub(crate) fn apply(&self, doc: &mut Document, now: f64) {
        for (target, track) in &self.tracks {
            let Some(node) = doc.node_mut(*target) else { continue };
            let raw = ((now - track.start) / track.seconds.max(0.001)).clamp(0.0, 1.0);
            let effect = sample(track.preset, track.easing.eval(raw), node.width, node.height, self.reduced);
            // Children inherit this transform and group alpha through the normal renderer. Editing
            // every descendant would double-transform nested groups and multiply their alpha twice.
            let center = (node.width / 2.0, node.height / 2.0);
            node.transform = node.transform
                * Affine::translate((effect.x, effect.y))
                * Affine::translate(center)
                * Affine::scale(effect.scale)
                * Affine::translate((-center.0, -center.1));
            node.opacity *= effect.opacity;
        }
    }
}

fn sample(preset: AnimationPreset, progress: f64, width: f64, height: f64, reduced: bool) -> Effect {
    use AnimationPreset as P;
    let p = progress.clamp(0.0, 1.0);
    let wave = (std::f64::consts::PI * p).sin().max(0.0);
    let mut effect = Effect { x: 0.0, y: 0.0, scale: 1.0, opacity: 1.0 };
    match preset {
        P::FadeIn => effect.opacity = p as f32,
        P::FadeOut => effect.opacity = (1.0 - p) as f32,
        P::SlideInLeft => effect.x = -width * (1.0 - progress),
        P::SlideInRight => effect.x = width * (1.0 - progress),
        P::SlideInTop => effect.y = -height * (1.0 - progress),
        P::SlideInBottom => effect.y = height * (1.0 - progress),
        P::SlideOutLeft => effect.x = -width * progress,
        P::SlideOutRight => effect.x = width * progress,
        P::SlideOutTop => effect.y = -height * progress,
        P::SlideOutBottom => effect.y = height * progress,
        P::ScalePop => {
            effect.scale = 0.95 + 0.05 * progress;
            effect.opacity = p as f32;
        }
        P::Pulse => effect.scale = 1.0 + 0.08 * wave,
        P::Shake => effect.x = width.min(100.0) * 0.08 * wave * (p * std::f64::consts::TAU * 4.0).sin(),
        P::Bounce => effect.y = -height.min(100.0) * 0.2 * wave * (p * std::f64::consts::PI * 2.0).sin().abs(),
        P::Blink => effect.opacity = (1.0 - 0.8 * (p * std::f64::consts::PI * 2.0).sin().powi(2)) as f32,
    }
    if reduced {
        effect.x = 0.0;
        effect.y = 0.0;
        effect.scale = 1.0;
        effect.opacity = match preset {
            P::SlideInLeft | P::SlideInRight | P::SlideInTop | P::SlideInBottom => p as f32,
            P::SlideOutLeft | P::SlideOutRight | P::SlideOutTop | P::SlideOutBottom => (1.0 - p) as f32,
            P::Pulse | P::Shake | P::Bounce | P::Blink => (1.0 - 0.15 * wave) as f32,
            _ => effect.opacity,
        };
    }
    effect
}

/// Resolve a main-component target to the particular instance that fired the action.
pub(crate) fn target_for(doc: &Document, firing: NodeId, target: NodeId) -> NodeId {
    for instance in std::iter::once(firing).chain(doc.ancestors(firing)) {
        let Some(NodeKind::Instance { component, .. }) = doc.node(instance).map(|n| &n.kind) else { continue };
        if target == *component
            || (doc.node(target).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. }))
                && figcraft_engine::components::variant_set(doc, target)
                    .is_some_and(|set| Some(set) == figcraft_engine::components::variant_set(doc, *component)))
        {
            return instance;
        }
        if let Some(copy) = doc.subtree(instance).into_iter().find(|id| doc.node(*id).is_some_and(|n| n.source == Some(target))) {
            return copy;
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("Effects");
        let page = doc.first_page().unwrap();
        let group = doc.create(page, NodeKind::Group, "Target", 20.0, 30.0, 200.0, 100.0).unwrap();
        let child = doc.create(group, NodeKind::Rectangle, "Child", 5.0, 10.0, 20.0, 20.0).unwrap();
        doc.node_mut(group).unwrap().opacity = 0.6;
        (doc, group, child)
    }

    #[test]
    fn retrigger_restarts_from_design_and_completion_restores_entire_subtree() {
        let (doc, target, child) = scene();
        let mut animations = Animations::default();
        assert!(animations.start(&doc, target, AnimationPreset::SlideInLeft, 1000, Easing::Linear, 2.0));
        let mut middle = doc.clone();
        animations.apply(&mut middle, 2.5);
        assert_eq!(middle.node(target).unwrap().transform.as_coeffs()[4], -80.0);
        assert_eq!(middle.world_transform(child).as_coeffs()[4], -75.0);
        assert_eq!(doc.node(target).unwrap().transform.as_coeffs()[4], 20.0);
        assert!(animations.start(&doc, target, AnimationPreset::SlideInLeft, 1000, Easing::Linear, 2.5));
        let mut restarted = doc.clone();
        animations.apply(&mut restarted, 2.5);
        assert_eq!(restarted.node(target).unwrap().transform.as_coeffs()[4], -180.0);
        assert!(animations.advance(3.6, false));
        assert!(!animations.active());
        let mut restored = doc.clone();
        animations.apply(&mut restored, 3.6);
        assert_eq!(restored.node(target), doc.node(target));
        assert_eq!(restored.node(child), doc.node(child));
        assert!(!animations.advance(4.0, false), "idle effects stop requesting frames");
    }

    #[test]
    fn fade_multiplies_designed_opacity_and_reduced_slide_keeps_final_effect() {
        let (doc, target, _) = scene();
        let mut animations = Animations::default();
        assert!(animations.start(&doc, target, AnimationPreset::FadeOut, 1000, Easing::Linear, 0.0));
        let mut middle = doc.clone();
        animations.apply(&mut middle, 0.5);
        assert!((middle.node(target).unwrap().opacity - 0.3).abs() < 1e-6);
        let mut end = doc.clone();
        animations.apply(&mut end, 1.0);
        assert_eq!(end.node(target).unwrap().opacity, 0.0);
        animations.start(&doc, target, AnimationPreset::SlideOutRight, 1000, Easing::Linear, 0.0);
        animations.advance(1.0, true);
        let mut reduced = doc.clone();
        animations.apply(&mut reduced, 1.0);
        assert_eq!(reduced.node(target).unwrap().transform, doc.node(target).unwrap().transform);
        assert_eq!(reduced.node(target).unwrap().opacity, 0.0);
        animations.advance(1.1, true);
        let mut restored = doc.clone();
        animations.apply(&mut restored, 1.1);
        assert_eq!(restored.node(target), doc.node(target));
    }

    #[test]
    fn easing_and_validation_use_upstream_curves() {
        let (doc, target, _) = scene();
        let mut animations = Animations::default();
        assert!(animations.start(&doc, target, AnimationPreset::FadeIn, 1000, Easing::EaseIn, 0.0));
        let mut eased = doc.clone();
        animations.apply(&mut eased, 0.5);
        assert!((eased.node(target).unwrap().opacity - 0.6 * Easing::EaseIn.eval(0.5) as f32).abs() < 1e-6);
        for duration in [0, MAX_ANIMATION_MS + 1] {
            assert!(!animations.start(&doc, target, AnimationPreset::Pulse, duration, Easing::Linear, 0.0));
        }
        assert!(!animations.start(&doc, target, AnimationPreset::Pulse, 100, Easing::Linear, f64::NAN));
        assert!(!animations.start(
            &doc,
            target,
            AnimationPreset::Pulse,
            100,
            Easing::CustomSpring { mass: f64::INFINITY, stiffness: 1.0, damping: 1.0 },
            0.0
        ));
        assert!(!animations.start(&doc, doc.first_page().unwrap(), AnimationPreset::Pulse, 100, Easing::Linear, 0.0));
    }

    #[test]
    fn component_effect_targets_resolve_to_the_firing_instance_only() {
        let (mut doc, _, _) = scene();
        let page = doc.first_page().unwrap();
        let main = doc
            .create(
                page,
                NodeKind::Component { description: String::new(), props: Default::default(), prop_order: Vec::new() },
                "Button",
                0.0,
                0.0,
                100.0,
                50.0,
            )
            .unwrap();
        let icon = doc.create(main, NodeKind::Rectangle, "Icon", 4.0, 5.0, 20.0, 20.0).unwrap();
        let a = figcraft_engine::components::create_instance(&mut doc, main, page, 100.0, 0.0).unwrap();
        let b = figcraft_engine::components::create_instance(&mut doc, main, page, 200.0, 0.0).unwrap();
        let a_icon = doc.node(a).unwrap().children[0];
        assert_eq!(target_for(&doc, a, main), a);
        assert_eq!(target_for(&doc, b, main), b);
        assert_eq!(target_for(&doc, a_icon, main), a);
        assert_eq!(target_for(&doc, a, icon), a_icon);
        assert_eq!(target_for(&doc, main, icon), icon);
        let mut animations = Animations::default();
        animations.start(&doc, target_for(&doc, a, main), AnimationPreset::FadeIn, 1000, Easing::Linear, 0.0);
        let mut shown = doc.clone();
        animations.apply(&mut shown, 0.0);
        assert_eq!(shown.node(a).unwrap().opacity, 0.0);
        assert_eq!(shown.node(b).unwrap().opacity, 1.0);
        assert_eq!(shown.node(main).unwrap().opacity, 1.0);
    }

    #[test]
    fn every_preset_has_finite_endpoints_and_reduced_motion_has_no_displacement() {
        for preset in AnimationPreset::ALL {
            for p in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let normal = sample(preset, p, 200.0, 100.0, false);
                assert!([normal.x, normal.y, normal.scale, f64::from(normal.opacity)].iter().all(|v| v.is_finite()));
                let reduced = sample(preset, p, 200.0, 100.0, true);
                assert_eq!((reduced.x, reduced.y, reduced.scale), (0.0, 0.0, 1.0));
            }
        }
        assert_eq!(sample(AnimationPreset::FadeIn, 0.0, 1.0, 1.0, false).opacity, 0.0);
        assert_eq!(sample(AnimationPreset::FadeIn, 1.0, 1.0, 1.0, false).opacity, 1.0);
        assert_eq!(sample(AnimationPreset::FadeOut, 1.0, 1.0, 1.0, false).opacity, 0.0);
        assert_eq!(sample(AnimationPreset::SlideInLeft, 0.0, 200.0, 100.0, false).x, -200.0);
        assert_eq!(sample(AnimationPreset::SlideOutBottom, 1.0, 200.0, 100.0, false).y, 100.0);
        for preset in [AnimationPreset::ScalePop, AnimationPreset::Pulse, AnimationPreset::Shake, AnimationPreset::Bounce, AnimationPreset::Blink] {
            let end = sample(preset, 1.0, 200.0, 100.0, false);
            assert!(end.x.abs() < 1e-9 && end.y.abs() < 1e-9 && (end.scale - 1.0).abs() < 1e-9);
            assert_eq!(end.opacity, 1.0);
        }
    }
}
