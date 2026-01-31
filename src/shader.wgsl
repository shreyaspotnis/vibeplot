struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec4<f32>,
    camera_pos: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) color: vec3<f32>,
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

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light_dir = normalize(uniforms.light_dir.xyz);
    let view_dir = normalize(uniforms.camera_pos.xyz - in.world_position);

    // Ambient
    let ambient_strength = 0.15;
    let ambient = ambient_strength * in.color;

    // Diffuse (Lambertian)
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = diff * in.color;

    // Specular (Blinn-Phong)
    let halfway_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, halfway_dir), 0.0), 32.0);
    let specular_strength = 0.5;
    let specular = specular_strength * spec * vec3<f32>(1.0, 1.0, 1.0);

    let result = ambient + diffuse + specular;

    return vec4<f32>(result, 1.0);
}
