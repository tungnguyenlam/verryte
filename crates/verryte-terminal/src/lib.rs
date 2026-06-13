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
pub use dialogue::{DialogueBox, DialogueState, DialogueTheme, Portrait, PortraitAnimation};
pub use grid::{
    draw_sparkline, wrap_text, write_wrapped_text, Cell, CellAttrs, CellChange, FowVisibility,
    Grid, RichTextSegment, SvgOptions,
};
pub use layer::{Layer, Layers};
pub use layout::{Alignment, BorderStyle, Constraint, GridLayout, Layout, Rect};
pub use math::easing;
pub use palette::ColorPalette;
pub use sprite::{Frame, ResolutionTier, Sprite, SpriteSheet};
pub use vfx::{
    blend_color, emit_bloom, emit_burst, emit_fire, emit_heal, emit_homeward, emit_ice,
    emit_lightning, emit_shatter, emit_shockwave, emit_slash, AoeRing, Aura, EasingMode, Flash,
    FloatingText, Particle, RingPulse, ScreenShake, SpatialHighlight, Trail, Trajectory,
    VfxEmitter, VfxSystem,
};
pub use viewport::TileViewport;
pub use widgets::{
    Button, MenuView, MessageLogView, Panel, PerformanceOverlay, ProgressBar, Table, Tooltip,
    VerticalProgressBar,
};

/// Register built-in terminal resources for snapshotting.
#[cfg(feature = "serde")]
pub fn register_terminal_resources(reg: &mut verryte_core::snapshot::WorldRegistry) {
    reg.register_resource::<Camera>("Camera");
    // VFX system can be large and contain transient state, but the base system is serializable
    // reg.register_resource::<VfxSystem>("VfxSystem");
}
