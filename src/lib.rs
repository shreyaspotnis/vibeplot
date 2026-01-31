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

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    // Create uniform buffer for triangle transform (offset + scale)
    // Layout: vec4(offset_x, offset_y, scale, padding)
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[0.0f32, 0.0f32, 1.0f32, 0.0f32]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
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
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    // Triangle vertices with colors
    let vertices: &[Vertex] = &[
        Vertex { position: [0.0, 0.5, 0.0], color: [1.0, 0.0, 0.0] },
        Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
        Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
    ];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Shared state for mouse interaction
    let state = Rc::new(RefCell::new(InteractionState {
        is_dragging: false,
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
        drag_start_x: 0.0,
        drag_start_y: 0.0,
        initial_offset_x: 0.0,
        initial_offset_y: 0.0,
    }));

    // Set up mouse event handlers
    setup_mouse_handlers(&canvas, state.clone(), width as f32, height as f32);
    setup_wheel_handler(&canvas, state.clone());

    // Render loop
    let surface = Rc::new(RefCell::new(surface));
    let render_pipeline = Rc::new(render_pipeline);
    let vertex_buffer = Rc::new(vertex_buffer);
    let bind_group = Rc::new(bind_group);
    let uniform_buffer = Rc::new(uniform_buffer);

    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let window_clone = window.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        let state = state.borrow();

        // Update uniform buffer with current offset and scale
        queue.write_buffer(
            &uniform_buffer,
            0,
            bytemuck::cast_slice(&[state.offset_x, state.offset_y, state.scale, 0.0f32]),
        );

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
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&render_pipeline);
            render_pass.set_bind_group(0, Some(&*bind_group), &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..3, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Request next frame
        window_clone
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .expect("Failed to request animation frame");
    }));

    // Start the render loop
    window
        .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .expect("Failed to request animation frame");

    log::info!("Interactive triangle started!");
}

fn setup_mouse_handlers(
    canvas: &web_sys::HtmlCanvasElement,
    state: Rc<RefCell<InteractionState>>,
    width: f32,
    height: f32,
) {
    // Mouse down
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state.borrow_mut();
            state.is_dragging = true;
            state.drag_start_x = event.offset_x() as f32;
            state.drag_start_y = event.offset_y() as f32;
            state.initial_offset_x = state.offset_x;
            state.initial_offset_y = state.offset_y;
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
                let dx = (event.offset_x() as f32 - state.drag_start_x) / width * 2.0;
                let dy = -(event.offset_y() as f32 - state.drag_start_y) / height * 2.0;
                state.offset_x = state.initial_offset_x + dx;
                state.offset_y = state.initial_offset_y + dy;
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

    // Mouse leave (stop dragging if mouse leaves canvas)
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

        // Zoom factor - smaller values = smoother zoom
        let zoom_speed = 0.001;
        let zoom_factor = 1.0 - delta * zoom_speed;

        // Apply zoom with limits (0.1x to 10x)
        state.scale = (state.scale * zoom_factor).clamp(0.1, 10.0);
    });

    // Use non-passive listener to allow preventDefault
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

struct InteractionState {
    is_dragging: bool,
    offset_x: f32,
    offset_y: f32,
    scale: f32,
    drag_start_x: f32,
    drag_start_y: f32,
    initial_offset_x: f32,
    initial_offset_y: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
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
            ],
        }
    }
}
