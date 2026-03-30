"""
vibeplot - Python client for vibeplot 3D model viewer.

Usage:
    import vibeplot
    vibeplot.start()  # Opens browser and waits for connection
    vibeplot.load_model(model_string)
    vibeplot.show()   # Keep running until Ctrl+C
"""

import asyncio
import json
import threading
import webbrowser
from typing import Optional

try:
    import websockets
    from websockets.server import serve
except ImportError:
    raise ImportError("websockets package required. Install with: pip install websockets")

__version__ = "0.1.0"
__all__ = ["start", "load_model", "load_volume", "load_voxels", "show", "reset_zoom", "reset_rotation", "VibePlotConnection"]

DEFAULT_PORT = 9753
DEFAULT_HOST = "0.0.0.0"


class VibePlotConnection:
    """Manages WebSocket connection to vibeplot browser."""

    def __init__(self, host: str = DEFAULT_HOST, port: int = DEFAULT_PORT):
        self.host = host
        self.port = port
        self._websocket = None
        self._server = None
        self._server_thread: Optional[threading.Thread] = None
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._connected = threading.Event()
        self._ready = threading.Event()
        self._started = threading.Event()

    async def _handle_client(self, websocket):
        """Handle incoming browser connection."""
        self._websocket = websocket
        self._connected.set()
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if data.get("type") == "ready":
                        self._ready.set()
                        print("vibeplot: Browser connected")
                    elif data.get("type") == "ack" and not data.get("success"):
                        print(f"vibeplot error: {data.get('error')}")
                except json.JSONDecodeError:
                    pass
        except websockets.exceptions.ConnectionClosed:
            pass
        finally:
            print("vibeplot: Browser disconnected")
            self._websocket = None
            self._connected.clear()
            self._ready.clear()

    async def _start_server(self):
        """Start WebSocket server."""
        self._server = await serve(
            self._handle_client,
            self.host,
            self.port,
        )
        self._started.set()
        print(f"vibeplot: Server started at ws://{self.host}:{self.port}")
        await self._server.wait_closed()

    def _run_loop(self):
        """Run asyncio event loop in background thread."""
        self._loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self._loop)
        self._loop.run_until_complete(self._start_server())

    def start(self, open_browser: bool = False, browser_url: str = "http://localhost:8000"):
        """Start the WebSocket server in background thread."""
        self._server_thread = threading.Thread(target=self._run_loop, daemon=True)
        self._server_thread.start()

        # Wait for server to start
        self._started.wait(timeout=5.0)

        if open_browser:
            webbrowser.open(browser_url)

    def wait_for_connection(self, timeout: Optional[float] = None) -> bool:
        """Block until browser connects and is ready. Returns True if ready, False on timeout."""
        return self._ready.wait(timeout=timeout)

    @property
    def is_connected(self) -> bool:
        """Check if browser is connected."""
        return self._websocket is not None

    def _send(self, message: dict):
        """Send message to browser."""
        if not self._websocket:
            raise RuntimeError("No browser connected. Open http://localhost:8000 in your browser.")

        if not self._loop:
            raise RuntimeError("Server not started.")

        future = asyncio.run_coroutine_threadsafe(
            self._websocket.send(json.dumps(message)),
            self._loop
        )
        future.result(timeout=5.0)

    def load_model(self, model_text: str):
        """Send model to browser for rendering."""
        self._send({
            "type": "load_model",
            "data": model_text
        })

    def reset_zoom(self):
        """Reset zoom to default."""
        self._send({"type": "reset_zoom"})

    def reset_rotation(self):
        """Reset rotation to default."""
        self._send({"type": "reset_rotation"})

    def close(self):
        """Shutdown the server."""
        if self._server:
            self._server.close()


# Module-level connection instance
_connection: Optional[VibePlotConnection] = None


def start(
    host: str = DEFAULT_HOST,
    port: int = DEFAULT_PORT,
    open_browser: bool = True,
    wait: bool = True,
    timeout: Optional[float] = None,
) -> VibePlotConnection:
    """
    Start vibeplot server and open browser.

    Args:
        host: Host to bind to (default: localhost)
        port: Port to bind to (default: 9753)
        open_browser: Open default browser to vibeplot page (default: True)
        wait: Wait for browser connection before returning
        timeout: Max seconds to wait for connection (None = forever)

    Returns:
        VibePlotConnection instance

    Example:
        import vibeplot
        vibeplot.start()
        vibeplot.load_model(model_string)
    """
    global _connection
    _connection = VibePlotConnection(host, port)
    _connection.start(open_browser=open_browser)

    if wait:
        import socket
        try:
            lan_ip = socket.gethostbyname(socket.gethostname())
        except Exception:
            lan_ip = "localhost"
        print(f"vibeplot: Open http://localhost:8000 (or http://{lan_ip}:8000 from another device)")
        connected = _connection.wait_for_connection(timeout=timeout)
        if not connected:
            raise TimeoutError("Timed out waiting for browser connection")

    return _connection


def load_model(model_text: str):
    """Send model to connected browser."""
    if not _connection:
        raise RuntimeError("Not started. Call vibeplot.start() first.")
    _connection.load_model(model_text)


def reset_zoom():
    """Reset zoom in connected browser."""
    if not _connection:
        raise RuntimeError("Not started. Call vibeplot.start() first.")
    _connection.reset_zoom()


def reset_rotation():
    """Reset rotation in connected browser."""
    if not _connection:
        raise RuntimeError("Not started. Call vibeplot.start() first.")
    _connection.reset_rotation()


def load_volume(volume, level: float = 0.0):
    """
    Visualize a 3D scalar field by extracting and rendering its isosurface.

    Uses the Marching Cubes algorithm to extract the isosurface at the given
    threshold level and sends it to the connected browser. Vertices are colored
    by their surface normal direction (X→red, Y→green, Z→blue), giving intuitive
    orientation cues.

    Args:
        volume: 3D numpy array of scalar values, shape (Nx, Ny, Nz)
        level:  Isosurface threshold value (default 0.0)

    Requires:
        numpy  (pip install numpy)

    Example:
        import numpy as np
        import vibeplot

        n = 40
        t = np.linspace(-2, 2, n)
        x, y, z = np.meshgrid(t, t, t, indexing='ij')
        volume = x**2 + y**2 + z**2 - 1  # unit sphere
        vibeplot.start()
        vibeplot.load_volume(volume, level=0.0)
    """
    try:
        import numpy as np
    except ImportError:
        raise ImportError("numpy is required for load_volume(). Install with: pip install numpy")

    from vibeplot.marching_cubes import march

    positions, normals, faces = march(volume, level)

    if not faces:
        raise ValueError(
            f"No isosurface found at level={level}. "
            f"Volume range: [{volume.min():.3f}, {volume.max():.3f}]"
        )

    lines = ["# Volume isosurface"]
    for pos, nrm in zip(positions, normals):
        r = (nrm[0] + 1.0) / 2.0
        g = (nrm[1] + 1.0) / 2.0
        b = (nrm[2] + 1.0) / 2.0
        lines.append(
            f"vertex {pos[0]:.4f} {pos[1]:.4f} {pos[2]:.4f} "
            f"{nrm[0]:.4f} {nrm[1]:.4f} {nrm[2]:.4f} "
            f"{r:.3f} {g:.3f} {b:.3f}"
        )
    for f in faces:
        lines.append(f"face {f[0]} {f[1]} {f[2]}")

    if not _connection:
        raise RuntimeError("Not started. Call vibeplot.start() first.")
    _connection.load_model("\n".join(lines))


def load_voxels(volume, colormap: str = "plasma", threshold: float = 0.05,
                alpha_scale: float = 1.0, max_voxels: int = 2000):
    """
    Visualize a 3D scalar field as semi-transparent voxel cubes.

    Each grid cell is rendered as a small cube whose color and opacity are
    determined by its normalized scalar value.  Voxels below ``threshold``
    (in normalized units) are skipped entirely.  The remaining voxels are
    sorted back-to-front by distance from the volume centre so that alpha
    blending composites correctly when viewed from the default camera angle.

    Args:
        volume:     3-D numpy array of shape (nx, ny, nz).
        colormap:   One of ``'plasma'``, ``'viridis'``, ``'hot'``, ``'cool'``
                    (default ``'plasma'``).
        threshold:  Minimum normalised value [0–1] to include (default 0.05).
        alpha_scale: Overall opacity multiplier – reduce below 1.0 for more
                    transparent voxels (default 1.0).
        max_voxels: Hard cap on the number of rendered voxels to stay within
                    the GPU vertex-buffer limit (default 2000).

    Example::

        import numpy as np
        import vibeplot

        n = 20
        t = np.linspace(-2, 2, n)
        x, y, z = np.meshgrid(t, t, t, indexing='ij')
        vol = np.exp(-(x**2 + y**2 + z**2))   # 3-D Gaussian blob
        vibeplot.start()
        vibeplot.load_voxels(vol)
        vibeplot.show()
    """
    try:
        import numpy as np
    except ImportError:
        raise ImportError("numpy is required for load_voxels(). pip install numpy")

    volume = np.asarray(volume, dtype=float)
    if volume.ndim != 3:
        raise ValueError(f"volume must be 3-D, got shape {volume.shape}")

    nx, ny, nz = volume.shape
    vmin, vmax = float(volume.min()), float(volume.max())
    vrange = vmax - vmin
    if vrange < 1e-12:
        raise ValueError("Volume has no variation (all values identical).")

    norm = (volume - vmin) / vrange  # shape (nx, ny, nz), values in [0, 1]

    # ── Colormap functions ────────────────────────────────────────────────────
    def _plasma(t):
        r = min(1.0, 0.05 + 2.0 * t)
        g = max(0.0, min(1.0, 3.2 * t * (1 - t) - 0.05))
        b = max(0.0, min(1.0, 0.95 - 1.6 * t))
        return r, g, b

    def _viridis(t):
        r = max(0.0, min(1.0, -0.37 + 2.63 * t - 1.65 * t * t))
        g = max(0.0, min(1.0,  0.14 + 1.10 * t - 0.30 * t * t))
        b = max(0.0, min(1.0,  0.55 - 0.50 * t))
        return r, g, b

    def _hot(t):
        return min(1.0, 3 * t), max(0.0, min(1.0, 3 * t - 1)), max(0.0, 3 * t - 2)

    def _cool(t):
        return t, 1.0 - t, 1.0

    _cmaps = {"plasma": _plasma, "viridis": _viridis, "hot": _hot, "cool": _cool}
    cmap_fn = _cmaps.get(colormap, _plasma)

    # ── Collect active voxels, sort back-to-front ─────────────────────────────
    sx, sy, sz = 1.0 / nx, 1.0 / ny, 1.0 / nz
    hx, hy, hz = sx * 0.48, sy * 0.48, sz * 0.48

    voxels = []
    for ix in range(nx):
        for iy in range(ny):
            for iz in range(nz):
                t = float(norm[ix, iy, iz])
                if t < threshold:
                    continue
                cx = (ix + 0.5) * sx - 0.5
                cy = (iy + 0.5) * sy - 0.5
                cz = (iz + 0.5) * sz - 0.5
                voxels.append((cx * cx + cy * cy + cz * cz, t, cx, cy, cz))

    # Farthest first → correct back-to-front alpha blending
    voxels.sort(key=lambda v: -v[0])

    # ── Face definitions for a voxel cube ────────────────────────────────────
    FACES = [
        ((0, 0,  1), ((-1,-1, 1), ( 1,-1, 1), ( 1, 1, 1), (-1, 1, 1))),
        ((0, 0, -1), (( 1,-1,-1), (-1,-1,-1), (-1, 1,-1), ( 1, 1,-1))),
        (( 1, 0, 0), (( 1,-1, 1), ( 1,-1,-1), ( 1, 1,-1), ( 1, 1, 1))),
        ((-1, 0, 0), ((-1,-1,-1), (-1,-1, 1), (-1, 1, 1), (-1, 1,-1))),
        ((0,  1, 0), ((-1, 1, 1), ( 1, 1, 1), ( 1, 1,-1), (-1, 1,-1))),
        ((0, -1, 0), ((-1,-1,-1), ( 1,-1,-1), ( 1,-1, 1), (-1,-1, 1))),
    ]

    # ── Build model text ──────────────────────────────────────────────────────
    vert_lines = [f"# Voxels {nx}x{ny}x{nz} ({colormap})"]
    face_lines = []
    vc = 0

    for count, (_, t, cx, cy, cz) in enumerate(voxels):
        if count >= max_voxels or vc + 24 > 64000:
            break
        alpha = min(t * alpha_scale, 1.0)
        r, g, b = cmap_fn(t)
        for (fnx, fny, fnz), corners in FACES:
            base = vc
            for (dx, dy, dz) in corners:
                px, py, pz = cx + dx * hx, cy + dy * hy, cz + dz * hz
                vert_lines.append(
                    f"vertex {px:.4f} {py:.4f} {pz:.4f} "
                    f"{fnx} {fny} {fnz} "
                    f"{r:.3f} {g:.3f} {b:.3f} {alpha:.3f}"
                )
                vc += 1
            face_lines.append(f"face {base} {base+1} {base+2}")
            face_lines.append(f"face {base} {base+2} {base+3}")

    if not face_lines:
        raise ValueError(
            f"No voxels above threshold={threshold}. "
            f"Try a lower threshold or check your volume data."
        )

    if not _connection:
        raise RuntimeError("Not started. Call vibeplot.start() first.")
    _connection.load_model("\n".join(vert_lines + face_lines))


def show():
    """
    Block until Ctrl+C or browser disconnects.

    Similar to matplotlib.pyplot.show(), this keeps the script running
    so the browser can interact with the visualization.

    Example:
        import vibeplot
        vibeplot.start()
        vibeplot.load_model(model_string)
        vibeplot.show()  # Script stays running
    """
    if not _connection:
        raise RuntimeError("Not started. Call vibeplot.start() first.")

    print("vibeplot: Press Ctrl+C to exit")
    try:
        while _connection.is_connected:
            _connection._connected.wait(timeout=0.5)
    except KeyboardInterrupt:
        print("\nvibeplot: Shutting down")
    finally:
        _connection.close()
