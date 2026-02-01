/// Ray-triangle intersection and face picking.

use crate::math::{cross, dot, mat4_mul, mat4_rotate_x, mat4_rotate_y, mat4_scale, normalize, sub, transform_point};
use crate::state::InteractionState;

// Camera constants (must match renderer)
pub const FIELD_OF_VIEW_DEG: f32 = 45.0;
pub const CAMERA_POSITION: [f32; 3] = [0.0, 0.0, 3.0];

/// Möller–Trumbore ray-triangle intersection algorithm.
/// Returns the distance along the ray if intersection occurs.
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

    if !(0.0..=1.0).contains(&u) {
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

/// Convert screen coordinates to a ray in view space.
fn screen_to_ray(
    x: f32,
    y: f32,
    width: u32,
    height: u32,
) -> ([f32; 3], [f32; 3]) {
    let aspect = width as f32 / height as f32;
    let fov = FIELD_OF_VIEW_DEG.to_radians();
    let tan_fov = (fov / 2.0).tan();

    // Convert screen coords to normalized device coords (-1 to 1)
    let ndc_x = (2.0 * x / width as f32) - 1.0;
    let ndc_y = 1.0 - (2.0 * y / height as f32); // Flip Y

    // Convert to view space ray direction
    let ray_dir = normalize([
        ndc_x * aspect * tan_fov,
        ndc_y * tan_fov,
        -1.0,
    ]);

    (CAMERA_POSITION, ray_dir)
}

/// Pick a face given screen coordinates. Returns -1 if no face hit.
pub fn pick_face(x: f32, y: f32, state: &InteractionState) -> i32 {
    let (ray_origin, ray_view_dir) = screen_to_ray(
        x, y,
        state.canvas_width,
        state.canvas_height,
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
