#!/usr/bin/env python3
import sys

# Define HSL-like RGB colors for premium WuWa aesthetic
GREY = (180, 180, 190)    # Rover hair
SKIN = (245, 200, 175)    # Face skin
DARK = (45, 45, 55)       # Dark cloak
YELLOW = (255, 215, 0)    # Glowing spectral weapon
TEAL = (0, 180, 160)      # Jiyan armor/hair
GOLD = (230, 175, 40)     # Jiyan gold accents
RED = (220, 40, 60)       # Target/Hazard red
BLACK = (15, 15, 20)      # Floor/background black

def ansi_block(top_rgb, bottom_rgb):
    # \033[38;2;R;G;Bm sets foreground (top pixel)
    # \033[48;2;R;G;Bm sets background (bottom pixel)
    # ▀ is the upper half block
    return f"\033[38;2;{top_rgb[0]};{top_rgb[1]};{top_rgb[2]}m\033[48;2;{bottom_rgb[0]};{bottom_rgb[1]};{bottom_rgb[2]}m▀\033[0m"

# Rover 4x4 sub-pixel layout (4 cols x 4 rows)
# Row 0: [GREY,   GREY,   DARK,  BLACK]
# Row 1: [SKIN,   SKIN,   DARK,  YELLOW]
# Row 2: [DARK,   DARK,   DARK,  YELLOW]
# Row 3: [DARK,   BLACK,  DARK,  BLACK]

# Translates to 4x2 terminal cells:
rover_cells = [
    # Terminal Row 0
    [(GREY, SKIN), (GREY, SKIN), (DARK, DARK), (BLACK, YELLOW)],
    # Terminal Row 1
    [(DARK, DARK), (DARK, BLACK), (DARK, DARK), (YELLOW, BLACK)]
]

# Jiyan 4x4 sub-pixel layout
# Row 0: [TEAL,  TEAL,  BLACK, TEAL]   (Teal hair/helm)
# Row 1: [SKIN,  TEAL,  GOLD,  GOLD]   (Face and shoulder armor)
# Row 2: [TEAL,  TEAL,  TEAL,  GOLD]   (Teal chest armor and spear)
# Row 3: [TEAL,  BLACK, TEAL,  BLACK]  (Teal boots/greaves)

jiyan_cells = [
    # Terminal Row 0
    [(TEAL, SKIN), (TEAL, TEAL), (BLACK, GOLD), (TEAL, GOLD)],
    # Terminal Row 1
    [(TEAL, TEAL), (TEAL, BLACK), (TEAL, TEAL), (GOLD, BLACK)]
]

print("\n--- ROVER 4x4 Sub-Pixel Terminal Sprite ---")
for row in rover_cells:
    line = "".join(ansi_block(top, bottom) for (top, bottom) in row)
    print("  " + line)

print("\n--- JIYAN 4x4 Sub-Pixel Terminal Sprite ---")
for row in jiyan_cells:
    line = "".join(ansi_block(top, bottom) for (top, bottom) in row)
    print("  " + line)
print()
