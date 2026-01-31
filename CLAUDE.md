# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build WASM package (requires wasm-pack)
wasm-pack build --target web --release

# Serve locally (then open http://localhost:8000)
python3 -m http.server 8000
```

Prerequisites: Rust with `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`), `wasm-pack`, and a WebGPU-capable browser (Chrome 113+, Edge 113+).

## Architecture

This is a Rust/WebAssembly project rendering an interactive models with WebGPU. The canvas fills the entire browser window.

### Key Files

- `src/lib.rs` - Main Rust code: WebGPU initialization, render pipeline, event handlers, animation loop, matrix math utilities, and exported command functions
- `src/shader.wgsl` - WGSL shaders with Phong/Blinn-Phong lighting (ambient + diffuse + specular)
- `index.html` - Web entry point with full-screen canvas, debug panel, command palette UI, and JavaScript for palette logic
- `pkg/` - Generated WASM output (built by wasm-pack)

### User Controls

| Control | Action |
|---------|--------|
| Drag | Rotate cube (X/Y Euler angles) |
| Scroll | Zoom in/out (scale 0.1–5.0) |
| `/` | Toggle debug panel |
| `Cmd+Shift+P` | Open command palette |

### Command Palette

VS Code-style command palette with fuzzy search. Commands are defined in `index.html` JavaScript and call exported Rust functions:
- **Reset Zoom** - calls `reset_zoom()`
- **Reset Rotation** - calls `reset_rotation()`

To add new commands:
1. Export a function from Rust with `#[wasm_bindgen]`
2. Access state via `INTERACTION_STATE` thread-local
3. Import the function in `index.html` and add to `commands` array

### State Management

`InteractionState` struct holds rotation angles, scale (zoom), and drag tracking. State is stored in a `thread_local!` (`INTERACTION_STATE`) to allow access from both the render loop and exported `#[wasm_bindgen]` functions.

### Rendering Pipeline

1. `run()` initializes WebGPU (instance → adapter → device → surface)
2. Creates render pipeline with depth testing and back-face culling (CCW front face)
3. Cube geometry: 24 vertices (6 faces × 4 verts), 36 indices, each vertex has position/normal/color
4. Animation loop via `requestAnimationFrame` updates MVP matrices and renders each frame
5. Debug panel text updated each frame with current state

### Matrix Convention

Matrices are row-major in Rust but WGSL expects column-major. The row-by-row serialization effectively transposes, so multiplication order is reversed: `model * view * proj` instead of `proj * view * model`.

### Uniform Buffer Layout (160 bytes)

- MVP matrix (64 bytes)
- Model matrix (64 bytes)
- Light direction (16 bytes)
- Camera position (16 bytes)
