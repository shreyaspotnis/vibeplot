/// Application state and global state management.

use std::cell::RefCell;
use std::rc::Rc;

// Interaction constants
pub const DEFAULT_ROTATION_X: f32 = -0.5;
pub const DEFAULT_ROTATION_Y: f32 = 0.7;
pub const DEFAULT_SCALE: f32 = 1.0;
pub const ZOOM_MIN: f32 = 0.1;
pub const ZOOM_MAX: f32 = 5.0;

/// Holds all interactive state for the 3D viewer.
pub struct InteractionState {
    // Drag state
    pub is_dragging: bool,
    pub drag_start_x: f32,
    pub drag_start_y: f32,
    pub initial_rotation_x: f32,
    pub initial_rotation_y: f32,

    // Transform state
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub scale: f32,

    // Touch/pinch state
    pub is_pinching: bool,
    pub initial_pinch_distance: f32,
    pub initial_scale: f32,

    // Face selection
    pub selected_face: i32,

    // Model geometry for picking (triangles as 3 vertices each)
    pub model_triangles: Vec<[[f32; 3]; 3]>,

    // Canvas dimensions
    pub canvas_width: u32,
    pub canvas_height: u32,
}

impl InteractionState {
    pub fn new(canvas_width: u32, canvas_height: u32) -> Self {
        Self {
            is_dragging: false,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            initial_rotation_x: 0.0,
            initial_rotation_y: 0.0,
            rotation_x: DEFAULT_ROTATION_X,
            rotation_y: DEFAULT_ROTATION_Y,
            scale: DEFAULT_SCALE,
            is_pinching: false,
            initial_pinch_distance: 0.0,
            initial_scale: DEFAULT_SCALE,
            selected_face: -1,
            model_triangles: Vec::new(),
            canvas_width,
            canvas_height,
        }
    }
}

/// GPU resources needed for dynamic model loading.
pub struct GpuResources {
    pub device: Rc<wgpu::Device>,
}

/// Buffers for the currently loaded model.
pub struct ModelResources {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

// Thread-local storage for global state access from wasm_bindgen exports
thread_local! {
    pub static INTERACTION_STATE: RefCell<Option<Rc<RefCell<InteractionState>>>> = RefCell::new(None);
    pub static GPU_RESOURCES: RefCell<Option<GpuResources>> = RefCell::new(None);
    pub static MODEL_RESOURCES: RefCell<Option<Rc<RefCell<ModelResources>>>> = RefCell::new(None);
}
