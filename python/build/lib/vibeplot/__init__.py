"""
vibeplot - Python client for vibeplot 3D model viewer.

Usage:
    import vibeplot
    vibeplot.connect()  # Blocks until browser connects
    vibeplot.load_model(model_string)
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
__all__ = ["connect", "load_model", "reset_zoom", "reset_rotation", "VibePlotConnection"]

DEFAULT_PORT = 9753
DEFAULT_HOST = "localhost"


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
        self._started = threading.Event()

    async def _handle_client(self, websocket):
        """Handle incoming browser connection."""
        self._websocket = websocket
        self._connected.set()
        print(f"vibeplot: Browser connected")
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if data.get("type") == "ack" and not data.get("success"):
                        print(f"vibeplot error: {data.get('error')}")
                except json.JSONDecodeError:
                    pass
        except websockets.exceptions.ConnectionClosed:
            pass
        finally:
            print("vibeplot: Browser disconnected")
            self._websocket = None
            self._connected.clear()

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
        """Block until browser connects. Returns True if connected, False on timeout."""
        return self._connected.wait(timeout=timeout)

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


def connect(
    host: str = DEFAULT_HOST,
    port: int = DEFAULT_PORT,
    open_browser: bool = False,
    wait: bool = True,
    timeout: Optional[float] = None,
) -> VibePlotConnection:
    """
    Start vibeplot WebSocket server and optionally wait for browser.

    Args:
        host: Host to bind to (default: localhost)
        port: Port to bind to (default: 9753)
        open_browser: Open default browser to vibeplot page
        wait: Wait for browser connection before returning
        timeout: Max seconds to wait for connection (None = forever)

    Returns:
        VibePlotConnection instance

    Example:
        import vibeplot
        vibeplot.connect()
        vibeplot.load_model(model_string)
    """
    global _connection
    _connection = VibePlotConnection(host, port)
    _connection.start(open_browser=open_browser)

    if wait:
        if not open_browser:
            print("vibeplot: Waiting for browser... Open http://localhost:8000")
        connected = _connection.wait_for_connection(timeout=timeout)
        if not connected:
            raise TimeoutError("Timed out waiting for browser connection")

    return _connection


def load_model(model_text: str):
    """Send model to connected browser."""
    if not _connection:
        raise RuntimeError("Not connected. Call vibeplot.connect() first.")
    _connection.load_model(model_text)


def reset_zoom():
    """Reset zoom in connected browser."""
    if not _connection:
        raise RuntimeError("Not connected. Call vibeplot.connect() first.")
    _connection.reset_zoom()


def reset_rotation():
    """Reset rotation in connected browser."""
    if not _connection:
        raise RuntimeError("Not connected. Call vibeplot.connect() first.")
    _connection.reset_rotation()
