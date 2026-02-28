"""
Gyroid Volume Example

Visualizes a gyroid surface — a beautiful triply-periodic minimal surface
defined by: sin(x)cos(y) + sin(y)cos(z) + sin(z)cos(x) = 0

The surface is colored by its normal direction (X→red, Y→green, Z→blue),
which helps reveal its 3D structure as you rotate it.

Run with:
    python examples/volume_gyroid.py

Requirements:
    pip install numpy
    pip install vibeplot  (or install from source)
"""

import numpy as np
import vibeplot


def make_gyroid(n=30, periods=2):
    """
    Build a 3D scalar field for a gyroid surface.

    f(x, y, z) = sin(x)cos(y) + sin(y)cos(z) + sin(z)cos(x)

    The isosurface at f = 0 is the gyroid.

    Args:
        n:       Grid resolution per axis (higher = more detail, slower)
        periods: Number of gyroid periods to show per axis
    """
    t = np.linspace(0, 2 * np.pi * periods, n, dtype=np.float32)
    x, y, z = np.meshgrid(t, t, t, indexing="ij")
    return np.sin(x) * np.cos(y) + np.sin(y) * np.cos(z) + np.sin(z) * np.cos(x)


def main():
    print("Gyroid Volume Example")
    print("=====================")
    print()

    vibeplot.start()

    print("Generating gyroid scalar field (30³ grid, 2 periods)...")
    volume = make_gyroid(n=30, periods=2)
    print(f"  Volume shape: {volume.shape}, range: [{volume.min():.2f}, {volume.max():.2f}]")

    print("Extracting isosurface with Marching Cubes...")
    vibeplot.load_volume(volume, level=0.0)
    vibeplot.reset_rotation()

    print()
    print("Gyroid loaded! Controls:")
    print("  Drag      — rotate")
    print("  Scroll    — zoom")
    print("  /         — toggle debug panel")
    print()
    print("To view from your phone, open the URL printed above.")
    print("Press Ctrl+C to exit.")

    vibeplot.show()


if __name__ == "__main__":
    main()
