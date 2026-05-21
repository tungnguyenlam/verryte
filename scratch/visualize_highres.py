#!/usr/bin/env python3
# Visualize a premium compact 12x16 chibi-style Rover portrait with dynamic color gradients

def lerp_color(c1, c2, t):
    t = max(0.0, min(1.0, t))
    return tuple(int(a + (b - a) * t) for a, b in zip(c1, c2))

# Color Palette Anchors
BG_TOP = (15, 15, 25)
BG_BOTTOM = (35, 20, 40)
HAIR_TIP = (255, 255, 255)
HAIR_BASE = (175, 180, 200)
HAIR_SHADOW = (90, 80, 110)
SKIN_LIGHT = (255, 225, 210)
SKIN_SHADOW = (215, 155, 135)
EYE_GLOW = (255, 220, 0)
EYE_DARK = (150, 90, 0)
CLOAK_LIGHT = (60, 60, 80)
CLOAK_DARK = (20, 20, 30)
WEAPON_CORE = (255, 255, 255)
WEAPON_GLOW = (255, 200, 0)
WEAPON_EDGE = (255, 50, 0)
GOLD_BRIGHT = (245, 190, 40)
GOLD_DARK = (130, 85, 15)

# Build a grid of 16 rows x 12 columns of colors using procedural gradients
portrait_pixels = []

for r in range(16):
    row = []
    # Vertical gradient factors
    y_factor = r / 15.0
    bg_color = lerp_color(BG_TOP, BG_BOTTOM, y_factor)
    
    # Hair gradient interpolation
    hair_color = lerp_color(HAIR_TIP, HAIR_BASE, y_factor)
    hair_shadow_color = lerp_color(HAIR_BASE, HAIR_SHADOW, y_factor)
    
    # Skin gradient interpolation
    skin_color = lerp_color(SKIN_LIGHT, SKIN_SHADOW, y_factor)
    
    # Cloak gradient interpolation
    cloak_color = lerp_color(CLOAK_LIGHT, CLOAK_DARK, y_factor)
    
    # Weapon flame gradient (Core -> Glow -> Edge)
    weapon_outer = lerp_color(WEAPON_GLOW, WEAPON_EDGE, y_factor)
    weapon_inner = lerp_color(WEAPON_CORE, WEAPON_GLOW, y_factor)
    
    # Gold armor gradient
    gold_color = lerp_color(GOLD_BRIGHT, GOLD_DARK, y_factor)

    for c in range(12):
        # Center horizontal distance
        x_factor = abs(c - 5.5) / 5.5
        
        # 16-row Chibi Rover layout
        if r < 3:
            # Spikey big hair head top
            if (r == 0 and c in [3, 4, 5, 6, 7, 8]) or \
               (r == 1 and c in [2, 3, 4, 5, 6, 7, 8, 9]) or \
               (r == 2 and c in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]):
                row.append(lerp_color(hair_color, HAIR_TIP, 1.0 - x_factor))
            else:
                row.append(bg_color)
        elif r < 8:
            # Cute big head, giant eyes and face skin
            if c in [0, 11] or (r == 3 and c in [1, 10]) or (r == 7 and c in [1, 2, 9, 10]):
                row.append(hair_shadow_color)
            elif c in [1, 2, 9, 10] and r < 6:
                row.append(hair_shadow_color)
            else:
                # Face Area
                if r in [4, 5] and c in [3, 4, 7, 8]:
                    # Giant cute eyes
                    eye_t = (r - 4)
                    row.append(lerp_color(EYE_GLOW, EYE_DARK, eye_t))
                elif r == 6 and c in [5, 6]:
                    row.append((255, 150, 150))  # Cute little blush mouth
                else:
                    row.append(lerp_color(skin_color, (255, 185, 170), 1.0 - x_factor))
        elif r < 10:
            # Tiny Chibi Neck & shoulders
            if c in [4, 5, 6, 7] and r == 8:
                row.append(SKIN_SHADOW)
            elif c in [3, 4, 7, 8]:
                row.append(cloak_color)
            elif c in [2, 9]:
                row.append(gold_color)
            else:
                row.append(bg_color)
        elif r < 14:
            # Tiny Chibi body and oversized glowing spectral sword
            # Sword diagonal slash: r + c == 14 or 15 or 16
            if abs((r + c) - 15) <= 1 and c in [2, 3, 4, 5, 6]:
                if abs((r + c) - 15) == 0:
                    row.append(weapon_inner)
                else:
                    row.append(weapon_outer)
            elif c in [2, 3, 7, 8, 9]:
                row.append(cloak_color)
            elif c in [4, 5, 6]:
                row.append(gold_color)
            else:
                row.append(bg_color)
        else:
            # Tiny cloak bottom
            fold_factor = (c % 2) / 1.0
            fold_color = lerp_color(cloak_color, CLOAK_DARK, fold_factor)
            if c in [4, 5, 6, 7]:
                row.append(fold_color)
            else:
                row.append(bg_color)
                
    portrait_pixels.append(row)

def ansi_block(top_rgb, bottom_rgb):
    return f"\033[38;2;{top_rgb[0]};{top_rgb[1]};{top_rgb[2]}m\033[48;2;{bottom_rgb[0]};{bottom_rgb[1]};{bottom_rgb[2]}m▀\033[0m"

def render_portrait():
    for cell_row in range(8):
        top_pixels = portrait_pixels[cell_row * 2]
        bottom_pixels = portrait_pixels[cell_row * 2 + 1]
        line = "".join(ansi_block(t, b) for t, b in zip(top_pixels, bottom_pixels))
        print("  " + line)

print("\n=== CHIBI ROVER 12x16 GRADIENT PORTRAIT (12x8 cells) ===")
render_portrait()
print()
