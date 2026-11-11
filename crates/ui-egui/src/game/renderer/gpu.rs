use super::{SceneTheme, Snapshot, Target, Uniform, dynamic, linear, mesh};
use egui_wgpu::wgpu::{self, util::DeviceExt as _};
use std::{borrow::Cow, ops::Range};

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

pub(super) struct Resources {
    pub target: Target,
    uniform: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    scene_group: wgpu::BindGroup,
    post_layout: wgpu::BindGroupLayout,
    linear_sampler: wgpu::Sampler,
    noise: wgpu::Texture,
    grays: [[f32; 3]; 6],
    shadow_view: wgpu::TextureView,
    shadow_pipeline: wgpu::RenderPipeline,
    scene_pipeline: wgpu::RenderPipeline,
    threshold_pipeline: wgpu::RenderPipeline,
    horizontal_pipeline: wgpu::RenderPipeline,
    vertical_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    static_buffer: wgpu::Buffer,
    static_ranges: [Range<u32>; 6],
    /// The third-person world, built the first time that game is drawn.
    tps: Option<super::tps::TpsGpu>,
    dynamic_buffer: wgpu::Buffer,
    dynamic_capacity: u64,
    dynamic_vertices: Vec<mesh::Vertex>,
    pistol: Vec<mesh::Vertex>,
    frame: Option<Frame>,
}

struct Frame {
    size: [u32; 2],
    color: wgpu::TextureView,
    emission: wgpu::TextureView,
    depth: wgpu::TextureView,
    threshold: wgpu::TextureView,
    horizontal: wgpu::TextureView,
    blur: wgpu::TextureView,
    threshold_group: wgpu::BindGroup,
    horizontal_group: wgpu::BindGroup,
    vertical_group: wgpu::BindGroup,
    composite_group: wgpu::BindGroup,
}

fn texture(device: &wgpu::Device, label: &str, size: [u32; 2], format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn texture_entry(binding: u32, depth: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: if depth { wgpu::TextureSampleType::Depth } else { wgpu::TextureSampleType::Float { filterable: true } },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, comparison: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(if comparison { wgpu::SamplerBindingType::Comparison } else { wgpu::SamplerBindingType::Filtering }),
        count: None,
    }
}

fn uniform_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Greybox camera/light uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
            },
            count: None,
        }],
    })
}

struct Pipeline<'a> {
    vertex: &'a str,
    fragment: Option<&'a str>,
    mesh: bool,
    formats: &'a [wgpu::TextureFormat],
    depth: Option<wgpu::TextureFormat>,
    samples: u32,
    shadow: bool,
}

fn pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, shader: &wgpu::ShaderModule, spec: Pipeline<'_>) -> wgpu::RenderPipeline {
    let targets: Vec<_> =
        spec.formats.iter().map(|format| Some(wgpu::ColorTargetState { format: *format, blend: None, write_mask: wgpu::ColorWrites::ALL })).collect();
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<mesh::Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &mesh::ATTRIBUTES,
    };
    let vertex_buffers = [Some(vertex_layout)];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.fragment.unwrap_or(spec.vertex)),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(spec.vertex),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: if spec.mesh { &vertex_buffers } else { &[] },
        },
        primitive: wgpu::PrimitiveState { cull_mode: if spec.mesh { Some(wgpu::Face::Back) } else { None }, ..Default::default() },
        depth_stencil: spec.depth.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(spec.mesh),
            depth_compare: Some(if spec.mesh { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Always }),
            stencil: wgpu::StencilState::default(),
            bias: if spec.shadow { wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 } } else { wgpu::DepthBiasState::default() },
        }),
        multisample: wgpu::MultisampleState { count: spec.samples, ..Default::default() },
        fragment: spec.fragment.map(|entry| wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn layout(device: &wgpu::Device, groups: &[Option<&wgpu::BindGroupLayout>]) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Greybox pipeline layout"),
        bind_group_layouts: groups,
        immediate_size: 0,
    })
}

fn noise_pixels(grays: [[f32; 3]; 6]) -> [u8; 96 * 16 * 4] {
    let mut pixels = [0; 96 * 16 * 4];
    for (tone, base) in grays.into_iter().enumerate() {
        let mut seed = tone as u32 + 1;
        for y in 0..16 {
            for x in 0..16 {
                seed = (seed * 9301 + 49297) % 233280;
                let variation = 0.82 + seed as f32 / 233280.0 * 0.36;
                let offset = (y * 96 + tone * 16 + x) * 4;
                for channel in 0..3 {
                    pixels[offset + channel] = (base[channel] * variation * 255.0).clamp(0.0, 255.0) as u8;
                }
                pixels[offset + 3] = 255;
            }
        }
    }
    pixels
}

fn upload_noise(queue: &wgpu::Queue, texture: &wgpu::Texture, grays: [[f32; 3]; 6]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &noise_pixels(grays),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(96 * 4), rows_per_image: Some(16) },
        wgpu::Extent3d { width: 96, height: 16, depth_or_array_layers: 1 },
    );
}

fn static_mesh(device: &wgpu::Device, mut objects: Vec<figcraft_game::arena::SceneObject>) -> (wgpu::Buffer, [Range<u32>; 6]) {
    objects.sort_by_key(|object| object.tone.min(5));
    let mut vertices = Vec::with_capacity(objects.len() * 36);
    let mut iter = objects.into_iter().peekable();
    let ranges = std::array::from_fn(|tone| {
        let start = vertices.len() as u32;
        while iter.peek().is_some_and(|object| object.tone.min(5) == tone as u32) {
            if let Some(object) = iter.next() {
                mesh::object(&mut vertices, &object, glam::Mat4::IDENTITY, false);
            }
        }
        start..vertices.len() as u32
    });
    (
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Greybox static arena batches by tone"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        ranges,
    )
}

impl Resources {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: Target, theme: &SceneTheme) -> Self {
        let uniform_layout = uniform_layout(device);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Greybox scene uniforms"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Greybox scene uniforms"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Greybox noise and shadow textures"),
            entries: &[texture_entry(0, false), sampler_entry(1, false), texture_entry(2, true), sampler_entry(3, true)],
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Greybox postprocess inputs"),
            entries: &[texture_entry(0, false), texture_entry(1, false), sampler_entry(2, false)],
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Greybox nearest 16x16 pixels"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Greybox bloom linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let comparison = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Greybox shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let noise = texture(device, "Greybox six deterministic noise tiles", [96, 16], wgpu::TextureFormat::Rgba8UnormSrgb);
        upload_noise(queue, &noise, theme.grays);
        let noise_view = noise.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_view =
            texture(device, "Greybox directional shadow map 2048", [2048, 2048], DEPTH).create_view(&wgpu::TextureViewDescriptor::default());
        let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Greybox material resources"),
            layout: &scene_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&noise_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&nearest) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&comparison) },
            ],
        });
        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Greybox flat shading and shadows"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("scene.wgsl"))),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Greybox accent-only bloom"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("post.wgsl"))),
        });
        let shadow_layout = layout(device, &[Some(&uniform_layout)]);
        let scene_pipeline_layout = layout(device, &[Some(&uniform_layout), Some(&scene_layout)]);
        let post_pipeline_layout = layout(device, &[Some(&uniform_layout), Some(&post_layout)]);
        let shadow_pipeline = pipeline(
            device,
            &shadow_layout,
            &scene_shader,
            Pipeline { vertex: "shadow_vertex", fragment: None, mesh: true, formats: &[], depth: Some(DEPTH), samples: 1, shadow: true },
        );
        let scene_pipeline = pipeline(
            device,
            &scene_pipeline_layout,
            &scene_shader,
            Pipeline {
                vertex: "scene_vertex",
                fragment: Some("scene_fragment"),
                mesh: true,
                formats: &[COLOR, COLOR],
                depth: Some(DEPTH),
                samples: 1,
                shadow: false,
            },
        );
        let post_pipeline = |entry| {
            pipeline(
                device,
                &post_pipeline_layout,
                &post_shader,
                Pipeline { vertex: "fullscreen", fragment: Some(entry), mesh: false, formats: &[COLOR], depth: None, samples: 1, shadow: false },
            )
        };
        let threshold_pipeline = post_pipeline("threshold");
        let horizontal_pipeline = post_pipeline("horizontal");
        let vertical_pipeline = post_pipeline("vertical");
        let composite_pipeline = pipeline(
            device,
            &post_pipeline_layout,
            &post_shader,
            Pipeline {
                vertex: "fullscreen",
                fragment: Some(if target.format.is_srgb() { "composite_linear" } else { "composite_gamma" }),
                mesh: false,
                formats: &[target.format],
                depth: target.depth,
                samples: target.samples,
                shadow: false,
            },
        );
        let (static_buffer, static_ranges) = static_mesh(device, figcraft_game::arena::objects());
        let dynamic_capacity = (std::mem::size_of::<mesh::Vertex>() * 4096) as u64;
        let dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Greybox dynamic geometry"),
            size: dynamic_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            target,
            uniform,
            uniform_group,
            scene_group,
            post_layout,
            linear_sampler,
            noise,
            grays: theme.grays,
            shadow_view,
            shadow_pipeline,
            scene_pipeline,
            threshold_pipeline,
            horizontal_pipeline,
            vertical_pipeline,
            composite_pipeline,
            static_buffer,
            static_ranges,
            tps: None,
            dynamic_buffer,
            dynamic_capacity,
            dynamic_vertices: Vec::with_capacity(4096),
            pistol: mesh::pistol(),
            frame: None,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        size: [u32; 2],
        snapshot: &Snapshot,
        theme: &SceneTheme,
    ) {
        if self.frame.as_ref().is_none_or(|frame| frame.size != size) {
            self.frame = Some(Frame::new(device, size, &self.post_layout, &self.linear_sampler));
        }
        if self.grays != theme.grays {
            upload_noise(queue, &self.noise, theme.grays);
            self.grays = theme.grays;
        }
        let third_person = snapshot.tps.is_some();
        if let Some(draw) = &snapshot.tps {
            let noise = &self.noise;
            let tps = self
                .tps
                .get_or_insert_with(|| super::tps::TpsGpu::new(device, noise, super::tps::SceneData::build(&figcraft_game::tps::World::new())));
            tps.update(queue, draw.pose.position, draw.time, snapshot.eye);
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&Uniform::new(snapshot, theme, size)));
        let world_count = dynamic(&mut self.dynamic_vertices, snapshot, &self.pistol);
        let bytes = bytemuck::cast_slice(&self.dynamic_vertices);
        let required = bytes.len() as u64;
        if required > self.dynamic_capacity {
            self.dynamic_capacity = required.next_power_of_two();
            self.dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Greybox expanded dynamic geometry"),
                size: self.dynamic_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.dynamic_buffer, 0, bytes);
        }
        self.render_shadow(encoder, world_count, third_person);
        if let Some(frame) = &self.frame {
            self.render_scene(encoder, frame, world_count, theme, third_person);
            self.render_bloom(encoder, frame);
        }
    }

    fn render_shadow(&self, encoder: &mut wgpu::CommandEncoder, world_count: u32, third_person: bool) {
        // Graybox: the third-person world draws its own shadow map.
        if let (true, Some(tps)) = (third_person, &self.tps) {
            tps.render_shadow(encoder, &self.uniform_group, &self.dynamic_buffer, world_count);
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Greybox directional shadow pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(depth_attachment(&self.shadow_view)),
            ..Default::default()
        });
        pass.set_pipeline(&self.shadow_pipeline);
        pass.set_bind_group(0, &self.uniform_group, &[]);
        pass.set_vertex_buffer(0, self.static_buffer.slice(..));
        for range in &self.static_ranges {
            pass.draw(range.clone(), 0..1);
        }
        pass.set_vertex_buffer(0, self.dynamic_buffer.slice(..));
        pass.draw(0..world_count, 0..1);
    }

    fn render_scene(&self, encoder: &mut wgpu::CommandEncoder, frame: &Frame, world_count: u32, theme: &SceneTheme, third_person: bool) {
        let [r, g, b] = linear(theme.background);
        let background = wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: 1.0 };
        // Graybox: the third-person world draws its own picture (no gun to draw after it).
        if let (true, Some(tps)) = (third_person, &self.tps) {
            let targets = super::tps::Targets { color: &frame.color, emission: &frame.emission, depth: &frame.depth, background };
            tps.render_scene(encoder, &targets, &self.uniform_group, &self.dynamic_buffer, world_count);
            return;
        }
        {
            let attachments = [
                Some(color_attachment(&frame.color, wgpu::LoadOp::Clear(background))),
                Some(color_attachment(&frame.emission, wgpu::LoadOp::Clear(wgpu::Color::BLACK))),
            ];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Greybox arena and sentinels"),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(depth_attachment(&frame.depth)),
                ..Default::default()
            });
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.uniform_group, &[]);
            pass.set_bind_group(1, &self.scene_group, &[]);
            pass.set_vertex_buffer(0, self.static_buffer.slice(..));
            for range in &self.static_ranges {
                pass.draw(range.clone(), 0..1);
            }
            pass.set_vertex_buffer(0, self.dynamic_buffer.slice(..));
            pass.draw(0..world_count, 0..1);
        }
        // A separate depth clear keeps the viewmodel out of nearby wall geometry.
        let attachments = [Some(color_attachment(&frame.color, wgpu::LoadOp::Load)), Some(color_attachment(&frame.emission, wgpu::LoadOp::Load))];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Greybox pistol and 50ms muzzle flash"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(depth_attachment(&frame.depth)),
            ..Default::default()
        });
        pass.set_pipeline(&self.scene_pipeline);
        pass.set_bind_group(0, &self.uniform_group, &[]);
        pass.set_bind_group(1, &self.scene_group, &[]);
        pass.set_vertex_buffer(0, self.dynamic_buffer.slice(..));
        pass.draw(world_count..self.dynamic_vertices.len() as u32, 0..1);
    }

    fn render_bloom(&self, encoder: &mut wgpu::CommandEncoder, frame: &Frame) {
        // Threshold at half resolution, then blur while downsampling to a quarter.
        // The filtered reconstruction gives a compact mip-style bloom without
        // float render targets, compute, storage buffers, or optional features.
        for (label, view, pipeline, group) in [
            ("Greybox orange threshold", &frame.threshold, &self.threshold_pipeline, &frame.threshold_group),
            ("Greybox bloom horizontal downsample", &frame.horizontal, &self.horizontal_pipeline, &frame.horizontal_group),
            ("Greybox bloom vertical", &frame.blur, &self.vertical_pipeline, &frame.vertical_group),
        ] {
            let attachments = [Some(color_attachment(view, wgpu::LoadOp::Clear(wgpu::Color::BLACK)))];
            let mut pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some(label), color_attachments: &attachments, ..Default::default() });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.uniform_group, &[]);
            pass.set_bind_group(1, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    pub fn paint(&self, pass: &mut wgpu::RenderPass<'static>) {
        if let Some(frame) = &self.frame {
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &self.uniform_group, &[]);
            pass.set_bind_group(1, &frame.composite_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

fn depth_attachment(view: &wgpu::TextureView) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
        stencil_ops: None,
    }
}

fn color_attachment(view: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } }
}

impl Frame {
    fn new(device: &wgpu::Device, size: [u32; 2], layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler) -> Self {
        let view = |label, size, format| texture(device, label, size, format).create_view(&wgpu::TextureViewDescriptor::default());
        let color = view("Greybox linear scene", size, COLOR);
        let emission = view("Greybox encoded orange emission", size, COLOR);
        let depth = view("Greybox scene depth", size, DEPTH);
        let half = [size[0].div_ceil(2).max(1), size[1].div_ceil(2).max(1)];
        let quarter = [size[0].div_ceil(4).max(1), size[1].div_ceil(4).max(1)];
        let threshold = view("Greybox bloom threshold half resolution", half, COLOR);
        let horizontal = view("Greybox bloom quarter resolution horizontal", quarter, COLOR);
        let blur = view("Greybox bloom quarter resolution vertical", quarter, COLOR);
        let group = |image: &wgpu::TextureView, glow: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Greybox postprocess textures"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(image) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(glow) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        let threshold_group = group(&emission, &emission);
        let horizontal_group = group(&threshold, &threshold);
        let vertical_group = group(&horizontal, &horizontal);
        let composite_group = group(&color, &blur);
        Self { size, color, emission, depth, threshold, horizontal, blur, threshold_group, horizontal_group, vertical_group, composite_group }
    }
}
