//! Graybox: the Eraser outside boards. In a design file it removes the strokes it is dragged
//! over: visible, unlocked vectors with a visible stroke, no visible fill and an open path (pencil
//! strokes, pen lines). Shapes, frames, text and filled paths stay. On a board it keeps erasing
//! marker and highlighter strokes only.

use figcraft_doc::{BoardItem, Node, NodeKind};

/// Whether the eraser removes `n` (`board`: the file is a board).
pub(crate) fn erasable(n: &Node, board: bool) -> bool {
    if board {
        return n.board == Some(BoardItem::Ink);
    }
    let open = matches!(&n.kind, NodeKind::Vector { network } if network.regions.is_empty());
    open && n.visible && !n.locked && n.board.is_none() && n.has_visible_stroke() && !n.fills.iter().any(|f| f.visible)
}
