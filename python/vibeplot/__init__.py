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
__all__ = ["start", "load_model", "load_volume", "show", "reset_zoom", "reset_rotation", "VibePlotConnection"]

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
