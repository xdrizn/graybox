---
name: graybox-egui-tests
description: Use when writing or debugging egui UI tests in Graybox (press/click not registering, drag stealing a click, Tab order, texture leaks) or when interpreting failing workspace tests.
---

# egui UI test pitfalls

Tests drive `egui::Context::run` with raw events (see `crates/ui-egui/src/*_tests.rs`, e.g. `a11y_tests.rs`).

- **Presses use the previous frame's widgets.** egui assigns a press to the widget that existed
  *last* frame. A widget created only while a key/modifier is held never receives the press. Hold
  modifiers (Alt/Shift/Cmd) **one frame before** the pointer-down, and run one frame after
  creating a widget before clicking it.
- **Separate frames for press and release** (and for the two clicks of a double click).
- **Drop `FullOutput` cleanly**: call `out.textures_delta.clear()` before dropping a
  `FullOutput`, or the test leaks texture handles (egui debug-asserts).
- **A drag-only widget on top of a click widget steals the click.** Register the drag widget
  *underneath* (earlier) the click widget, so the click widget stays on top.
- **Tab order**: `Sense::DRAG` (the constant) keeps a widget out of keyboard Tab order;
  `Sense::drag()` makes it focusable. Use the constant for purely pointer handles.
- Use `egui::Id::unique`, not `Id::new` (deprecated; clippy fails).
- Prefer asserting engine state after the frame (`app.session`) over pixel checks; for real
  appearance use `graybox-app-check`.
- Pages in Graybox start **dark**; tests that assume a light page (pencil preview, contrast) must
  set a light page themselves. F9 is `game.play`.

## Failing tests that are not your fault (this machine)
Run `cargo test --workspace --no-fail-fast`; these two fail before your change and are expected:
- `components_scale_tests::editing_deep_mains_keeps_every_override_and_matches_a_full_rebuild`
- `fig_export_assets_tests::every_example_keeps_its_variables_and_styles`

Report them as "known failures", not as new ones. Any other failure is yours to fix or explain.
