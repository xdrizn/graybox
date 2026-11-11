//! The third-person world on the GPU: its own shadow map, pipelines and buffers, drawn with the
//! shared scene uniforms (camera, fog, lights) of the first-person renderer.
//!
//! Per frame the world costs a handful of draw calls: the ground and ruins in one, the water in
//! one, one per kind of prop (instanced), one per row of grass chunks near the camera (instanced),
//! and the hero and sky from the dynamic buffer.
use super::super::{Uniform, mesh};
use super::grass::{FADE_END, FADE_START, Grass, TUFT_VERTICES};
use super::look::TpsLook;
use super::scene::{SHADOW_SIZE, SceneData};
use egui_wgpu::wgpu::{self, util::DeviceExt as _};
use glam::Vec3;
use std::{borrow::Cow, ops::Range};

/// The picture is stored sRGB-encoded (the GPU converts as it writes and reads), so the dark gradients
/// of the sky and fog keep their precision instead of showing steps. The emission buffer stays linear.
pub(in crate::game::renderer) const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const EMISSION: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
/// How hard the wind blows on the grass, and how far from the hero it bends away (metres).
const WIND: f32 = 0.9;
const BEND_RADIUS: f32 = 1.8;
/// An instance of a prop is 8 floats (32 bytes); a grass tuft is 4 floats (16 bytes).
const PROP_STRIDE: u64 = 32;
const TUFT_STRIDE: u64 = 16;
/// The wind-and-sky uniform: hero, wind, horizon and zenith, four floats each.
const PARAMS_SIZE: u64 = 64;

const PROP_ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4];
const TUFT_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![3 => Float32x4];

struct PropBatch {
    vertices: wgpu::Buffer,
    vertex_count: u32,
    instances: wgpu::Buffer,
    instance_count: u32,
}

/// The views the picture is drawn into.
pub(in crate::game::renderer) struct Targets<'a> {
    pub color: &'a wgpu::TextureView,
    pub emission: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub background: wgpu::Color,
}

pub(in crate::game::renderer) struct TpsGpu {
    shadow_view: wgpu::TextureView,
    scene_group: wgpu::BindGroup,
    params: wgpu::Buffer,
    params_group: wgpu::BindGroup,
    shadow_static: wgpu::RenderPipeline,
    shadow_prop: wgpu::RenderPipeline,
    scene_static: wgpu::RenderPipeline,
    scene_prop: wgpu::RenderPipeline,
    scene_grass: wgpu::RenderPipeline,
    scene_water: wgpu::RenderPipeline,
    scene_sky: wgpu::RenderPipeline,
    statics: wgpu::Buffer,
    solid: Range<u32>,
    backdrop: Range<u32>,
    water: Range<u32>,
    sky: Range<u32>,
    props: Vec<PropBatch>,
    tuft: wgpu::Buffer,
    tufts: wgpu::Buffer,
    grass: Grass,
    camera: Vec3,
}

fn buffer(device: &wgpu::Device, label: &str, contents: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
}

fn entry(binding: u32, depth: bool) -> wgpu::BindGroupLayoutEntry {
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

fn uniform_entry(size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

/// How a pipeline of this world differs from the others.
struct Spec<'a> {
    vertex: &'a str,
    fragment: Option<&'a str>,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    cull: bool,
    /// The sky dome: drawn first, behind everything (it neither tests nor writes depth).
    sky: bool,
}

fn pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, shader: &wgpu::ShaderModule, spec: Spec<'_>) -> wgpu::RenderPipeline {
    let shadow = spec.fragment.is_none();
    let targets = [
        Some(wgpu::ColorTargetState { format: COLOR, blend: None, write_mask: wgpu::ColorWrites::ALL }),
        Some(wgpu::ColorTargetState { format: EMISSION, blend: None, write_mask: wgpu::ColorWrites::ALL }),
    ];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.fragment.unwrap_or(spec.vertex)),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(spec.vertex),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: spec.buffers,
        },
        primitive: wgpu::PrimitiveState { cull_mode: if spec.cull { Some(wgpu::Face::Back) } else { None }, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(!spec.sky),
            depth_compare: Some(if spec.sky { wgpu::CompareFunction::Always } else { wgpu::CompareFunction::LessEqual }),
            stencil: wgpu::StencilState::default(),
            bias: if shadow { wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 } } else { wgpu::DepthBiasState::default() },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: spec.fragment.map(|entry_point| wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn depth_attachment(view: &wgpu::TextureView) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
        stencil_ops: None,
    }
}

fn color_attachment(view: &wgpu::TextureView, load: wgpu::Color) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(load), store: wgpu::StoreOp::Store },
    }
}

impl TpsGpu {
    pub fn new(device: &wgpu::Device, noise: &wgpu::Texture, mut data: SceneData) -> Self {
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Greybox camera/light uniforms"),
            entries: &[uniform_entry(std::mem::size_of::<Uniform>() as u64)],
        });
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Greybox noise and shadow textures"),
            entries: &[entry(0, false), sampler_entry(1, false), entry(2, true), sampler_entry(3, true)],
        });
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Greybox third-person wind and hero"),
            entries: &[uniform_entry(PARAMS_SIZE)],
        });
        let shadow_view = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Greybox third-person shadow map"),
                size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Greybox nearest 16x16 pixels"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let comparison = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Greybox shadow comparison"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            // Linear: each tap is itself a 2 x 2 blend, so the soft shadow edge has no steps.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let noise_view = noise.create_view(&wgpu::TextureViewDescriptor::default());
        let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Greybox third-person material resources"),
            layout: &scene_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&noise_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&nearest) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&comparison) },
            ],
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Greybox third-person wind and hero"),
            size: PARAMS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Greybox third-person wind and hero"),
            layout: &params_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() }],
        });
        let source = format!("{}\n{}", include_str!("../scene.wgsl"), include_str!("tps.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Greybox third-person world"),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(source)),
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Greybox third-person shadow layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let scene_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Greybox third-person layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&scene_layout), Some(&params_layout)],
            immediate_size: 0,
        });
        let vertex_buffer = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<mesh::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &mesh::ATTRIBUTES,
        };
        let prop_buffers = [
            Some(vertex_buffer.clone()),
            Some(wgpu::VertexBufferLayout { array_stride: PROP_STRIDE, step_mode: wgpu::VertexStepMode::Instance, attributes: &PROP_ATTRIBUTES }),
        ];
        let grass_buffers = [
            Some(vertex_buffer.clone()),
            Some(wgpu::VertexBufferLayout { array_stride: TUFT_STRIDE, step_mode: wgpu::VertexStepMode::Instance, attributes: &TUFT_ATTRIBUTES }),
        ];
        let single = [Some(vertex_buffer)];
        let shadow_static =
            pipeline(device, &shadow_layout, &shader, Spec { vertex: "shadow_vertex", fragment: None, buffers: &single, cull: true, sky: false });
        let shadow_prop = pipeline(
            device,
            &shadow_layout,
            &shader,
            Spec { vertex: "prop_shadow_vertex", fragment: None, buffers: &prop_buffers, cull: true, sky: false },
        );
        let lit = |vertex, fragment, buffers: &[Option<wgpu::VertexBufferLayout<'_>>], cull| {
            pipeline(device, &scene_pipeline_layout, &shader, Spec { vertex, fragment: Some(fragment), buffers, cull, sky: false })
        };
        let scene_sky = pipeline(
            device,
            &scene_pipeline_layout,
            &shader,
            Spec { vertex: "scene_vertex", fragment: Some("sky_fragment"), buffers: &single, cull: false, sky: true },
        );
        let scene_static = lit("scene_vertex", "scene_fragment", &single, true);
        let scene_prop = lit("prop_vertex", "scene_fragment", &prop_buffers, true);
        let scene_grass = lit("grass_vertex", "scene_fragment", &grass_buffers, false);
        let scene_water = lit("water_vertex", "water_fragment", &single, true);

        let statics =
            buffer(device, "Greybox third-person ground and ruins", bytemuck::cast_slice(&data.statics.vertices), wgpu::BufferUsages::VERTEX);
        let props = data
            .batches
            .iter()
            .filter(|batch| !batch.instances.is_empty() && !batch.vertices.is_empty())
            .map(|batch| PropBatch {
                vertices: buffer(device, "Greybox prop mesh", bytemuck::cast_slice(&batch.vertices), wgpu::BufferUsages::VERTEX),
                vertex_count: batch.vertices.len() as u32,
                instances: buffer(device, "Greybox prop instances", bytemuck::cast_slice(&batch.instances), wgpu::BufferUsages::VERTEX),
                instance_count: batch.instances.len() as u32,
            })
            .collect();
        let tuft = buffer(device, "Greybox grass tuft", bytemuck::cast_slice(&data.tuft), wgpu::BufferUsages::VERTEX);
        let tufts = buffer(device, "Greybox grass tufts", bytemuck::cast_slice(&data.grass.tufts), wgpu::BufferUsages::VERTEX);
        // The placements live on the GPU now; the chunk table is all the CPU keeps.
        data.grass.tufts = Vec::new();
        Self {
            shadow_view,
            scene_group,
            params,
            params_group,
            shadow_static,
            shadow_prop,
            scene_static,
            scene_prop,
            scene_grass,
            scene_water,
            scene_sky,
            statics,
            solid: data.statics.solid.clone(),
            backdrop: data.statics.backdrop.clone(),
            water: data.statics.water.clone(),
            sky: data.statics.sky.clone(),
            props,
            tuft,
            tufts,
            grass: data.grass,
            camera: Vec3::ZERO,
        }
    }

    /// Per frame: where the hero is (the grass bends away from it), the time (the wind) and the camera
    /// (which grass is worth drawing).
    pub fn update(&mut self, queue: &wgpu::Queue, hero: Vec3, time: f32, eye: Vec3, look: &TpsLook) {
        self.camera = eye;
        let (horizon, zenith) = look.sky();
        let mut values = [0.0_f32; 16];
        values[..8].copy_from_slice(&[hero.x, hero.y, hero.z, BEND_RADIUS, time, WIND, FADE_START, FADE_END]);
        values[8..12].copy_from_slice(&horizon);
        values[12..].copy_from_slice(&zenith);
        queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&values));
    }

    pub fn render_shadow(&self, encoder: &mut wgpu::CommandEncoder, uniform_group: &wgpu::BindGroup, dynamic: &wgpu::Buffer, dynamic_count: u32) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Greybox third-person shadow pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(depth_attachment(&self.shadow_view)),
            ..Default::default()
        });
        pass.set_bind_group(0, uniform_group, &[]);
        pass.set_pipeline(&self.shadow_static);
        pass.set_vertex_buffer(0, self.statics.slice(..));
        pass.draw(self.solid.clone(), 0..1);
        pass.set_vertex_buffer(0, dynamic.slice(..));
        pass.draw(0..dynamic_count, 0..1);
        pass.set_pipeline(&self.shadow_prop);
        for batch in &self.props {
            pass.set_vertex_buffer(0, batch.vertices.slice(..));
            pass.set_vertex_buffer(1, batch.instances.slice(..));
            pass.draw(0..batch.vertex_count, 0..batch.instance_count);
        }
    }

    pub fn render_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        targets: &Targets<'_>,
        uniform_group: &wgpu::BindGroup,
        dynamic: &wgpu::Buffer,
        dynamic_count: u32,
    ) {
        let attachments = [Some(color_attachment(targets.color, targets.background)), Some(color_attachment(targets.emission, wgpu::Color::BLACK))];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Greybox third-person scene"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(depth_attachment(targets.depth)),
            ..Default::default()
        });
        pass.set_bind_group(0, uniform_group, &[]);
        pass.set_bind_group(1, &self.scene_group, &[]);
        pass.set_bind_group(2, &self.params_group, &[]);
        // The sky first: everything else is drawn over it.
        pass.set_pipeline(&self.scene_sky);
        pass.set_vertex_buffer(0, self.statics.slice(..));
        pass.draw(self.sky.clone(), 0..1);
        pass.set_pipeline(&self.scene_static);
        pass.set_vertex_buffer(0, self.statics.slice(..));
        pass.draw(self.solid.start..self.backdrop.end, 0..1);
        pass.set_vertex_buffer(0, dynamic.slice(..));
        pass.draw(0..dynamic_count, 0..1);
        pass.set_pipeline(&self.scene_water);
        pass.set_vertex_buffer(0, self.statics.slice(..));
        pass.draw(self.water.clone(), 0..1);
        pass.set_pipeline(&self.scene_prop);
        for batch in &self.props {
            pass.set_vertex_buffer(0, batch.vertices.slice(..));
            pass.set_vertex_buffer(1, batch.instances.slice(..));
            pass.draw(0..batch.vertex_count, 0..batch.instance_count);
        }
        pass.set_pipeline(&self.scene_grass);
        pass.set_vertex_buffer(0, self.tuft.slice(..));
        for run in self.grass.visible(self.camera, FADE_END) {
            pass.set_vertex_buffer(1, self.tufts.slice(u64::from(run.start) * TUFT_STRIDE..u64::from(run.end) * TUFT_STRIDE));
            pass.draw(0..TUFT_VERTICES, 0..run.end - run.start);
        }
    }
}
