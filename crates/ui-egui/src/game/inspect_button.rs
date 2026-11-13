//! Graybox: Inspect (Developer Mode) as a small button left of Play at the top of the right
//! panel (G9). The bottom toolbar's mode switcher no longer has it, and the right panel's tabs are
//! only Design and Prototype.

use egui::{CornerRadius, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;

use crate::FigApp;
use crate::icons::{self, Icon};
use crate::theme::Tokens;

/// The button left of `play` (the Play button's rect); returns its rect.
pub(crate) fn show(app: &mut FigApp, ui: &mut Ui, play: Rect) -> Rect {
    let t = Tokens::get(ui.ctx());
    let r = Rect::from_min_size(pos2(play.min.x - 32.0, play.min.y), vec2(28.0, play.height()));
    let on = app.ui.right_tab == crate::RightTab::Inspect;
    let keys = crate::toolbar::pretty_shortcut(crate::shortcuts::shortcut("view.devMode").unwrap_or(""));
    let resp = ui.interact(r, ui.scope_id().with("graybox_inspect"), Sense::click()).on_hover_text(format!("Inspect  {keys}"));
    crate::a11y::toggle(&resp, "Inspect", on);
    if on {
        ui.painter().rect_filled(r, CornerRadius::same(5), t.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(5), t.hover);
    }
    icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Icon::Code, if on { t.accent_on } else { t.icon });
    if resp.clicked() {
        app.run("view.devMode", json!({}));
    }
    r
}

#[cfg(test)]
mod tests {
    use egui::accesskit::Role;
    use figcraft_engine::Session;

    use crate::RightTab;
    use crate::layers_drag_tests::Ui;

    #[test]
    fn the_inspect_button_by_play_toggles_developer_mode() {
        let mut u = Ui::new(Session::new());
        u.ctx.enable_accesskit();
        u.frame(vec![]);
        let out = u.frame_output(vec![]);
        let nodes = out.platform_output.accesskit_update.expect("an AccessKit update").nodes;
        let b = nodes.iter().map(|(_, n)| n).find(|n| n.role() == Role::Button && n.label() == Some("Inspect")).expect("Inspect").bounds().unwrap();
        let at = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        assert!(at.x > 1200.0 && at.y < 100.0, "top right, by Play: {at:?}");
        for want in [RightTab::Inspect, RightTab::Design] {
            u.button(at, true);
            u.button(at, false);
            u.frame(vec![]);
            assert_eq!(u.app.ui.right_tab, want);
        }
    }
}
