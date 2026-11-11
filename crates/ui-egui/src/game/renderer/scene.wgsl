struct Scene {
    view_projection: mat4x4<f32>,
    light_projection: mat4x4<f32>,
    camera: vec4<f32>,
    fog: vec4<f32>,
    accent: vec4<f32>,
    tones: array<vec4<f32>, 6>,
    key_position: vec4<f32>,
    key_color: vec4<f32>,
    hemi_sky: vec4<f32>,
    hemi_ground: vec4<f32>,
    portal: vec4<f32>,
    settings: vec4<f32>,
    bloom: vec4<f32>,
};
@group(0) @binding(0) var<uniform> scene: Scene;
@group(1) @binding(0) var noise: texture_2d<f32>;
@group(1) @binding(1) var nearest_sampler: sampler;
@group(1) @binding(2) var shadow_map: texture_depth_2d;
@group(1) @binding(3) var shadow_sampler: sampler_comparison;
struct Input {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) material: vec4<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) material: vec4<f32>,
    @location(3) shadow: vec4<f32>,
};
@vertex fn shadow_vertex(input: Input) -> @builtin(position) vec4<f32> {
    return scene.light_projection * vec4(input.position, 1.0);
}
@vertex fn scene_vertex(input: Input) -> Output {
    var output: Output;
    output.position = scene.view_projection * vec4(input.position, 1.0);
    output.world = input.position;
    output.normal = input.normal;
    output.material = input.material;
    output.shadow = scene.light_projection * vec4(input.position, 1.0);
    return output;
}
fn visibility(shadow: vec4<f32>, normal: vec3<f32>, light: vec3<f32>) -> f32 {
    let ndc = shadow.xyz / shadow.w;
    let uv = vec2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    // Always sample (uniform control flow is required by WebGL2 derivatives).
    let bias = max(0.0004, 0.0012 * (1.0 - dot(normal, light)));
    var sum = 0.0;
    let texel = select(1.0 / 2048.0, scene.bloom.w, scene.bloom.w > 0.0);
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            // Graybox: the third-person scene sets the shadow map's texel size (1/1024 in the browser).
            sum += textureSampleCompare(shadow_map, shadow_sampler, uv + vec2(f32(x), f32(y)) * texel, ndc.z - bias);
        }
    }
    let inside = all(uv >= vec2(0.0)) && all(uv <= vec2(1.0)) && ndc.z >= 0.0 && ndc.z <= 1.0;
    return select(1.0, sum / 9.0, inside);
}
struct Lit {
    @location(0) color: vec4<f32>,
    @location(1) emission: vec4<f32>,
};
@fragment fn scene_fragment(input: Output) -> Lit {
    return shade(input);
}
// Graybox: the body of scene_fragment as a function, so the third-person water can call it.
fn shade(input: Output) -> Lit {
    let normal = normalize(input.normal);
    let weights = abs(normal);
    var uv = input.world.xy * 0.5;
    if weights.y > weights.x && weights.y > weights.z { uv = input.world.xz * 0.5; }
    else if weights.x > weights.z { uv = input.world.zy * 0.5; }
    let tone = u32(clamp(input.material.x, 0.0, 5.0));
    let atlas_uv = vec2((f32(tone) + fract(uv.x)) / 6.0, fract(uv.y));
    // Explicit LOD avoids derivative restrictions where face selection branches.
    let textured = textureSampleLevel(noise, nearest_sampler, atlas_uv, 0.0).rgb;
    let base = mix(scene.tones[tone].rgb, textured, scene.settings.x);
    let to_key = normalize(scene.key_position.xyz);
    let shadow = visibility(input.shadow, normal, to_key);
    let hemi = mix(scene.hemi_ground.rgb, scene.hemi_sky.rgb, normal.y * 0.5 + 0.5) * scene.hemi_sky.w;
    let illumination = vec3(scene.settings.y) + hemi + scene.key_color.rgb * scene.key_color.w * max(dot(normal, to_key), 0.0) * shadow;
    let to_portal = scene.portal.xyz - input.world;
    let distance_to_portal = max(length(to_portal), 0.1);
    let light_direction = to_portal / distance_to_portal;
    let falloff = pow(max(1.0 - distance_to_portal / scene.settings.z, 0.0), 2.0) * scene.portal.w / pow(max(distance_to_portal, 1.0), scene.settings.w);
    let diffuse_portal = scene.accent.rgb * falloff * max(dot(normal, light_direction), 0.0);
    let view_direction = normalize(scene.camera.xyz - input.world);
    let half_direction = normalize(light_direction + view_direction);
    let roughness = clamp(input.material.z, 0.05, 1.0);
    let specular_power = mix(180.0, 4.0, roughness * roughness);
    let specular = pow(max(dot(normal, half_direction), 0.0), specular_power) * (1.0 - roughness) * falloff;
    let emission = scene.accent.rgb * input.material.y * (scene.accent.w / 1.6);
    let viewmodel = input.material.w;
    let gun_light = vec3(1.25) + vec3(max(normal.y, 0.0) * 0.5);
    let diffuse = base * mix(illumination + diffuse_portal, gun_light, viewmodel);
    // Diffuse stays below the bloom threshold; only accent emission enters bloom.
    var color = min(diffuse, vec3(0.88)) + scene.accent.rgb * specular + emission;
    let distance_to_camera = distance(scene.camera.xyz, input.world);
    let fog_amount = (1.0 - exp(-pow(distance_to_camera * scene.fog.w, 2.0))) * (1.0 - viewmodel);
    color = mix(color, scene.fog.rgb, fog_amount);
    var output: Lit;
    output.color = vec4(color, 1.0);
    // Encode emissive HDR in portable RGBA8 instead of requiring float-render-target extensions.
    output.emission = vec4(emission * (1.0 - fog_amount) * 0.25, 1.0);
    return output;
}
