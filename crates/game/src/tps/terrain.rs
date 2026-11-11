//! The valley's ground: a low-poly heightfield of 2 m cells, shaped in code.
//!
//! The same grid is what the renderer draws and what the hero stands on, so a footstep and a picture
//! can never disagree. Every number comes from integer hashes and plain arithmetic (no sine, no
//! power), so the valley is bit-for-bit the same on every machine and in the browser.
//!
//! The map, seen from above (north is -Z, the way the camera first looks):
//! - a hill in the south where the hero starts, a meadow level with a few terraces to its north;
//! - a cliff that drops from the meadow into a river valley, and a ridge closing the valley in the north;
//! - a high plateau in the west with ruins on it, where the river rises and falls into the valley;
//! - a raised terrace in the east, and a wall of rim all around so the map has an edge.
use glam::Vec3;

/// The ground reaches this far from the middle in every direction (metres).
pub const HALF_EXTENT: f32 = 80.0;
/// The side of a heightfield cell (metres).
pub const CELL: f32 = 2.0;
/// Cells along one side.
pub const CELLS: usize = 80;
const SIDE: usize = CELLS + 1;

/// The river bed in the valley and on the plateau (metres), and how deep the water stands over it.
const VALLEY_BED: f32 = -1.3;
const PLATEAU_BED: f32 = 7.7;
pub const WATER_DEPTH: f32 = 1.0;
/// Where the river meets the plateau's edge (the waterfall) and how far it runs either way.
pub const FALL_X: f32 = -44.0;
pub const RIVER_END: f32 = 66.0;
/// Half the width of the water's ribbon.
pub const RIVER_HALF_WIDTH: f32 = 3.6;
const PLATEAU: f32 = 9.0;

/// A round piece of ground flattened (and maybe raised) for ruins: flat inside `radius`, blending
/// into the land over six more metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    /// Metres above what the land would have been at the middle.
    pub lift: f32,
}

/// The ruined courtyard, the watchtower's pad, the plateau ruin, the eastern terrace, the start.
pub const ZONES: [Zone; 5] = [
    Zone { x: 34.0, z: 10.0, radius: 10.0, lift: 0.0 },
    Zone { x: -10.0, z: 6.0, radius: 6.0, lift: 0.0 },
    Zone { x: -64.0, z: 8.0, radius: 10.0, lift: 0.0 },
    Zone { x: 54.0, z: 36.0, radius: 11.0, lift: 3.5 },
    Zone { x: 2.0, z: 52.0, radius: 3.0, lift: 0.0 },
];

/// A deterministic number in `[0, 1)` for a cell of the integer grid.
fn hash(ix: i32, iz: i32, seed: u32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x9E37_79B1) ^ (iz as u32).wrapping_mul(0x85EB_CA6B) ^ seed.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 0 at `edge0`, 1 at `edge1`, smooth between (the edges may be in either order).
pub(crate) fn smooth(edge0: f32, edge1: f32, value: f32) -> f32 {
    let span = edge1 - edge0;
    if span == 0.0 || !value.is_finite() {
        return 0.0;
    }
    let t = ((value - edge0) / span).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth value noise in `[-1, 1]`.
pub(crate) fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let (fx, fz) = (x.floor(), z.floor());
    let (ix, iz) = (fx as i32, fz as i32);
    let (u, v) = (smooth(0.0, 1.0, x - fx), smooth(0.0, 1.0, z - fz));
    let top = lerp(hash(ix, iz, seed), hash(ix.wrapping_add(1), iz, seed), u);
    let bottom = lerp(hash(ix, iz.wrapping_add(1), seed), hash(ix.wrapping_add(1), iz.wrapping_add(1), seed), u);
    lerp(top, bottom, v) * 2.0 - 1.0
}

/// A rounded bump: 1 in the middle, 0 at `radius` and beyond.
fn bump(dx: f32, dz: f32, radius: f32) -> f32 {
    let t = (1.0 - (dx * dx + dz * dz) / (radius * radius)).max(0.0);
    t * t
}

/// Where the cliff between the meadow and the valley runs, at this `x`.
pub fn cliff_z(x: f32) -> f32 {
    -8.0 + 5.0 * noise(x * 0.03, 0.5, 2)
}

/// Where the river runs (its middle line), at this `x`.
pub fn river_z(x: f32) -> f32 {
    -28.0 + 6.0 * noise(x * 0.03 + 7.0, 3.3, 5)
}

/// 1 on the plateau, 0 in the valley and on the meadow, with the plateau's rim (a cliff above the
/// valley, a gentle ramp from the meadow) in between.
fn plateau_mask(x: f32, z: f32) -> f32 {
    let width = lerp(4.0, 14.0, smooth(-12.0, 4.0, z));
    let across = smooth(FALL_X + width * 0.5, FALL_X - width * 0.5, x);
    let along = smooth(-46.0, -40.0, z) * (1.0 - smooth(34.0, 46.0, z));
    across * along
}

/// The land before the river is cut and the ruins' ground is levelled.
fn land(x: f32, z: f32) -> f32 {
    let meadow = 5.0 + 0.9 * noise(x * 0.05, z * 0.05, 1) + 0.35 * noise(x * 0.13 + 9.0, z * 0.13, 2);
    let hill = 5.0 * bump(x - 2.0, z - 58.0, 30.0);
    let zc = cliff_z(x);
    let to_valley = smooth(zc + 3.0, zc - 3.0, z);
    let valley = 0.5 * noise(x * 0.07, z * 0.07, 3);
    let mut h = lerp(meadow + hill, valley, to_valley);
    h += 16.0 * smooth(-54.0, -74.0, z);
    h += 9.0 * smooth(50.0, 76.0, x) * (1.0 - to_valley);
    h = lerp(h, PLATEAU + 0.4 * noise(x * 0.08, z * 0.08, 4), plateau_mask(x, z));
    let edge = (HALF_EXTENT - x.abs()).min(HALF_EXTENT - z.abs());
    h + 18.0 * smooth(14.0, 0.0, edge)
}

/// The bed of the river under `x`: low in the valley, high on the plateau.
fn river_bed(x: f32) -> f32 {
    lerp(VALLEY_BED, PLATEAU_BED, plateau_mask(x, river_z(x)))
}

/// Cut the river's channel into `h` at (`x`, `z`).
fn cut_river(h: f32, x: f32, z: f32) -> f32 {
    let distance = (z - river_z(x)).abs();
    let channel = (1.0 - smooth(2.0, 3.6, distance)) * (1.0 - smooth(60.0, 68.0, x.abs()));
    h + (river_bed(x).min(h) - h) * channel
}

/// Flatten (and lift) the ground around the ruins.
fn level_zones(h: f32, x: f32, z: f32) -> f32 {
    let mut out = h;
    for zone in ZONES {
        let (dx, dz) = (x - zone.x, z - zone.z);
        let around = (dx * dx + dz * dz).sqrt();
        let level = cut_river(land(zone.x, zone.z), zone.x, zone.z) + zone.lift;
        out = lerp(out, level, 1.0 - smooth(zone.radius, zone.radius + 6.0, around));
    }
    out
}

/// The height of the valley at a point, before it is sampled onto the grid.
fn raw_height(x: f32, z: f32) -> f32 {
    level_zones(cut_river(land(x, z), x, z), x, z)
}

/// The surface of the river's water at this `x`: a metre over the bed (flat in the valley and on
/// the plateau, falling between them).
pub fn water_level(x: f32) -> f32 {
    river_bed(x) + WATER_DEPTH
}

/// The heightfield: `(CELLS + 1)²` heights on a 2 m grid, `-HALF_EXTENT` to `HALF_EXTENT`.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    heights: Vec<f32>,
}

impl Default for Terrain {
    fn default() -> Self {
        Self::new()
    }
}

impl Terrain {
    pub fn new() -> Self {
        let mut heights = Vec::with_capacity(SIDE * SIDE);
        for j in 0..SIDE {
            for i in 0..SIDE {
                heights.push(raw_height(-HALF_EXTENT + i as f32 * CELL, -HALF_EXTENT + j as f32 * CELL));
            }
        }
        Self { heights }
    }

    /// Bytes of heights held (for the memory budget).
    pub fn bytes(&self) -> usize {
        self.heights.len() * std::mem::size_of::<f32>()
    }

    /// The height of the grid point (`i`, `j`); the edge repeats beyond the grid.
    pub fn vertex(&self, i: usize, j: usize) -> f32 {
        let (i, j) = (i.min(CELLS), j.min(CELLS));
        self.heights.get(j * SIDE + i).copied().unwrap_or(0.0)
    }

    /// Where the grid point (`i`, `j`) is.
    pub fn point(&self, i: usize, j: usize) -> Vec3 {
        Vec3::new(-HALF_EXTENT + i as f32 * CELL, self.vertex(i, j), -HALF_EXTENT + j as f32 * CELL)
    }

    /// Whether the cell's two triangles are split from corner (0,0) to corner (1,1); otherwise
    /// from (1,0) to (0,1). The split alternates like a chessboard so the facets do not all lean one way.
    fn splits_main(i: usize, j: usize) -> bool {
        (i + j).is_multiple_of(2)
    }

    /// The two triangles of the cell (`i`, `j`), wound so each faces up.
    pub fn cell_triangles(&self, i: usize, j: usize) -> [[Vec3; 3]; 2] {
        let (a, b, c, d) = (self.point(i, j), self.point(i + 1, j), self.point(i, j + 1), self.point(i + 1, j + 1));
        if Self::splits_main(i, j) { [[a, c, d], [a, d, b]] } else { [[a, c, b], [b, c, d]] }
    }

    /// Every triangle of the ground (2 · CELLS² of them).
    pub fn triangles(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        (0..CELLS).flat_map(move |j| (0..CELLS).flat_map(move |i| self.cell_triangles(i, j)))
    }

    /// The height of the ground at a point, on the same triangles that are drawn. Outside the map
    /// the nearest edge applies.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        if !x.is_finite() || !z.is_finite() {
            return 0.0;
        }
        let gx = ((x + HALF_EXTENT) / CELL).clamp(0.0, CELLS as f32);
        let gz = ((z + HALF_EXTENT) / CELL).clamp(0.0, CELLS as f32);
        let (i, j) = ((gx.floor() as usize).min(CELLS - 1), (gz.floor() as usize).min(CELLS - 1));
        let (u, v) = (gx - i as f32, gz - j as f32);
        let (h00, h10, h01, h11) = (self.vertex(i, j), self.vertex(i + 1, j), self.vertex(i, j + 1), self.vertex(i + 1, j + 1));
        if Self::splits_main(i, j) {
            if u <= v { h00 + v * (h01 - h00) + u * (h11 - h01) } else { h00 + u * (h10 - h00) + v * (h11 - h10) }
        } else if u + v <= 1.0 {
            h00 + u * (h10 - h00) + v * (h01 - h00)
        } else {
            h11 + (1.0 - u) * (h01 - h11) + (1.0 - v) * (h10 - h11)
        }
    }

    /// How steep the ground is at a point: rise over run, from the heights a metre either side.
    pub fn slope(&self, x: f32, z: f32) -> f32 {
        let dx = (self.height(x + 1.0, z) - self.height(x - 1.0, z)) * 0.5;
        let dz = (self.height(x, z + 1.0) - self.height(x, z - 1.0)) * 0.5;
        (dx * dx + dz * dz).sqrt()
    }

    /// A number that changes if any height changes (the test of determinism uses it).
    pub fn checksum(&self) -> u64 {
        self.heights.iter().fold(0xcbf2_9ce4_8422_2325_u64, |sum, h| (sum ^ u64::from(h.to_bits())).wrapping_mul(0x0100_0000_01b3))
    }
}

/// A deterministic number in `[0, 1)` for a point of the map, at a given scale: what the world's
/// scatter uses to decide where clumps of things are.
pub(crate) fn patch(x: f32, z: f32, scale: f32, seed: u32) -> f32 {
    noise(x * scale, z * scale, seed) * 0.5 + 0.5
}
