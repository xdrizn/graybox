//! Play for the file's chosen game (G11): which simulation runs, what draws it, what it reports.
//!
//! The first-person game is `Play::game` and is never touched here. The third-person game is
//! `Play::tps`, set only for a file whose game is third person. Both take the same input
//! (`Play::game.input`, filled each frame from the keys, mouse and controller by `input::read`
//! and the presenter), so the file's bindings keep working.
use super::{Play, renderer};
use figcraft_doc::game::GameKind;
use figcraft_game::tps::{ClipId, Tps};
use serde_json::{Value, json};

/// The line at the bottom of Play while the third-person game runs.
pub(crate) const HELP: &str = "WASD: move  ·  mouse: camera  ·  Space: jump  ·  Shift: run  ·  C: crouch  ·  Ctrl: roll  ·  click: attack  ·  1–4: weapon  ·  F10 / Del: editor";
/// The same while the clip preview (debug builds) shows a clip.
const PREVIEW_HELP: &str = "Clip preview  ·  F6 / Shift+F6: next / previous clip  ·  F7: end  ·  mouse: camera  ·  F10 / Del: editor";

/// The help line for the third-person game.
pub(crate) fn help(play: &Play) -> &'static str {
    if play.tps.as_ref().is_some_and(|tps| tps.hero.animator.previewing().is_some()) { PREVIEW_HELP } else { HELP }
}

/// Debug builds only: F6 shows the hero's next clip on the spot, Shift+F6 the previous one, F7 ends
/// the preview (G11.3, to check the clips by eye; the control channel sends the same keys).
pub(crate) fn debug_keys(play: &mut Play, ctx: &egui::Context) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(tps) = play.tps.as_mut() else {
        return;
    };
    let (next, previous, end) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::F6),
            i.consume_key(egui::Modifiers::SHIFT, egui::Key::F6),
            i.consume_key(egui::Modifiers::NONE, egui::Key::F7),
        )
    });
    if end {
        tps.preview_clip(None);
    } else if next || previous {
        tps.preview_clip(Some(step_clip(tps.hero.animator.previewing(), next)));
    }
}

/// The clip after (or before) `current` in the list, round and round; the first one if none.
fn step_clip(current: Option<ClipId>, forward: bool) -> ClipId {
    let count = ClipId::ALL.len();
    let index = match current.and_then(|clip| ClipId::ALL.iter().position(|c| *c == clip)) {
        Some(index) if forward => (index + 1) % count,
        Some(index) => (index + count - 1) % count,
        None if forward => 0,
        None => count - 1,
    };
    ClipId::ALL.get(index).copied().unwrap_or(ClipId::Idle)
}

/// Make Play run the game the file chose. Called every frame; it only acts when the file's game
/// and the running one differ (the first frame, or after the game was changed in the editor).
pub(crate) fn sync(play: &mut Play, kind: GameKind) {
    match (kind, play.tps.is_some()) {
        (GameKind::ThirdPerson, false) => play.tps = Some(Tps::new()),
        (GameKind::Fps, true) => play.tps = None,
        _ => {}
    }
}

/// Advance the third-person game by `dt` seconds if that is the game being played: the steps run.
pub(crate) fn advance(play: &mut Play, dt: f64) -> Option<u32> {
    let tps = play.tps.as_mut()?;
    tps.input = play.game.input;
    tps.paused = play.game.paused;
    Some(tps.advance(dt))
}

/// Draw the game being played below the screens.
pub(crate) fn paint(ui: &egui::Ui, rect: egui::Rect, play: &Play, theme: &renderer::SceneTheme) {
    match &play.tps {
        Some(tps) => {
            renderer::paint_tps(ui, rect, tps, theme);
            preview_label(ui, rect, tps);
        }
        None => renderer::paint(ui, rect, &play.game, theme),
    }
}

/// The name of the clip the preview shows, top left (debug).
fn preview_label(ui: &egui::Ui, rect: egui::Rect, tps: &Tps) {
    let Some(clip) = tps.hero.animator.previewing() else {
        return;
    };
    let index = ClipId::ALL.iter().position(|c| *c == clip).map_or(0, |i| i + 1);
    let text = format!("{}  ({index}/{})", clip.name(), ClipId::ALL.len());
    let t = crate::theme::Tokens::get(ui.ctx());
    ui.painter().text(rect.min + egui::vec2(16.0, 14.0), egui::Align2::LEFT_TOP, text, crate::theme::regular(16.0), t.text);
}

/// What the control channel reports for a third-person game (None for the first-person one).
pub(crate) fn inspect(play: &Play) -> Option<Value> {
    let tps = play.tps.as_ref()?;
    Some(json!({
        "kind": GameKind::ThirdPerson.id(),
        "position": tps.hero.position.to_array(), "facing": tps.hero.facing,
        "speed": tps.hero.speed(), "grounded": tps.hero.grounded,
        "cameraYaw": tps.camera.yaw, "cameraPitch": tps.camera.pitch,
        "clip": tps.hero.animator.current().name(), "preview": tps.hero.animator.previewing().map(ClipId::name),
        "weapon": tps.hero.animator.weapon().name(), "weaponInHand": tps.hero.animator.in_hand(),
        "action": tps.hero.moves.action.name(), "stamina": tps.hero.moves.stamina,
        "strike": tps.hero.combat.striking().map(ClipId::name), "aiming": tps.hero.combat.aiming(),
        "paused": tps.paused, "captured": play.captured, "ticks": tps.ticks(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_game::STEP_SECS;

    #[test]
    fn play_runs_the_game_the_file_chose() {
        let mut play = Play::new();
        assert!(play.tps.is_none(), "first person unless told otherwise");
        sync(&mut play, GameKind::ThirdPerson);
        assert!(play.tps.is_some());
        let first = play.tps.as_ref().map(|t| t.hero.position);
        sync(&mut play, GameKind::ThirdPerson);
        assert_eq!(play.tps.as_ref().map(|t| t.hero.position), first, "asking again changes nothing");
        sync(&mut play, GameKind::Fps);
        assert!(play.tps.is_none());
    }

    #[test]
    fn the_third_person_game_moves_by_the_shared_input_and_leaves_the_first_person_one_alone() {
        let mut play = Play::new();
        sync(&mut play, GameKind::ThirdPerson);
        play.frame(0.0);
        let start = play.tps.as_ref().unwrap().hero.position;
        play.game.input.forward = 1.0;
        // Frames of 1/60 s (a frame runs at most 5 steps).
        let steps: u32 = (1..=30).map(|i| play.frame(f64::from(i) / 60.0)).sum();
        assert_eq!(steps, 30);
        let tps = play.tps.as_ref().unwrap();
        assert!(tps.hero.position.z < start.z - 1.0, "walked forward from {start:?}: {:?}", tps.hero.position);
        assert_eq!(play.game.ticks(), 0, "the first-person simulation did not run");
        assert!((tps.time() - 0.5).abs() < STEP_SECS);
        // Paused (a screen asked for it): it stands still.
        play.game.paused = true;
        let before = play.tps.as_ref().unwrap().hero.clone();
        assert_eq!(play.frame(1.0), 0, "paused");
        assert_eq!(play.tps.as_ref().unwrap().hero, before);
    }

    #[test]
    fn the_first_person_game_still_runs_when_no_other_was_chosen() {
        let mut play = Play::new();
        play.frame(0.0);
        play.game.input.forward = 1.0;
        let steps: u32 = (1..=30).map(|i| play.frame(f64::from(i) / 60.0)).sum();
        assert_eq!(steps, 30);
        assert_eq!(play.game.ticks(), 30);
        assert!(inspect(&play).is_none());
    }

    #[test]
    fn the_control_channel_reports_the_third_person_game() {
        let mut play = Play::new();
        sync(&mut play, GameKind::ThirdPerson);
        let report = play.inspect();
        assert_eq!(report["kind"], "thirdPerson");
        assert_eq!(report["grounded"], true, "it starts standing on the ground");
        assert!(report["cameraYaw"].is_number());
    }
}
