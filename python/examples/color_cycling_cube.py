"""
Color Cycling Cube Example

Displays a cube that changes face colors every second.
Run with: python examples/color_cycling_cube.py
"""

import time
import random
import vibeplot


def generate_cube(face_colors):
    """
    Generate a cube model with specified face colors.

    Args:
        face_colors: List of 6 RGB tuples [(r,g,b), ...] for each face:
                     [front, back, top, bottom, right, left]

    Returns:
        Model string in vibeplot format
    """
    # Face definitions: (normal, vertices)
    # Each face has 4 vertices at corners
    faces = [
        # Front face (z = 0.5)
        ((0, 0, 1), [
            (-0.5, -0.5, 0.5),
            (0.5, -0.5, 0.5),
            (0.5, 0.5, 0.5),
            (-0.5, 0.5, 0.5),
        ]),
        # Back face (z = -0.5)
        ((0, 0, -1), [
            (0.5, -0.5, -0.5),
            (-0.5, -0.5, -0.5),
            (-0.5, 0.5, -0.5),
            (0.5, 0.5, -0.5),
        ]),
        # Top face (y = 0.5)
        ((0, 1, 0), [
            (-0.5, 0.5, 0.5),
            (0.5, 0.5, 0.5),
            (0.5, 0.5, -0.5),
            (-0.5, 0.5, -0.5),
        ]),
        # Bottom face (y = -0.5)
        ((0, -1, 0), [
            (-0.5, -0.5, -0.5),
            (0.5, -0.5, -0.5),
            (0.5, -0.5, 0.5),
            (-0.5, -0.5, 0.5),
        ]),
        # Right face (x = 0.5)
        ((1, 0, 0), [
            (0.5, -0.5, 0.5),
            (0.5, -0.5, -0.5),
            (0.5, 0.5, -0.5),
            (0.5, 0.5, 0.5),
        ]),
        # Left face (x = -0.5)
        ((-1, 0, 0), [
            (-0.5, -0.5, -0.5),
            (-0.5, -0.5, 0.5),
            (-0.5, 0.5, 0.5),
            (-0.5, 0.5, -0.5),
        ]),
    ]

    lines = ["# Color Cycling Cube"]

    # Generate vertices
    for face_idx, (normal, vertices) in enumerate(faces):
        r, g, b = face_colors[face_idx]
        nx, ny, nz = normal
        for x, y, z in vertices:
            lines.append(f"vertex {x} {y} {z} {nx} {ny} {nz} {r:.2f} {g:.2f} {b:.2f}")

    # Generate faces (2 triangles per cube face)
    for face_idx in range(6):
        base = face_idx * 4
        lines.append(f"face {base} {base+1} {base+2}")
        lines.append(f"face {base} {base+2} {base+3}")

    return "\n".join(lines)


def random_color():
    """Generate a random bright color."""
    # Use HSV-like approach: one channel high, others random
    colors = [random.uniform(0.7, 1.0), random.uniform(0.2, 0.5), random.uniform(0.2, 0.5)]
    random.shuffle(colors)
    return tuple(colors)


def main():
    print("Color Cycling Cube Example")
    print("Press Ctrl+C to exit\n")

    # Start vibeplot (opens browser)
    vibeplot.start()

    # Initial colors for 6 faces
    face_colors = [random_color() for _ in range(6)]

    try:
        frame = 0
        while True:
            # Generate and send the cube
            model = generate_cube(face_colors)
            vibeplot.load_model(model)

            frame += 1
            print(f"Frame {frame}: Updated cube colors")

            # Wait 1 second
            time.sleep(1.0)

            # Randomize colors for next frame
            face_colors = [random_color() for _ in range(6)]

    except KeyboardInterrupt:
        print("\nExiting...")


if __name__ == "__main__":
    main()
