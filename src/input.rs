/// Event handlers for mouse, touch, wheel, and keyboard input.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::picking::pick_face;
use crate::state::{InteractionState, ZOOM_MAX, ZOOM_MIN};

// Input sensitivity constants
const MOUSE_SENSITIVITY: f32 = 0.01;
const ZOOM_SPEED: f32 = 0.001;

pub fn setup_mouse_handlers(canvas: &web_sys::HtmlCanvasElement, state: Rc<RefCell<InteractionState>>) {
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

pub fn setup_wheel_handler(canvas: &web_sys::HtmlCanvasElement, state: Rc<RefCell<InteractionState>>) {
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

pub fn setup_touch_handlers(canvas: &web_sys::HtmlCanvasElement, state: Rc<RefCell<InteractionState>>) {
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

pub fn setup_keyboard_handler(window: &web_sys::Window, debug_panel: &web_sys::HtmlElement, debug_hint: &web_sys::HtmlElement) {
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
