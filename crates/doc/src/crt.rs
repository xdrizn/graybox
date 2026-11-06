//! Graybox: the CRT effect's settings. A backdrop effect, like glass: what lies behind the layer,
//! inside its shape, looks like an old tube screen (bulge, scanlines, colour split, phosphor mask,
//! glow). Static: nothing in it animates.

use serde::{Deserialize, Serialize};

/// Which way a CRT's scanlines run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CrtDirection {
    #[default]
    Horizontal,
    Vertical,
}

impl CrtDirection {
    pub const ALL: [CrtDirection; 2] = [CrtDirection::Horizontal, CrtDirection::Vertical];
    pub fn label(self) -> &'static str {
        match self {
            CrtDirection::Horizontal => "Horizontal",
            CrtDirection::Vertical => "Vertical",
        }
    }
}

/// A CRT effect. Shares are 0–1; `line_size` is in document units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crt {
    /// Barrel bulge of the backdrop.
    pub curvature: f64,
    /// Darkening towards the edges.
    pub vignette: f64,
    /// One scanline every this many document units.
    #[serde(alias = "lineSize")]
    pub line_size: f64,
    /// Share of each line period that is dark.
    pub thickness: f64,
    /// How dark the dark part of a line is.
    #[serde(alias = "lineOpacity")]
    pub line_opacity: f64,
    /// How much bright backdrop pixels push through the lines.
    pub bleed: f64,
    pub direction: CrtDirection,
    /// Red and blue sampled apart, growing towards the edges.
    #[serde(alias = "rgbSplit")]
    pub rgb_split: f64,
    /// Red/green/blue column mask.
    pub phosphor: f64,
    /// Soft halo of bright areas.
    pub glow: f64,
    /// Overall gain (0.5–2).
    pub brightness: f64,
}

impl Default for Crt {
    fn default() -> Self {
        Self {
            curvature: 0.3,
            vignette: 0.4,
            line_size: 4.0,
            thickness: 0.5,
            line_opacity: 0.5,
            bleed: 0.5,
            direction: CrtDirection::Horizontal,
            rgb_split: 0.2,
            phosphor: 0.2,
            glow: 0.3,
            brightness: 1.2,
        }
    }
}

impl Crt {
    pub const MIN_LINE_SIZE: f64 = 1.0;
    pub const MAX_LINE_SIZE: f64 = 64.0;
    pub const MIN_BRIGHTNESS: f64 = 0.5;
    pub const MAX_BRIGHTNESS: f64 = 2.0;
    /// The glow's blur radius at full glow (document units).
    const GLOW_RADIUS: f64 = 12.0;
    /// The furthest red and blue move apart at full split, at a corner (document units).
    const MAX_SPLIT: f64 = 6.0;

    /// The values made safe to draw: finite (non-finite ones at their defaults) and in range.
    pub fn sanitized(&self) -> Crt {
        let d = Crt::default();
        let ok = |v: f64, dv: f64, lo: f64, hi: f64| if v.is_finite() { v.clamp(lo, hi) } else { dv };
        let share = |v: f64, dv: f64| ok(v, dv, 0.0, 1.0);
        Crt {
            curvature: share(self.curvature, d.curvature),
            vignette: share(self.vignette, d.vignette),
            line_size: ok(self.line_size, d.line_size, Self::MIN_LINE_SIZE, Self::MAX_LINE_SIZE),
            thickness: share(self.thickness, d.thickness),
            line_opacity: share(self.line_opacity, d.line_opacity),
            bleed: share(self.bleed, d.bleed),
            direction: self.direction,
            rgb_split: share(self.rgb_split, d.rgb_split),
            phosphor: share(self.phosphor, d.phosphor),
            glow: share(self.glow, d.glow),
            brightness: ok(self.brightness, d.brightness, Self::MIN_BRIGHTNESS, Self::MAX_BRIGHTNESS),
        }
    }

    /// The glow's blur radius (document units).
    pub fn glow_radius(&self) -> f64 {
        self.glow * Self::GLOW_RADIUS
    }

    /// How far red and blue move apart at most (document units).
    pub fn max_split(&self) -> f64 {
        self.rgb_split * Self::MAX_SPLIT
    }
}
