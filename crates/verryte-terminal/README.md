# verryte-terminal

Terminal rendering primitives for the Verryte engine.

## Features

- **Grid-based Rendering**: Efficient cell grid with colors, styles, and transparency.
- **Adaptive Resolution**: Support for multiple fidelity tiers (ASCII, TINY, SMALL, MEDIUM, LARGE, XLARGE, ULTRA).
- **Smooth Camera**: Integrated camera system with smooth panning, anchored zooming, and screen shake.
- **VFX System**: Lightweight particle and effect system (rain, snow, embers, flashes, etc.).
- **TUI Widgets**: Reusable UI components (buttons, progress bars, panels, dialogue boxes).
- **Asset Registry**: Central management for sprites and visual assets.

## Adaptive Resolution

Verryte-terminal uses a "resolution tier" system to handle different terminal sizes.
Sprites can be baked into multiple grids, one for each tier. At runtime, the engine
chooses the best tier based on the current window dimensions.

```rust
use verryte_terminal::ResolutionTier;

let tier = ResolutionTier::from_size(width, height);
let (tile_w, tile_h) = tier.tile_dimensions();
```

### Supported Tiers

- `ASCII`: 1x1 cells. Ideal for minimal feedback or very small terminals.
- `TINY` to `ULTRA`: Various density levels using half-block characters (`▀`, `▄`) to double the vertical resolution.

## Camera & Viewport

The `Camera` handles logical positioning and zoom. The `TileViewport` maps world coordinates
to screen cells, handling clipping and centering.

```rust
camera.zoom_at_world(cursor_x, cursor_y, next_zoom, screen_w, screen_h);
```
