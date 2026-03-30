"""
Voxel Volume Examples

Demonstrates vibeplot.load_voxels() — rendering a 3D scalar field as
semi-transparent voxel cubes coloured by value.

Three scenes are loaded into separate figures:

1. Gaussian Blob    — single spherically-symmetric density peak (plasma)
2. Two Blobs        — two offset Gaussian blobs side by side (hot)
3. Spherical Shell  — hollow thin shell at r ≈ 1.2 (cool)

Run with:
    python examples/voxels_demos.py

Requirements:
    pip install numpy
    pip install vibeplot  (or install from source)
"""

import numpy as np
import vibeplot


def gaussian_blob(n=20):
    """Single 3-D Gaussian centred at the origin."""
    t = np.linspace(-2.5, 2.5, n, dtype=np.float32)
    x, y, z = np.meshgrid(t, t, t, indexing="ij")
    return np.exp(-(x**2 + y**2 + z**2))


def two_blobs(n=20):
    """Two offset Gaussian blobs along the x-axis."""
    t = np.linspace(-3.0, 3.0, n, dtype=np.float32)
    x, y, z = np.meshgrid(t, t, t, indexing="ij")
    return (
        np.exp(-((x - 1.4)**2 + y**2 + z**2) / 1.2) +
        np.exp(-((x + 1.4)**2 + y**2 + z**2) / 1.2)
    )


def spherical_shell(n=22):
    """Thin hollow shell at radius ≈ 1.2."""
    t = np.linspace(-2.0, 2.0, n, dtype=np.float32)
    x, y, z = np.meshgrid(t, t, t, indexing="ij")
    r = np.sqrt(x**2 + y**2 + z**2)
    return np.exp(-((r - 1.2)**2) / 0.06)


def main():
    print("Voxel Volume Examples")
    print("=" * 40)
    print()

    vibeplot.start()

    # ── Figure 1: Gaussian blob ───────────────────────────────────────────────
    print("Loading Gaussian blob (plasma colormap)...")
    vol1 = gaussian_blob(n=20)
    print(f"  shape={vol1.shape}, range=[{vol1.min():.3f}, {vol1.max():.3f}]")
    vibeplot.load_voxels(vol1, colormap="plasma", threshold=0.04)
    vibeplot.reset_rotation()
    print("  Done.")

    # ── Figure 2: Two blobs ───────────────────────────────────────────────────
    print("Loading two-blob scene (hot colormap)...")
    vol2 = two_blobs(n=20)
    print(f"  shape={vol2.shape}, range=[{vol2.min():.3f}, {vol2.max():.3f}]")
    vibeplot.load_voxels(vol2, colormap="hot", threshold=0.06)
    vibeplot.reset_rotation()
    print("  Done.")

    # ── Figure 3: Spherical shell ─────────────────────────────────────────────
    print("Loading spherical shell (cool colormap)...")
    vol3 = spherical_shell(n=22)
    print(f"  shape={vol3.shape}, range=[{vol3.min():.3f}, {vol3.max():.3f}]")
    vibeplot.load_voxels(vol3, colormap="cool", threshold=0.08, alpha_scale=0.75)
    vibeplot.reset_rotation()
    print("  Done.")

    print()
    print("All three volumes loaded into the active figure.")
    print("Use Cmd+T to open new figures and re-run individual vibeplot.load_voxels() calls.")
    print()
    print("Controls:  Drag=rotate  Scroll=zoom  /=debug  Cmd+Shift+P=palette")
    print("Press Ctrl+C to exit.")

    vibeplot.show()


if __name__ == "__main__":
    main()
