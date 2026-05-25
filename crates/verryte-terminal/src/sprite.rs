use crate::grid::Grid;

/// Visual fidelity tiers for adaptive resolution rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResolutionTier {
    #[default]
    TINY, // 6x8
    SMALL,  // 8x12
    MEDIUM, // 12x16
    LARGE,  // 16x20
    XLARGE, // 20x24
    ULTRA,  // 28x32
}

impl ResolutionTier {
    pub const ALL: [ResolutionTier; 6] = [
        ResolutionTier::TINY,
        ResolutionTier::SMALL,
        ResolutionTier::MEDIUM,
        ResolutionTier::LARGE,
        ResolutionTier::XLARGE,
        ResolutionTier::ULTRA,
    ];

    pub fn from_size(width: u16, height: u16) -> Self {
        if width >= 160 && height >= 48 {
            ResolutionTier::ULTRA
        } else if width >= 140 && height >= 42 {
            ResolutionTier::XLARGE
        } else if width >= 120 && height >= 36 {
            ResolutionTier::LARGE
        } else if width >= 100 && height >= 30 {
            ResolutionTier::MEDIUM
        } else if width >= 80 && height >= 24 {
            ResolutionTier::SMALL
        } else {
            ResolutionTier::TINY
        }
    }

    /// Recommended tile dimensions (width, height) for this resolution tier.
    pub fn tile_dimensions(&self) -> (u16, u16) {
        match self {
            Self::TINY => (6, 3),
            Self::SMALL => (8, 4),
            Self::MEDIUM => (12, 6),
            Self::LARGE => (16, 8),
            Self::XLARGE => (20, 10),
            Self::ULTRA => (28, 14),
        }
    }

    pub fn sprite_size(self) -> (u16, u16) {
        match self {
            ResolutionTier::TINY => (6, 4),
            ResolutionTier::SMALL => (8, 6),
            ResolutionTier::MEDIUM => (12, 8),
            ResolutionTier::LARGE => (16, 10),
            ResolutionTier::XLARGE => (20, 12),
            ResolutionTier::ULTRA => (28, 16),
        }
    }
}

/// A single animation frame: a [`Grid`] with an associated display duration.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame {
    pub grid: Grid,
    pub duration: u32,
}

impl Frame {
    pub fn new(grid: Grid, duration: u32) -> Self {
        Self { grid, duration }
    }
}

/// A named sequence of [`Frame`]s for terminal animation.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sprite {
    pub name: String,
    tiers: std::collections::BTreeMap<ResolutionTier, Vec<Frame>>,
    current_tier: ResolutionTier,
    current_frame: usize,
    elapsed: u32,
    paused: bool,
}

impl Sprite {
    pub fn new<S: Into<String>>(name: S, frames: Vec<Frame>) -> Self {
        let mut tiers = std::collections::BTreeMap::new();
        tiers.insert(ResolutionTier::default(), frames);
        Self {
            name: name.into(),
            tiers,
            current_tier: ResolutionTier::default(),
            current_frame: 0,
            elapsed: 0,
            paused: false,
        }
    }

    pub fn with_tier(mut self, tier: ResolutionTier, frames: Vec<Frame>) -> Self {
        self.tiers.insert(tier, frames);
        self
    }

    pub fn set_tier(&mut self, tier: ResolutionTier) {
        if self.tiers.contains_key(&tier) {
            self.current_tier = tier;
        } else {
            if let Some((&t, _)) = self.tiers.range(..=tier).next_back() {
                self.current_tier = t;
            } else if let Some((&t, _)) = self.tiers.range(tier..).next() {
                self.current_tier = t;
            }
        }
        if let Some(frames) = self.tiers.get(&self.current_tier) {
            if self.current_frame >= frames.len() {
                self.current_frame = 0;
                self.elapsed = 0;
            }
        }
    }

    pub fn current_frame_at(&self, tier: ResolutionTier) -> &Grid {
        let actual_tier = if self.tiers.contains_key(&tier) {
            tier
        } else if let Some((&t, _)) = self.tiers.range(..=tier).next_back() {
            t
        } else if let Some((&t, _)) = self.tiers.range(tier..).next() {
            t
        } else {
            panic!("Sprite has no tiers");
        };
        let frames = self.tiers.get(&actual_tier).expect("tier must exist");
        &frames[self.current_frame.min(frames.len() - 1)].grid
    }

    pub fn frame_at(&self, tier: ResolutionTier, index: usize) -> Option<&Grid> {
        let actual_tier = if self.tiers.contains_key(&tier) {
            Some(tier)
        } else if let Some((&t, _)) = self.tiers.range(..=tier).next_back() {
            Some(t)
        } else if let Some((&t, _)) = self.tiers.range(tier..).next() {
            Some(t)
        } else {
            None
        }?;
        let frames = self.tiers.get(&actual_tier)?;
        frames.get(index).map(|f| &f.grid)
    }

    pub fn tick(&mut self) -> bool {
        if self.paused {
            return false;
        }
        let Some(frames) = self.tiers.get(&self.current_tier) else {
            return false;
        };
        if frames.is_empty() {
            return false;
        }

        self.elapsed += 1;
        if self.elapsed >= frames[self.current_frame].duration {
            self.elapsed = 0;
            self.current_frame = (self.current_frame + 1) % frames.len();
            true
        } else {
            false
        }
    }

    pub fn current_frame(&self) -> &Grid {
        let frames = self.tiers.get(&self.current_tier).expect("tier must exist");
        &frames[self.current_frame].grid
    }

    pub fn current_index(&self) -> usize {
        self.current_frame
    }

    pub fn frame_count(&self) -> usize {
        self.tiers
            .get(&self.current_tier)
            .map(|f| f.len())
            .unwrap_or(0)
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.elapsed = 0;
    }

    pub fn set_frame(&mut self, index: usize) {
        if let Some(frames) = self.tiers.get(&self.current_tier) {
            if !frames.is_empty() {
                self.current_frame = index.min(frames.len() - 1);
                self.elapsed = 0;
            }
        }
    }
}

/// A collection of named [`Sprite`]s indexed by name.
#[derive(Clone, Debug, Default)]
pub struct SpriteSheet {
    sprites: Vec<Sprite>,
}

impl SpriteSheet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, sprite: Sprite) {
        if let Some(pos) = self.sprites.iter().position(|s| s.name == sprite.name) {
            self.sprites[pos] = sprite;
        } else {
            self.sprites.push(sprite);
        }
    }

    pub fn get(&self, name: &str) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.name == name)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.sprites.iter().position(|s| s.name == name) {
            self.sprites.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn tick_all(&mut self) {
        for sprite in &mut self.sprites {
            sprite.tick();
        }
    }

    pub fn reset_all(&mut self) {
        for sprite in &mut self.sprites {
            sprite.reset();
        }
    }

    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Sprite> {
        self.sprites.iter()
    }
}
