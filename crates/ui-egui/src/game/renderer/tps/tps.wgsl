// The third-person world's extra shaders. This file is appended to scene.wgsl (same module), so it
// shares the uniforms, `Input`, `Output`, `visibility` and `shade`.
//
// - prop_vertex / prop_shadow_vertex: one mesh drawn many times, each instance moved and turned.
// - grass_vertex: tufts of blades that sway in the wind, bend away from the hero and shrink with
//   distance, all in the vertex shader (no per-blade CPU work after placement).
// - water_vertex / water_fragment: the river and the waterfall, with a glow that runs along them.
struct Tps {
    // xyz: the hero's feet; w: how far from the hero the grass bends away.
    hero: vec4<f32>,
    // x: seconds; y: wind strength; z: grass fade starts (m); w: grass is gone (m).
    wind: vec4<f32>,
};
@group(2) @binding(0) var<uniform> tps: Tps;

struct Instance {
    // xyz: where the prop stands; w: turn about the vertical axis (radians).
    @location(3) at: vec4<f32>,
    // x: size (uniform scale).
    @location(4) size: vec4<f32>,
};
struct Tuft {
    // xyz: where the tuft grows; w: a number in 0..1 that picks its turn, height and shade.
    @location(3) at: vec4<f32>,
};

fn turn(v: vec3<f32>, yaw: f32) -> vec3<f32> {
    let c = cos(yaw);
    let s = sin(yaw);
    return vec3(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

fn finish(world: vec3<f32>, normal: vec3<f32>, material: vec4<f32>) -> Output {
    var output: Output;
    output.position = scene.view_projection * vec4(world, 1.0);
    output.world = world;
    output.normal = normal;
    output.material = material;
    output.shadow = scene.light_projection * vec4(world, 1.0);
    return output;
}

@vertex fn prop_shadow_vertex(input: Input, instance: Instance) -> @builtin(position) vec4<f32> {
    let world = turn(input.position * instance.size.x, instance.at.w) + instance.at.xyz;
    return scene.light_projection * vec4(world, 1.0);
}

@vertex fn prop_vertex(input: Input, instance: Instance) -> Output {
    let world = turn(input.position * instance.size.x, instance.at.w) + instance.at.xyz;
    return finish(world, turn(input.normal, instance.at.w), input.material);
}

@vertex fn grass_vertex(input: Input, tuft: Tuft) -> Output {
    let seed = tuft.at.w;
    let base = tuft.at.xyz;
    let time = tps.wind.x;
    // Shorter and thinner with distance, gone at the fade distance.
    let away = distance(base.xz, scene.camera.xz);
    let fade = 1.0 - smoothstep(tps.wind.z, tps.wind.w, away);
    let height = (0.32 + 0.45 * fract(seed * 7.31)) * fade;
    let bend = input.position.y * input.position.y;
    var local = turn(vec3(input.position.x * fade, 0.0, input.position.z * fade), seed * 6.2831);
    // Wind: a slow swell that travels across the field, with a faster flutter on top.
    let swell = sin(time * 1.4 + base.x * 0.21 + base.z * 0.13);
    let flutter = sin(time * 3.1 + base.x * 0.9 - base.z * 0.7) * 0.35;
    let gust = 0.5 + 0.5 * sin(time * 0.4 + base.x * 0.05 + base.z * 0.04);
    let sway = (swell + flutter) * gust * tps.wind.y * bend;
    local.x += sway * 0.32;
    local.z += sway * 0.16;
    // The hero pushes the blades away, and flattens them a little.
    let rel = base.xz + local.xz - tps.hero.xz;
    let reach = length(rel);
    let push = 1.0 - smoothstep(0.25, tps.hero.w, reach);
    let outward = rel / max(reach, 0.05);
    local.x += outward.x * push * 0.75 * bend;
    local.z += outward.y * push * 0.75 * bend;
    let lowered = 1.0 - 0.5 * push * input.position.y;
    let world = base + vec3(local.x, input.position.y * height * lowered, local.z);
    var material = input.material;
    // A few tufts are a shade lighter, so a field is not one flat colour.
    material.x = min(material.x + select(0.0, 1.0, fract(seed * 13.7) > 0.8), 5.0);
    return finish(world, turn(input.normal, seed * 6.2831), material);
}

struct Water {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) material: vec4<f32>,
    @location(3) shadow: vec4<f32>,
    // 0 along the middle of the ribbon, 1 on its banks.
    @location(4) edge: f32,
};

@vertex fn water_vertex(input: Input) -> Water {
    var output: Water;
    output.position = scene.view_projection * vec4(input.position, 1.0);
    output.world = input.position;
    output.normal = input.normal;
    output.material = vec4(input.material.xyz, 0.0);
    output.shadow = scene.light_projection * vec4(input.position, 1.0);
    output.edge = input.material.w;
    return output;
}

fn lane_hash(lane: f32) -> f32 {
    return fract(sin(lane * 12.9898) * 43758.5453);
}

@fragment fn water_fragment(input: Water) -> Lit {
    var lit: Output;
    lit.position = input.position;
    lit.world = input.world;
    lit.normal = input.normal;
    lit.shadow = input.shadow;
    lit.material = input.material;
    let time = tps.wind.x;
    // Tone 0 is the river (a dark mirror with a glowing bank); tone 2 is the waterfall.
    let falls = input.material.x > 1.0;
    let lane = floor(input.world.z * 1.6);
    let phase = lane_hash(lane);
    var glow = 0.0;
    if falls {
        let streak = fract(input.world.y * 0.45 + time * (0.7 + phase * 0.6) + phase);
        glow = 0.35 + 0.65 * smoothstep(0.5, 0.95, streak) + 0.3 * input.edge;
    } else {
        let ripple = fract(input.world.x * 0.18 - time * 0.25 + lane_hash(floor(input.world.z * 0.9)));
        glow = smoothstep(0.55, 1.0, input.edge) * (0.65 + 0.35 * sin(ripple * 6.2831)) + 0.05 * smoothstep(0.7, 1.0, ripple);
    }
    lit.material.y = input.material.y * glow;
    return shade(lit);
}
