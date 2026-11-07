//! The fork's product name, shown where upstream shows "FigCraft" (window title, About, Help menu,
//! Home sidebar). Settings folders, file extensions and command ids keep upstream's names.

use crate::i18n::tr_noop;
use crate::theme::Tokens;

/// Upstream's About texts, which our About replaces. Still marked for translation so the shared
/// language catalogs (`locales/*.toml`) stay as upstream has them and merge cleanly.
#[allow(dead_code)]
const UPSTREAM_ABOUT: [&str; 5] = [
    tr_noop("About FigCraft"),
    tr_noop(
        "Interface design and prototyping. A free, open-source, clean-room design tool written from scratch in Rust, part of the ArtCraft Crafting Apps.",
    ),
    tr_noop("Join us on Discord"),
    tr_noop("Dual-licensed MIT OR Apache-2.0. Made by the ArtCraft team and community."),
    // The Help menu's FigCraft update check, which Graybox leaves out.
    tr_noop("Check for updates…"),
];

/// The short name, for sentences.
pub const NAME: &str = "Graybox";
/// The full product name, for titles.
pub const FULL_NAME: &str = "Graybox by INDIRECT®";
/// The About window title and Help menu item.
pub const ABOUT: &str = "About Graybox";

/// The window title for an open file (or none).
pub fn window_title(file: Option<(&str, bool)>) -> String {
    match file {
        Some((name, dirty)) => format!("{name}{} – {FULL_NAME}", if dirty { " •" } else { "" }),
        None => FULL_NAME.to_string(),
    }
}

/// The About window's text under the wordmark. Keeps the credit and licence of FigCraft, the
/// open-source app Graybox is built on.
pub fn about_text(ui: &mut egui::Ui, t: &Tokens) {
    ui.label("Game UI prototyping: design an interface, then play it over a test scene with keyboard, mouse and controller.");
    ui.add_space(8.0);
    ui.hyperlink_to("github.com/xdrizn/graybox", "https://github.com/xdrizn/graybox");
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Built on FigCraft by the ArtCraft team and community (getartcraft.com/apps/figcraft). Dual-licensed MIT OR Apache-2.0.")
            .color(t.text_secondary)
            .font(crate::theme::regular(11.0)),
    );
}

/// The two-tone "Graybox" wordmark (the studio credit is only in the window title).
pub fn paint_wordmark(painter: &egui::Painter, left_center: egui::Pos2, size: f32, t: &Tokens) {
    let font = crate::theme::brand(size);
    let gray = painter.layout_no_wrap("Gray".into(), font.clone(), t.text);
    let gray_width = gray.size().x;
    painter.galley(egui::pos2(left_center.x, left_center.y - gray.size().y / 2.0), gray, t.text);
    painter.text(egui::pos2(left_center.x + gray_width, left_center.y), egui::Align2::LEFT_CENTER, "box", font, t.accent_text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_title_carries_the_full_name() {
        assert_eq!(window_title(None), "Graybox by INDIRECT®");
        assert_eq!(window_title(Some(("HUD", false))), "HUD – Graybox by INDIRECT®");
        assert_eq!(window_title(Some(("HUD", true))), "HUD • – Graybox by INDIRECT®");
    }
}
