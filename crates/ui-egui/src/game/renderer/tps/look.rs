//! The third-person game's whole look in one place (Adriel's "clean" change to the spec).
//!
//! Clean means: solid flat-shaded greys (the theme's tones, with no pixel-noise textures and no grain),
//! a smooth sky and fog gradient, soft shadows, bloom only on the emissive parts and smooth edges. A
//! second look (for the first-person game, say) is another value of this struct plus a call that applies
//! it, so nothing in the shaders or meshes needs to change.
use super::super::SceneTheme;
use glam::Vec3;

/// How the third-person scene looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TpsLook {
    /// The sky straight up (sRGB). The clear colour of the picture.
    pub zenith: [f32; 3],
    /// The sky at the horizon, and the colour the fog fades to (sRGB), so far shapes melt into it.
    pub horizon: [f32; 3],
    /// How thick the fog is, against the first-person arena's.
    pub fog_scale: f32,
    /// How strong the lights are, against the first-person arena's.
    pub key_scale: f32,
    pub hemi_scale: f32,
    pub ambient_scale: f32,
    /// How far the shadow edge is softened, in shadow-map texels (the filter is 4 x 4 bilinear taps, so
    /// the edge has no steps). 0 would be the arena's hard 3 x 3 filter.
    pub shadow_softness: f32,
    /// Smooth the edges of shapes in the final picture (a cheap luma-based filter: works everywhere the
    /// game does, costs no extra memory, unlike multisampling a three-target scene).
    pub edge_aa: bool,
    /// Pixel-noise textures on the surfaces. Off: every surface is its tone's flat colour.
    pub pixel_textures: bool,
    /// How far the beacon's light reaches (metres).
    pub beacon_range: f32,
}

impl Default for TpsLook {
    fn default() -> Self {
        Self {
            zenith: [4.0 / 255.0; 3],
            horizon: [30.0 / 255.0; 3],
            fog_scale: 0.55,
            key_scale: 0.55,
            hemi_scale: 0.6,
            ambient_scale: 0.7,
            shadow_softness: 1.5,
            edge_aa: true,
            pixel_textures: false,
            beacon_range: 30.0,
        }
    }
}

impl TpsLook {
    /// The theme this look makes out of the project's (the project's greys stay the palette).
    pub fn apply(&self, base: &SceneTheme, beacon: Vec3) -> SceneTheme {
        SceneTheme {
            background: self.zenith,
            fog_color: self.horizon,
            portal_position: beacon,
            portal_range: self.beacon_range,
            fog_density: base.fog_density * self.fog_scale,
            key_intensity: base.key_intensity * self.key_scale,
            hemi_intensity: base.hemi_intensity * self.hemi_scale,
            ambient: base.ambient * self.ambient_scale,
            pixel_textures: self.pixel_textures,
            ..*base
        }
    }

    /// What the shaders read from the scene uniform: x the shadow softness, y 1 when edges are smoothed.
    pub fn uniform(&self) -> [f32; 4] {
        let softness = if self.shadow_softness.is_nan() { 0.0 } else { self.shadow_softness.clamp(0.0, 4.0) };
        [softness, if self.edge_aa { 1.0 } else { 0.0 }, 0.0, 0.0]
    }

    /// The sky colours in linear light, for the sky shader: (horizon, zenith).
    pub fn sky(&self) -> ([f32; 4], [f32; 4]) {
        let linear = |color: [f32; 3]| {
            let [r, g, b] = color.map(|c| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
            [r, g, b, 1.0]
        };
        (linear(self.horizon), linear(self.zenith))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_look_is_clean() {
        let look = TpsLook::default();
        let theme = look.apply(&SceneTheme::default(), Vec3::ZERO);
        assert!(!theme.pixel_textures, "no pixel-noise textures");
        assert!(look.edge_aa && look.shadow_softness > 0.0, "smooth edges and soft shadows");
        assert_eq!(theme.grays, SceneTheme::default().grays, "the project's greys stay the palette");
        assert!(theme.fog_density < SceneTheme::default().fog_density);
        let (horizon, zenith) = look.sky();
        assert!(horizon[0] > zenith[0], "the sky is lighter at the horizon");
    }

    #[test]
    fn the_uniform_stays_in_range_whatever_the_look() {
        let wild = TpsLook { shadow_softness: f32::MAX, ..TpsLook::default() };
        assert_eq!(wild.uniform()[0], 4.0);
        let nan = TpsLook { shadow_softness: f32::NAN, ..TpsLook::default() };
        assert!(nan.uniform()[0].is_finite());
    }
}
