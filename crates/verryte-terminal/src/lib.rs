//! Terminal rendering primitives for Verryte.
//!
//! Provides the core grid-based rendering engine, including colors, layers,
//! sprites, and basic TUI widgets.

pub mod assets;
pub mod camera;
pub mod color;
pub mod dialogue;
pub mod grid;
pub mod layer;
pub mod layout;
pub mod math;
pub mod palette;
pub mod sprite;
pub mod vfx;
pub mod viewport;
pub mod widgets;

// Re-exports for convenience
#[cfg(feature = "image")]
pub use assets::{image_to_grid, image_to_grid_with_chroma_key};
pub use assets::{VisualAsset, VisualRegistry};
pub use camera::Camera;
pub use color::{BlendMode, Color};
pub use dialogue::{DialogueBox, DialogueState, DialogueTheme};
pub use grid::{
    draw_sparkline, wrap_text, write_wrapped_text, Cell, CellAttrs, CellChange, Grid,
    RichTextSegment,
};
pub use layer::{Layer, Layers};
pub use layout::{Alignment, BorderStyle, Constraint, Layout, Rect};
pub use math::easing;
pub use palette::ColorPalette;
pub use sprite::{Frame, ResolutionTier, Sprite, SpriteSheet};
pub use vfx::{
    blend_color, emit_bloom, emit_burst, emit_fire, emit_heal, emit_ice, emit_lightning,
    emit_shatter, emit_shockwave, emit_slash, AoeRing, EasingMode, Flash, FloatingText, Particle,
    ScreenShake, SpatialHighlight, Trajectory, VfxEmitter, VfxSystem,
};
pub use viewport::TileViewport;
pub use widgets::{
    MenuView, MessageLogView, Panel, PerformanceOverlay, ProgressBar, Tooltip, VerticalProgressBar,
};
