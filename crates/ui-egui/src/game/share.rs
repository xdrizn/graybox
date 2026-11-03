//! Browser Play sharing in the existing game-settings window. Configuration is a document
//! command; publication and its asynchronous readiness are upstream collaboration state.

use egui::{Id, Ui};
use figcraft_engine::cmd::game_share;
use serde_json::json;

use crate::i18n::tr;
use crate::theme::Tokens;
use crate::{FigApp, widgets};

pub fn show(app: &mut FigApp, ui: &mut Ui) {
    let Ok(ds) = app.session.ds() else { return };
    let (uid, web_url) = (ds.uid, ds.doc.game.web_url.clone());
    let tokens = Tokens::get(ui.ctx());
    ui.push_id(("game_sharing", uid), |ui| {
        ui.heading(tr("Share browser Play"));
        widgets::label(ui, tr("Publish this prototype through the sharing server. Visitors open a read-only scene directly in Play."), tokens.text_secondary);
        ui.add_space(8.0);
        widgets::label(ui, tr("Browser build URL"), tokens.text);
        if let Some(value) = widgets::text_field_named(ui, Id::unique(("game_web_url", uid)), &web_url, ui.available_width().clamp(160.0, 460.0), tr("Browser build URL")) {
            app.run("game.configure", json!({"webUrl": value}));
        }
        widgets::label(ui, tr("The address hosting your browser build, for example http://localhost:8080/. Saved in this file; no invitation or server keys."), tokens.text_secondary);
        ui.add_space(8.0);
        let state = game_share::state(&app.session, None);
        let busy = matches!(app.session.live().status, figcraft_engine::live::Status::Connecting);
        let valid_base = app.session.doc().is_ok_and(|doc| game_share::browser_base(&doc.game.web_url).is_ok());
        if widgets::button(ui, if busy { tr("Publishing…") } else { tr("Create Play link") }, true, valid_base && !busy).clicked() {
            app.run("game.share", json!({}));
        }
        widgets::label(ui, tr("Uses the online sharing server in Preferences. The server operator can read this file. Anyone with the Play link can view it, but cannot edit."), tokens.text_secondary);
        match state {
            Ok(state) => {
                if let Some(link) = state["link"].as_str() {
                    ui.add_space(8.0);
                    widgets::label(ui, tr("Read-only Play link"), tokens.text);
                    let mut display = link.to_string();
                    let response = ui.add(egui::TextEdit::singleline(&mut display).desired_width(ui.available_width()).interactive(false));
                    crate::a11y::text(&response, tr("Read-only Play link"), link);
                    if widgets::button(ui, tr("Copy Play link"), false, true).clicked() {
                        app.clipboard_set_text(link);
                        app.toast(tr("Play link copied"));
                    }
                    widgets::label(ui, tr("This link carries a view-only secret. Share it only with your audience. Stop sharing in the Share menu to revoke it."), tokens.text_secondary);
                } else if let Some(reason) = state["reason"].as_str() {
                    widgets::label(ui, tr(reason), tokens.text_secondary);
                } else if busy {
                    widgets::label(ui, tr("Waiting for the server to publish the file and supply its read-only invitation…"), tokens.text_secondary);
                }
            }
            Err(_) if web_url.trim().is_empty() => {
                widgets::label(ui, tr("Enter your browser build URL to create a Play link."), tokens.text_secondary);
            }
            Err(error) => { widgets::label(ui, &error.to_string(), tokens.text_secondary); }
        }
    });
}
