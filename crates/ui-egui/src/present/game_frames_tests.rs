//! Play redraws a changed screen off the UI thread: the last image shows meanwhile.

use std::sync::Arc;

use figcraft_doc::{Document, NodeKind};

use crate::present::Presenter;

#[test]
fn a_changed_screen_keeps_its_last_image_until_the_new_one_is_drawn() {
    let mut d = Document::new("frames");
    let page = d.first_page().unwrap();
    let screen = d.create(page, NodeKind::Frame, "Screen", 0.0, 0.0, 120.0, 80.0).unwrap();
    let ctx = egui::Context::default();
    let mut p = Presenter::new(screen);
    p.game = Some(crate::game::Play::new());
    p.sync(&Arc::new(d.clone()), 0.0);
    let doc = p.display().unwrap();
    // The first image is drawn at once (nothing to show meanwhile).
    let first = p.texture(&ctx, &doc, screen, 1.0, 0.0).unwrap();
    // A change (a HUD value): the last image comes back at once, the new one is asked for.
    p.rev += 1;
    let doc = p.display().unwrap();
    let meanwhile = p.texture(&ctx, &doc, screen, 1.0, 0.0).unwrap();
    assert_eq!(meanwhile.tex.id(), first.tex.id(), "the last image, not a fresh one");
    // Soon the new image replaces it.
    let mut newest = meanwhile.key;
    for _ in 0..500 {
        newest = p.texture(&ctx, &doc, screen, 1.0, 0.0).unwrap().key;
        if newest.0 == p.rev {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(newest.0, p.rev, "the redrawn image arrived");
}
