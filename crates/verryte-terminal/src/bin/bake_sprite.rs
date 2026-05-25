use std::path::Path;
use verryte_terminal::{ResolutionTier, image_to_grid_with_chroma_key, Color};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("Usage: bake_sprite <input_png> <output_vbyte> [tier]");
        println!("Tiers: tiny, small, medium, large, xlarge, ultra");
        return Ok(());
    }

    let input_path = &args[1];
    let output_path = &args[2];
    let tier_str = args.get(3).map(|s| s.as_str()).unwrap_or("medium");

    let tier = match tier_str.to_lowercase().as_str() {
        "tiny" => ResolutionTier::TINY,
        "small" => ResolutionTier::SMALL,
        "medium" => ResolutionTier::MEDIUM,
        "large" => ResolutionTier::LARGE,
        "xlarge" => ResolutionTier::XLARGE,
        "ultra" => ResolutionTier::ULTRA,
        _ => {
            println!("Unknown tier: {}. Defaulting to medium.", tier_str);
            ResolutionTier::MEDIUM
        }
    };

    let img = image::open(input_path)?;
    let (cols, rows) = tier.sprite_size();
    
    // Grid assumes rows are half-blocks, so we need 2x pixel height
    let resized = img.resize_exact(cols as u32, rows as u32 * 2, image::imageops::FilterType::Lanczos3);
    
    // Default chroma key for Wuthering Terminal assets is white
    let grid = image_to_grid_with_chroma_key(&resized, Color(255, 255, 255), 30);
    let bytes = grid.to_bytes();
    
    std::fs::write(output_path, bytes)?;
    println!("Baked {} into {} (Tier: {:?}, {}x{} cells)", input_path, output_path, tier, cols, rows);

    Ok(())
}
