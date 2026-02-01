/// WebGPU rendering: pipeline creation, textures, and render loop.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

use crate::math::{mat4_look_at, mat4_mul, mat4_perspective, mat4_rotate_x, mat4_rotate_y, mat4_scale, mat4_to_array, normalize};
use crate::picking::{CAMERA_POSITION, FIELD_OF_VIEW_DEG};
use crate::state::{InteractionState, ModelResources};
use crate::vertex::{Vertex, WireframeVertex};

// Rendering constants
const NEAR_PLANE: f32 = 0.1;
const FAR_PLANE: f32 = 100.0;
const BACKGROUND_COLOR: wgpu::Color = wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 };
const LIGHT_DIRECTION: [f32; 3] = [1.0, 1.0, 1.0];
pub const MSAA_SAMPLE_COUNT: u32 = 4;

/// Create MSAA and depth textures for rendering.
pub fn create_textures(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    surface_format: wgpu::TextureFormat,
) -> (wgpu::TextureView, wgpu::TextureView) {
    let msaa_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MSAA Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: surface_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

    (msaa_view, depth_view)
}

/// Create the main render pipeline and wireframe pipeline.
pub fn create_pipelines(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Vertex::desc()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: MSAA_SAMPLE_COUNT,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    let wireframe_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Wireframe Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_wireframe"),
            buffers: &[WireframeVertex::desc()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_wireframe"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: MSAA_SAMPLE_COUNT,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    (render_pipeline, wireframe_pipeline)
}

/// Create bind group layout and bind group for uniforms.
pub fn create_bind_group(
    device: &wgpu::Device,
    uniform_buffer: &wgpu::Buffer,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    (bind_group_layout, bind_group)
}

/// Create the uniform buffer.
pub fn create_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform Buffer"),
        size: (16 + 16 + 4 + 4 + 4) * 4, // 176 bytes (MVP + model + light_dir + camera_pos + selected_face)
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create the wireframe vertex buffer.
pub fn create_wireframe_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Wireframe Buffer"),
        size: (std::mem::size_of::<WireframeVertex>() * 6) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create vertex and index buffers for a model.
pub fn create_model_buffers(
    device: &wgpu::Device,
    vertices: &[Vertex],
    indices: &[u16],
) -> (wgpu::Buffer, wgpu::Buffer) {
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    (vertex_buffer, index_buffer)
}

/// Context holding all resources needed for the render loop.
pub struct RenderContext {
    pub device: Rc<wgpu::Device>,
    pub queue: Rc<wgpu::Queue>,
    pub surface: Rc<RefCell<wgpu::Surface<'static>>>,
    pub msaa_view: Rc<wgpu::TextureView>,
    pub depth_view: Rc<wgpu::TextureView>,
    pub render_pipeline: Rc<wgpu::RenderPipeline>,
    pub wireframe_pipeline: Rc<wgpu::RenderPipeline>,
    pub wireframe_buffer: Rc<wgpu::Buffer>,
    pub bind_group: Rc<wgpu::BindGroup>,
    pub uniform_buffer: Rc<wgpu::Buffer>,
    pub model_resources: Rc<RefCell<ModelResources>>,
    pub state: Rc<RefCell<InteractionState>>,
    pub aspect: f32,
}

/// Start the render loop.
pub fn start_render_loop(
    ctx: RenderContext,
    window: web_sys::Window,
    debug_panel: web_sys::HtmlElement,
) {
    let animation_callback: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let animation_callback_clone = animation_callback.clone();

    let window_clone = window.clone();
    *animation_callback_clone.borrow_mut() = Some(Closure::new(move || {
        render_frame(&ctx, &debug_panel);

        window_clone
            .request_animation_frame(animation_callback.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .expect("Failed to request animation frame");
    }));

    window
        .request_animation_frame(animation_callback_clone.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .expect("Failed to request animation frame");
}

/// Render a single frame.
fn render_frame(ctx: &RenderContext, debug_panel: &web_sys::HtmlElement) {
    let state = ctx.state.borrow();

    // Update debug panel
    let rotation_x_deg = state.rotation_x.to_degrees();
    let rotation_y_deg = state.rotation_y.to_degrees();
    let debug_text = format!(
        "Debug Panel\n\
         ───────────────────\n\
         Rotation X: {:.1}°\n\
         Rotation Y: {:.1}°\n\
         Zoom: {:.2}x\n\
         Camera: (0, 0, 3)\n\n\
         Controls\n\
         ───────────────────\n\
         /\tToggle debug panel\n\
         ⌘⇧P\tCommand palette\n\
         Drag\tRotate cube\n\
         Scroll\tZoom in/out",
        rotation_x_deg,
        rotation_y_deg,
        state.scale
    );
    debug_panel.set_inner_text(&debug_text);

    // Create matrices
    let model = mat4_mul(
        mat4_mul(mat4_scale(state.scale), mat4_rotate_x(state.rotation_x)),
        mat4_rotate_y(state.rotation_y)
    );
    let view = mat4_look_at(CAMERA_POSITION, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let proj = mat4_perspective(FIELD_OF_VIEW_DEG.to_radians(), ctx.aspect, NEAR_PLANE, FAR_PLANE);
    let mvp = mat4_mul(mat4_mul(model, view), proj);

    // Light direction (world space, pointing from light to origin)
    let light_dir = normalize(LIGHT_DIRECTION);

    // Write uniforms
    let mut uniform_data = Vec::with_capacity(44);
    uniform_data.extend_from_slice(&mat4_to_array(mvp));
    uniform_data.extend_from_slice(&mat4_to_array(model));
    uniform_data.extend_from_slice(&[light_dir[0], light_dir[1], light_dir[2], 0.0]);
    uniform_data.extend_from_slice(&[CAMERA_POSITION[0], CAMERA_POSITION[1], CAMERA_POSITION[2], 0.0]);
    uniform_data.extend_from_slice(&[state.selected_face as f32, 0.0, 0.0, 0.0]);

    ctx.queue.write_buffer(&ctx.uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));

    // Update wireframe buffer if a face is selected
    let draw_wireframe = if state.selected_face >= 0 {
        let face_idx = state.selected_face as usize;
        if face_idx < state.model_triangles.len() {
            let tri = &state.model_triangles[face_idx];
            let wireframe_vertices = [
                WireframeVertex { position: tri[0] },
                WireframeVertex { position: tri[1] },
                WireframeVertex { position: tri[1] },
                WireframeVertex { position: tri[2] },
                WireframeVertex { position: tri[2] },
                WireframeVertex { position: tri[0] },
            ];
            ctx.queue.write_buffer(&ctx.wireframe_buffer, 0, bytemuck::cast_slice(&wireframe_vertices));
            true
        } else {
            false
        }
    } else {
        false
    };

    // Render
    let surface = ctx.surface.borrow();
    let output = surface.get_current_texture().expect("Failed to get texture");
    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ctx.msaa_view,
                resolve_target: Some(&view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(BACKGROUND_COLOR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &ctx.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&ctx.render_pipeline);
        render_pass.set_bind_group(0, Some(&*ctx.bind_group), &[]);

        let model_res = ctx.model_resources.borrow();
        render_pass.set_vertex_buffer(0, model_res.vertex_buffer.slice(..));
        render_pass.set_index_buffer(model_res.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..model_res.num_indices, 0, 0..1);

        // Draw wireframe around selected face
        if draw_wireframe {
            render_pass.set_pipeline(&ctx.wireframe_pipeline);
            render_pass.set_bind_group(0, Some(&*ctx.bind_group), &[]);
            render_pass.set_vertex_buffer(0, ctx.wireframe_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }
    }

    ctx.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
