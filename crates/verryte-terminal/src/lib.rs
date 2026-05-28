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
pub use assets::{image_to_grid, image_to_grid_with_chroma_key, VisualAsset, VisualRegistry};
pub use camera::Camera;
pub use color::{BlendMode, Color};
pub use dialogue::{DialogueBox, DialogueState, DialogueTheme};
pub use grid::{wrap_text, write_wrapped_text, Cell, CellAttrs, CellChange, Grid, RichTextSegment};
pub use layer::{Layer, Layers};
pub use layout::{Alignment, BorderStyle, Constraint, Layout, Rect};
pub use math::easing;
pub use palette::ColorPalette;
pub use sprite::{Frame, ResolutionTier, Sprite, SpriteSheet};
pub use vfx::{EasingMode, Flash, Particle, Trajectory, VfxEmitter};
pub use viewport::TileViewport;
pub use widgets::{MenuView, MessageLogView, PerformanceOverlay, ProgressBar};
