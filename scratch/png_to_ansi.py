#!/usr/bin/env python3
import sys
import os
from PIL import Image

def png_to_ansi(image_path, width=12, height=16, bg_rgb=(15, 15, 20)):
    """
    Converts any PNG image to a high-fidelity sub-pixel ANSI terminal block string.
    Each cell vertically maps 2 pixels (top & bottom half blocks).
    """
    if not os.path.exists(image_path):
        print(f"Error: File '{image_path}' not found.", file=sys.stderr)
        return

    # Load and resize image to exact sub-pixel dimensions
    img = Image.open(image_path).convert("RGBA")
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    pixels = img.load()

    # Process pixel grid: height should be even since each terminal cell is 2 pixels tall
    # If height is odd, pad by 1 pixel at the bottom
    if height % 2 != 0:
        height += 1
        
    lines = []
    for r in range(0, height, 2):
        cell_line = []
        for c in range(width):
            # Top pixel
            tr, tg, tb, ta = pixels[c, r] if r < img.height else (0, 0, 0, 0)
            if ta < 128 or (tr > 240 and tg > 240 and tb > 240):
                top_rgb = bg_rgb
            else:
                top_rgb = (tr, tg, tb)
                
            # Bottom pixel
            br, bg, bb, ba = pixels[c, r + 1] if (r + 1) < img.height else (0, 0, 0, 0)
            if ba < 128 or (br > 240 and bg > 240 and bb > 240):
                bottom_rgb = bg_rgb
            else:
                bottom_rgb = (br, bg, bb)
                
            # Render half block: \033[38;2;R;G;Bm for top foreground, \033[48;2;R;G;Bm for bottom background
            block = f"\033[38;2;{top_rgb[0]};{top_rgb[1]};{top_rgb[2]}m\033[48;2;{bottom_rgb[0]};{bottom_rgb[1]};{bottom_rgb[2]}m▀\033[0m"
            cell_line.append(block)
        lines.append("  " + "".join(cell_line))
        
    return "\n".join(lines)

def generate_rust_const(image_path, var_name, width=12, height=16):
    """
    Generates a Rust const array representing the RGB pixels, suitable for direct embedding.
    """
    if not os.path.exists(image_path):
        return ""
    img = Image.open(image_path).convert("RGBA")
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    pixels = img.load()
    
    rust_code = []
    rust_code.append(f"/// Generated from {os.path.basename(image_path)}")
    rust_code.append(f"pub const {var_name.upper()}: [[(u8, u8, u8); {width}]; {height}] = [")
    for r in range(height):
        row_elements = []
        for c in range(width):
            pr, pg, pb, pa = pixels[c, r]
            # Handle alpha and white background by defaulting to zero (black/transparent)
            if pa < 128 or (pr > 240 and pg > 240 and pb > 240):
                row_elements.append("(0, 0, 0)")
            else:
                row_elements.append(f"({pr}, {pg}, {pb})")
        rust_code.append("    [" + ", ".join(row_elements) + "],")
    rust_code.append("];")
    return "\n".join(rust_code)

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 png_to_ansi.py <path_to_image> [width] [height]")
        sys.exit(1)
        
    img_path = sys.argv[1]
    w = int(sys.argv[2]) if len(sys.argv) > 2 else 12
    h = int(sys.argv[3]) if len(sys.argv) > 3 else 16
    
    print(f"\n--- Previewing '{img_path}' at {w}x{h} sub-pixels ---")
    ansi_art = png_to_ansi(img_path, w, h)
    if ansi_art:
        print(ansi_art)
        print("\n--- Rust Source Representation ---")
        var_name = os.path.splitext(os.path.basename(img_path))[0].replace("-", "_")
        print(generate_rust_const(img_path, var_name, w, h))
        print()
