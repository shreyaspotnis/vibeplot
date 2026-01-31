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

This is a Rust/WebAssembly project rendering an interactive 3D cube with WebGPU.

### Key Files

- `src/lib.rs` - Main Rust code: WebGPU initialization, render pipeline, event handlers, animation loop, and matrix math utilities
- `src/shader.wgsl` - WGSL shaders with Phong/Blinn-Phong lighting (ambient + diffuse + specular)
- `index.html` - Web entry point that loads the WASM module and checks WebGPU support
- `pkg/` - Generated WASM output (built by wasm-pack)

### Rendering Pipeline

1. `run()` initializes WebGPU (instance → adapter → device → surface)
2. Creates render pipeline with depth testing and back-face culling
3. Cube geometry: 24 vertices (6 faces × 4 verts), 36 indices, each vertex has position/normal/color
4. Animation loop via `requestAnimationFrame` updates MVP matrices and renders each frame

### Interaction State

`InteractionState` struct holds rotation angles, scale (zoom), and drag tracking. Event handlers update this state:
- Mouse drag: rotates cube (X/Y Euler angles)
- Mouse wheel: zooms (scale clamped to 0.1–5.0)

### Resource Sharing Pattern

Uses `Rc<RefCell<T>>` for shared mutable state across async boundaries and event closures. GPU resources and interaction state are wrapped in `Rc` to persist across animation frames.

### Uniform Buffer Layout (160 bytes)

- MVP matrix (64 bytes)
- Model matrix (64 bytes)
- Light direction (16 bytes)
- Camera position (16 bytes)
