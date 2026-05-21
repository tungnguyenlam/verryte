#!/usr/bin/env python3
"""Batch preview all chibi sprites at multiple resolutions for visual QA."""
import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from png_to_ansi import png_to_ansi

ASSETS = os.path.join(os.path.dirname(__file__), "..", "prototype", "wuthering-terminal", "assets")

sprites = {
    "ROVER": None,
    "JIYAN": None,
    "BAIZHI": None,
    "CROWNLESS": None,
}

# Find files by prefix match
for f in sorted(os.listdir(ASSETS)):
    for key in sprites:
        if f.lower().startswith(key.lower()) and f.endswith(".png"):
            sprites[key] = os.path.join(ASSETS, f)

RESOLUTIONS = [
    ("8x12 (8x6 cells)", 8, 12),
    ("12x16 (12x8 cells)", 12, 16),
    ("16x20 (16x10 cells)", 16, 20),
    ("20x24 (20x12 cells)", 20, 24),
]

for res_label, w, h in RESOLUTIONS:
    print(f"\n{'='*60}")
    print(f"  RESOLUTION: {res_label}")
    print(f"{'='*60}")
    for name, path in sprites.items():
        if path is None:
            print(f"\n  [{name}] -- NOT FOUND")
            continue
        print(f"\n  [{name}]")
        result = png_to_ansi(path, w, h)
        if result:
            print(result)
    print()

print("\n--- SIDE-BY-SIDE at 12x16 (12x8 cells) ---")
# Render all 4 side by side at the standard 12x16 resolution
from PIL import Image

def get_pixel_grid(path, w, h, bg=(15, 15, 20)):
    img = Image.open(path).convert("RGBA")
    img = img.resize((w, h), Image.Resampling.LANCZOS)
    px = img.load()
    grid = []
    for r in range(h):
        row = []
        for c in range(w):
            pr, pg, pb, pa = px[c, r]
            if pa < 128 or (pr > 240 and pg > 240 and pb > 240):
                row.append(bg)
            else:
                row.append((pr, pg, pb))
        grid.append(row)
    return grid

def ansi_block(top, bot):
    return f"\033[38;2;{top[0]};{top[1]};{top[2]}m\033[48;2;{bot[0]};{bot[1]};{bot[2]}m▀\033[0m"

W, H = 12, 16
SPACER = "  "
grids = []
labels = []
for name, path in sprites.items():
    if path:
        grids.append(get_pixel_grid(path, W, H))
        labels.append(name)

# Print labels
print("  " + SPACER.join(f"{lbl:^{W}s}" for lbl in labels))
# Print rows
for cell_row in range(H // 2):
    parts = []
    for g in grids:
        top_row = g[cell_row * 2]
        bot_row = g[cell_row * 2 + 1]
        parts.append("".join(ansi_block(t, b) for t, b in zip(top_row, bot_row)))
    print("  " + SPACER.join(parts))

print()
