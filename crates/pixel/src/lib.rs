//! Graybox pixel art (G16): the sprite a pixel layer carries and the operations that draw on it.
//!
//! A sprite is a small picture (`width` × `height` art pixels) made of layers and frames; the
//! picture of one layer in one frame is a cel ([`Canvas`]). Everything here is pure and
//! deterministic: the engine's `pixel.*` commands and the editor's live preview run the same code,
//! so what the editor shows while a stroke is drawn is exactly what the command stores.
//!
//! Storage ([`store`]) is the layer's plugin data: the sprite's description as JSON under
//! [`store::KEY`] and each cel as a base64 PNG under its own key, so a stroke rewrites one cel.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod canvas;
pub mod paint;
pub mod raster;
pub mod sprite;
pub mod store;

pub use canvas::{CLEAR, Canvas, Rgba};
pub use paint::{Brush, BrushShape, Ink, Symmetry};
pub use sprite::{Frame, Layer, Sprite, Tag};
