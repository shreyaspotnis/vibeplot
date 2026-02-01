# vibeplot

> **Disclaimer:** This is a 100% vibe-coded project, built entirely for fun with AI assistance. No tests, no guarantees, no warranties. Code quality ranges from "surprisingly decent" to "what was I thinking?" Use at your own risk. If it breaks, you get to keep both pieces.

A WebGPU-powered 3D model viewer that runs entirely in your browser. Built with Rust and WebAssembly.

## Features

- **WebGPU Rendering** - Hardware-accelerated 3D graphics with Phong/Blinn-Phong lighting
- **Interactive Controls** - Mouse drag to rotate, scroll to zoom
- **Touch Support** - Pinch to zoom, single finger to rotate on touchscreens
- **Python Client** - Send models from Python scripts via WebSocket
- **Load Custom Models** - From disk, URL, or generate with AI
- **Command Palette** - VS Code-style command palette (`Cmd+Shift+P` / `Ctrl+Shift+P`)
- **Debug Panel** - Real-time display of rotation and zoom values

## Demo

Visit the live demo: [https://shreyaspotnis.github.io/vibeplot/](https://shreyaspotnis.github.io/vibeplot/)

## Controls

| Control | Action |
|---------|--------|
| Drag | Rotate model |
| Scroll | Zoom in/out |
| Pinch (touch) | Zoom in/out |
| Single finger (touch) | Rotate model |
| `/` | Toggle debug panel |
| `Cmd+Shift+P` | Open command palette |

## Commands

Access via command palette (`Cmd+Shift+P` / `Ctrl+Shift+P`):

- **Generate Model** - Create models with AI assistance
- **Load Model from disk** - Load a `.txt` model file
- **Load Model from URL** - Load a model from a URL
- **Reset Zoom** - Reset zoom to default
- **Reset Rotation** - Reset rotation to default

## Model Format

Models are plain text files with vertices and faces:

```
# Comments start with #

# Vertex: position (x y z), normal (nx ny nz), color (r g b)
# Colors are 0.0 to 1.0
vertex x y z nx ny nz r g b

# Face: three vertex indices (0-indexed, counter-clockwise winding)
face i0 i1 i2
```

### Example: Triangle

```
vertex  0.0  0.5  0.0   0.0 0.0 1.0   1.0 0.0 0.0
vertex -0.5 -0.5  0.0   0.0 0.0 1.0   0.0 1.0 0.0
vertex  0.5 -0.5  0.0   0.0 0.0 1.0   0.0 0.0 1.0
face 0 1 2
```

See the `models/` directory for more examples.

## Building from Source

### Prerequisites

- Rust with `wasm32-unknown-unknown` target
- wasm-pack

```bash
# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack
```

### Build

```bash
# Build WASM package
wasm-pack build --target web --release

# Serve locally
python3 -m http.server 8000
```

Then open [http://localhost:8000](http://localhost:8000) in a WebGPU-capable browser (Chrome 113+, Edge 113+).

## Python Client

Send models to vibeplot from Python scripts. Useful for visualizing programmatically generated geometry.

### Installation

```bash
cd vibeplot
python3 -m venv .venv
source .venv/bin/activate
pip install python/
```

### Usage

```python
import vibeplot

vibeplot.start()  # Opens browser and waits for connection

vibeplot.load_model("""
vertex  0.0  0.5  0.0   0.0 0.0 1.0   1.0 0.0 0.0
vertex -0.5 -0.5  0.0   0.0 0.0 1.0   0.0 1.0 0.0
vertex  0.5 -0.5  0.0   0.0 0.0 1.0   0.0 0.0 1.0
face 0 1 2
""")

vibeplot.show()  # Keep running until Ctrl+C
```

### API

- `vibeplot.start()` - Start server and open browser (blocks until connected)
- `vibeplot.load_model(text)` - Send model to browser
- `vibeplot.reset_zoom()` - Reset zoom to default
- `vibeplot.reset_rotation()` - Reset rotation to default
- `vibeplot.show()` - Block until Ctrl+C (like matplotlib)

**Note:** The HTTP server (`python3 -m http.server 8000`) must be running for the browser to load vibeplot.

## Browser Support

Requires a browser with WebGPU support:
- Chrome 113+
- Edge 113+
- Firefox (behind flag)
- Safari (behind flag)

## License

MIT
