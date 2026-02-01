//! Vibeplot - Interactive 3D model viewer with WebGPU.
//!
//! ## Module Structure
//! - `math` - Matrix and vector operations
//! - `vertex` - Vertex types and GPU buffer layouts
//! - `state` - Application state and global state management
//! - `model` - Model parsing and geometry utilities
//! - `picking` - Ray-triangle intersection and face picking
//! - `input` - Event handlers for mouse, touch, wheel, keyboard
//! - `renderer` - WebGPU pipeline creation and render loop

mod input;
mod math;
mod model;
mod picking;
mod renderer;
mod state;
mod vertex;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

use model::{extract_triangles, parse_model};
use renderer::RenderContext;
use state::{
    GpuResources, InteractionState, ModelResources, DEFAULT_ROTATION_X, DEFAULT_ROTATION_Y,
    DEFAULT_SCALE, GPU_RESOURCES, INTERACTION_STATE, MODEL_RESOURCES,
};

// Built-in models (embedded at compile time)
const CUBE_MODEL: &str = include_str!("../models/cube.txt");
const PYRAMID_MODEL: &str = include_str!("../models/pyramid.txt");

// ============================================================================
// Exported wasm_bindgen functions (called from JavaScript)
// ============================================================================

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
pub fn get_rotation() -> js_sys::Float32Array {
    INTERACTION_STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            let s = state.borrow();
            let arr = js_sys::Float32Array::new_with_length(2);
            arr.set_index(0, s.rotation_x);
            arr.set_index(1, s.rotation_y);
            arr
        } else {
            js_sys::Float32Array::new_with_length(2)
        }
    })
}

#[wasm_bindgen]
pub fn set_rotation(x: f32, y: f32) {
    INTERACTION_STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            let mut s = state.borrow_mut();
            s.rotation_x = x;
            s.rotation_y = y;
        }
    });
}

#[wasm_bindgen]
pub fn get_zoom() -> f32 {
    INTERACTION_STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|s| s.borrow().scale)
            .unwrap_or(DEFAULT_SCALE)
    })
}

#[wasm_bindgen]
pub fn set_zoom(scale: f32) {
    INTERACTION_STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().scale = scale;
        }
    });
}

#[wasm_bindgen]
pub fn load_model(model_text: &str) -> Result<(), JsValue> {
    let (vertices, indices) = parse_model(model_text).map_err(|e| JsValue::from_str(&e))?;

    // Extract triangles for picking
    let model_triangles = extract_triangles(&vertices, &indices);

    GPU_RESOURCES.with(|gpu| {
        MODEL_RESOURCES.with(|model| {
            let gpu = gpu.borrow();
            let model = model.borrow();

            if let (Some(gpu), Some(model)) = (gpu.as_ref(), model.as_ref()) {
                let vertex_buffer =
                    gpu.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Vertex Buffer"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                let index_buffer =
                    gpu.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
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

// ============================================================================
// Application entry point
// ============================================================================

#[wasm_bindgen(start)]
pub async fn run() {
    // Initialize panic hook and logger
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    // Get DOM elements
    let (window, canvas, debug_panel, debug_hint) = get_dom_elements();

    // Set canvas size
    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    // Initialize WebGPU
    let (device, queue, surface, surface_format) = init_webgpu(&canvas).await;
    let device = Rc::new(device);
    let queue = Rc::new(queue);

    // Store GPU resources for access from exported functions
    GPU_RESOURCES.with(|gpu| {
        *gpu.borrow_mut() = Some(GpuResources {
            device: device.clone(),
        });
    });

    // Configure surface
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    // Create rendering resources
    let (msaa_view, depth_view) = renderer::create_textures(&device, width, height, surface_format);
    let uniform_buffer = renderer::create_uniform_buffer(&device);
    let (bind_group_layout, bind_group) = renderer::create_bind_group(&device, &uniform_buffer);
    let (render_pipeline, wireframe_pipeline) =
        renderer::create_pipelines(&device, surface_format, &bind_group_layout);
    let wireframe_buffer = renderer::create_wireframe_buffer(&device);

    // Load default model
    let (vertices, indices) = parse_model(CUBE_MODEL).expect("Failed to parse default model");
    let (vertex_buffer, index_buffer) = renderer::create_model_buffers(&device, &vertices, &indices);
    let model_triangles = extract_triangles(&vertices, &indices);

    let model_resources = Rc::new(RefCell::new(ModelResources {
        vertex_buffer,
        index_buffer,
        num_indices: indices.len() as u32,
    }));

    MODEL_RESOURCES.with(|m| {
        *m.borrow_mut() = Some(model_resources.clone());
    });

    // Create interaction state
    let mut state = InteractionState::new(width, height);
    state.model_triangles = model_triangles;
    let state = Rc::new(RefCell::new(state));

    INTERACTION_STATE.with(|s| {
        *s.borrow_mut() = Some(state.clone());
    });

    // Set up event handlers
    input::setup_mouse_handlers(&canvas, state.clone());
    input::setup_wheel_handler(&canvas, state.clone());
    input::setup_touch_handlers(&canvas, state.clone());
    input::setup_keyboard_handler(&window, &debug_panel, &debug_hint);

    // Create render context and start render loop
    let ctx = RenderContext {
        device,
        queue,
        surface: Rc::new(RefCell::new(surface)),
        msaa_view: Rc::new(msaa_view),
        depth_view: Rc::new(depth_view),
        render_pipeline: Rc::new(render_pipeline),
        wireframe_pipeline: Rc::new(wireframe_pipeline),
        wireframe_buffer: Rc::new(wireframe_buffer),
        bind_group: Rc::new(bind_group),
        uniform_buffer: Rc::new(uniform_buffer),
        model_resources,
        state,
        aspect: width as f32 / height as f32,
    };

    renderer::start_render_loop(ctx, window, debug_panel);

    log::info!("Interactive cube started!");
}

// ============================================================================
// Initialization helpers
// ============================================================================

fn get_dom_elements() -> (
    web_sys::Window,
    web_sys::HtmlCanvasElement,
    web_sys::HtmlElement,
    web_sys::HtmlElement,
) {
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

    (window, canvas, debug_panel, debug_hint)
}

async fn init_webgpu(
    canvas: &web_sys::HtmlCanvasElement,
) -> (wgpu::Device, wgpu::Queue, wgpu::Surface<'static>, wgpu::TextureFormat) {
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

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    (device, queue, surface, surface_format)
}
