//! Moving a block by dragging its handle (G13.2): while it is dragged, a line shows where it will land
//! (between two blocks, at the level of the block below the line, or of the block above it when the
//! pointer is that far right); letting go moves it there with what is inside it. A block never lands
//! inside its own content.

use egui::{Key, Modifiers, Pos2, Rect, Stroke, Ui, pos2, vec2};
use figcraft_doc::docs::{BlockId, DocId};
use serde_json::json;

use super::blocks::{STEP, View, current};
use super::state::State;
use crate::FigApp;
use crate::theme::Tokens;

/// How close to the top or bottom of the editor the pointer makes it scroll.
const EDGE: f32 = 48.0;

/// Where a dragged block lands, next to which block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    Before(BlockId),
    After(BlockId),
    /// The end of the document.
    End,
}

/// Where the line goes: the spot, the line's height on screen and its level.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot {
    spot: Spot,
    y: f32,
    depth: usize,
}

/// The slot for a block `moving` dropped at `at`, given the blocks on screen (`views`, each drawn in
/// `rows`) and the left edge of the text column `left`; `None` inside the block's own content.
fn slot(views: &[View], rows: &[Rect], moving: BlockId, at: Pos2, left: f32) -> Option<Slot> {
    let shown: Vec<(&View, &Rect)> = views.iter().zip(rows).filter(|(_, r)| r.is_positive()).collect();
    // The gap the pointer is in: before the first row whose middle is below it.
    let k = shown.iter().position(|(_, r)| at.y < r.center().y).unwrap_or(shown.len());
    let above = k.checked_sub(1).and_then(|i| shown.get(i));
    let below = shown.get(k);
    // To the right, under a block deeper than the one below the gap: land after it, at its level.
    let deeper_above = above.filter(|(a, _)| below.is_none_or(|(b, _)| a.depth > b.depth) && at.x > left + (a.depth as f32) * STEP);
    let s = match (deeper_above, below, above) {
        (Some((a, r)), _, _) => Slot { spot: Spot::After(a.id), y: r.max.y, depth: a.depth },
        (None, Some((b, r)), _) => Slot { spot: Spot::Before(b.id), y: r.min.y - 2.0, depth: b.depth },
        (None, None, Some((_, r))) => Slot { spot: Spot::End, y: r.max.y, depth: 0 },
        (None, None, None) => return None,
    };
    // The block and what is inside it: the rows after it that are deeper.
    let m = views.iter().position(|v| v.id == moving)?;
    let depth = views.get(m)?.depth;
    let inside: Vec<BlockId> = views.iter().skip(m + 1).take_while(|v| v.depth > depth).map(|v| v.id).collect();
    let anchor = match s.spot {
        Spot::Before(b) | Spot::After(b) => Some(b),
        Spot::End => None,
    };
    let into_itself = anchor.is_some_and(|b| inside.contains(&b));
    (!into_itself).then_some(s)
}

/// The line while a block is dragged, and the move when it is let go. Call after every block is drawn.
#[allow(clippy::too_many_arguments)]
pub(super) fn drop_ui(app: &mut FigApp, ui: &Ui, st: &mut State, t: &Tokens, doc: DocId, views: &[View], rows: &[Rect], col: Rect) {
    let Some(moving) = st.block_drag else { return };
    let at = ui.input(|i| i.pointer.latest_pos());
    let s = at.and_then(|p| slot(views, rows, moving, p, col.min.x));
    if st.block_drop {
        st.block_drag = None;
        st.block_drop = false;
        if let Some(s) = s {
            land(app, doc, moving, s.spot);
        }
        return;
    }
    // Let go somewhere the handle did not see (the window lost the pointer), or Escape: forget it.
    if !ui.input(|i| i.pointer.any_down()) || ui.ctx().input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        st.block_drag = None;
        return;
    }
    // Near the top or bottom of the editor, the page scrolls, faster the closer the pointer is.
    if let Some(p) = at {
        let seen = ui.clip_rect();
        let top = EDGE - (p.y - seen.min.y);
        let bottom = EDGE - (seen.max.y - p.y);
        let speed = if top > 0.0 {
            top.min(EDGE)
        } else if bottom > 0.0 {
            -bottom.min(EDGE)
        } else {
            0.0
        };
        if speed != 0.0 {
            ui.scroll_with_delta(vec2(0.0, speed * 0.5));
            ui.ctx().request_repaint();
        }
    }
    if let Some(s) = s {
        let x0 = col.min.x + s.depth as f32 * STEP;
        let y = s.y + 1.0;
        ui.painter().line_segment([pos2(x0, y), pos2(col.max.x, y)], Stroke::new(2.0, t.accent));
        ui.painter().circle_stroke(pos2(x0, y), 3.0, Stroke::new(2.0, t.accent));
    }
}

/// Move `moving` to `spot`.
fn land(app: &mut FigApp, doc: DocId, moving: BlockId, spot: Spot) {
    let Some(v) = current(app, doc) else { return };
    let Some((from_parent, from)) = v.locate(moving) else { return };
    let (parent, index) = match spot {
        Spot::Before(b) => match v.locate(b) {
            Some(p) => p,
            None => return,
        },
        Spot::After(a) => match v.locate(a) {
            Some((p, i)) => (p, i + 1),
            None => return,
        },
        Spot::End => (None, v.blocks.len()),
    };
    // The index counts the blocks once the moving one is taken out of its list.
    let index = if from_parent == parent && from < index { index - 1 } else { index };
    if from_parent == parent && from == index {
        return;
    }
    let mut params = json!({"doc": doc.0, "id": moving.0, "index": index});
    if let Some(p) = parent {
        params["parent"] = json!(p.0);
    }
    app.run("doc.block.move", params);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::docs::blocks::Look;

    fn view(id: u64, depth: usize) -> View {
        View {
            id: BlockId(id),
            depth,
            look: Look::Paragraph,
            text: String::new(),
            runs: Vec::new(),
            color: None,
            background: None,
            size: None,
            has_font: false,
            mono: false,
            parent: None,
            before: None,
        }
    }

    fn rows(n: usize) -> Vec<Rect> {
        (0..n).map(|i| Rect::from_min_size(pos2(0.0, i as f32 * 30.0), egui::vec2(600.0, 30.0))).collect()
    }

    #[test]
    fn the_gap_is_before_the_first_row_whose_middle_is_below_the_pointer() {
        let v = [view(1, 0), view(2, 0), view(3, 0)];
        let s = slot(&v, &rows(3), BlockId(3), pos2(10.0, 20.0), 40.0);
        assert_eq!(s.map(|s| s.spot), Some(Spot::Before(BlockId(2))));
        let s = slot(&v, &rows(3), BlockId(1), pos2(10.0, 200.0), 40.0);
        assert_eq!(s.map(|s| s.spot), Some(Spot::End));
    }

    #[test]
    fn far_right_under_a_nested_block_lands_at_its_level() {
        // 1, 2 inside 1, then 3.
        let v = [view(1, 0), view(2, 1), view(3, 0)];
        let near = slot(&v, &rows(3), BlockId(3), pos2(10.0, 62.0), 40.0);
        assert_eq!(near.map(|s| (s.spot, s.depth)), Some((Spot::Before(BlockId(3)), 0)));
        let right = slot(&v, &rows(3), BlockId(3), pos2(100.0, 62.0), 40.0);
        assert_eq!(right.map(|s| (s.spot, s.depth)), Some((Spot::After(BlockId(2)), 1)));
    }

    #[test]
    fn a_block_never_lands_inside_itself() {
        let v = [view(1, 0), view(2, 1), view(3, 1), view(4, 0)];
        assert_eq!(slot(&v, &rows(4), BlockId(1), pos2(10.0, 50.0), 40.0), None);
        assert!(slot(&v, &rows(4), BlockId(1), pos2(10.0, 200.0), 40.0).is_some());
    }
}
