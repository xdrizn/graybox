use super::Play;
use figcraft_game::STEP_SECS;

#[test]
fn the_first_frame_starts_the_clock_and_later_ones_advance_it() {
    let mut p = Play::new();
    assert_eq!(p.frame(100.0), 0, "no time has passed in the game yet");
    assert_eq!(p.frame(100.0 + STEP_SECS * 3.5), 3);
    assert_eq!(p.game().ticks(), 3);
}

#[test]
fn a_clock_going_backwards_or_junk_times_change_nothing() {
    let mut p = Play::new();
    p.frame(5.0);
    assert_eq!(p.frame(4.0), 0);
    assert_eq!(p.frame(f64::NAN), 0);
    // A junk time isn't kept: the next frame measures from the last good one.
    assert_eq!(p.frame(4.0 + STEP_SECS * 1.5), 1);
    assert_eq!(p.game().ticks(), 1);
}
