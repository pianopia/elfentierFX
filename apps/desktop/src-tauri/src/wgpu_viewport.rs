//! Native wgpu offscreen smoke preview (Vulkan/Metal/DX12 backends).

use elfentier_core::viewport::{raymarch_smoke_cpu, SmokePreviewImage, ViewportSmoke};
use pollster::block_on;
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct Params {
    bounds_min: vec4f,
    bounds_max: vec4f,
    dims: vec4f,
    max_density: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var density_tex: texture_3d<f32>;
@group(0) @binding(1) var density_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var positions = array(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0),
    );
    var out: VsOut;
    out.pos = vec4f(positions[vid], 0.0, 1.0);
    out.uv = positions[vid] * vec2f(0.5, -0.5) + vec2f(0.5, 0.5);
    return out;
}

fn sample_density(p: vec3f) -> f32 {
    let dims = params.dims.xyz;
    let cs = (params.bounds_max.xyz - params.bounds_min.xyz) / dims;
    let g = (p - params.bounds_min.xyz) / cs - vec3f(0.5);
    if (any(g < vec3f(0.0)) || any(g > dims - vec3f(1.001))) {
        return 0.0;
    }
    let uvw = (g + vec3f(0.5)) / dims;
    return textureSampleLevel(density_tex, density_sampler, uvw, 0.0).r;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    let center = (params.bounds_min.xyz + params.bounds_max.xyz) * 0.5;
    let extent = max(
        max(params.bounds_max.x - params.bounds_min.x, params.bounds_max.y - params.bounds_min.y),
        params.bounds_max.z - params.bounds_min.z
    );
    let ro = vec3f(
        center.x + (in.uv.x - 0.5) * extent * 1.2,
        center.y + (in.uv.y - 0.5) * extent * 0.8,
        center.z - extent * 1.4
    );
    let rd = vec3f(0.0, 0.0, 1.0);
    var accum = 0.0;
    let steps = 48.0;
    for (var i = 0.0; i < steps; i = i + 1.0) {
        let t = (i + 0.5) / steps;
        let p = ro + rd * t * extent * 2.2;
        accum = accum + sample_density(p) * (extent * 2.2 / steps);
    }
    let alpha = clamp(1.0 - exp(-accum * 2.5), 0.0, 1.0);
    return vec4f(0.86, 0.82, 0.90, 1.0) * alpha + vec4f(0.04, 0.05, 0.07, 1.0) * (1.0 - alpha);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct UniformParams {
    bounds_min: [f32; 4],
    bounds_max: [f32; 4],
    dims: [f32; 4],
    max_density: f32,
    _pad: [f32; 3],
}

/// Renders a smoke volume preview with native wgpu, falling back to CPU raymarch.
pub fn render_smoke_preview(
    smoke: &ViewportSmoke,
    width: u32,
    height: u32,
) -> Result<SmokePreviewImage, String> {
    match render_smoke_wgpu(smoke, width, height) {
        Ok(image) => Ok(image),
        Err(err) => {
            eprintln!("wgpu smoke preview fallback ({err})");
            Ok(raymarch_smoke_cpu(smoke, width, height))
        }
    }
}

fn render_smoke_wgpu(
    smoke: &ViewportSmoke,
    width: u32,
    height: u32,
) -> Result<SmokePreviewImage, String> {
    block_on(render_smoke_wgpu_async(smoke, width, height))
}

async fn render_smoke_wgpu_async(
    smoke: &ViewportSmoke,
    width: u32,
    height: u32,
) -> Result<SmokePreviewImage, String> {
    let w = width.max(16);
    let h = height.max(16);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or("no wgpu adapter")?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("elfentier smoke preview"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|e| e.to_string())?;

    let density_data: Vec<f32> = smoke
        .density
        .iter()
        .map(|&d| d / smoke.max_density.max(1e-6))
        .collect();

    let volume_texture = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: Some("smoke density"),
            size: wgpu::Extent3d {
                width: smoke.nx,
                height: smoke.ny,
                depth_or_array_layers: smoke.nz,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        bytemuck::cast_slice(&density_data),
    );

    let volume_view = volume_texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("smoke sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let uniforms = UniformParams {
        bounds_min: [
            smoke.bounds_min[0],
            smoke.bounds_min[1],
            smoke.bounds_min[2],
            0.0,
        ],
        bounds_max: [
            smoke.bounds_max[0],
            smoke.bounds_max[1],
            smoke.bounds_max[2],
            0.0,
        ],
        dims: [smoke.nx as f32, smoke.ny as f32, smoke.nz as f32, 0.0],
        max_density: smoke.max_density,
        _pad: [0.0; 3],
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("smoke params"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smoke bind layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("smoke bind group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&volume_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
        ],
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("smoke raymarch"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("smoke pipeline layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let color_target = wgpu::ColorTargetState {
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    };

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smoke raymarch pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(color_target)],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke preview target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let output_view = output_texture.create_view(&Default::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("smoke preview encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("smoke preview pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.04,
                        g: 0.05,
                        b: 0.07,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    let bytes_per_row = wgpu::util::align_to(w * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer_size = bytes_per_row as u64 * h as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);

    let mapped = slice.get_mapped_range();
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for row in 0..h as usize {
        let src_start = row * bytes_per_row as usize;
        let dst_start = row * w as usize * 4;
        rgba[dst_start..dst_start + w as usize * 4]
            .copy_from_slice(&mapped[src_start..src_start + w as usize * 4]);
    }
    drop(mapped);
    readback.unmap();

    Ok(SmokePreviewImage {
        width: w,
        height: h,
        rgba,
    })
}
