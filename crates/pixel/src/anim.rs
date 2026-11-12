//! Playing a sprite (G16.4): which frame shows at a moment, for the whole sprite or one tag, in
//! its direction (forward, reverse or ping-pong), looping or played a set number of times.

use crate::sprite::{Direction, Sprite, Tag};

/// The frame indices one run of `tag` (or of every frame when None) shows, in order. Ping-pong
/// goes there and back without showing either end twice in a row.
pub fn sequence(sprite: &Sprite, tag: Option<&Tag>) -> Vec<usize> {
    let last = sprite.frames.len().saturating_sub(1);
    let (from, to, direction) = match tag {
        Some(t) => (t.from.min(last), t.to.min(last), t.direction),
        None => (0, last, Direction::Forward),
    };
    let (from, to) = (from.min(to), to.max(from));
    let forward: Vec<usize> = (from..=to).collect();
    match direction {
        Direction::Forward => forward,
        Direction::Reverse => forward.into_iter().rev().collect(),
        Direction::PingPong => {
            let back = forward.iter().rev().skip(1).take(forward.len().saturating_sub(2)).copied().collect::<Vec<_>>();
            forward.into_iter().chain(back).collect()
        }
    }
}

/// The tag the sprite plays (see [`Sprite::play`]); None plays every frame.
pub fn played(sprite: &Sprite) -> Option<&Tag> {
    sprite.play.as_deref().and_then(|name| sprite.tag(name))
}

/// The frame `seq` shows `t` seconds after it starts, and the seconds until it shows another
/// (None when it never will: one frame, or the last run is over). `repeat` 0 plays forever.
pub fn frame_in(sprite: &Sprite, seq: &[usize], repeat: u32, t: f64) -> Option<(usize, Option<f64>)> {
    let first = *seq.first()?;
    let ms = |i: usize| sprite.frames.get(i).map_or(100.0, |f| f64::from(f.duration.max(1)));
    let cycle: f64 = seq.iter().map(|i| ms(*i)).sum();
    if seq.len() < 2 || cycle <= 0.0 || !cycle.is_finite() {
        return Some((first, None));
    }
    let t = if t.is_finite() { (t * 1000.0).max(0.0) } else { 0.0 };
    if repeat > 0 && t >= cycle * f64::from(repeat) {
        return Some((*seq.last()?, None));
    }
    let mut at = t % cycle;
    for i in seq {
        let d = ms(*i);
        if at < d {
            return Some((*i, Some((d - at) / 1000.0)));
        }
        at -= d;
    }
    Some((*seq.last()?, Some(0.001)))
}

/// The frame the sprite shows `t` seconds into playing what it plays, and the seconds until it
/// changes (None when it doesn't move).
pub fn frame_at(sprite: &Sprite, t: f64) -> (usize, Option<f64>) {
    let tag = played(sprite);
    let seq = sequence(sprite, tag);
    frame_in(sprite, &seq, tag.map_or(0, |t| t.repeat), t).unwrap_or((0, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::{Direction, Sprite, Tag};

    /// A sprite with frames lasting `ms` each.
    fn sprite(ms: &[u32]) -> Sprite {
        let mut s = Sprite::new(2, 2, 1).unwrap();
        s.frames[0].duration = ms[0];
        for (i, d) in ms.iter().enumerate().skip(1) {
            s.insert_frame(i, *d).unwrap();
        }
        s
    }

    fn tag(from: usize, to: usize, direction: Direction, repeat: u32) -> Tag {
        Tag { name: "t".into(), from, to, direction, repeat }
    }

    #[test]
    fn a_sequence_runs_its_tag_in_its_direction() {
        let s = sprite(&[100, 100, 100, 100, 100]);
        assert_eq!(sequence(&s, None), [0, 1, 2, 3, 4]);
        assert_eq!(sequence(&s, Some(&tag(1, 3, Direction::Forward, 0))), [1, 2, 3]);
        assert_eq!(sequence(&s, Some(&tag(1, 3, Direction::Reverse, 0))), [3, 2, 1]);
        assert_eq!(sequence(&s, Some(&tag(1, 4, Direction::PingPong, 0))), [1, 2, 3, 4, 3, 2], "the ends show once a turn");
        assert_eq!(sequence(&s, Some(&tag(2, 2, Direction::PingPong, 0))), [2]);
        assert_eq!(sequence(&s, Some(&tag(3, 99, Direction::Forward, 0))), [3, 4], "a tag past the end stops at the last frame");
    }

    #[test]
    fn the_frame_at_a_moment_follows_each_frames_duration_and_loops() {
        let s = sprite(&[100, 300, 50]);
        let seq = [0, 1, 2];
        // 0–100 ms frame 0, 100–400 frame 1, 400–450 frame 2, then again.
        assert_eq!(frame_in(&s, &seq, 0, 0.0), Some((0, Some(0.1))));
        assert_eq!(frame_in(&s, &seq, 0, 0.25).map(|f| f.0), Some(1));
        let (f, next) = frame_in(&s, &seq, 0, 0.42).unwrap();
        assert_eq!(f, 2);
        assert!((next.unwrap() - 0.03).abs() < 1e-9);
        assert_eq!(frame_in(&s, &seq, 0, 0.46).map(|f| f.0), Some(0), "450 ms later it starts over");
        assert_eq!(frame_in(&s, &[1], 0, 5.0), Some((1, None)), "one frame never moves");
        assert_eq!(frame_in(&s, &seq, 0, f64::NAN).map(|f| f.0), Some(0));
        assert_eq!(frame_in(&s, &[], 0, 1.0), None);
    }

    #[test]
    fn a_tag_played_a_number_of_times_stops_on_its_last_frame() {
        let s = sprite(&[100, 100, 100]);
        let seq = [0, 1, 2];
        assert_eq!(frame_in(&s, &seq, 2, 0.35).map(|f| f.0), Some(0), "second run");
        assert_eq!(frame_in(&s, &seq, 2, 0.61), Some((2, None)), "after two runs it holds the last frame");
    }

    #[test]
    fn a_sprite_plays_its_chosen_tag_and_shows_its_first_frame_at_rest() {
        let mut s = sprite(&[100, 100, 100, 100]);
        assert_eq!(frame_at(&s, 0.15), (1, Some(0.05)), "no tag chosen: every frame");
        s.tags.push(Tag { name: "hover".into(), from: 2, to: 3, direction: Direction::Reverse, repeat: 0 });
        s.play = Some("hover".into());
        assert_eq!(frame_at(&s, 0.0).0, 3, "reverse starts at its last frame");
        assert_eq!(frame_at(&s, 0.15).0, 2);
        s.play = Some("gone".into());
        assert_eq!(frame_at(&s, 0.15).0, 1, "a tag that no longer exists plays every frame");
    }
}
