#!/usr/bin/env python3
"""Build Cloudshell's production icon files from the approved source artwork.

The artwork is a cyan cloud-terminal mark: it conveys both the product name and
its SSH/terminal purpose without relying on text or small decorative details.
"""

from collections import deque
from pathlib import Path

from PIL import Image


ASSETS = Path(__file__).resolve().parent
SOURCE = ASSETS / "icon-source.png"


def remove_edge_background(image: Image.Image) -> Image.Image:
    """Turn only the white background connected to an outer edge transparent.

    The source artwork intentionally keeps a pale inner highlight on the cloud;
    edge-connected flood filling preserves that detail while making rounded app
    icon corners transparent on platforms that support alpha.
    """
    rgba = image.convert("RGBA")
    pixels = rgba.load()
    width, height = rgba.size
    seen = bytearray(width * height)
    queue = deque()

    def is_background(x: int, y: int) -> bool:
        r, g, b, _ = pixels[x, y]
        return r >= 244 and g >= 244 and b >= 244 and max(r, g, b) - min(r, g, b) <= 10

    def add(x: int, y: int) -> None:
        index = y * width + x
        if not seen[index] and is_background(x, y):
            seen[index] = 1
            queue.append((x, y))

    for x in range(width):
        add(x, 0)
        add(x, height - 1)
    for y in range(height):
        add(0, y)
        add(width - 1, y)

    while queue:
        x, y = queue.popleft()
        r, g, b, _ = pixels[x, y]
        pixels[x, y] = (r, g, b, 0)
        for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
            if 0 <= nx < width and 0 <= ny < height:
                add(nx, ny)
    return rgba


def main() -> None:
    if not SOURCE.exists():
        raise SystemExit(f"Missing source artwork: {SOURCE}")
    artwork = remove_edge_background(Image.open(SOURCE))
    icon_512 = artwork.resize((512, 512), Image.Resampling.LANCZOS)
    icon_512.save(ASSETS / "icon@512.png")
    icon_512.resize((256, 256), Image.Resampling.LANCZOS).save(ASSETS / "icon.png")
    icon_512.save(
        ASSETS / "cloudshell.ico",
        format="ICO",
        sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)],
    )
    print("Updated icon.png, icon@512.png, cloudshell.ico")


if __name__ == "__main__":
    main()
