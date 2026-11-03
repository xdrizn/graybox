use super::{FixedClock, Game, MAX_STEPS, STEP_SECS};

#[test]
fn a_second_of_frames_is_sixty_steps_at_any_frame_rate() {
    for fps in [30.0, 60.0, 144.0, 240.0] {
        let mut g = Game::new();
        let frames = fps as usize;
        let steps: u32 = (0..frames).map(|_| g.advance(1.0 / fps)).sum();
        // Rounding may leave the last step a hair short of due.
        assert!((59..=60).contains(&steps), "{fps} fps: {steps} steps");
        assert_eq!(g.ticks(), u64::from(steps));
        assert!((g.time() - f64::from(steps) * STEP_SECS).abs() < 1e-9);
    }
}

#[test]
fn time_shorter_than_a_step_waits_for_the_next() {
    let mut c = FixedClock::default();
    assert_eq!(c.advance(STEP_SECS * 0.4), 0);
    assert!((c.alpha() - 0.4).abs() < 1e-9);
    assert_eq!(c.advance(STEP_SECS * 0.7), 1);
    assert!((c.alpha() - 0.1).abs() < 1e-9);
}

#[test]
fn a_late_frame_runs_a_few_steps_and_drops_the_backlog() {
    let mut g = Game::new();
    assert_eq!(g.advance(10.0), MAX_STEPS);
    assert_eq!(g.alpha(), 0.0, "the backlog is gone");
    // The next ordinary frame is one ordinary step, not a catch-up.
    assert_eq!(g.advance(STEP_SECS), 1);
}

#[test]
fn junk_frame_times_change_nothing() {
    for dt in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut g = Game::new();
        assert_eq!(g.advance(dt), 0, "{dt}");
        assert_eq!(g, Game::new(), "{dt}");
    }
    // Huge but finite: bounded, and the clock stays usable.
    let mut g = Game::new();
    assert_eq!(g.advance(f64::MAX), MAX_STEPS);
    assert_eq!(g.alpha(), 0.0);
    assert_eq!(g.advance(STEP_SECS * 0.5), 0);
    assert_eq!(g.advance(f64::MAX), MAX_STEPS, "a second huge frame doesn't overflow");
}
