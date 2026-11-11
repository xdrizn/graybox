//! The Claude panel: a floating window at the bottom right of the canvas.

use egui::{Align2, Color32, CornerRadius, Frame, Id, Key, Margin, Modifiers, Sense, Stroke, Ui, pos2, vec2};
use egui_extras::{Size, StripBuilder};

use super::markdown::{self, Block, Span};
use super::state::{Chat, Conversation, Message};
use crate::FigApp;
use crate::theme::{Tokens, regular, semibold};
use crate::widgets;

const MIN_W: f32 = 300.0;
const MIN_H: f32 = 260.0;
const HEADER_H: f32 = 40.0;
const FOOTER_H: f32 = 118.0;
const PAD: f32 = 12.0;

const EMPTY: &str = "Ask Claude to design in this file. For example: create a 400×300 frame named Menu with a centred title.";

/// Draw the panel for the file `uid`.
pub fn show(app: &mut FigApp, ctx: &egui::Context, chat: &mut Chat, uid: u64) {
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    // Left of the Design panel, above the toolbar.
    let right = app.ui.right_panel_rect.map_or(screen.max.x, |r| r.min.x.min(screen.max.x)) - 12.0;
    let bottom = screen.max.y - 76.0;
    let (w, h) = chat.size;
    let w = w.clamp(MIN_W, (right - screen.min.x - 24.0).max(MIN_W));
    let h = h.clamp(MIN_H, (bottom - screen.min.y - 56.0).max(MIN_H));
    let mut close = false;
    let mut resize = vec2(0.0, 0.0);
    let focus = std::mem::take(&mut chat.focus);
    let conv = chat.convs.entry(uid).or_default();
    let frame = Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(12)).shadow(egui::epaint::Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(60),
    });
    egui::Area::new(Id::unique("claude_chat")).order(egui::Order::Foreground).fixed_pos(pos2(right, bottom)).pivot(Align2::RIGHT_BOTTOM).show(
        ctx,
        |ui| {
            let out = frame.show(ui, |ui| {
                ui.set_min_size(vec2(w, h));
                ui.set_max_size(vec2(w, h));
                contents(ui, &t, conv, &mut close, focus);
            });
            resize = grip(ui, &t, out.response.rect);
        },
    );
    chat.size = ((w - resize.x).clamp(MIN_W, 900.0), (h - resize.y).clamp(MIN_H, 1200.0));
    if close {
        chat.open = false;
    }
}

/// The top-left corner handle: dragging it grows the panel up and to the left.
fn grip(ui: &mut Ui, t: &Tokens, rect: egui::Rect) -> egui::Vec2 {
    let r = egui::Rect::from_min_size(rect.min, vec2(16.0, 16.0));
    let resp = ui.interact(r, Id::unique("claude_chat_grip"), Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    for k in [5.0, 9.0] {
        ui.painter().line_segment([r.min + vec2(3.0, k), r.min + vec2(k, 3.0)], Stroke::new(1.0, t.text_tertiary));
    }
    resp.drag_delta()
}

fn contents(ui: &mut Ui, t: &Tokens, conv: &mut Conversation, close: &mut bool, focus: bool) {
    StripBuilder::new(ui).size(Size::exact(HEADER_H)).size(Size::remainder()).size(Size::exact(FOOTER_H)).vertical(|mut strip| {
        strip.cell(|ui| header(ui, t, conv, close));
        strip.cell(|ui| messages(ui, t, conv));
        strip.cell(|ui| footer(ui, t, conv, focus));
    });
}

fn header(ui: &mut Ui, t: &Tokens, conv: &mut Conversation, close: &mut bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    let mid = rect.center().y;
    ui.painter().text(pos2(rect.min.x + 22.0, mid), Align2::LEFT_CENTER, "Claude", semibold(13.0), t.text);
    // Close ×
    let x = egui::Rect::from_center_size(pos2(rect.max.x - 20.0, mid), vec2(24.0, 24.0));
    let xr = ui.interact(x, Id::unique("claude_close"), Sense::click());
    crate::a11y::button(&xr, "Close Claude chat");
    if xr.hovered() {
        ui.painter().rect_filled(x, CornerRadius::same(5), t.hover);
    }
    let c = x.center();
    for (a, b) in [((-4.0, -4.0), (4.0, 4.0)), ((-4.0, 4.0), (4.0, -4.0))] {
        ui.painter().line_segment([c + vec2(a.0, a.1), c + vec2(b.0, b.1)], Stroke::new(1.3, t.icon));
    }
    if xr.clicked() {
        *close = true;
    }
    // New chat
    let label = "New chat";
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(11.0), t.text);
    let nr = egui::Rect::from_center_size(pos2(x.min.x - 8.0 - (galley.size().x + 16.0) / 2.0, mid), vec2(galley.size().x + 16.0, 24.0));
    let resp = ui.interact(nr, Id::unique("claude_new_chat"), Sense::click());
    crate::a11y::button(&resp, label);
    ui.painter().rect_filled(nr, CornerRadius::same(5), if resp.hovered() { t.hover } else { Color32::TRANSPARENT });
    ui.painter().rect_stroke(nr, CornerRadius::same(5), Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
    ui.painter().text(nr.center(), Align2::CENTER_CENTER, label, regular(11.0), t.text);
    if resp.clicked() {
        conv.reset();
    }
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, t.border));
}

fn messages(ui: &mut Ui, t: &Tokens, conv: &Conversation) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        Frame::NONE.inner_margin(Margin::symmetric(PAD as i8, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if conv.messages.is_empty() && !conv.busy {
                ui.add(egui::Label::new(egui::RichText::new(EMPTY).font(regular(12.0)).color(t.text_secondary)).wrap());
                #[cfg(target_arch = "wasm32")]
                ui.add(egui::Label::new(egui::RichText::new(WEB_NOTE).font(regular(12.0)).color(t.warning)).wrap());
            }
            for m in &conv.messages {
                message(ui, t, m);
                ui.add_space(10.0);
            }
            if conv.busy {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0).color(t.accent_text));
                    let line = conv.trace.as_deref().unwrap_or("Working…");
                    ui.add(egui::Label::new(egui::RichText::new(line).font(regular(11.5)).color(t.text_secondary)).wrap());
                });
            }
        });
    });
}

fn message(ui: &mut Ui, t: &Tokens, m: &Message) {
    match m {
        Message::User(text) => {
            Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(egui::RichText::new(text).font(regular(12.0)).color(t.text)).wrap());
            });
        }
        Message::Claude(text) => {
            for block in markdown::blocks(text) {
                block_ui(ui, t, &block);
            }
        }
        Message::Note(text) => {
            ui.add(egui::Label::new(egui::RichText::new(text).font(regular(11.0)).italics().color(t.text_tertiary)).wrap());
        }
        Message::Error(text) => {
            Frame::NONE.stroke(Stroke::new(1.0, t.danger)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(10, 8)).show(
                ui,
                |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(egui::RichText::new(text).font(regular(12.0)).color(t.danger)).wrap());
                },
            );
        }
    }
}

fn block_ui(ui: &mut Ui, t: &Tokens, block: &Block) {
    match block {
        Block::Para(spans) => {
            ui.add(egui::Label::new(job(spans, t, 12.0, t.text)).wrap());
            ui.add_space(4.0);
        }
        Block::Heading(spans) => {
            ui.add(egui::Label::new(job(spans, t, 13.0, t.text)).wrap());
            ui.add_space(4.0);
        }
        Block::Bullet(spans) => {
            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("•").font(regular(12.0)).color(t.text_secondary));
                ui.add(egui::Label::new(job(spans, t, 12.0, t.text)).wrap());
            });
        }
        Block::Code(code) => {
            Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(6)).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(egui::RichText::new(code).font(egui::FontId::monospace(11.0)).color(t.text)).wrap());
            });
            ui.add_space(4.0);
        }
    }
}

fn job(spans: &[Span], t: &Tokens, size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for s in spans {
        let format = if s.code {
            egui::TextFormat { font_id: egui::FontId::monospace(size - 1.0), color, background: t.field, ..Default::default() }
        } else if s.bold {
            egui::TextFormat { font_id: semibold(size), color, ..Default::default() }
        } else {
            egui::TextFormat { font_id: regular(size), color, ..Default::default() }
        };
        job.append(&s.text, 0.0, format);
    }
    job
}

#[cfg(target_arch = "wasm32")]
const WEB_NOTE: &str = "The Claude chat works in the Graybox desktop app. In the browser it is only shown here.";

fn footer(ui: &mut Ui, t: &Tokens, conv: &mut Conversation, focus: bool) {
    let id = Id::unique("claude_chat_input");
    // Enter sends; Shift+Enter is a new line (the text box gets it only then).
    let mut send = false;
    if ui.memory(|m| m.has_focus(id)) && ui.input_mut(|i| !i.modifiers.shift && i.consume_key(Modifiers::NONE, Key::Enter)) {
        send = true;
    }
    Frame::NONE.inner_margin(Margin::symmetric(PAD as i8, 8)).show(ui, |ui| {
        Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let out = egui::TextEdit::multiline(&mut conv.input)
                .id(id)
                .hint_text("Ask Claude…  (Enter sends, Shift+Enter new line)")
                .font(regular(12.0))
                .frame(Frame::NONE)
                .desired_rows(3)
                .desired_width(ui.available_width())
                .show(ui);
            if focus {
                out.response.response.request_focus();
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ready = !conv.input.trim().is_empty() && !conv.busy;
            if widgets::button(ui, "Send", true, ready).clicked() {
                send = true;
            }
            if conv.busy && widgets::button(ui, "Stop", false, true).clicked() {
                stop(conv);
            }
        });
    });
    if send {
        submit(conv, ui.ctx());
    }
}

fn stop(conv: &mut Conversation) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(run) = conv.run.take() {
        // At once: the process may take a moment to close its pipe, and the person pressed Stop.
        run.stop();
        conv.apply(super::events::Event::Stopped);
        return;
    }
    // Still waiting for the connection: nothing runs yet.
    if conv.queued.take().is_some() {
        conv.apply(super::events::Event::Stopped);
    }
}

fn submit(conv: &mut Conversation, ctx: &egui::Context) {
    #[cfg(not(target_arch = "wasm32"))]
    super::send(conv, Vec::new(), ctx.input(|i| i.time));
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        if !conv.input.trim().is_empty() {
            conv.messages.push(Message::Error("The Claude chat works in the Graybox desktop app. In the browser it is only shown here.".to_string()));
        }
    }
}
