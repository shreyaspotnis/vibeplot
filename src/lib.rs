use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

#[wasm_bindgen(start)]
pub async fn run() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let canvas = document
        .get_element_by_id("canvas")
        .expect("No canvas element")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Not a canvas");
    let stats_element = document
        .get_element_by_id("stats")
        .expect("No stats element")
        .dyn_into::<web_sys::HtmlElement>()
        .expect("Not an HtmlElement");

    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });

    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .expect("Failed to create surface");

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to get adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .expect("Failed to get device");

    let device = Rc::new(device);
    let queue = Rc::new(queue);

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Create depth texture
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    // Uniforms: 4x4 MVP matrix + 4x4 model matrix + vec4 light_dir + vec4 camera_pos
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform Buffer"),
        size: (16 + 16 + 4 + 4) * 4, // 160 bytes
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

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

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
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
                format: config.format,
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
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    // Create cube geometry
    let (vertices, indices) = create_cube();

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Index Buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    let num_indices = indices.len() as u32;

    // Shared state for interaction
    let state = Rc::new(RefCell::new(InteractionState {
        is_dragging: false,
        rotation_x: -0.5,
        rotation_y: 0.7,
        scale: 1.0,
        drag_start_x: 0.0,
        drag_start_y: 0.0,
        initial_rotation_x: 0.0,
        initial_rotation_y: 0.0,
        time: 0.0,
    }));

    // Set up event handlers
    setup_mouse_handlers(&canvas, state.clone());
    setup_wheel_handler(&canvas, state.clone());
    setup_keyboard_handler(&window, &stats_element);

    // Render loop
    let surface = Rc::new(RefCell::new(surface));
    let depth_view = Rc::new(depth_view);
    let render_pipeline = Rc::new(render_pipeline);
    let vertex_buffer = Rc::new(vertex_buffer);
    let index_buffer = Rc::new(index_buffer);
    let bind_group = Rc::new(bind_group);
    let uniform_buffer = Rc::new(uniform_buffer);

    let aspect = width as f32 / height as f32;

    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let window_clone = window.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        let mut state = state.borrow_mut();
        state.time += 0.016; // ~60fps

        // Update stats display
        let rotation_x_deg = state.rotation_x.to_degrees();
        let rotation_y_deg = state.rotation_y.to_degrees();
        let stats_text = format!(
            "Rotation X: {:.1}°\nRotation Y: {:.1}°\nZoom: {:.2}x\nCamera: (0, 0, 3)",
            rotation_x_deg,
            rotation_y_deg,
            state.scale
        );
        stats_element.set_inner_text(&stats_text);

        // Create matrices
        // Note: matrices are row-major in Rust but WGSL expects column-major.
        // The row-by-row serialization effectively transposes, so we reverse multiplication order.
        let model = mat4_mul(mat4_mul(mat4_scale(state.scale), mat4_rotate_x(state.rotation_x)), mat4_rotate_y(state.rotation_y));
        let view = mat4_look_at([0.0, 0.0, 3.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let proj = mat4_perspective(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        let mvp = mat4_mul(mat4_mul(model, view), proj);

        // Light direction (world space, pointing from light to origin)
        let light_dir = normalize([1.0, 1.0, 1.0]);
        let camera_pos = [0.0f32, 0.0, 3.0];

        // Write uniforms
        let mut uniform_data = Vec::with_capacity(40);
        uniform_data.extend_from_slice(&mat4_to_array(mvp));
        uniform_data.extend_from_slice(&mat4_to_array(model));
        uniform_data.extend_from_slice(&[light_dir[0], light_dir[1], light_dir[2], 0.0]);
        uniform_data.extend_from_slice(&[camera_pos[0], camera_pos[1], camera_pos[2], 0.0]);

        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));

        // Render
        let surface = surface.borrow();
        let output = surface.get_current_texture().expect("Failed to get texture");
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&render_pipeline);
            render_pass.set_bind_group(0, Some(&*bind_group), &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..num_indices, 0, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        drop(state);

        window_clone
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .expect("Failed to request animation frame");
    }));

    window
        .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .expect("Failed to request animation frame");

    log::info!("Interactive cube started!");
}

fn create_cube() -> (Vec<Vertex>, Vec<u16>) {
    // Each face has 4 vertices with the same normal but different positions
    // Colors: Front=Red, Back=Green, Top=Blue, Bottom=Yellow, Right=Cyan, Left=Magenta
    let vertices = vec![
        // Front face (z = 0.5) - Red
        Vertex { position: [-0.5, -0.5,  0.5], normal: [0.0, 0.0, 1.0], color: [1.0, 0.2, 0.2] },
        Vertex { position: [ 0.5, -0.5,  0.5], normal: [0.0, 0.0, 1.0], color: [1.0, 0.2, 0.2] },
        Vertex { position: [ 0.5,  0.5,  0.5], normal: [0.0, 0.0, 1.0], color: [1.0, 0.2, 0.2] },
        Vertex { position: [-0.5,  0.5,  0.5], normal: [0.0, 0.0, 1.0], color: [1.0, 0.2, 0.2] },

        // Back face (z = -0.5) - Green
        Vertex { position: [ 0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], color: [0.2, 1.0, 0.2] },
        Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], color: [0.2, 1.0, 0.2] },
        Vertex { position: [-0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0], color: [0.2, 1.0, 0.2] },
        Vertex { position: [ 0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0], color: [0.2, 1.0, 0.2] },

        // Top face (y = 0.5) - Blue
        Vertex { position: [-0.5,  0.5,  0.5], normal: [0.0, 1.0, 0.0], color: [0.2, 0.2, 1.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], normal: [0.0, 1.0, 0.0], color: [0.2, 0.2, 1.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], normal: [0.0, 1.0, 0.0], color: [0.2, 0.2, 1.0] },
        Vertex { position: [-0.5,  0.5, -0.5], normal: [0.0, 1.0, 0.0], color: [0.2, 0.2, 1.0] },

        // Bottom face (y = -0.5) - Yellow
        Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0], color: [1.0, 1.0, 0.2] },
        Vertex { position: [ 0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0], color: [1.0, 1.0, 0.2] },
        Vertex { position: [ 0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0], color: [1.0, 1.0, 0.2] },
        Vertex { position: [-0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0], color: [1.0, 1.0, 0.2] },

        // Right face (x = 0.5) - Cyan
        Vertex { position: [ 0.5, -0.5,  0.5], normal: [1.0, 0.0, 0.0], color: [0.2, 1.0, 1.0] },
        Vertex { position: [ 0.5, -0.5, -0.5], normal: [1.0, 0.0, 0.0], color: [0.2, 1.0, 1.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], normal: [1.0, 0.0, 0.0], color: [0.2, 1.0, 1.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], normal: [1.0, 0.0, 0.0], color: [0.2, 1.0, 1.0] },

        // Left face (x = -0.5) - Magenta
        Vertex { position: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0], color: [1.0, 0.2, 1.0] },
        Vertex { position: [-0.5, -0.5,  0.5], normal: [-1.0, 0.0, 0.0], color: [1.0, 0.2, 1.0] },
        Vertex { position: [-0.5,  0.5,  0.5], normal: [-1.0, 0.0, 0.0], color: [1.0, 0.2, 1.0] },
        Vertex { position: [-0.5,  0.5, -0.5], normal: [-1.0, 0.0, 0.0], color: [1.0, 0.2, 1.0] },
    ];

    let indices: Vec<u16> = vec![
        0,  1,  2,  0,  2,  3,  // Front
        4,  5,  6,  4,  6,  7,  // Back
        8,  9,  10, 8,  10, 11, // Top
        12, 13, 14, 12, 14, 15, // Bottom
        16, 17, 18, 16, 18, 19, // Right
        20, 21, 22, 20, 22, 23, // Left
    ];

    (vertices, indices)
}

fn setup_mouse_handlers(canvas: &web_sys::HtmlCanvasElement, state: Rc<RefCell<InteractionState>>) {
    // Mouse down
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state.borrow_mut();
            state.is_dragging = true;
            state.drag_start_x = event.offset_x() as f32;
            state.drag_start_y = event.offset_y() as f32;
            state.initial_rotation_x = state.rotation_x;
            state.initial_rotation_y = state.rotation_y;
        });
        canvas
            .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse move
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state.borrow_mut();
            if state.is_dragging {
                let dx = (event.offset_x() as f32 - state.drag_start_x) * 0.01;
                let dy = (event.offset_y() as f32 - state.drag_start_y) * 0.01;
                state.rotation_y = state.initial_rotation_y + dx;
                state.rotation_x = state.initial_rotation_x + dy;
            }
        });
        canvas
            .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse up
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            state.borrow_mut().is_dragging = false;
        });
        canvas
            .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // Mouse leave
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            state.borrow_mut().is_dragging = false;
        });
        canvas
            .add_event_listener_with_callback("mouseleave", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
}

fn setup_wheel_handler(canvas: &web_sys::HtmlCanvasElement, state: Rc<RefCell<InteractionState>>) {
    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
        event.prevent_default();

        let mut state = state.borrow_mut();
        let delta = event.delta_y() as f32;
        let zoom_speed = 0.001;
        let zoom_factor = 1.0 - delta * zoom_speed;
        state.scale = (state.scale * zoom_factor).clamp(0.1, 5.0);
    });

    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);

    canvas
        .add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            closure.as_ref().unchecked_ref(),
            &options,
        )
        .unwrap();
    closure.forget();
}

fn setup_keyboard_handler(window: &web_sys::Window, stats_element: &web_sys::HtmlElement) {
    let stats_element = stats_element.clone();
    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
        if event.key() == "/" {
            event.prevent_default();
            let style = stats_element.style();
            let current_display = style.get_property_value("display").unwrap_or_default();
            if current_display == "none" {
                style.set_property("display", "block").unwrap();
            } else {
                style.set_property("display", "none").unwrap();
            }
        }
    });
    window
        .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();
}

struct InteractionState {
    is_dragging: bool,
    rotation_x: f32,
    rotation_y: f32,
    scale: f32,
    drag_start_x: f32,
    drag_start_y: f32,
    initial_rotation_x: f32,
    initial_rotation_y: f32,
    time: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// Matrix math utilities
type Mat4 = [[f32; 4]; 4];

fn mat4_identity() -> Mat4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_scale(s: f32) -> Mat4 {
    [
        [s, 0.0, 0.0, 0.0],
        [0.0, s, 0.0, 0.0],
        [0.0, 0.0, s, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_rotate_x(angle: f32) -> Mat4 {
    let c = angle.cos();
    let s = angle.sin();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, s, 0.0],
        [0.0, -s, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_rotate_y(angle: f32) -> Mat4 {
    let c = angle.cos();
    let s = angle.sin();
    [
        [c, 0.0, -s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [s, 0.0, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov / 2.0).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, (far + near) / (near - far), -1.0],
        [0.0, 0.0, (2.0 * far * near) / (near - far), 0.0],
    ]
}

fn mat4_look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Mat4 {
    let f = normalize([
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    ]);
    let s = normalize(cross(f, up));
    let u = cross(s, f);

    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}

fn mat4_to_array(m: Mat4) -> [f32; 16] {
    [
        m[0][0], m[0][1], m[0][2], m[0][3],
        m[1][0], m[1][1], m[1][2], m[1][3],
        m[2][0], m[2][1], m[2][2], m[2][3],
        m[3][0], m[3][1], m[3][2], m[3][3],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
