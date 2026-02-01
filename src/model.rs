/// Model parsing and geometry utilities.

use crate::vertex::Vertex;

/// Parse a text-based model format into vertices and indices.
///
/// Format:
/// - `v x y z nx ny nz r g b` or `vertex ...` - Define a vertex with position, normal, color
/// - `f i0 i1 i2` or `face/tri/triangle ...` - Define a triangle face with vertex indices
/// - Lines starting with `#` are comments
pub fn parse_model(text: &str) -> Result<(Vec<Vertex>, Vec<u16>), String> {
    let mut raw_vertices: Vec<([f32; 3], [f32; 3], [f32; 3])> = Vec::new();
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

/// Extract triangle positions from vertices and indices for picking.
pub fn extract_triangles(vertices: &[Vertex], indices: &[u16]) -> Vec<[[f32; 3]; 3]> {
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
