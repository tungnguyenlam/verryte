#!/usr/bin/env python3
"""
Proof-of-concept: adaptive resolution sprite selection based on terminal size.
Simulates the exact logic that will run in Rust at runtime.
"""

import os, sys, shutil

sys.path.insert(0, os.path.dirname(__file__))
from png_to_ansi import png_to_ansi

ASSETS = os.path.join(
    os.path.dirname(__file__), "..", "prototype", "wuthering-terminal", "assets"
)

# Find kael sprite
kael_path = None
for f in sorted(os.listdir(ASSETS)):
    if f.startswith("kael") and f.endswith(".png"):
        kael_path = os.path.join(ASSETS, f)
        break

# Resolution tiers: (label, sprite_w, sprite_h, min_cols, min_rows)
TIERS = [
    ("TINY", 6, 8, 60, 20),  # Fits anywhere
    ("SMALL", 8, 12, 80, 24),  # Standard 80x24
    ("MEDIUM", 12, 16, 100, 30),  # Moderate window
    ("LARGE", 16, 20, 120, 36),  # Large window
    ("XLARGE", 20, 24, 140, 42),  # Fullscreen / ultrawide
    ("ULTRA", 28, 32, 160, 48),  # 4K / massive monitor
]

# Detect actual terminal size
cols, rows = shutil.get_terminal_size((80, 24))

# Select best tier that fits
selected = TIERS[0]
for tier in TIERS:
    label, sw, sh, min_c, min_r = tier
    if cols >= min_c and rows >= min_r:
        selected = tier

label, sw, sh, min_c, min_r = selected

print(f"\n╔══════════════════════════════════════════════╗")
print(f"║  Terminal detected: {cols} cols × {rows} rows")
print(f"║  Selected tier: {label} ({sw}×{sh} sub-pixels, {sw}×{sh // 2} cells)")
print(f"╚══════════════════════════════════════════════╝")

# Show ALL tiers with markers
print(f"\n--- All Resolution Tiers (selected: {label}) ---\n")
for tier in TIERS:
    t_label, t_sw, t_sh, t_min_c, t_min_r = tier
    fits = "✓" if cols >= t_min_c and rows >= t_min_r else "✗"
    sel = " ◀ ACTIVE" if t_label == label else ""
    print(
        f"  [{fits}] {t_label:8s}  {t_sw:2d}×{t_sh:2d} sub-px  ({t_sw}×{t_sh // 2} cells)  min {t_min_c}×{t_min_r}{sel}"
    )

# Render the selected tier
print(f"\n--- ROVER at {label} resolution ({sw}×{sh} → {sw}×{sh // 2} cells) ---")
result = png_to_ansi(kael_path, sw, sh)
if result:
    print(result)

# Also show what it looks like one tier up and one tier down for comparison
tier_idx = TIERS.index(selected)
if tier_idx > 0:
    prev = TIERS[tier_idx - 1]
    print(f"\n--- (one tier down: {prev[0]} {prev[1]}×{prev[2]}) ---")
    r = png_to_ansi(kael_path, prev[1], prev[2])
    if r:
        print(r)

if tier_idx < len(TIERS) - 1:
    nxt = TIERS[tier_idx + 1]
    print(f"\n--- (one tier up: {nxt[0]} {nxt[1]}×{nxt[2]}) ---")
    r = png_to_ansi(kael_path, nxt[1], nxt[2])
    if r:
        print(r)

print()
