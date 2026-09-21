#!/usr/bin/env python3
"""Create transparent FileGo application icons from owner-provided artwork.

The source images contain a light checkerboard baked into their RGB pixels. This
script separates the strongly blue logo from that near-neutral background,
reconstructs an alpha channel for anti-aliased edge pixels, crops the foreground,
and places it on a square transparent canvas before high-quality downsampling.

Requires Pillow, NumPy, SciPy, and OpenCV. It is an asset-maintenance tool,
not part of the application build or GitHub Actions workflow.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter
from scipy import ndimage

ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "assets" / "source"
OUTPUT_DIR = ROOT / "assets" / "icons"

SOURCES = {
    "main": SOURCE_DIR / "main-logo-original.png",
    "tray": SOURCE_DIR / "tray-logo-original.png",
}

MAIN_OUTPUT = OUTPUT_DIR / "filego.png"
TRAY_OUTPUT = OUTPUT_DIR / "filego-tray.png"
ICO_OUTPUT = OUTPUT_DIR / "filego.ico"

# Pixels in the artwork are blue while the baked checkerboard is almost neutral.
# A smooth threshold retains anti-aliased blue edge coverage and excludes neutral
# JPEG/PNG noise. The connected-component mask below prevents isolated background
# color noise from becoming visible.
ALPHA_LOW = 3.0
ALPHA_HIGH = 118.0
SEED_THRESHOLD = 20.0
SOURCE_MARGIN = 12
CANVAS_PADDING_RATIO = 0.07


def _connected_foreground(seed: np.ndarray) -> np.ndarray:
    """Return all substantial connected blue components in a binary mask."""
    try:
        import cv2
    except ImportError as exc:  # pragma: no cover - maintenance environment guard
        raise RuntimeError("OpenCV (cv2) is required to regenerate icon assets") from exc

    count, labels, stats, _ = cv2.connectedComponentsWithStats(
        seed.astype(np.uint8), connectivity=8
    )
    if count <= 1:
        raise ValueError("no blue foreground components found")

    areas = stats[1:, cv2.CC_STAT_AREA]
    largest = int(areas.max())
    keep = np.zeros(count, dtype=bool)
    # The magnifying glass is a separate component inside the folder outline.
    # Retain every significant component while dropping isolated color noise.
    for label, area in enumerate(areas, start=1):
        if area >= max(256, largest * 0.05):
            keep[label] = True
    return keep[labels]


def remove_checkerboard(path: Path) -> Image.Image:
    source = Image.open(path).convert("RGB")
    rgb = np.asarray(source, dtype=np.float32)
    blue_dominance = rgb[..., 2] - np.maximum(rgb[..., 0], rgb[..., 1])

    connected = _connected_foreground(blue_dominance > SEED_THRESHOLD)
    # Grow significant components enough to include anti-aliased fringe pixels.
    support = Image.fromarray((connected * 255).astype(np.uint8)).filter(
        ImageFilter.MaxFilter(11)
    )
    support_mask = np.asarray(support, dtype=np.float32) / 255.0

    alpha = np.clip(
        (blue_dominance - ALPHA_LOW) / (ALPHA_HIGH - ALPHA_LOW), 0.0, 1.0
    )
    # Smooth only the recovered coverage; color remains sourced from the artwork.
    alpha_image = Image.fromarray(np.uint8(np.round(alpha * support_mask * 255))).filter(
        ImageFilter.GaussianBlur(0.45)
    )
    alpha = np.asarray(alpha_image, dtype=np.float32) / 255.0

    # Preserve the supplied blue gradient for solid pixels. Edge pixels contain
    # the neutral checkerboard matte, so borrow color from their nearest solid
    # foreground neighbour; alpha retains the original anti-aliased coverage.
    core_mask = (blue_dominance > 100.0) & connected
    if not np.any(core_mask):
        raise ValueError(f"no solid logo pixels found in {path}")
    nearest = ndimage.distance_transform_edt(
        ~core_mask, return_distances=False, return_indices=True
    )
    foreground = rgb[nearest[0], nearest[1]]
    foreground[core_mask] = rgb[core_mask]

    rgba = np.dstack((foreground.astype(np.uint8), np.uint8(np.round(alpha * 255))))
    image = Image.fromarray(rgba, "RGBA")

    bbox = image.getchannel("A").point(lambda value: 255 if value >= 2 else 0).getbbox()
    if bbox is None:
        raise ValueError(f"no visible foreground recovered from {path}")
    left, top, right, bottom = bbox
    left = max(0, left - SOURCE_MARGIN)
    top = max(0, top - SOURCE_MARGIN)
    right = min(image.width, right + SOURCE_MARGIN)
    bottom = min(image.height, bottom + SOURCE_MARGIN)
    return image.crop((left, top, right, bottom))


def square_icon(image: Image.Image, size: int, padding_ratio: float) -> Image.Image:
    available = max(1, round(size * (1.0 - 2.0 * padding_ratio)))
    scale = min(available / image.width, available / image.height)
    target = (
        max(1, round(image.width * scale)),
        max(1, round(image.height * scale)),
    )
    resized = image.resize(target, Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.alpha_composite(resized, ((size - target[0]) // 2, (size - target[1]) // 2))
    return canvas


def save_png(image: Image.Image, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    image.save(destination, format="PNG", optimize=True)


def main() -> None:
    missing = [str(path) for path in SOURCES.values() if not path.is_file()]
    if missing:
        raise FileNotFoundError("missing source artwork: " + ", ".join(missing))

    main_art = remove_checkerboard(SOURCES["main"])
    tray_art = remove_checkerboard(SOURCES["tray"])

    main_512 = square_icon(main_art, 512, CANVAS_PADDING_RATIO)
    tray_256 = square_icon(tray_art, 256, 0.055)
    tray_32 = tray_256.resize((32, 32), Image.Resampling.LANCZOS)

    save_png(main_512, MAIN_OUTPUT)
    save_png(tray_32, TRAY_OUTPUT)

    # Pillow writes a valid multi-resolution Windows icon. Generate the ICO from
    # the large main artwork so Windows can choose an appropriate shell size.
    main_512.save(
        ICO_OUTPUT,
        format="ICO",
        sizes=[(16, 16), (20, 20), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    for destination in (MAIN_OUTPUT, TRAY_OUTPUT, ICO_OUTPUT):
        print(f"wrote {destination.relative_to(ROOT)} ({destination.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
