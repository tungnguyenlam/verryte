use crate::action::Action;
use crate::components::{GameState, Outcome, Position, Stats, Team, CharacterClass, GameEvent, BattleStats};
use crate::map::TacticalMap;
use crate::snapshot::ActionOutcome;
use verryte_core::{Events, GameClock, MessageLog, Schedule, World};
use verryte_input::InputRouter;
use verryte_terminal::{Camera, Color, Grid, VisualRegistry};

pub mod actions;
pub mod init;
pub mod render;

pub fn saves_dir() -> &'static str {
    let dir = if std::path::Path::new("prototype/wuthering-terminal").exists() {
        "prototype/wuthering-terminal/saves"
    } else {
        "saves"
    };
    let _ = std::fs::create_dir_all(dir);
    dir
}

#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    Empty,
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::Empty => write!(f, "map is empty"),
        }
    }
}

impl std::error::Error for MapError {}

pub struct Game {
    pub world: World,
    pub schedule: Schedule,
    pub router: InputRouter<Action>,
    pub camera: Camera,
    pub camera_locked: bool,
    pub last_outcome: ActionOutcome,
    /// Set to true by `check_boss_phase_transition` when the boss crossed into
    /// phase 2 during this step. Reset by `apply_action` at the start of each step.
    pub boss_transitioned: bool,
    pub _audio_stream: Option<verryte_audio::OutputStream>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn vfx(&self) -> &verryte_terminal::vfx::VfxSystem {
        self.world
            .resource::<verryte_terminal::vfx::VfxSystem>()
            .expect("VfxSystem resource must be registered")
    }

    pub fn vfx_mut(&mut self) -> &mut verryte_terminal::vfx::VfxSystem {
        self.world
            .resource_mut::<verryte_terminal::vfx::VfxSystem>()
            .expect("VfxSystem resource must be registered")
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        if let Some(log) = self.world.resource_mut::<MessageLog>() {
            log.push(msg);
        }
    }

    pub fn get_class_name(class: CharacterClass) -> &'static str {
        match class {
            CharacterClass::Warrior => "Kael",
            CharacterClass::Mage => "Lyra",
            CharacterClass::Healer => "Mira",
            CharacterClass::Rogue => "Jax",
            CharacterClass::Boss => "Blight Sovereign",
            CharacterClass::ShadowStalker => "Shadow Stalker",
            CharacterClass::CorruptedSpore => "Corrupted Spore",
            CharacterClass::CursedSentinel => "Cursed Sentinel",
            CharacterClass::PlagueWraith => "Plague Wraith",
            CharacterClass::GlacialGolem => "Glacial Golem",
            CharacterClass::EnemyCleric => "Dark Cleric",
            CharacterClass::VoidTerror => "Void Terror",
            CharacterClass::Berserker => "Berserker",
            CharacterClass::Tactician => "Tactician",
            CharacterClass::Summoner => "Summoner",
            CharacterClass::Assassin => "Assassin",
            CharacterClass::EliteBerserker => "Elite Berserker",
            CharacterClass::EliteTactician => "Elite Tactician",
            CharacterClass::EliteSummoner => "Elite Summoner",
            CharacterClass::EliteAssassin => "Elite Assassin",
            CharacterClass::DestructibleObject => "Object",
        }
    }

    pub fn get_entity_at(&self, pos: Position) -> Option<(verryte_core::Entity, Team, Stats, CharacterClass)> {
        for (e, p, team) in self.world.query2::<Position, Team>() {
            if *p == pos {
                let stats = self.world.get::<Stats>(e)?.clone();
                let class = *self.world.get::<CharacterClass>(e)?;
                return Some((e, *team, stats, class));
            }
        }
        None
    }

    pub fn is_occupied_except(&self, pos: Position, except: verryte_core::Entity) -> bool {
        for (e, p) in self.world.query::<Position>() {
            if e != except && *p == pos && self.world.get::<Team>(e).is_some() {
                return true;
            }
        }
        false
    }

    pub fn get_tile_dimensions(&self) -> (u16, u16) {
        let (term_w, term_h) = verryte_tty::terminal_size();
        let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
        tier.tile_dimensions()
    }

    pub fn get_tile_center_pixels(&self, pos: Position) -> (f32, f32) {
        let (tile_w, tile_h) = self.get_tile_dimensions();
        let cx = pos.x as f32 * tile_w as f32 + (tile_w as f32 / 2.0);
        let cy = pos.y as f32 * tile_h as f32 + (tile_h as f32 / 2.0);
        (cx, cy)
    }

    pub fn play_spatial_sfx(&mut self, name: &str, emitter_pos: Position) {
        let listener = self
            .world
            .resource::<GameState>()
            .map(|s| s.cursor)
            .unwrap_or(Position::new(12, 8));
        let dx = (emitter_pos.x - listener.x) as f32;
        let dy = (emitter_pos.y - listener.y) as f32;
        let dist = (dx * dx + dy * dy).sqrt();
        let max_range = 15.0_f32;
        let volume = (1.0 - (dist / max_range)).clamp(0.0, 1.0);
        let pan = if max_range > 0.0 {
            (dx / max_range).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        if let Some(events) = self
            .world
            .resource_mut::<Events<verryte_core::AudioEvent>>()
        {
            events.send(
                verryte_core::AudioEvent::play(name)
                    .with_volume(volume)
                    .with_pan(pan),
            );
        }
    }

    pub fn outcome(&self) -> Outcome {
        self.world.resource::<GameState>().unwrap().outcome
    }

    pub fn take_events(&mut self) -> Vec<GameEvent> {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .unwrap()
            .drain()
            .collect()
    }

    pub fn boss_phase(&self) -> crate::components::BossPhase {
        self.world
            .resource::<GameState>()
            .map(|s| s.boss_phase)
            .unwrap_or(crate::components::BossPhase::Phase1)
    }

    pub fn boss_just_transitioned(&self) -> bool {
        self.boss_transitioned
    }

    pub fn diagnostics(&self) -> crate::snapshot::GameDiagnostics {
        let state = self.world.resource::<GameState>().unwrap();
        let mut alive = 0usize;
        let mut dead = 0usize;
        let mut characters = Vec::new();

        for (e, _team, stats, class) in self.world.query3::<Team, Stats, CharacterClass>() {
            let name = Self::get_class_name(*class).to_string();
            let status = self
                .world
                .get::<crate::components::ElementalStatus>(e)
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "None".to_string());
            let is_alive = stats.hp > 0;
            if is_alive {
                alive += 1;
            } else {
                dead += 1;
            }
            let prestige_str = self
                .world
                .get::<crate::components::PrestigeProgress>(e)
                .map(|p| format!("{:?}", p.class))
                .unwrap_or_default();
            let morale_val = self
                .world
                .get::<crate::components::Morale>(e)
                .map(|m| m.value)
                .unwrap_or(70);
            let morale_state_str = self
                .world
                .get::<crate::components::Morale>(e)
                .map(|m| {
                    crate::components::MoraleState::from_morale(m.value)
                        .display_name()
                        .to_string()
                })
                .unwrap_or_else(|| "Steady".to_string());
            let fatigue_val = self
                .world
                .get::<crate::components::Fatigue>(e)
                .map(|f| f.value)
                .unwrap_or(0);
            characters.push(crate::snapshot::CharacterDiag {
                name,
                hp: stats.hp,
                max_hp: stats.max_hp,
                ap: stats.ap,
                max_ap: stats.max_ap,
                status,
                alive: is_alive,
                prestige: prestige_str,
                morale: morale_val,
                morale_state: morale_state_str,
                fatigue: fatigue_val,
            });
        }

        let weather = self
            .world
            .resource::<crate::components::Weather>()
            .map(|w| format!("{:?}", w.current))
            .unwrap_or_else(|| "Sunny".to_string());

        crate::snapshot::GameDiagnostics {
            alive_entities: alive,
            dead_entities: dead,
            current_phase: state.phase,
            weather,
            floor: state.floor,
            turn: state.turn,
            characters,
            combo_count: state.combo_count,
            concert_energy: state.concert_energy,
        }
    }

    pub fn save_state(&self) -> Result<String, serde_json::Error> {
        let registry = crate::snapshot::create_registry();
        let snapshot = registry.snapshot(&self.world);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();
        let state = crate::snapshot::FullSaveState {
            magic: "VERRYTE_SAVE".to_string(),
            version: crate::snapshot::CURRENT_SAVE_VERSION,
            timestamp,
            world: snapshot,
            migrations_applied: Vec::new(),
        };

        serde_json::to_string(&state)
    }

    pub fn load_state(&mut self, state_str: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut state: crate::snapshot::FullSaveState = serde_json::from_str(state_str)?;

        if state.magic != "VERRYTE_SAVE" {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid magic signature in save state",
            )));
        }
        if state.version > crate::snapshot::CURRENT_SAVE_VERSION {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Unsupported save state version: {} (max supported: {})",
                    state.version,
                    crate::snapshot::CURRENT_SAVE_VERSION
                ),
            )));
        }

        // Apply migrations for older save versions.
        if state.version < crate::snapshot::CURRENT_SAVE_VERSION {
            state = Self::migrate_save_state(state)?;
        }

        let registry = crate::snapshot::create_registry();
        registry.apply(&mut self.world, state.world)?;

        // Re-insert non-snapshotted resources
        let mut asset_registry = verryte_terminal::assets::VisualRegistry::new();
        crate::generated_assets::register_assets(&mut asset_registry);
        self.world.insert_resource(asset_registry);

        if self.world.resource::<Events<GameEvent>>().is_none() {
            self.world
                .insert_resource(Events::<GameEvent>::with_capacity(16));
        }
        if self
            .world
            .resource::<Events<verryte_core::AudioEvent>>()
            .is_none()
        {
            self.world
                .insert_resource(Events::<verryte_core::AudioEvent>::new());
        }
        if self
            .world
            .resource::<crate::components::UndoStack>()
            .is_none()
        {
            self.world
                .insert_resource(crate::components::UndoStack::default());
        }
        if self
            .world
            .resource::<crate::components::RedoStack>()
            .is_none()
        {
            self.world
                .insert_resource(crate::components::RedoStack::default());
        }
        if self
            .world
            .resource::<crate::components::Weather>()
            .is_none()
        {
            self.world
                .insert_resource(crate::components::Weather::default());
        }
        if self
            .world
            .resource::<crate::components::Bestiary>()
            .is_none()
        {
            self.world.insert_resource(Self::create_initial_bestiary());
        }
        if self
            .world
            .resource::<crate::components::LoreJournal>()
            .is_none()
        {
            self.world
                .insert_resource(Self::create_initial_lore_journal());
        }

        // Sync camera from resource
        if let Some(camera) = self.world.resource::<verryte_terminal::Camera>() {
            self.camera = camera.clone();
        }

        Ok(())
    }

    fn migrate_save_state(
        mut state: crate::snapshot::FullSaveState,
    ) -> Result<crate::snapshot::FullSaveState, Box<dyn std::error::Error>> {
        if state.version == 1 {
            state
                .migrations_applied
                .push("v1_to_v2: added migrations_applied tracking".to_string());
            state.version = 2;
        }
        Ok(state)
    }

    pub fn save<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.save_state()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        self.load_state(&data)?;
        Ok(())
    }

    pub fn handle_mouse_click(
        &mut self,
        term_w: u16,
        term_h: u16,
        mouse_x: u16,
        mouse_y: u16,
    ) -> bool {
        let (shake_x, shake_y) =
            if let Some(vfx) = self.world.resource::<verryte_terminal::vfx::VfxSystem>() {
                vfx.shake_offset()
            } else {
                (0, 0)
            };
        let hud_h = 6;
        let board_h = term_h.saturating_sub(hud_h);

        let rx = mouse_x as i32 - shake_x as i32;
        let ry = mouse_y as i32 - shake_y as i32;
        if rx >= 0 && ry >= 0 && ry < board_h as i32 && rx < term_w as i32 {
            let map = self.world.resource::<TacticalMap>().unwrap();
            let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
            let (tile_w, tile_h) = tier.tile_dimensions();
            let mut viewport = verryte_terminal::TileViewport::new(
                verryte_terminal::Rect::new(0, 0, term_w, board_h),
                tile_w,
                tile_h,
            );
            viewport.camera = self.camera.clone();
            viewport.camera.clamp_to_bounds(
                0.0,
                0.0,
                map.width as f32 * tile_w as f32,
                map.height as f32 * tile_h as f32,
                viewport.rect.width,
                viewport.rect.height,
            );
            let (tx, ty) = viewport.screen_to_world(rx, ry);
            let tx = tx.floor() as i32;
            let ty = ty.floor() as i32;
            if tx >= 0 && tx < map.width as i32 && ty >= 0 && ty < map.height as i32 {
                let point = verryte_map::Point::new(tx as i16, ty as i16);
                self.apply_action(Action::Inspect(point), verryte_input::ActionSource::Terminal);
                return true;
            }
        }
        false
    }

    pub fn run_pending_reports(&mut self) -> Vec<crate::snapshot::StepReport> {
        let mut reports = Vec::new();
        while let Some(action) = self.router.pop_action() {
            reports.push(self.apply_action(action.action, action.source));
        }
        reports
    }
}
