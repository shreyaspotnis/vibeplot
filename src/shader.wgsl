// Lighting constants
const AMBIENT_STRENGTH: f32 = 0.15;
const SPECULAR_STRENGTH: f32 = 0.5;
const SPECULAR_SHININESS: f32 = 32.0;

// Selection highlight constants
const HIGHLIGHT_BRIGHTNESS: f32 = 1.3;
const HIGHLIGHT_BLUE_TINT: vec3<f32> = vec3<f32>(0.1, 0.1, 0.3);

struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec4<f32>,
    camera_pos: vec4<f32>,
    selected_face: vec4<f32>, // x component is face id, -1 means none selected
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) face_id: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) @interpolate(flat) face_id: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform position
    out.clip_position = uniforms.mvp * vec4<f32>(in.position, 1.0);

    // Transform normal to world space (using upper-left 3x3 of model matrix)
    let normal_matrix = mat3x3<f32>(
        uniforms.model[0].xyz,
        uniforms.model[1].xyz,
        uniforms.model[2].xyz
    );
    out.world_normal = normalize(normal_matrix * in.normal);

    // Transform position to world space
    out.world_position = (uniforms.model * vec4<f32>(in.position, 1.0)).xyz;

    out.color = in.color;
    out.face_id = in.face_id;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light_dir = normalize(uniforms.light_dir.xyz);
    let view_dir = normalize(uniforms.camera_pos.xyz - in.world_position);

    // Ambient
    let ambient = AMBIENT_STRENGTH * in.color;

    // Diffuse (Lambertian)
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = diff * in.color;

    // Specular (Blinn-Phong)
    let halfway_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, halfway_dir), 0.0), SPECULAR_SHININESS);
    let specular = SPECULAR_STRENGTH * spec * vec3<f32>(1.0, 1.0, 1.0);

    var result = ambient + diffuse + specular;

    // Apply highlight if this face is selected
    let selected = i32(uniforms.selected_face.x);
    if (selected >= 0 && u32(selected) == in.face_id) {
        result = result * HIGHLIGHT_BRIGHTNESS + HIGHLIGHT_BLUE_TINT;
    }

    return vec4<f32>(result, 1.0);
}

// Wireframe shader for selected face outline
struct WireframeVertexInput {
    @location(0) position: vec3<f32>,
}

struct WireframeVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_wireframe(in: WireframeVertexInput) -> WireframeVertexOutput {
    var out: WireframeVertexOutput;
    out.clip_position = uniforms.mvp * vec4<f32>(in.position, 1.0);
    return out;
}

@fragment
fn fs_wireframe(in: WireframeVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0); // White wireframe
}
