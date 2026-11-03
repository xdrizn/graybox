struct Scene {
    view_projection: mat4x4<f32>, light_projection: mat4x4<f32>,
    camera: vec4<f32>, fog: vec4<f32>, accent: vec4<f32>, tones: array<vec4<f32>, 6>,
    key_position: vec4<f32>, key_color: vec4<f32>, hemi_sky: vec4<f32>, hemi_ground: vec4<f32>,
    portal: vec4<f32>, settings: vec4<f32>, bloom: vec4<f32>,
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
fn composite(uv: vec2<f32>) -> vec3<f32> {
    let base = textureSampleLevel(image, linear_sampler, uv, 0.0).rgb;
    let bloom = textureSampleLevel(glow, linear_sampler, uv, 0.0).rgb * (4.0 * scene.bloom.x);
    return min(base + bloom, vec3(1.0));
}
@fragment fn composite_linear(input: Output) -> @location(0) vec4<f32> { return vec4(composite(input.uv), 1.0); }
@fragment fn composite_gamma(input: Output) -> @location(0) vec4<f32> {
    let linear = composite(input.uv);
    let srgb = select(linear * 12.92, 1.055 * pow(max(linear, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055, linear > vec3(0.0031308));
    return vec4(srgb, 1.0);
}
