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
const MSAA_SAMPLE_COUNT: u32 = 4;

// Built-in models (embedded at compile time)
const CUBE_MODEL: &str = include_str!("../models/cube.txt");
const PYRAMID_MODEL: &str = include_str!("../models/pyramid.txt");

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

    // Extract triangles for picking
    let model_triangles = extract_triangles(&vertices, &indices);

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

    // Update model triangles for picking and reset selection
    INTERACTION_STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            let mut state = state.borrow_mut();
            state.model_triangles = model_triangles;
            state.selected_face = -1;
        }
    });

    Ok(())
}

#[wasm_bindgen]
pub fn load_cube_model() -> Result<(), JsValue> {
    load_model(CUBE_MODEL)
}

#[wasm_bindgen]
pub fn load_pyramid_model() -> Result<(), JsValue> {
    load_model(PYRAMID_MODEL)
}

fn parse_model(text: &str) -> Result<(Vec<Vertex>, Vec<u16>), String> {
    let mut raw_vertices: Vec<([ f32; 3], [f32; 3], [f32; 3])> = Vec::new();
    let mut raw_faces: Vec<[u16; 3]> = Vec::new();

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
                raw_vertices.push((position, normal, color));
            }
            "f" | "face" | "tri" | "triangle" => {
                if parts.len() < 4 {
                    return Err(format!("Invalid face line: {}", line));
                }
                raw_faces.push([
                    parts[1].parse::<u16>().map_err(|e| e.to_string())?,
                    parts[2].parse::<u16>().map_err(|e| e.to_string())?,
                    parts[3].parse::<u16>().map_err(|e| e.to_string())?,
                ]);
            }
            _ => {}
        }
    }

    if raw_vertices.is_empty() {
        return Err("No vertices found in model".to_string());
    }
    if raw_faces.is_empty() {
        return Err("No faces found in model".to_string());
    }

    // Expand vertices so each face has unique vertices with face_id
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for (face_id, face) in raw_faces.iter().enumerate() {
        let base_idx = vertices.len() as u16;
        for &idx in face.iter() {
            let (position, normal, color) = raw_vertices[idx as usize];
            vertices.push(Vertex {
                position,
                normal,
                color,
                face_id: face_id as u32,
            });
        }
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
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

    // Create MSAA texture for anti-aliasing
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

    // Create depth texture (with MSAA)
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

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    // Uniforms: 4x4 MVP matrix + 4x4 model matrix + vec4 light_dir + vec4 camera_pos
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Uniform Buffer"),
        size: (16 + 16 + 4 + 4 + 4) * 4, // 176 bytes (MVP + model + light_dir + camera_pos + selected_face)
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
            count: MSAA_SAMPLE_COUNT,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    // Create wireframe pipeline for selected face outline
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
                format: config.format,
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
            depth_compare: wgpu::CompareFunction::LessEqual, // Occlude wireframe behind other faces
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

    // Create wireframe vertex buffer (6 vertices for 3 edges)
    let wireframe_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Wireframe Buffer"),
        size: (std::mem::size_of::<WireframeVertex>() * 6) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Load default model (embedded at compile time)
    let (vertices, indices) = parse_model(CUBE_MODEL).expect("Failed to parse default model");

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
    // Extract triangles for picking
    let model_triangles = extract_triangles(&vertices, &indices);

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
        selected_face: -1,
        model_triangles,
        canvas_width: width,
        canvas_height: height,
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
    let msaa_view = Rc::new(msaa_view);
    let depth_view = Rc::new(depth_view);
    let render_pipeline = Rc::new(render_pipeline);
    let wireframe_pipeline = Rc::new(wireframe_pipeline);
    let wireframe_buffer = Rc::new(wireframe_buffer);
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
        let mut uniform_data = Vec::with_capacity(44);
        uniform_data.extend_from_slice(&mat4_to_array(mvp));
        uniform_data.extend_from_slice(&mat4_to_array(model));
        uniform_data.extend_from_slice(&[light_dir[0], light_dir[1], light_dir[2], 0.0]);
        uniform_data.extend_from_slice(&[CAMERA_POSITION[0], CAMERA_POSITION[1], CAMERA_POSITION[2], 0.0]);
        // Selected face as float (will be cast to int in shader), padded to vec4
        uniform_data.extend_from_slice(&[state.selected_face as f32, 0.0, 0.0, 0.0]);

        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));

        // Update wireframe buffer if a face is selected (must happen before render pass)
        let draw_wireframe = if state.selected_face >= 0 {
            let face_idx = state.selected_face as usize;
            if face_idx < state.model_triangles.len() {
                let tri = &state.model_triangles[face_idx];
                // Create 6 vertices for 3 edges (LineList topology)
                let wireframe_vertices = [
                    WireframeVertex { position: tri[0] },
                    WireframeVertex { position: tri[1] },
                    WireframeVertex { position: tri[1] },
                    WireframeVertex { position: tri[2] },
                    WireframeVertex { position: tri[2] },
                    WireframeVertex { position: tri[0] },
                ];
                queue.write_buffer(&wireframe_buffer, 0, bytemuck::cast_slice(&wireframe_vertices));
                true
            } else {
                false
            }
        } else {
            false
        };

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
                    view: &msaa_view,
                    resolve_target: Some(&view), // Resolve MSAA to surface texture
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

            // Draw wireframe around selected face
            if draw_wireframe {
                render_pass.set_pipeline(&wireframe_pipeline);
                render_pass.set_bind_group(0, Some(&*bind_group), &[]);
                render_pass.set_vertex_buffer(0, wireframe_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }
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

    // Mouse up - also handles click detection for face picking
    {
        let state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;

            let (is_click, face) = {
                let state = state.borrow();
                // Check if this was a click (minimal movement)
                let dx = x - state.drag_start_x;
                let dy = y - state.drag_start_y;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance < 5.0 {
                    // This is a click - pick face
                    (true, pick_face(x, y, &state))
                } else {
                    (false, -1)
                }
            };

            let mut state = state.borrow_mut();
            if is_click {
                state.selected_face = face;
            }
            state.is_dragging = false;
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

    // Touch end - also handles tap detection for face picking
    {
        let state = state.clone();
        let canvas_clone = canvas.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
            event.prevent_default();
            let touches = event.touches();
            let changed_touches = event.changed_touches();

            if touches.length() == 0 && changed_touches.length() == 1 {
                // Single finger lifted - check if it was a tap
                if let Some(touch) = changed_touches.get(0) {
                    // Get touch position relative to canvas
                    let rect = canvas_clone.get_bounding_client_rect();
                    let x = touch.client_x() as f32 - rect.left() as f32;
                    let y = touch.client_y() as f32 - rect.top() as f32;

                    let (is_tap, face) = {
                        let state = state.borrow();
                        // Check if this was a tap (minimal movement from start)
                        let dx = x - state.drag_start_x;
                        let dy = y - state.drag_start_y;
                        let distance = (dx * dx + dy * dy).sqrt();

                        if distance < 10.0 && !state.is_pinching {
                            (true, pick_face(x, y, &state))
                        } else {
                            (false, -1)
                        }
                    };

                    let mut state = state.borrow_mut();
                    if is_tap {
                        state.selected_face = face;
                    }
                    state.is_dragging = false;
                    state.is_pinching = false;
                    return;
                }
            }

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
    // Face selection
    selected_face: i32,
    // Model geometry for picking (stored as triangles: each 9 floats = 3 vertices * 3 coords)
    model_triangles: Vec<[[f32; 3]; 3]>,
    canvas_width: u32,
    canvas_height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    face_id: u32,
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
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 3) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct WireframeVertex {
    position: [f32; 3],
}

impl WireframeVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WireframeVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
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

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

// Extract triangle positions from vertices and indices for picking
fn extract_triangles(vertices: &[Vertex], indices: &[u16]) -> Vec<[[f32; 3]; 3]> {
    let mut triangles = Vec::new();
    for chunk in indices.chunks(3) {
        if chunk.len() == 3 {
            triangles.push([
                vertices[chunk[0] as usize].position,
                vertices[chunk[1] as usize].position,
                vertices[chunk[2] as usize].position,
            ]);
        }
    }
    triangles
}

// Möller–Trumbore ray-triangle intersection
fn ray_triangle_intersect(
    ray_origin: [f32; 3],
    ray_dir: [f32; 3],
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
) -> Option<f32> {
    const EPSILON: f32 = 0.0000001;

    let edge1 = sub(v1, v0);
    let edge2 = sub(v2, v0);
    let h = cross(ray_dir, edge2);
    let a = dot(edge1, h);

    if a > -EPSILON && a < EPSILON {
        return None; // Ray is parallel to triangle
    }

    let f = 1.0 / a;
    let s = sub(ray_origin, v0);
    let u = f * dot(s, h);

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let q = cross(s, edge1);
    let v = f * dot(ray_dir, q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * dot(edge2, q);

    if t > EPSILON {
        Some(t)
    } else {
        None
    }
}

// Convert screen coordinates to ray in world space
fn screen_to_ray(
    x: f32,
    y: f32,
    width: u32,
    height: u32,
    _rotation_x: f32,
    _rotation_y: f32,
    _scale: f32,
) -> ([f32; 3], [f32; 3]) {
    let aspect = width as f32 / height as f32;
    let fov = FIELD_OF_VIEW_DEG.to_radians();
    let tan_fov = (fov / 2.0).tan();

    // Convert screen coords to normalized device coords (-1 to 1)
    let ndc_x = (2.0 * x / width as f32) - 1.0;
    let ndc_y = 1.0 - (2.0 * y / height as f32); // Flip Y

    // Convert to view space ray direction
    let ray_view = normalize([
        ndc_x * aspect * tan_fov,
        ndc_y * tan_fov,
        -1.0,
    ]);

    // Camera is at CAMERA_POSITION looking at origin
    // We need to transform the ray to world space and account for model transform
    // Since we're picking in model space, we transform the ray by inverse model matrix

    let ray_origin = CAMERA_POSITION;

    // For simplicity, we'll transform triangles to world space during picking instead
    // The ray direction in world space (camera looks down -Z)
    let ray_dir = ray_view;

    (ray_origin, ray_dir)
}

// Pick a face given screen coordinates
fn pick_face(
    x: f32,
    y: f32,
    state: &InteractionState,
) -> i32 {
    let (ray_origin, ray_view_dir) = screen_to_ray(
        x, y,
        state.canvas_width,
        state.canvas_height,
        state.rotation_x,
        state.rotation_y,
        state.scale,
    );

    // Build model matrix to transform triangles
    let model_mat = mat4_mul(
        mat4_mul(mat4_scale(state.scale), mat4_rotate_x(state.rotation_x)),
        mat4_rotate_y(state.rotation_y)
    );

    let mut closest_face: i32 = -1;
    let mut closest_t = f32::MAX;

    for (face_id, tri) in state.model_triangles.iter().enumerate() {
        // Transform triangle vertices by model matrix
        let v0 = transform_point(tri[0], &model_mat);
        let v1 = transform_point(tri[1], &model_mat);
        let v2 = transform_point(tri[2], &model_mat);

        if let Some(t) = ray_triangle_intersect(ray_origin, ray_view_dir, v0, v1, v2) {
            if t < closest_t {
                closest_t = t;
                closest_face = face_id as i32;
            }
        }
    }

    closest_face
}

// Transform point using the same convention as the GPU (column-major interpretation)
fn transform_point(p: [f32; 3], m: &Mat4) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}
