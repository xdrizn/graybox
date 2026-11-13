//! Play for the file's chosen game (G11): which simulation runs, what draws it, what it reports.
//!
//! The first-person game is `Play::game` and is never touched here. The third-person game is
//! `Play::tps`, set only for a file whose game is third person. Both take the same input
//! (`Play::game.input`, filled each frame from the keys, mouse and controller by `input::read`
//! and the presenter), so the file's bindings keep working.
use super::{Play, renderer};
use figcraft_doc::game::GameKind;
use figcraft_game::GameEvent;
use figcraft_game::tps::{Action, ClipId, STAMINA_MAX, Tps, Weapon};
use serde_json::{Value, json};

/// The line at the bottom of Play while the third-person game runs.
pub(crate) const HELP: &str = "WASD: move  ·  mouse: camera  ·  Space: jump  ·  Shift: run  ·  C: crouch  ·  Ctrl: roll  ·  click: attack  ·  1–4: weapon  ·  right click: aim (bow)  ·  mouse 4 / 5: shoulder  ·  F10 / Del: editor";
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

/// Draw the game being played below the screens. `crosshair`: the third-person game draws its own
/// in aim mode (off when the file's UI draws one).
pub(crate) fn paint(ui: &egui::Ui, rect: egui::Rect, play: &Play, theme: &renderer::SceneTheme, crosshair: bool) {
    match &play.tps {
        Some(tps) => {
            renderer::paint_tps(ui, rect, tps, theme);
            if crosshair {
                paint_crosshair(ui.painter(), rect, tps);
            }
            preview_label(ui, rect, tps);
        }
        None => renderer::paint(ui, rect, &play.game, theme),
    }
}

/// G11.6: a small neutral dot in the middle of the screen while aiming (where arrows go), fading
/// in and out with the camera. Light with a dark ring, so it shows on sky and on grass.
fn paint_crosshair(painter: &egui::Painter, rect: egui::Rect, tps: &Tps) {
    let shown = crosshair_strength(tps);
    if shown < 0.05 {
        return;
    }
    let alpha = |a: f32| (a * shown.clamp(0.0, 1.0) * 255.0).round() as u8;
    let centre = rect.center();
    painter.circle_filled(centre, 3.5, egui::Color32::from_black_alpha(alpha(0.45)));
    painter.circle_filled(centre, 2.0, egui::Color32::from_white_alpha(alpha(0.95)));
}

/// How much the crosshair shows (0..1): only in aim mode, as far as the camera has come in.
fn crosshair_strength(tps: &Tps) -> f32 {
    let shown = if tps.hero.combat.aiming() { tps.camera.aim_blend() } else { 0.0 };
    if shown.is_finite() { shown.clamp(0.0, 1.0) } else { 0.0 }
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

/// The Game variables the third-person game keeps up to date in Play (G11.5): the same names as
/// the first-person game where they mean the same, plus `stamina`, `staminaMax`, `escalando` and
/// `mirando`. Fixed arrays, so publishing every frame allocates nothing; a name the file has no
/// Game variable for is skipped.
pub(crate) struct Variables<'a> {
    pub numbers: [(&'static str, f64); 5],
    pub flags: [(&'static str, bool); 4],
    pub texts: [(&'static str, &'a str); 2],
}

/// The weapon's name for the HUD.
fn weapon_label(weapon: Weapon) -> &'static str {
    match weapon {
        Weapon::None => "Unarmed",
        Weapon::Sword => "Sword",
        Weapon::Spear => "Spear",
        Weapon::Club => "Club",
        Weapon::Bow => "Bow",
    }
}

/// The values of the Game variables now, if the third-person game is the one playing.
pub(crate) fn variables(play: &Play) -> Option<Variables<'_>> {
    let tps = play.tps.as_ref()?;
    let hero = &tps.hero;
    let vitals = &hero.vitals;
    let action = hero.moves.action;
    let climbing = matches!(action, Action::Ladder { .. } | Action::Wall { .. } | Action::Mantle { .. });
    let crouching = tps.input.crouch && action == Action::Free && hero.grounded;
    Some(Variables {
        numbers: [
            ("vida", f64::from(vitals.health)),
            ("vidaMax", f64::from(vitals.max)),
            ("ultimoDano", f64::from(vitals.last_damage)),
            ("stamina", f64::from(hero.moves.stamina)),
            ("staminaMax", f64::from(STAMINA_MAX)),
        ],
        flags: [("agachado", crouching), ("escalando", climbing), ("mirando", hero.combat.aiming()), ("emChamas", false)],
        texts: [("atacante", vitals.attacker), ("arma", weapon_label(hero.animator.weapon()))],
    })
}

/// How full a game bar following `name` is (0..1), if the third-person game is playing.
pub(crate) fn bar_share(play: &Play, name: &str) -> Option<f64> {
    let hero = &play.tps.as_ref()?.hero;
    let (value, max) = match name {
        "vida" => (hero.vitals.health, hero.vitals.max),
        "stamina" => (hero.moves.stamina, STAMINA_MAX),
        // No ammo in this game: those bars stay full.
        _ => (1.0, 1.0),
    };
    Some(if max > 0.0 && value.is_finite() { f64::from((value / max).clamp(0.0, 1.0)) } else { 0.0 })
}

/// The third-person hero is down (Enter respawns it).
pub(crate) fn dead(play: &Play) -> bool {
    play.tps.as_ref().is_some_and(Tps::dead)
}

/// A dead hero comes back at the start when the respawn key was pressed.
pub(crate) fn respawn_on_request(play: &mut Play) {
    if let Some(tps) = play.tps.as_mut()
        && tps.dead()
        && play.game.input.respawn
    {
        tps.respawn();
        play.game.input.respawn = false;
    }
}

/// The game events since the last frame, from the game being played.
pub(crate) fn drain_events(play: &mut Play) -> Vec<GameEvent> {
    match play.tps.as_mut() {
        Some(tps) => tps.drain_events(),
        None => play.game.drain_events(),
    }
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
        "health": tps.hero.vitals.health, "dead": tps.dead(), "attacker": tps.hero.vitals.attacker,
        "foes": tps.foes.iter().map(|foe| json!({
            "kind": foe.kind.name(), "state": foe.state.name(), "health": foe.health, "position": foe.position.to_array(),
        })).collect::<Vec<_>>(),
        "arrows": tps.arrows.len(),
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

    #[test]
    fn the_hud_reads_the_third_person_hero() {
        let mut play = Play::new();
        assert!(variables(&play).is_none() && bar_share(&play, "vida").is_none(), "first person publishes its own");
        sync(&mut play, GameKind::ThirdPerson);
        let values = variables(&play).unwrap();
        let number = |name: &str| values.numbers.iter().find(|(n, _)| *n == name).map(|(_, v)| *v);
        assert_eq!(number("vida"), Some(100.0));
        assert_eq!(number("stamina"), Some(100.0));
        assert_eq!(number("staminaMax"), Some(f64::from(STAMINA_MAX)));
        assert!(values.texts.contains(&("arma", "Sword")), "the sword on its back");
        assert!(values.flags.contains(&("escalando", false)) && values.flags.contains(&("mirando", false)));
        let tps = play.tps.as_mut().unwrap();
        tps.hero.vitals.health = 25.0;
        tps.hero.moves.stamina = 50.0;
        tps.hero.animator.set_weapon(Weapon::Bow, false);
        assert_eq!(bar_share(&play, "vida"), Some(0.25));
        assert_eq!(bar_share(&play, "stamina"), Some(0.5));
        assert_eq!(bar_share(&play, "municao"), Some(1.0), "no ammo: full");
        assert!(variables(&play).unwrap().texts.contains(&("arma", "Bow")));
    }

    #[test]
    fn aiming_the_bow_shows_the_crosshair_and_sets_mirando() {
        let mut play = Play::new();
        sync(&mut play, GameKind::ThirdPerson);
        play.frame(0.0);
        play.game.input.weapon = 4;
        play.frame(1.0 / 60.0);
        play.game.input.weapon = 0;
        let mut time = 1.0 / 60.0;
        let mut run = |play: &mut Play, seconds: f64| {
            for _ in 0..(seconds * 60.0).round() as usize {
                time += 1.0 / 60.0;
                play.frame(time);
            }
        };
        run(&mut play, 1.0);
        let tps = play.tps.as_ref().unwrap();
        assert_eq!(crosshair_strength(tps), 0.0, "no crosshair outside aim mode");
        assert!(variables(&play).unwrap().flags.contains(&("mirando", false)));
        play.game.input.aim = true;
        run(&mut play, 0.8);
        assert!(crosshair_strength(play.tps.as_ref().unwrap()) > 0.95);
        assert!(variables(&play).unwrap().flags.contains(&("mirando", true)));
        assert_eq!(inspect(&play).unwrap()["aiming"], true);
        play.game.input.aim = false;
        run(&mut play, 1.0 / 60.0);
        assert_eq!(crosshair_strength(play.tps.as_ref().unwrap()), 0.0, "gone the moment aim ends");
    }

    #[test]
    fn a_dead_hero_respawns_on_the_respawn_key_and_the_events_reach_the_screens() {
        let mut play = Play::new();
        sync(&mut play, GameKind::ThirdPerson);
        play.game.input.respawn = true;
        respawn_on_request(&mut play);
        assert!(drain_events(&mut play).is_empty(), "alive: the key does nothing");
        let tps = play.tps.as_mut().unwrap();
        tps.hero.moves.action = Action::Dead { time: 0.0 };
        tps.hero.vitals.health = 0.0;
        assert!(dead(&play));
        play.game.input.respawn = true;
        respawn_on_request(&mut play);
        assert!(!dead(&play) && !play.game.input.respawn);
        assert_eq!(play.tps.as_ref().unwrap().hero.vitals.health, 100.0);
        let events = drain_events(&mut play);
        assert!(events.contains(&GameEvent::Respawned) && events.contains(&GameEvent::HealthChanged), "{events:?}");
        assert!(drain_events(&mut play).is_empty(), "taken once");
    }
}
