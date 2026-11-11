//! Choosing which game a file plays under its screens (`GameSettings::kind`):
//! the "Game" dropdown at the top of the Prototype tab, and the card a new file shows.
use crate::theme::{Tokens, regular, semibold};
use crate::{FigApp, a11y, dialogs, widgets};
use egui::{Align2, Context, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use figcraft_doc::game::GameKind;
use serde_json::json;
use std::cell::Cell;

thread_local! {
    /// The file (its uid) that was just created and has not been asked yet which game it plays.
    static ASKING: Cell<Option<u64>> = const { Cell::new(None) };
}

fn current(app: &FigApp) -> GameKind {
    app.session.doc().map(|doc| doc.game.kind).unwrap_or_default()
}

/// What each game is, in one line, as the cards and the dropdown say it.
fn blurb(kind: GameKind) -> &'static str {
    match kind {
        GameKind::Fps => "Through the player's eyes",
        GameKind::ThirdPerson => "A hero seen from behind",
    }
}

/// The "Game" dropdown (Prototype tab, nothing selected). Changing it is undoable (`game.kind.set`).
pub(crate) fn picker(app: &mut FigApp, ui: &mut Ui) {
    let selected = current(app);
    let t = Tokens::get(ui.ctx());
    widgets::label(ui, "Game", t.text_secondary);
    let response = widgets::dropdown(ui, selected.label(), ui.available_width(), None);
    crate::right::menu_popup(&response).show(|ui| {
        for kind in GameKind::ALL {
            if crate::right::menu_check(ui, kind == selected, kind.label(), "") {
                app.run("game.kind.set", json!({"kind": kind.id()}));
                ui.close();
            }
        }
    });
}

/// Called after every command the UI runs: a new design file asks which game it plays. Not in this
/// crate's own tests, which create files all the time and must not meet the card.
pub(crate) fn after_run(app: &FigApp, id: &str, succeeded: bool) {
    if id == "file.new" && succeeded && !cfg!(test) {
        ask(app);
    }
}

/// Ask which game the file in front plays.
pub(crate) fn ask(app: &FigApp) {
    ASKING.with(|asking| asking.set(app.session.ds().ok().map(|ds| ds.uid)));
}

/// The card, while a new file waits for its game. First person is what closing it keeps.
pub(crate) fn show(app: &mut FigApp, ctx: &Context) {
    let Some(uid) = ASKING.with(Cell::get) else { return };
    // The file was closed or another one came to the front: nothing to ask any more.
    if app.session.ds().ok().map(|ds| ds.uid) != Some(uid) || app.ui.home {
        ASKING.with(|asking| asking.set(None));
        return;
    }
    let mut open = true;
    let mut chosen = None;
    dialogs::window("Choose the game", ctx).open(&mut open).show(ctx, |ui| {
        let t = Tokens::get(ui.ctx());
        ui.set_width(2.0 * CARD_W + 12.0);
        ui.label(egui::RichText::new("Which game should Play run under this file's screens?").font(semibold(13.0)).color(t.text));
        ui.label(
            egui::RichText::new("You can change it later in the Prototype tab, with nothing selected.").font(regular(11.0)).color(t.text_secondary),
        );
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for kind in GameKind::ALL {
                if card(ui, &t, kind) {
                    chosen = Some(kind);
                }
            }
        });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        open = false;
    }
    if let Some(kind) = chosen {
        if kind != current(app) {
            app.run("game.kind.set", json!({"kind": kind.id()}));
        }
        open = false;
    }
    if !open {
        ASKING.with(|asking| asking.set(None));
    }
}

const CARD_W: f32 = 230.0;
const CARD_H: f32 = 132.0;

/// A card for one game: a small drawing, its name and a line about it. True when chosen.
fn card(ui: &mut Ui, t: &Tokens, kind: GameKind) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
    a11y::button(&resp, kind.label());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), if resp.hovered() { t.hover } else { t.panel });
    painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, t.border), StrokeKind::Inside);
    if resp.has_focus() {
        painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(2.0, t.focus), StrokeKind::Outside);
    }
    let art = Rect::from_min_size(rect.min + vec2(12.0, 12.0), vec2(rect.width() - 24.0, 56.0));
    painter.rect_filled(art, CornerRadius::same(5), t.field);
    draw_art(painter, art, kind, t);
    painter.text(pos2(rect.min.x + 12.0, rect.min.y + 80.0), Align2::LEFT_CENTER, kind.label(), semibold(12.0), t.text);
    let text = widgets::elide(ui, blurb(kind), rect.width() - 24.0);
    ui.painter().text(pos2(rect.min.x + 12.0, rect.min.y + 100.0), Align2::LEFT_TOP, text, regular(11.0), t.text_secondary);
    resp.clicked()
}

/// A tiny drawing of the game, made of lines and shapes (no image files).
fn draw_art(painter: &egui::Painter, art: Rect, kind: GameKind, t: &Tokens) {
    let c: Pos2 = art.center();
    let ink = Stroke::new(1.5, t.text_secondary);
    match kind {
        GameKind::Fps => {
            // A cross-hair.
            for (a, b) in [((-14.0, 0.0), (-5.0, 0.0)), ((5.0, 0.0), (14.0, 0.0)), ((0.0, -14.0), (0.0, -5.0)), ((0.0, 5.0), (0.0, 14.0))] {
                painter.line_segment([c + vec2(a.0, a.1), c + vec2(b.0, b.1)], ink);
            }
            painter.circle_filled(c, 2.0, t.accent);
        }
        GameKind::ThirdPerson => {
            // A hero in the middle and the arc the camera moves along behind it.
            painter.circle_stroke(c + vec2(0.0, 6.0), 20.0, Stroke::new(1.0, t.border));
            painter.rect_filled(Rect::from_center_size(c, vec2(10.0, 22.0)), CornerRadius::same(5), t.text_secondary);
            painter.rect_filled(Rect::from_center_size(c + vec2(0.0, -4.0), vec2(6.0, 2.0)), CornerRadius::ZERO, t.accent);
        }
    }
}
