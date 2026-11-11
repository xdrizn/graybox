struct Scene {
    view_projection: mat4x4<f32>, light_projection: mat4x4<f32>,
    camera: vec4<f32>, fog: vec4<f32>, accent: vec4<f32>, tones: array<vec4<f32>, 6>,
    key_position: vec4<f32>, key_color: vec4<f32>, hemi_sky: vec4<f32>, hemi_ground: vec4<f32>,
    portal: vec4<f32>, settings: vec4<f32>, bloom: vec4<f32>, look: vec4<f32>,
};
@group(0) @binding(0) var<uniform> scene: Scene;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var glow: texture_2d<f32>;
@group(1) @binding(2) var linear_sampler: sampler;
struct Output { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn fullscreen(@builtin(vertex_index) index: u32) -> Output {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var output: Output;
    output.position = vec4(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    output.uv = vec2(x, 1.0 - y);
    return output;
}
@fragment fn threshold(input: Output) -> @location(0) vec4<f32> {
    let emission = textureSampleLevel(image, linear_sampler, input.uv, 0.0).rgb * 4.0;
    let brightness = max(emission.r, max(emission.g, emission.b));
    let weight = smoothstep(scene.bloom.y - scene.bloom.z, scene.bloom.y + scene.bloom.z, brightness);
    return vec4(emission * weight * 0.25, 1.0);
}
fn blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32> {
    let step = direction / vec2<f32>(textureDimensions(image));
    var color = textureSampleLevel(image, linear_sampler, uv, 0.0).rgb * 0.227027;
    color += textureSampleLevel(image, linear_sampler, uv + step * 1.384615, 0.0).rgb * 0.316216;
    color += textureSampleLevel(image, linear_sampler, uv - step * 1.384615, 0.0).rgb * 0.316216;
    color += textureSampleLevel(image, linear_sampler, uv + step * 3.230769, 0.0).rgb * 0.070270;
    color += textureSampleLevel(image, linear_sampler, uv - step * 3.230769, 0.0).rgb * 0.070270;
    return vec4(color, 1.0);
}
@fragment fn horizontal(input: Output) -> @location(0) vec4<f32> { return blur(input.uv, vec2(1.0, 0.0)); }
@fragment fn vertical(input: Output) -> @location(0) vec4<f32> { return blur(input.uv, vec2(0.0, 1.0)); }
// Graybox: edge smoothing for the third-person picture (FXAA's luma-edge idea, in its cheap form).
// Brightness is compared on the square root of the colour, so edges in the dark count as much as
// edges in the light.
fn luma(color: vec3<f32>) -> f32 {
    return dot(sqrt(max(color, vec3(0.0))), vec3(0.299, 0.587, 0.114));
}
fn smooth_edges(uv: vec2<f32>) -> vec3<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(image));
    let middle = textureSampleLevel(image, linear_sampler, uv, 0.0).rgb;
    let nw = textureSampleLevel(image, linear_sampler, uv + vec2(-1.0, -1.0) * texel, 0.0).rgb;
    let ne = textureSampleLevel(image, linear_sampler, uv + vec2(1.0, -1.0) * texel, 0.0).rgb;
    let sw = textureSampleLevel(image, linear_sampler, uv + vec2(-1.0, 1.0) * texel, 0.0).rgb;
    let se = textureSampleLevel(image, linear_sampler, uv + vec2(1.0, 1.0) * texel, 0.0).rgb;
    let l_nw = luma(nw);
    let l_ne = luma(ne);
    let l_sw = luma(sw);
    let l_se = luma(se);
    let l_m = luma(middle);
    let l_min = min(l_m, min(min(l_nw, l_ne), min(l_sw, l_se)));
    let l_max = max(l_m, max(max(l_nw, l_ne), max(l_sw, l_se)));
    var direction = vec2(-((l_nw + l_ne) - (l_sw + l_se)), (l_nw + l_sw) - (l_ne + l_se));
    let reduce = max((l_nw + l_ne + l_sw + l_se) * 0.03125, 0.0078125);
    let scale = 1.0 / (min(abs(direction.x), abs(direction.y)) + reduce);
    direction = clamp(direction * scale, vec2(-8.0), vec2(8.0)) * texel;
    let a = 0.5 * (textureSampleLevel(image, linear_sampler, uv + direction * (1.0 / 3.0 - 0.5), 0.0).rgb
        + textureSampleLevel(image, linear_sampler, uv + direction * (2.0 / 3.0 - 0.5), 0.0).rgb);
    let b = a * 0.5 + 0.25 * (textureSampleLevel(image, linear_sampler, uv + direction * -0.5, 0.0).rgb
        + textureSampleLevel(image, linear_sampler, uv + direction * 0.5, 0.0).rgb);
    let l_b = luma(b);
    return select(b, a, l_b < l_min || l_b > l_max);
}
fn composite(uv: vec2<f32>) -> vec3<f32> {
    var base = textureSampleLevel(image, linear_sampler, uv, 0.0).rgb;
    if scene.look.y > 0.5 { base = smooth_edges(uv); }
    let bloom = textureSampleLevel(glow, linear_sampler, uv, 0.0).rgb * (4.0 * scene.bloom.x);
    return min(base + bloom, vec3(1.0));
}
@fragment fn composite_linear(input: Output) -> @location(0) vec4<f32> { return vec4(composite(input.uv), 1.0); }
@fragment fn composite_gamma(input: Output) -> @location(0) vec4<f32> {
    let linear = composite(input.uv);
    let srgb = select(linear * 12.92, 1.055 * pow(max(linear, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055, linear > vec3(0.0031308));
    return vec4(srgb, 1.0);
}
