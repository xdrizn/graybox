//! Focus movement inside a navigable menu (list, row or grid), independent of any UI toolkit.

/// A direction pressed on the keyboard, the D-pad or the stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// How focus behaves at the edges and on disabled items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavOptions {
    /// Past the last item of a row or column, come back to its first one.
    pub wrap: bool,
    /// Disabled items are passed over instead of taking focus.
    pub skip_disabled: bool,
}

/// The menu's items in row-major order (hidden children are not passed). `columns`: list = 1,
/// row = item count, grid = its column count, wrapping flow = the items on its first line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuGrid {
    pub columns: usize,
    pub enabled: Vec<bool>,
}

/// First repeat when a direction is held, then the interval between repeats (seconds).
pub const REPEAT_DELAY: f64 = 0.4;
pub const REPEAT_EVERY: f64 = 0.12;

impl MenuGrid {
    fn columns(&self) -> usize {
        self.columns.clamp(1, self.enabled.len().max(1))
    }

    fn enabled(&self, i: usize) -> bool {
        self.enabled.get(i).copied().unwrap_or(false)
    }

    /// One move from `i` (in range), or `None` at an edge without wrapping.
    fn neighbour(&self, i: usize, dir: Dir, wrap: bool) -> Option<usize> {
        let len = self.enabled.len();
        let cols = self.columns();
        let last = len.saturating_sub(1);
        let (row, col) = (i / cols, i % cols);
        let last_row = last / cols;
        let row_start = row * cols;
        let row_end = (row_start + cols - 1).min(last);
        match dir {
            Dir::Right if i < row_end => Some(i + 1),
            Dir::Right => wrap.then_some(row_start),
            Dir::Left if i > row_start => Some(i - 1),
            Dir::Left => wrap.then_some(row_end),
            Dir::Down if i + cols < len => Some(i + cols),
            // Into an incomplete last row: its last item.
            Dir::Down if row < last_row => Some(last),
            Dir::Down => wrap.then_some(col),
            Dir::Up if i >= cols => Some(i - cols),
            Dir::Up => wrap.then(|| {
                let bottom = last_row * cols + col;
                if bottom > last { bottom.saturating_sub(cols) } else { bottom }
            }),
        }
    }
}

/// The item focus moves to from `from` when `dir` is pressed (`from` itself when it stays).
/// An empty menu gives 0; `from` out of range is read as the last item.
pub fn step(grid: &MenuGrid, from: usize, dir: Dir, opts: NavOptions) -> usize {
    let len = grid.enabled.len();
    if len == 0 {
        return 0;
    }
    let from = from.min(len - 1);
    let mut cur = from;
    for _ in 0..len {
        let Some(next) = grid.neighbour(cur, dir, opts.wrap) else { return from };
        if next == cur || next == from {
            return from;
        }
        if !opts.skip_disabled || grid.enabled(next) {
            return next;
        }
        cur = next;
    }
    from
}

/// Where focus starts: `start` when it can take focus, else the next item that can (going
/// forward and round). `start` out of range is read as the first item; `None` when nothing can.
pub fn first_focusable(grid: &MenuGrid, start: usize, opts: NavOptions) -> Option<usize> {
    let len = grid.enabled.len();
    if len == 0 {
        return None;
    }
    let start = if start < len { start } else { 0 };
    if !opts.skip_disabled {
        return Some(start);
    }
    (0..len).map(|k| (start + k) % len).find(|i| grid.enabled(*i))
}

/// Hold-to-repeat for a direction: fires on the press, after [`REPEAT_DELAY`], then every
/// [`REPEAT_EVERY`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Repeat {
    held: Option<Dir>,
    due: f64,
}

impl Repeat {
    /// `held`: the direction held this frame; `now` in seconds. Returns the move to make, if any.
    pub fn update(&mut self, held: Option<Dir>, now: f64) -> Option<Dir> {
        if !now.is_finite() {
            return None;
        }
        let Some(dir) = held else {
            self.held = None;
            return None;
        };
        if self.held != Some(dir) {
            self.held = Some(dir);
            self.due = now + REPEAT_DELAY;
            return Some(dir);
        }
        if now + 1e-6 >= self.due {
            // A long stall does not fire a burst of moves.
            self.due = (self.due + REPEAT_EVERY).max(now + REPEAT_EVERY * 0.5);
            return Some(dir);
        }
        None
    }
}
