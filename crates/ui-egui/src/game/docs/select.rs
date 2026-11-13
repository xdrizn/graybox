//! Several blocks at once (G13.2), as in Notion: Escape in a block selects the block, Shift+arrows
//! and Shift+click grow the selection, and then Delete removes them, Ctrl+C / Ctrl+X copy or cut
//! them, Ctrl+V pastes copied blocks after them (or after the block being written in), Enter writes
//! in the first and Escape lets go. Each is one step to undo.

use std::cell::RefCell;

use egui::{CornerRadius, Event, Key, Modifiers, Rect, Ui, pos2};
use figcraft_doc::docs::{Block, BlockId, DocId};
use serde_json::json;

use super::blocks::{View, place_after};
use super::editor::commit;
use super::state::{State, Target};
use crate::FigApp;
use crate::theme::Tokens;

thread_local! {
    /// The blocks copied last, and the text the clipboard got for them: a paste of that same text
    /// pastes the blocks themselves.
    static CLIP: RefCell<Option<(String, Vec<Block>)>> = const { RefCell::new(None) };
}

/// The copied blocks, when `text` is what was copied with them.
fn clip_for(text: &str) -> Option<Vec<Block>> {
    CLIP.with(|c| c.borrow().as_ref().filter(|(t, _)| t.trim() == text.trim()).map(|(_, b)| b.clone()))
}

/// The blocks of `ids` in the version on screen (`viewing`, else the last), with what is inside them,
/// in the order shown.
fn picked_blocks(app: &FigApp, doc: DocId, viewing: Option<(DocId, u32)>, ids: &[BlockId]) -> Vec<Block> {
    let Some(d) = app.session.doc().ok().and_then(|f| f.docs.get(doc)) else { return Vec::new() };
    let picked = viewing.filter(|(o, _)| *o == doc).and_then(|(_, n)| d.version(n));
    let Some(v) = picked.or_else(|| d.current()) else { return Vec::new() };
    // A block inside another picked one goes with it, not twice.
    let inside = |id: BlockId| ids.iter().any(|o| *o != id && v.block(*o).is_some_and(|b| contains(b, id)));
    ids.iter().filter(|id| !inside(**id)).filter_map(|id| v.block(*id).cloned()).collect()
}

fn contains(b: &Block, id: BlockId) -> bool {
    b.children.iter().any(|c| c.id == id || contains(c, id))
}

/// Put `blocks` on the clipboard: Markdown for other apps, the blocks for a paste here.
fn copy(ui: &Ui, blocks: Vec<Block>) {
    let text = figcraft_doc::docs_md::blocks_markdown(&blocks);
    ui.ctx().copy_text(text.clone());
    CLIP.with(|c| *c.borrow_mut() = Some((text, blocks)));
}

/// Paste `blocks` right after block `after`; the keyboard goes to the end of the last one pasted
/// that has words, else the paste stays selected.
fn paste(app: &mut FigApp, doc: DocId, after: BlockId, blocks: Vec<Block>) -> Option<Vec<BlockId>> {
    let (parent, index) = place_after(app, doc, after)?;
    let out = app.run("doc.block.paste", json!({"doc": doc.0, "blocks": blocks, "parent": parent.map(|p| p.0), "index": index}))?;
    let ids: Vec<BlockId> = out.get("ids")?.as_array()?.iter().filter_map(|v| v.as_u64().map(BlockId)).collect();
    Some(ids)
}

/// Before the block `v` being written in draws its field: a paste of copied blocks pastes the
/// blocks after it (in its place when it is empty) instead of their text.
pub(super) fn paste_into(app: &mut FigApp, ui: &mut Ui, st: &mut State, doc: DocId, v: &View) {
    let pasted = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            Event::Paste(t) => clip_for(t),
            _ => None,
        })
    });
    let Some(blocks) = pasted else { return };
    take_paste(ui);
    if let Some(p) = st.pending.take() {
        commit(app, &p);
    }
    let Some(ids) = paste(app, doc, v.id, blocks) else { return };
    if v.text.is_empty() {
        app.run("doc.block.delete", json!({"doc": doc.0, "id": v.id.0}));
    }
    st.focus = None;
    ui.memory_mut(|m| m.surrender_focus(super::blocks::field_id(doc, v.id)));
    st.picked = ids;
}

/// Escape in block `id`, which takes the keyboard from its field before the field is drawn (`left`: the
/// field lost it this frame, and not to close the `/` menu): the block is selected.
pub(super) fn escaped(ui: &mut Ui, st: &mut State, id: BlockId, left: bool) {
    if left && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        pick(st, id);
    }
}

fn pick(st: &mut State, id: BlockId) {
    st.picked = vec![id];
    st.pick_from = Some(id);
    st.focus = None;
}

/// After the blocks are drawn (`rows`, one per block of `views`): the selection's wash, Shift+click,
/// and the keys that act on the selected blocks.
pub(super) fn after(app: &mut FigApp, ui: &mut Ui, st: &mut State, t: &Tokens, doc: DocId, views: &[View], rows: &[Rect], col: Rect, editable: bool) {
    let shown: Vec<BlockId> = views.iter().map(|v| v.id).collect();
    st.picked.retain(|id| shown.contains(id));
    let row_at = |p: egui::Pos2| rows.iter().position(|r| r.contains(p));
    let (press, shift) = ui.input(|i| (i.pointer.primary_pressed().then(|| i.pointer.interact_pos()).flatten(), i.modifiers.shift));
    if let Some(p) = press {
        // The block written in before this press (the press itself may have moved the keyboard).
        match (shift, row_at(p), st.pick_from.or(st.writing)) {
            // Shift+click: everything from the block written in (or selected first) to this one.
            (true, Some(to), Some(from)) => {
                if let Some(from_i) = shown.iter().position(|id| *id == from) {
                    let (a, b) = (from_i.min(to), from_i.max(to));
                    st.picked = shown.get(a..=b).map(<[BlockId]>::to_vec).unwrap_or_default();
                    st.pick_from = Some(from);
                    st.focus = None;
                    st.shift_pick = true;
                    ui.memory_mut(|m| m.stop_text_input());
                }
            }
            _ if !st.picked.is_empty() => {
                st.picked.clear();
                st.pick_from = None;
            }
            _ => {}
        }
    }
    // The wash over the selected blocks.
    for (i, id) in shown.iter().enumerate() {
        if st.picked.contains(id)
            && let Some(r) = rows.get(i)
        {
            // Blocks selected one after the other read as one band.
            let next = shown.get(i + 1).filter(|n| st.picked.contains(n)).and(rows.get(i + 1));
            let bottom = next.map_or(r.max.y, |n| n.min.y.max(r.max.y));
            let r = Rect::from_min_max(pos2(col.min.x - 6.0, r.min.y), pos2(r.max.x, bottom));
            ui.painter().rect_filled(r, CornerRadius::same(4), t.accent.gamma_multiply(0.14));
        }
    }
    // The field clicked with Shift takes the keyboard when the button is let go: the selection keeps it.
    let (down, released) = ui.input(|i| (i.pointer.any_down(), i.pointer.any_released()));
    if st.shift_pick && (down || released) {
        if let Some(f) = focused_block(ui, doc, views) {
            ui.memory_mut(|m| m.surrender_focus(super::blocks::field_id(doc, f)));
        }
    } else {
        st.shift_pick = false;
    }
    st.writing = focused_block(ui, doc, views);
    if st.picked.is_empty() || ui.memory(|m| m.focused().is_some()) {
        return;
    }
    keys(app, ui, st, doc, views, editable);
}

/// Take this frame's paste out of the input, its keys too, so the canvas behind does not paste as well.
fn take_paste(ui: &mut Ui) {
    ui.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Paste(_) | Event::Key { key: Key::V, .. })));
}

/// The block whose words have the keyboard, if one does.
fn focused_block(ui: &Ui, doc: DocId, views: &[View]) -> Option<BlockId> {
    let f = ui.memory(|m| m.focused())?;
    views.iter().find(|v| super::blocks::field_id(doc, v.id) == f).map(|v| v.id)
}

/// The keys that act on the selected blocks, while no field has the keyboard.
fn keys(app: &mut FigApp, ui: &mut Ui, st: &mut State, doc: DocId, views: &[View], editable: bool) {
    let shown: Vec<BlockId> = views.iter().map(|v| v.id).collect();
    let first = shown.iter().position(|id| st.picked.contains(id));
    let last = shown.iter().rposition(|id| st.picked.contains(id));
    let (Some(first), Some(last)) = (first, last) else { return };
    let from = st.pick_from.and_then(|f| shown.iter().position(|id| *id == f)).unwrap_or(first);
    // What the keyboard asked for.
    enum Do {
        Grow(bool),
        Step(bool),
        Delete,
        Copy,
        Cut,
        Paste(Vec<Block>),
        Write,
        Drop,
    }
    let what = ui.input_mut(|i| {
        let mut what = None;
        for e in &i.events {
            match e {
                Event::Copy => what = Some(Do::Copy),
                Event::Cut => what = Some(Do::Cut),
                Event::Paste(t) => what = clip_for(t).map(Do::Paste).or(what),
                _ => {}
            }
        }
        if what.is_some() {
            i.events.retain(|e| !matches!(e, Event::Copy | Event::Cut | Event::Paste(_) | Event::Key { key: Key::V | Key::C | Key::X, .. }));
            return what;
        }
        if i.consume_key(Modifiers::SHIFT, Key::ArrowUp) {
            Some(Do::Grow(true))
        } else if i.consume_key(Modifiers::SHIFT, Key::ArrowDown) {
            Some(Do::Grow(false))
        } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            Some(Do::Step(true))
        } else if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            Some(Do::Step(false))
        } else if i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) {
            Some(Do::Delete)
        } else if i.consume_key(Modifiers::NONE, Key::Enter) {
            Some(Do::Write)
        } else if i.consume_key(Modifiers::NONE, Key::Escape) {
            Some(Do::Drop)
        } else {
            None
        }
    });
    let ids: Vec<BlockId> = shown.iter().filter(|id| st.picked.contains(id)).copied().collect();
    match what {
        Some(Do::Grow(up)) => {
            // The end away from where the selection started moves.
            let moving = if from == first { last } else { first };
            let to = if up { moving.saturating_sub(1) } else { (moving + 1).min(shown.len().saturating_sub(1)) };
            let (a, b) = (from.min(to), from.max(to));
            st.picked = shown.get(a..=b).map(<[BlockId]>::to_vec).unwrap_or_default();
        }
        Some(Do::Step(up)) => {
            let to = if up { first.saturating_sub(1) } else { (last + 1).min(shown.len().saturating_sub(1)) };
            if let Some(id) = shown.get(to) {
                st.picked = vec![*id];
                st.pick_from = Some(*id);
            }
        }
        Some(Do::Delete) if editable => {
            app.run("doc.block.delete", json!({"doc": doc.0, "ids": ids.iter().map(|b| b.0).collect::<Vec<_>>()}));
            st.picked.clear();
            st.pick_from = None;
        }
        Some(Do::Copy) => copy(ui, picked_blocks(app, doc, st.viewing, &ids)),
        Some(Do::Cut) if editable => {
            copy(ui, picked_blocks(app, doc, st.viewing, &ids));
            app.run("doc.block.delete", json!({"doc": doc.0, "ids": ids.iter().map(|b| b.0).collect::<Vec<_>>()}));
            st.picked.clear();
            st.pick_from = None;
        }
        Some(Do::Paste(blocks)) if editable => {
            if let Some(after) = shown.get(last).copied()
                && let Some(new) = paste(app, doc, after, blocks)
            {
                st.pick_from = new.first().copied();
                st.picked = new;
            }
        }
        Some(Do::Write) => {
            // The first selected block with words takes the keyboard.
            st.focus = views.iter().filter(|v| ids.contains(&v.id)).find(|v| v.look.writable()).map(|v| Target::Block(v.id));
            st.picked.clear();
            st.pick_from = None;
        }
        Some(Do::Drop) => {
            st.picked.clear();
            st.pick_from = None;
        }
        _ => {}
    }
}
