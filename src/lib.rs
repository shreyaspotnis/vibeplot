use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

// Rendering constants
const FIELD_OF_VIEW_DEG: f32 = 45.0;
const NEAR_PLANE: f32 = 0.1;
const FAR_PLANE: f32 = 100.0;
const BACKGROUND_COLOR: wgpu::Color = wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 };
const CAMERA_POSITION: [f32; 3] = [0.0, 0.0, 3.0];
const LIGHT_DIRECTION: [f32; 3] = [1.0, 1.0, 1.0];

// Interaction constants
const DEFAULT_ROTATION_X: f32 = -0.5;
const DEFAULT_ROTATION_Y: f32 = 0.7;
const DEFAULT_SCALE: f32 = 1.0;
const MOUSE_SENSITIVITY: f32 = 0.01;
const ZOOM_SPEED: f32 = 0.001;
const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 5.0;

// Default model (embedded at compile time)
const DEFAULT_MODEL: &str = include_str!("../models/cube.txt");

struct ModelResources {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

struct GpuResources {
    device: Rc<wgpu::Device>,
}

thread_local! {
    static INTERACTION_STATE: RefCell<Option<Rc<RefCell<InteractionState>>>> = RefCell::new(None);
    static GPU_RESOURCES: RefCell<Option<GpuResources>> = RefCell::new(None);
    static MODEL_RESOURCES: RefCell<Option<Rc<RefCell<ModelResources>>>> = RefCell::new(None);
}

#[wasm_bindgen]
pub fn reset_zoom() {
    INTERACTION_STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().scale = DEFAULT_SCALE;
        }
    });
}

#[wasm_bindgen]
pub fn reset_rotation() {
    INTERACTION_STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            let mut s = state.borrow_mut();
            s.rotation_x = DEFAULT_ROTATION_X;
            s.rotation_y = DEFAULT_ROTATION_Y;
        }
    });
}

#[wasm_bindgen]
pub fn load_model(model_text: &str) -> Result<(), JsValue> {
    let (vertices, indices) = parse_model(model_text)
        .map_err(|e| JsValue::from_str(&e))?;

    GPU_RESOURCES.with(|gpu| {
        MODEL_RESOURCES.with(|model| {
            let gpu = gpu.borrow();
            let model = model.borrow();

            if let (Some(gpu), Some(model)) = (gpu.as_ref(), model.as_ref()) {
                let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let index_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                let mut model = model.borrow_mut();
                model.vertex_buffer = vertex_buffer;
                model.index_buffer = index_buffer;
                model.num_indices = indices.len() as u32;
            }
        });
    });

    Ok(())
}

fn parse_model(text: &str) -> Result<(Vec<Vertex>, Vec<u16>), String> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "v" | "vertex" => {
                if parts.len() < 10 {
                    return Err(format!("Invalid vertex line: {}", line));
                }
                let position = [
                    parts[1].parse::<f32>().map_err(|e| e.to_string())?,
                    parts[2].parse::<f32>().map_err(|e| e.to_string())?,
                    parts[3].parse::<f32>().map_err(|e| e.to_string())?,
                ];
                let normal = [
                    parts[4].parse::<f32>().map_err(|e| e.to_string())?,
                    parts[5].parse::<f32>().map_err(|e| e.to_string())?,
                    parts[6].parse::<f32>().map_err(|e| e.to_string())?,
                ];
                let color = [
                    parts[7].parse::<f32>().map_err(|e| e.to_string())?,
                    parts[8].parse::<f32>().map_err(|e| e.to_string())?,
                    parts[9].parse::<f32>().map_err(|e| e.to_string())?,
                ];
                vertices.push(Vertex { position, normal, color });
            }
            "f" | "face" | "tri" | "triangle" => {
                if parts.len() < 4 {
                    return Err(format!("Invalid face line: {}", line));
                }
                indices.push(parts[1].parse::<u16>().map_err(|e| e.to_string())?);
                indices.push(parts[2].parse::<u16>().map_err(|e| e.to_string())?);
                indices.push(parts[3].parse::<u16>().map_err(|e| e.to_string())?);
            }
            _ => {}
        }
    }

    if vertices.is_empty() {
        return Err("No vertices found in model".to_string());
    }
    if indices.is_empty() {
        return Err("No faces found in model".to_string());
    }

    Ok((vertices, indices))
}

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
    let debug_panel = document
        .get_element_by_id("debug-panel")
        .expect("No debug panel element")
        .dyn_into::<web_sys::HtmlElement>()
        .expect("Not an HtmlElement");
    let debug_hint = document
        .get_element_by_id("debug-hint")
        .expect("No debug hint element")
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

    // Store GPU resources for access from exported functions
    GPU_RESOURCES.with(|gpu| {
        *gpu.borrow_mut() = Some(GpuResources {
            device: device.clone(),
        });
    });

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

    // Load default model (embedded at compile time)
    let (vertices, indices) = parse_model(DEFAULT_MODEL).expect("Failed to parse default model");

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

    // Store model resources for dynamic loading
    let model_resources = Rc::new(RefCell::new(ModelResources {
        vertex_buffer,
        index_buffer,
        num_indices,
    }));

    MODEL_RESOURCES.with(|m| {
        *m.borrow_mut() = Some(model_resources.clone());
    });

    // Shared state for interaction
    let state = Rc::new(RefCell::new(InteractionState {
        is_dragging: false,
        rotation_x: DEFAULT_ROTATION_X,
        rotation_y: DEFAULT_ROTATION_Y,
        scale: DEFAULT_SCALE,
        drag_start_x: 0.0,
        drag_start_y: 0.0,
        initial_rotation_x: 0.0,
        initial_rotation_y: 0.0,
        is_pinching: false,
        initial_pinch_distance: 0.0,
        initial_scale: DEFAULT_SCALE,
    }));

    // Store state in thread_local for access from exported functions
    INTERACTION_STATE.with(|s| {
        *s.borrow_mut() = Some(state.clone());
    });

    // Set up event handlers
    setup_mouse_handlers(&canvas, state.clone());
    setup_wheel_handler(&canvas, state.clone());
    setup_touch_handlers(&canvas, state.clone());
    setup_keyboard_handler(&window, &debug_panel, &debug_hint);

    // Render loop
    let surface = Rc::new(RefCell::new(surface));
    let depth_view = Rc::new(depth_view);
    let render_pipeline = Rc::new(render_pipeline);
    let bind_group = Rc::new(bind_group);
    let uniform_buffer = Rc::new(uniform_buffer);

    let aspect = width as f32 / height as f32;

    let animation_callback: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let animation_callback_clone = animation_callback.clone();

    let window_clone = window.clone();
    *animation_callback_clone.borrow_mut() = Some(Closure::new(move || {
        let state = state.borrow();

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
        // Note: matrices are row-major in Rust but WGSL expects column-major.
        // The row-by-row serialization effectively transposes, so we reverse multiplication order.
        let model = mat4_mul(mat4_mul(mat4_scale(state.scale), mat4_rotate_x(state.rotation_x)), mat4_rotate_y(state.rotation_y));
        let view = mat4_look_at(CAMERA_POSITION, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let proj = mat4_perspective(FIELD_OF_VIEW_DEG.to_radians(), aspect, NEAR_PLANE, FAR_PLANE);
        let mvp = mat4_mul(mat4_mul(model, view), proj);

        // Light direction (world space, pointing from light to origin)
        let light_dir = normalize(LIGHT_DIRECTION);

        // Write uniforms
        let mut uniform_data = Vec::with_capacity(40);
        uniform_data.extend_from_slice(&mat4_to_array(mvp));
        uniform_data.extend_from_slice(&mat4_to_array(model));
        uniform_data.extend_from_slice(&[light_dir[0], light_dir[1], light_dir[2], 0.0]);
        uniform_data.extend_from_slice(&[CAMERA_POSITION[0], CAMERA_POSITION[1], CAMERA_POSITION[2], 0.0]);

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
                        load: wgpu::LoadOp::Clear(BACKGROUND_COLOR),
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

            let model = model_resources.borrow();
            render_pass.set_vertex_buffer(0, model.vertex_buffer.slice(..));
            render_pass.set_index_buffer(model.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..model.num_indices, 0, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        drop(state);

        window_clone
            .request_animation_frame(animation_callback.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .expect("Failed to request animation frame");
    }));

    window
        .request_animation_frame(animation_callback_clone.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .expect("Failed to request animation frame");

    log::info!("Interactive cube started!");
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
                let dx = (event.offset_x() as f32 - state.drag_start_x) * MOUSE_SENSITIVITY;
                let dy = (event.offset_y() as f32 - state.drag_start_y) * MOUSE_SENSITIVITY;
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
        let zoom_factor = 1.0 - delta * ZOOM_SPEED;
        state.scale = (state.scale * zoom_factor).clamp(ZOOM_MIN, ZOOM_MAX);
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

fn get_pinch_distance(event: &web_sys::TouchEvent) -> Option<f32> {
    let touches = event.touches();
    if touches.length() >= 2 {
        let t0 = touches.get(0)?;
        let t1 = touches.get(1)?;
        let dx = t1.client_x() as f32 - t0.client_x() as f32;
        let dy = t1.client_y() as f32 - t0.client_y() as f32;
        Some((dx * dx + dy * dy).sqrt())
    } else {
        None
    }
}

fn setup_touch_handlers(canvas: &web_sys::HtmlCanvasElement, state: Rc<RefCell<InteractionState>>) {
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);

    // Touch start
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
            event.prevent_default();
            let touches = event.touches();
            let mut state = state.borrow_mut();

            if touches.length() == 1 {
                // Single finger - start rotation
                if let Some(touch) = touches.get(0) {
                    state.is_dragging = true;
                    state.is_pinching = false;
                    state.drag_start_x = touch.client_x() as f32;
                    state.drag_start_y = touch.client_y() as f32;
                    state.initial_rotation_x = state.rotation_x;
                    state.initial_rotation_y = state.rotation_y;
                }
            } else if touches.length() >= 2 {
                // Two fingers - start pinch zoom
                state.is_dragging = false;
                state.is_pinching = true;
                if let Some(dist) = get_pinch_distance(&event) {
                    state.initial_pinch_distance = dist;
                    state.initial_scale = state.scale;
                }
            }
        });
        canvas
            .add_event_listener_with_callback_and_add_event_listener_options(
                "touchstart",
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .unwrap();
        closure.forget();
    }

    // Touch move
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
            event.prevent_default();
            let touches = event.touches();
            let mut state = state.borrow_mut();

            if state.is_pinching && touches.length() >= 2 {
                // Pinch zoom
                if let Some(dist) = get_pinch_distance(&event) {
                    if state.initial_pinch_distance > 0.0 {
                        let scale_factor = dist / state.initial_pinch_distance;
                        state.scale = (state.initial_scale * scale_factor).clamp(ZOOM_MIN, ZOOM_MAX);
                    }
                }
            } else if state.is_dragging && touches.length() == 1 {
                // Single finger rotation
                if let Some(touch) = touches.get(0) {
                    let dx = (touch.client_x() as f32 - state.drag_start_x) * MOUSE_SENSITIVITY;
                    let dy = (touch.client_y() as f32 - state.drag_start_y) * MOUSE_SENSITIVITY;
                    state.rotation_y = state.initial_rotation_y + dx;
                    state.rotation_x = state.initial_rotation_x + dy;
                }
            }
        });
        canvas
            .add_event_listener_with_callback_and_add_event_listener_options(
                "touchmove",
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .unwrap();
        closure.forget();
    }

    // Touch end
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
            event.prevent_default();
            let touches = event.touches();
            let mut state = state.borrow_mut();

            if touches.length() == 0 {
                // All fingers lifted
                state.is_dragging = false;
                state.is_pinching = false;
            } else if touches.length() == 1 && state.is_pinching {
                // Went from pinch to single finger - start rotation from current position
                if let Some(touch) = touches.get(0) {
                    state.is_pinching = false;
                    state.is_dragging = true;
                    state.drag_start_x = touch.client_x() as f32;
                    state.drag_start_y = touch.client_y() as f32;
                    state.initial_rotation_x = state.rotation_x;
                    state.initial_rotation_y = state.rotation_y;
                }
            }
        });
        canvas
            .add_event_listener_with_callback_and_add_event_listener_options(
                "touchend",
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .unwrap();
        closure.forget();
    }

    // Touch cancel
    {
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
            event.prevent_default();
            let mut state = state.borrow_mut();
            state.is_dragging = false;
            state.is_pinching = false;
        });
        canvas
            .add_event_listener_with_callback_and_add_event_listener_options(
                "touchcancel",
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .unwrap();
        closure.forget();
    }
}

fn setup_keyboard_handler(window: &web_sys::Window, debug_panel: &web_sys::HtmlElement, debug_hint: &web_sys::HtmlElement) {
    let debug_panel = debug_panel.clone();
    let debug_hint = debug_hint.clone();
    let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
        if event.key() == "/" {
            event.prevent_default();
            let panel_style = debug_panel.style();
            let hint_style = debug_hint.style();
            let panel_display = panel_style.get_property_value("display").unwrap_or_default();
            if panel_display == "none" {
                // Show full panel, hide hint
                panel_style.set_property("display", "block").unwrap();
                hint_style.set_property("display", "none").unwrap();
            } else {
                // Hide full panel, show hint
                panel_style.set_property("display", "none").unwrap();
                hint_style.set_property("display", "block").unwrap();
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
    // Touch/pinch state
    is_pinching: bool,
    initial_pinch_distance: f32,
    initial_scale: f32,
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
