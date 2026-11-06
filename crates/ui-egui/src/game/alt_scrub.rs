//! Graybox: holding Alt over a number field and dragging left or right changes its value from
//! anywhere on the field, not only from its label. Here Alt doesn't make the drag finer (it does
//! on the label); Shift still makes it ten times faster. Once a drag starts with Alt, letting go
//! of Alt doesn't stop it; a drag started without Alt changes nothing.

use egui::{CursorIcon, Id, Rect, Sense, Ui};

use crate::widgets::{self, FieldEdit};

/// The Alt-drag over `rect` (the field's value area) of the number field `id` showing `value`,
/// changing by `step` a point. Call it before the field's text editor is added, and not while
/// the field is being typed in. The drag area is there every frame (egui finds what the pointer
/// is on from the last frame's areas), and it senses drags only: clicks reach the field below,
/// and the keyboard (Tab) never stops on it.
pub(crate) fn scrub(ui: &mut Ui, id: Id, rect: Rect, value: Option<f64>, step: f64) -> Option<FieldEdit> {
    let on = id.with("graybox_alt_scrub_on");
    let resp = ui.interact(rect, id.with("graybox_alt_scrub"), Sense::DRAG);
    let alt = ui.input(|i| i.modifiers.alt);
    if resp.drag_started() && alt && value.is_some() {
        ui.data_mut(|d| d.insert_temp(on, true));
    }
    let going = ui.data(|d| d.get_temp::<bool>(on)).unwrap_or(false);
    if (resp.contains_pointer() && alt) || going {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    if !going {
        return None;
    }
    if resp.drag_stopped() || !resp.dragged() {
        ui.data_mut(|d| d.remove::<bool>(on));
    }
    widgets::scrub_rated(ui, id, &resp, value, step, false)
}
