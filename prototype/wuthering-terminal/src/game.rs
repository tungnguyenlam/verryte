use crate::action::{default_bindings, Action};
use crate::components::{
    BattleStats, CharacterClass, EquippedItems, GameEvent, GameState, Outcome, Position, Stats,
    Team, TurnPhase,
};
use crate::map::{TacticalMap, Tile};
use crate::snapshot::ActionOutcome;
use crate::spawn::Spawner;
use std::collections::HashSet;
use verryte_core::{Entity, Events, GameClock, MessageLog, Rng, Schedule, World};
use verryte_input::{ActionSource, InputRouter};
use verryte_terminal::{Camera, Cell, Color, Grid, VisualRegistry};

fn saves_dir() -> &'static str {
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
    pub fn new() -> Self {
        let mut world = World::new();
        let width = 24;
        let height = 16;
        let map = TacticalMap::tactical();

        world.insert_resource(map);
        world.insert_resource(GameState {
            turn: 1,
            phase: TurnPhase::Player,
            outcome: Outcome::Playing,
            cursor: Position::new(5, 5),
            selected_entity: None,
            concert_energy: 0,
            targeting: crate::components::TargetingMode::None,
            boss_phase: crate::components::BossPhase::Phase1,
            ui_state: crate::components::UIState::Normal,
            show_perf: false,
            auto_battle: false,
            is_recording: false,
            show_minimap: true,
            combo_count: 0,
            floor: 1,
        });
        world.insert_resource(crate::components::TelegraphZone::default());
        world.insert_resource(verryte_map::VisibilityMap::new(width, height));
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(1));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(Events::<verryte_core::AudioEvent>::new());
        world.insert_resource(MessageLog::with_max(50));
        world.insert_resource(verryte_input::ActionHistory::<Action>::default());
        world.insert_resource(verryte_terminal::vfx::VfxSystem::new());
        world.insert_resource(verryte_terminal::DialogueState::new("Narrative", ""));
        world.insert_resource(crate::components::EquippedEchoes::default());
        world.insert_resource(crate::components::TurnTransition::default());
        world.insert_resource(crate::components::ReplayState::default());
        world.insert_resource(crate::components::BossConfig::default());
        world.insert_resource(verryte_core::Diagnostics::new());
        world.insert_resource(BattleStats::default());
        world.insert_resource(crate::components::UndoStack::default());
        world.insert_resource(crate::components::RedoStack::default());
        world.insert_resource(crate::components::Weather::default());
        world.insert_resource(crate::components::AvailableCombos::default());
        world.insert_resource(crate::components::ActiveFloorModifiers::default());
        world.insert_resource(Self::create_initial_bestiary());
        world.insert_resource(Self::create_initial_lore_journal());

        let mut registry = VisualRegistry::new();
        crate::generated_assets::register_assets(&mut registry);
        world.insert_resource(registry);

        let mut schedule = Schedule::new();
        schedule.add_named("visibility", crate::systems::visibility_system);
        schedule.add_named("turn_management", crate::systems::turn_management_system);
        schedule.add_named("floor_modifier", crate::systems::floor_modifier_system);
        schedule.add_named("enemy_ai", crate::systems::enemy_ai_system);
        schedule.add_named("weather_cycle", crate::systems::weather_cycle_system);
        schedule.add_named("combo_detection", crate::systems::combo_detection_system);
        schedule.add_named("prestige", crate::systems::prestige_system);
        schedule.add_named("morale_fatigue", crate::systems::morale_fatigue_system);
        schedule.add_named("weather_ambient", crate::systems::weather_ambient_system);

        let mut audio_stream = None;
        if let Ok((mut player, stream)) = verryte_audio::AudioPlayer::try_new() {
            player.register("music_theme", vec![0; 100]);
            player.register("warrior_attack", vec![0; 100]);
            player.register("mage_attack", vec![0; 100]);
            player.register("healer_attack", vec![0; 100]);
            player.register("boss_attack", vec![0; 100]);
            player.register("enemy_attack", vec![0; 100]);
            player.register("swap_swoosh", vec![0; 100]);
            player.register("item_craft", vec![0; 100]);
            player.register("level_up", vec![0; 100]);
            player.register("boss_phase_transition", vec![0; 100]);
            player.register("dialogue_blip", vec![0; 100]);
            player.register("ambient_rain", vec![0; 100]);
            player.register("ambient_thunder", vec![0; 100]);
            player.register("ambient_birds", vec![0; 100]);
            player.register("ambient_wind", vec![0; 100]);

            player.play_music("music_theme", true);

            world.insert_resource(player);
            audio_stream = Some(stream);
        }

        let mut game = Self {
            world,
            schedule,
            router: InputRouter::new(default_bindings()),
            camera: Camera::new(0.0, 0.0).with_smooth(0.15),
            last_outcome: ActionOutcome::NoOp,
            boss_transitioned: false,
            _audio_stream: audio_stream,
        };
        let (cx, cy) = game.get_tile_center_pixels(Position::new(5, 5));
        game.camera.center_x = cx;
        game.camera.center_y = cy;
        game.camera.target_x = cx;
        game.camera.target_y = cy;

        game.world
            .spawn_character(Position::new(4, 4), Team::Player, CharacterClass::Warrior);
        game.world
            .spawn_character(Position::new(4, 8), Team::Player, CharacterClass::Mage);
        game.world
            .spawn_character(Position::new(4, 12), Team::Player, CharacterClass::Healer);
        game.world
            .spawn_character(Position::new(18, 8), Team::Enemy, CharacterClass::Boss);
        game.world.spawn_character(
            Position::new(14, 4),
            Team::Enemy,
            CharacterClass::ShadowStalker,
        );
        game.world.spawn_character(
            Position::new(14, 12),
            Team::Enemy,
            CharacterClass::ShadowStalker,
        );
        game.world.spawn_character(
            Position::new(12, 6),
            Team::Enemy,
            CharacterClass::CorruptedSpore,
        );
        game.world.spawn_character(
            Position::new(12, 10),
            Team::Enemy,
            CharacterClass::CorruptedSpore,
        );
        game.world.spawn_character(
            Position::new(16, 3),
            Team::Enemy,
            CharacterClass::CursedSentinel,
        );
        game.world.spawn_character(
            Position::new(10, 14),
            Team::Enemy,
            CharacterClass::PlagueWraith,
        );
        game.world.spawn_character(
            Position::new(15, 8),
            Team::Enemy,
            CharacterClass::EnemyCleric,
        );

        let potion = game
            .world
            .spawn_item("Healing Potion", crate::components::ItemEffect::Heal(30));
        let elixir = game.world.spawn_item(
            "Energy Elixir",
            crate::components::ItemEffect::ReplenishAp(2),
        );
        let remedy = game
            .world
            .spawn_item("Cleanse Remedy", crate::components::ItemEffect::Cleanse);
        let aegis = game.world.spawn_item(
            "Aegis Elixir",
            crate::components::ItemEffect::RestoreShield(
                crate::components::ShieldType::Physical,
                30,
            ),
        );

        if let Some(kael) = game
            .world
            .query3::<Position, Team, CharacterClass>()
            .iter()
            .find(|(_, _, team, class)| {
                **team == Team::Player && **class == CharacterClass::Warrior
            })
            .map(|(e, _, _, _)| *e)
        {
            if let Some(inv) = game.world.get_mut::<crate::components::Inventory>(kael) {
                inv.items.push(potion);
                inv.items.push(elixir);
                inv.items.push(aegis);
            }
        }
        if let Some(mira) = game
            .world
            .query3::<Position, Team, CharacterClass>()
            .iter()
            .find(|(_, _, team, class)| **team == Team::Player && **class == CharacterClass::Healer)
            .map(|(e, _, _, _)| *e)
        {
            if let Some(inv) = game.world.get_mut::<crate::components::Inventory>(mira) {
                inv.items.push(remedy);
            }
        }

        game.log("Wuthering Terminal Tactical RPG Initialized.");
        game.log("Move cursor: Arrows/WASD. Confirm: Enter. Cancel: Esc.");
        game.log("End Turn: E. Cycle: Tab. Bestiary/Lore: J.");

        game
    }

    fn create_initial_bestiary() -> crate::components::Bestiary {
        use crate::components::BestiaryEntry;
        crate::components::Bestiary {
            entries: vec![
                BestiaryEntry { class: CharacterClass::ShadowStalker, name: "Shadow Stalker".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Created from the shadows of the Sovereign's domain, these silent hunters vanish when struck. Area attacks are the only reliable way to pin them down.".to_string(), drop_table: vec!["Frostbite Echo".to_string(), "Shadow Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::CorruptedSpore, name: "Corrupted Spore".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Once benign forest spores, corrupted by the Blight into volatile living mines. They explode on proximity, dealing devastating area damage.".to_string(), drop_table: vec!["Nature Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::CursedSentinel, name: "Cursed Sentinel".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Ancient guardians turned to serve darkness. They prefer ranged attacks and retreat when approached.".to_string(), drop_table: vec!["Sentinel Core".to_string()] },
                BestiaryEntry { class: CharacterClass::PlagueWraith, name: "Plague Wraith".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Spirits of plague victims, bound to spread decay. Their touch applies Nature status.".to_string(), drop_table: vec!["Wraith Shard".to_string(), "Plague Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::GlacialGolem, name: "Glacial Golem".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Forged from eternal ice in the Sovereign's forge. Attacks apply Ice status, freezing victims in place.".to_string(), drop_table: vec!["Frost Core".to_string(), "Ice Walker Echo".to_string()] },
                BestiaryEntry { class: CharacterClass::EnemyCleric, name: "Dark Cleric".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Fallen healers who chose darkness over light. They prioritize healing wounded allies.".to_string(), drop_table: vec!["Dark Blessing".to_string()] },
                BestiaryEntry { class: CharacterClass::Boss, name: "Blight Sovereign".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "The source of all corruption. A multi-phase abomination. In Phase 2, it unleashes Celestial Ruin with telegraphed attacks that can be parried.".to_string(), drop_table: vec!["Sovereign's Echo".to_string(), "Blight Crystal".to_string()] },
            ],
        }
    }

    fn create_initial_lore_journal() -> crate::components::LoreJournal {
        use crate::components::{LoreCategory, LoreEntry};
        crate::components::LoreJournal {
            entries: vec![
                LoreEntry { id: "the_blight".to_string(), title: "The Blight".to_string(), text: "A creeping corruption from the Sovereign's domain, twisting living things into servants of darkness.".to_string(), category: LoreCategory::World, discovered: true, turn_discovered: 0 },
                LoreEntry { id: "concert_energy".to_string(), title: "Concert Energy".to_string(), text: "A mystical force that builds as heroes fight. At 100, enables QTE Swaps with devastating intro skills.".to_string(), category: LoreCategory::Mechanic, discovered: true, turn_discovered: 0 },
                LoreEntry { id: "echo_absorption".to_string(), title: "Echo Absorption".to_string(), text: "Defeated enemies leave Echoes. Absorb to gain Swift, Thorns, Frostbite, Stun, or Lifesteal.".to_string(), category: LoreCategory::Mechanic, discovered: true, turn_discovered: 0 },
                LoreEntry { id: "kael_origins".to_string(), title: "Kael's Origins".to_string(), text: "A sellsword whose homeland fell to the Blight. Swift Foot grants bonus AP each turn.".to_string(), category: LoreCategory::Character, discovered: true, turn_discovered: 0 },
                LoreEntry { id: "lyra_studies".to_string(), title: "Lyra's Studies".to_string(), text: "Arcane Academy prodigy pursuing the Blight's source. Storm Chaser amplifies lightning damage.".to_string(), category: LoreCategory::Character, discovered: true, turn_discovered: 0 },
                LoreEntry { id: "mira_calling".to_string(), title: "Mira's Calling".to_string(), text: "Temple healer who answered the cries of the afflicted. Purifying Touch cleanses on heal.".to_string(), category: LoreCategory::Character, discovered: true, turn_discovered: 0 },
                LoreEntry { id: "combat_insights".to_string(), title: "Combat Insights".to_string(), text: "Ice+Lightning=Shatter, Lightning+Nature=Overgrowth, Nature+Ice=Bloom. Combos amplify damage by 5% per chain.".to_string(), category: LoreCategory::Mechanic, discovered: false, turn_discovered: 0 },
                LoreEntry { id: "sovereigns_rage".to_string(), title: "The Sovereign's Rage".to_string(), text: "Below half health, the Sovereign enters frenzy. Power doubles, shield manifests, Celestial Ruin begins.".to_string(), category: LoreCategory::Enemy, discovered: false, turn_discovered: 0 },
                LoreEntry { id: "descent_into_darkness".to_string(), title: "Descent into Darkness".to_string(), text: "Floor 2 is a procedurally generated dungeon warped by the Blight. The Sovereign awaits in the deepest chamber.".to_string(), category: LoreCategory::World, discovered: false, turn_discovered: 0 },
                LoreEntry { id: "echo_lore".to_string(), title: "Echo Lore".to_string(), text: "Echoes are the crystallized will of the fallen. Each absorbed Echo reshapes the hero's soul.".to_string(), category: LoreCategory::Mechanic, discovered: false, turn_discovered: 0 },
            ],
        }
    }

    pub fn record_enemy_encounter(&mut self, class: CharacterClass) {
        let name = Self::get_class_name(class);
        let mut discovered = false;
        if let Some(bestiary) = self.world.resource_mut::<crate::components::Bestiary>() {
            for entry in &mut bestiary.entries {
                if entry.class == class && !entry.encountered {
                    entry.encountered = true;
                    discovered = true;
                }
            }
        }
        if discovered {
            self.log(format!(
                "[fg:FFD700]New bestiary entry discovered: {}![/fg]",
                name
            ));
        }
    }

    pub fn record_enemy_defeat(&mut self, class: CharacterClass) {
        let name = Self::get_class_name(class);
        let mut should_reveal = false;
        let mut weakness_str = String::new();
        if let Some(bestiary) = self.world.resource_mut::<crate::components::Bestiary>() {
            for entry in &mut bestiary.entries {
                if entry.class == class {
                    entry.defeated_count += 1;
                    if entry.defeated_count >= 3 && entry.known_weakness.is_none() {
                        let w = match class {
                            CharacterClass::ShadowStalker => {
                                "Area attacks reveal its position".to_string()
                            }
                            CharacterClass::CorruptedSpore => {
                                "Kill at range to avoid explosion".to_string()
                            }
                            CharacterClass::CursedSentinel => {
                                "Close the gap quickly; weak in melee".to_string()
                            }
                            CharacterClass::PlagueWraith => {
                                "Cleanse removes Nature status".to_string()
                            }
                            CharacterClass::GlacialGolem => {
                                "Fire and Lightning deal extra damage".to_string()
                            }
                            CharacterClass::EnemyCleric => {
                                "Focus fire first to stop healing".to_string()
                            }
                            CharacterClass::Boss => "Parry telegraphed attacks to stun".to_string(),
                            _ => "Unknown".to_string(),
                        };
                        entry.known_weakness = Some(w.clone());
                        should_reveal = true;
                        weakness_str = w;
                    }
                }
            }
        }
        if should_reveal {
            self.log(format!(
                "[fg:FFD700]Bestiary: {} weakness -- {}![/fg]",
                name, weakness_str
            ));
            self.unlock_lore("combat_insights");
        }
    }

    pub fn record_enemy_hit_taken(&mut self, class: CharacterClass) {
        let name = Self::get_class_name(class);
        let mut should_reveal = false;
        let mut resistance_str = String::new();
        if let Some(bestiary) = self.world.resource_mut::<crate::components::Bestiary>() {
            for entry in &mut bestiary.entries {
                if entry.class == class {
                    entry.hits_taken += 1;
                    if entry.hits_taken >= 10 && entry.known_resistance.is_none() {
                        let r = match class {
                            CharacterClass::ShadowStalker => {
                                "Resists single-target (vanishes)".to_string()
                            }
                            CharacterClass::CorruptedSpore => "Resists Nature damage".to_string(),
                            CharacterClass::CursedSentinel => "Resists ranged attacks".to_string(),
                            CharacterClass::PlagueWraith => "Resists Poison and Nature".to_string(),
                            CharacterClass::GlacialGolem => "Resists Ice damage".to_string(),
                            CharacterClass::EnemyCleric => {
                                "Resists Holy/Nature debuffs".to_string()
                            }
                            CharacterClass::Boss => "Shield blocks physical in Phase 2".to_string(),
                            _ => "Unknown".to_string(),
                        };
                        entry.known_resistance = Some(r.clone());
                        should_reveal = true;
                        resistance_str = r;
                    }
                }
            }
        }
        if should_reveal {
            self.log(format!(
                "[fg:FFD700]Bestiary: {} resistance -- {}![/fg]",
                name, resistance_str
            ));
        }
    }

    pub fn record_player_defeat_by(&mut self, attacker_class: CharacterClass) {
        if let Some(bestiary) = self.world.resource_mut::<crate::components::Bestiary>() {
            for entry in &mut bestiary.entries {
                if entry.class == attacker_class {
                    entry.times_killed_by += 1;
                }
            }
        }
    }

    pub fn unlock_lore(&mut self, lore_id: &str) {
        let mut newly_unlocked = false;
        let mut title = String::new();
        let turn = self
            .world
            .resource::<GameState>()
            .map(|s| s.turn)
            .unwrap_or(0);
        if let Some(journal) = self.world.resource_mut::<crate::components::LoreJournal>() {
            for entry in &mut journal.entries {
                if entry.id == lore_id && !entry.discovered {
                    entry.discovered = true;
                    entry.turn_discovered = turn;
                    newly_unlocked = true;
                    title = entry.title.clone();
                }
            }
        }
        if newly_unlocked {
            self.log(format!("[fg:9933FF]Lore unlocked: '{}'![/fg]", title));
        }
    }

    pub fn trigger_intro_dialogue(&mut self) {
        if let Some(dialogue) = self.world.resource_mut::<verryte_terminal::DialogueState>() {
            *dialogue = verryte_terminal::DialogueState::new(
                "Tactical Focus",
                "Choose Kael's Vanguard Focus for this battle:",
            )
            .with_choices(vec![
                "Pure Blade (+5 Attack for Kael)".to_string(),
                "Arcane Synergy (Start with +5 Concert Energy)".to_string(),
            ]);
        }
    }

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
            CharacterClass::Boss => "Blight Sovereign",
            CharacterClass::ShadowStalker => "Shadow Stalker",
            CharacterClass::CorruptedSpore => "Corrupted Spore",
            CharacterClass::CursedSentinel => "Cursed Sentinel",
            CharacterClass::PlagueWraith => "Plague Wraith",
            CharacterClass::GlacialGolem => "Glacial Golem",
            CharacterClass::EnemyCleric => "Dark Cleric",
        }
    }

    pub fn get_entity_at(&self, pos: Position) -> Option<(Entity, Team, Stats, CharacterClass)> {
        for (e, p, team) in self.world.query2::<Position, Team>() {
            if *p == pos {
                let stats = self.world.get::<Stats>(e)?.clone();
                let class = *self.world.get::<CharacterClass>(e)?;
                return Some((e, *team, stats, class));
            }
        }
        None
    }

    pub fn is_occupied_except(&self, pos: Position, except: Entity) -> bool {
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

    pub fn resolve_combat_hit(
        &mut self,
        attacker: Entity,
        target: Entity,
        base_damage: i32,
        attacker_name: &str,
        target_name: &str,
        pos: Position,
    ) -> (i32, bool) {
        let attacker_class = self.world.get::<CharacterClass>(attacker).copied();
        if let Some(class) = attacker_class {
            let sfx_name = match class {
                CharacterClass::Warrior => "warrior_attack",
                CharacterClass::Mage => "mage_attack",
                CharacterClass::Healer => "healer_attack",
                CharacterClass::Boss => "boss_attack",
                _ => "enemy_attack",
            };
            self.play_spatial_sfx(sfx_name, pos);
        }

        // Bestiary: record encounter and hit tracking
        if let Some(target_class) = self.world.get::<CharacterClass>(target).copied() {
            if self.world.get::<Team>(target) == Some(&Team::Enemy) {
                self.record_enemy_encounter(target_class);
                self.record_enemy_hit_taken(target_class);
            }
        }

        let equip_atk_bonus = self
            .world
            .get::<EquippedItems>(attacker)
            .map(|e| e.total_atk_bonus())
            .unwrap_or(0);
        let equip_def_bonus = self
            .world
            .get::<EquippedItems>(target)
            .map(|e| e.total_def_bonus())
            .unwrap_or(0);
        let mut boosted_base_damage = base_damage + equip_atk_bonus - equip_def_bonus;
        boosted_base_damage = boosted_base_damage.max(1);
        let mut is_player = false;
        let mut new_combo = 0;

        if self.world.get::<Team>(attacker) == Some(&Team::Player) {
            is_player = true;
            if let Some(state) = self.world.resource_mut::<GameState>() {
                state.combo_count += 1;
                new_combo = state.combo_count;
                let mult = 1.0 + ((new_combo.saturating_sub(1)) as f32 * 0.05);
                boosted_base_damage = (boosted_base_damage as f32 * mult) as i32;
            }
        }

        boosted_base_damage = crate::systems::weather_damage_modifier(
            &self.world,
            boosted_base_damage,
            attacker_name,
        );

        let attacker_morale_state = self
            .world
            .get::<crate::components::Morale>(attacker)
            .map(|m| crate::components::MoraleState::from_morale(m.value))
            .unwrap_or(crate::components::MoraleState::Steady);

        match attacker_morale_state {
            crate::components::MoraleState::Confident => {
                boosted_base_damage = (boosted_base_damage as f32 * 1.10) as i32;
            }
            crate::components::MoraleState::Stressed => {
                boosted_base_damage = (boosted_base_damage as f32 * 0.90) as i32;
            }
            crate::components::MoraleState::Breaking => {
                boosted_base_damage = (boosted_base_damage as f32 * 0.80) as i32;
            }
            _ => {}
        }

        let attacker_fatigue = self
            .world
            .get::<crate::components::Fatigue>(attacker)
            .map(|f| f.value)
            .unwrap_or(0);

        let is_blademaster = self
            .world
            .get::<crate::components::PrestigeProgress>(attacker)
            .is_some_and(|p| {
                p.class == crate::components::PrestigeClass::BladeMaster && p.promoted
            });

        let (is_crit, is_block, damage) = {
            let rng = self.world.resource_mut::<Rng>().unwrap();

            if attacker_morale_state == crate::components::MoraleState::Breaking {
                let fumble_roll = rng.next_u32(100);
                if fumble_roll < 10 {
                    self.log(format!("{} fumbled! (0 damage)", attacker_name));
                    return (0, false);
                }
            }

            let mut crit_threshold: i32 = 20;
            match attacker_morale_state {
                crate::components::MoraleState::Confident => crit_threshold -= 5,
                crate::components::MoraleState::Stressed => crit_threshold += 5,
                _ => {}
            }
            if attacker_fatigue > 50 {
                crit_threshold += attacker_fatigue / 10;
            }
            crit_threshold = crit_threshold.clamp(1, 95);

            let roll = rng.next_u32(100);
            if roll < crit_threshold as u32 {
                let crit_mult = if is_blademaster { 2.0 } else { 1.5 };
                (true, false, (boosted_base_damage as f32 * crit_mult) as i32)
            } else if roll < (crit_threshold + 15) as u32 {
                (false, true, (boosted_base_damage / 2).max(1))
            } else {
                (false, false, boosted_base_damage)
            }
        };

        if attacker_fatigue > 80 {
            let stun_roll = {
                let rng = self.world.resource_mut::<Rng>().unwrap();
                rng.next_u32(100)
            };
            if stun_roll < 5 {
                self.world
                    .insert(attacker, crate::components::Stunned { duration: 1 });
                self.log(format!(
                    "{} is exhausted and became Stunned!",
                    attacker_name
                ));
            }
        }

        let weather = self
            .world
            .resource::<crate::components::Weather>()
            .map(|w| w.current)
            .unwrap_or(crate::components::WeatherType::Sunny);
        let mut lightning_bonus = 0;
        if weather == crate::components::WeatherType::LightningStorm {
            let bonus_roll = {
                let rng = self.world.resource_mut::<Rng>().unwrap();
                rng.next_u32(100)
            };
            if bonus_roll < 25 {
                lightning_bonus = 15;
                self.log("[fg:FFFF64]Lightning strikes from the storm! Bonus 15 damage![/fg]");
            }
        }
        let damage = damage + lightning_bonus;

        let mut defeated = false;
        let mut final_hp = 0;
        if let Some(stats) = self.world.get_mut::<Stats>(target) {
            stats.hp -= damage;
            final_hp = stats.hp;
            if stats.hp <= 0 {
                defeated = true;
            }
        }

        if is_player {
            if let Some(progress) = self
                .world
                .get_mut::<crate::components::PrestigeProgress>(attacker)
            {
                progress.total_damage_dealt += damage;
                if defeated {
                    progress.kill_count += 1;
                }
            }
        }

        let crit_str = if is_crit {
            " (CRITICAL HIT!)"
        } else if is_block {
            " (BLOCKED!)"
        } else {
            ""
        };
        self.log(format!(
            "{} attacked {} for {} damage!{} (Target HP: {})",
            attacker_name, target_name, damage, crit_str, final_hp
        ));

        if is_player && new_combo > 0 {
            self.log(format!(
                "Combo! [fg:FF5555][b]x{} Combo[/][/fg]!",
                new_combo
            ));
            if new_combo % 3 == 0 {
                if let Some(stats) = self.world.get_mut::<Stats>(attacker) {
                    stats.hp = (stats.hp + 5).min(stats.max_hp);
                    self.log(format!("Combo Bonus! Healed {} for 5 HP.", attacker_name));
                }
                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.concert_energy = (state.concert_energy + 10).min(100);
                    self.log("[fg:FFD700]Combo Bonus! Gained +10 Concert Energy.[/fg]".to_string());
                }

                // Combo Milestone VFX and Audio
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(0.3, 1.5));
                self.vfx_mut().trigger_flash(Color(255, 215, 0), 0.2);
                let (tcx, tcy) = self.get_tile_center_pixels(pos);
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        tcx,
                        tcy,
                        18,
                        Color(255, 215, 0),
                        &['*', '✦', '✧'],
                    ));
                if let Some(events) = self
                    .world
                    .resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("combo_milestone"));
                }
            }
        }

        let (tcx, tcy) = self.get_tile_center_pixels(pos);
        let float_text = if is_crit {
            format!("CRIT! -{}", damage)
        } else if is_block {
            format!("BLOCK! -{}", damage)
        } else {
            format!("-{}", damage)
        };

        let float_color = if is_crit {
            Color(255, 215, 0) // Gold
        } else if is_block {
            Color(160, 160, 160) // Grey
        } else {
            Color(255, 50, 50) // Red
        };

        self.vfx_mut()
            .floating_texts
            .push(verryte_terminal::vfx::FloatingText::new(
                tcx,
                tcy - 2.0,
                &float_text,
                float_color,
                is_crit,
            ));

        if is_player && new_combo > 0 {
            self.vfx_mut()
                .floating_texts
                .push(verryte_terminal::vfx::FloatingText::new(
                    tcx - 2.0,
                    tcy - 3.5,
                    &format!("x{} COMBO!", new_combo),
                    Color(255, 120, 50),
                    true,
                ));
        }

        self.vfx_mut()
            .particles
            .extend(verryte_terminal::vfx::emit_slash(
                tcx,
                tcy,
                if is_crit { 2.0 } else { 1.0 },
            ));

        let shake_intensity = if is_crit { 3.5 } else { 1.5 };
        let shake_duration = if is_crit { 0.4 } else { 0.25 };
        self.vfx_mut()
            .shakes
            .push(verryte_terminal::vfx::ScreenShake::new(
                shake_intensity,
                shake_duration,
            ));

        // Play combat sound
        if let Some(events) = self
            .world
            .resource_mut::<verryte_core::Events<verryte_core::AudioEvent>>()
        {
            events.send(verryte_core::AudioEvent::play("slash"));
        }

        // Shadow Stalker Vanish ability
        if !defeated {
            if let Some(class) = self.world.get::<CharacterClass>(target) {
                if *class == CharacterClass::ShadowStalker {
                    self.trigger_vanish(target, pos);
                }
            }
        }

        // --- ECHO ABILITIES (Target) ---
        if !defeated && self.world.get::<Team>(target) == Some(&Team::Player) {
            let has_thorns = {
                let abilities = self
                    .world
                    .resource::<crate::components::EquippedEchoes>()
                    .unwrap();
                abilities
                    .abilities
                    .contains(&crate::components::EchoAbility::Thorns)
            };
            if has_thorns {
                let reflect = (damage as f32 * 0.2) as i32;
                if reflect > 0 {
                    if let Some(atk_stats) = self.world.get_mut::<Stats>(attacker) {
                        atk_stats.hp -= reflect;
                        self.log(format!("Thorns reflected {} damage back!", reflect));
                    }
                }
            }
        }

        // --- ECHO ABILITIES (Attacker) ---
        // We assume the attacker is the selected entity if it's the player's turn
        let phase = self.world.resource::<GameState>().unwrap().phase;
        if phase == TurnPhase::Player {
            if let Some(_sel_ent) = self.world.resource::<GameState>().unwrap().selected_entity {
                let (has_frostbite, has_stun, has_lifesteal) = {
                    let abilities = self
                        .world
                        .resource::<crate::components::EquippedEchoes>()
                        .unwrap();
                    (
                        abilities
                            .abilities
                            .contains(&crate::components::EchoAbility::Frostbite),
                        abilities
                            .abilities
                            .contains(&crate::components::EchoAbility::Stun),
                        abilities
                            .abilities
                            .contains(&crate::components::EchoAbility::Lifesteal),
                    )
                };
                if has_frostbite {
                    let mut apply_ice = false;
                    {
                        let rng = self.world.resource_mut::<Rng>().unwrap();
                        if rng.chance(0.3) {
                            apply_ice = true;
                        }
                    }
                    if apply_ice {
                        self.log("Frostbite triggered! Applying Ice status.");
                        self.apply_elemental_status(
                            target,
                            crate::components::ElementalStatus::Ice { duration: 2 },
                        );
                    }
                }
                if has_stun {
                    let mut apply_stun = false;
                    {
                        let rng = self.world.resource_mut::<Rng>().unwrap();
                        if rng.chance(0.15) {
                            apply_stun = true;
                        }
                    }
                    if apply_stun {
                        self.log("Stun Echo triggered! Target is stunned for 1 turn.");
                        self.world
                            .insert(target, crate::components::Stunned { duration: 1 });
                    }
                }
                if has_lifesteal {
                    let heal = (damage as f32 * 0.15) as i32;
                    if heal > 0 {
                        let attacker = self.world.resource::<GameState>().unwrap().selected_entity;
                        if let Some(ae) = attacker {
                            if let Some(stats) = self.world.get_mut::<Stats>(ae) {
                                stats.hp = (stats.hp + heal).min(stats.max_hp);
                                self.log(format!("Lifesteal healed attacker for {} HP!", heal));
                            }
                        }
                    }
                }
            }
        }

        let equipment_lifesteal_percent = self
            .world
            .get::<EquippedItems>(attacker)
            .map(|equipped| equipped.total_lifesteal_percent())
            .unwrap_or(0);
        let equipment_lifesteal = damage * equipment_lifesteal_percent as i32 / 100;
        if equipment_lifesteal > 0 {
            let mut healed = 0;
            if let Some(stats) = self.world.get_mut::<Stats>(attacker) {
                let before = stats.hp;
                stats.hp = (stats.hp + equipment_lifesteal).min(stats.max_hp);
                healed = stats.hp - before;
            }
            if healed > 0 {
                self.log(format!(
                    "{}'s equipment restored {} HP!",
                    attacker_name, healed
                ));
            }
        }

        if !defeated {
            let target_is_blademaster = self
                .world
                .get::<crate::components::PrestigeProgress>(target)
                .is_some_and(|p| {
                    p.class == crate::components::PrestigeClass::BladeMaster && p.promoted
                });
            if target_is_blademaster {
                let counter_roll = {
                    let rng = self.world.resource_mut::<Rng>().unwrap();
                    rng.next_u32(100)
                };
                if counter_roll < 25 {
                    let counter_damage =
                        self.world.get::<Stats>(target).map(|s| s.atk).unwrap_or(0);
                    if counter_damage > 0 {
                        if let Some(atk_stats) = self.world.get_mut::<Stats>(attacker) {
                            atk_stats.hp -= counter_damage;
                        }
                        let attacker_class = self
                            .world
                            .get::<CharacterClass>(attacker)
                            .copied()
                            .unwrap_or(CharacterClass::Warrior);
                        let attacker_name_str = Self::get_class_name(attacker_class);
                        self.log(format!(
                            "[fg:FFD700]BladeMaster counter-attack! {} strikes back for {} damage to {}![/fg]",
                            target_name, counter_damage, attacker_name_str
                        ));
                        let (tcx, tcy) = self.get_tile_center_pixels(pos);
                        self.vfx_mut()
                            .particles
                            .extend(verryte_terminal::vfx::emit_slash(tcx, tcy, 1.5));
                        self.vfx_mut()
                            .shakes
                            .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.3));
                    }
                }
            }
        }

        (damage, defeated)
    }

    pub fn trigger_vanish(&mut self, entity: Entity, current_pos: Position) {
        let map_w = self.world.resource::<TacticalMap>().unwrap().width;
        let map_h = self.world.resource::<TacticalMap>().unwrap().height;

        let mut candidate_tiles = Vec::new();
        for dy in -3..=3 {
            for dx in -3..=3 {
                let tx = current_pos.x + dx;
                let ty = current_pos.y + dy;
                if tx >= 0 && tx < map_w as i16 && ty >= 0 && ty < map_h as i16 {
                    let pos = Position::new(tx, ty);
                    if (dx.abs() + dy.abs()) >= 2 && !self.is_occupied_except(pos, entity) {
                        candidate_tiles.push(pos);
                    }
                }
            }
        }

        if !candidate_tiles.is_empty() {
            let next_pos = {
                let rng = self.world.resource_mut::<Rng>().unwrap();
                candidate_tiles[rng.next_u32(candidate_tiles.len() as u32) as usize]
            };

            if let Some(pos) = self.world.get_mut::<Position>(entity) {
                *pos = next_pos;
            }

            self.log("Shadow Stalker vanished and reappeared elsewhere!");

            // VFX
            let (cx, cy) = self.get_tile_center_pixels(current_pos);
            self.vfx_mut()
                .particles
                .extend(verryte_terminal::vfx::emit_burst(
                    cx,
                    cy,
                    20,
                    Color(50, 50, 50),
                    &['░', '▓', ' ', '·'],
                ));
            let (nx, ny) = self.get_tile_center_pixels(next_pos);
            self.vfx_mut()
                .particles
                .extend(verryte_terminal::vfx::emit_burst(
                    nx,
                    ny,
                    20,
                    Color(50, 50, 50),
                    &['░', '▓', ' ', '·'],
                ));
        }
    }

    pub fn get_reachable_tiles(&self, entity: Entity) -> Vec<Position> {
        let pos = match self.world.get::<Position>(entity) {
            Some(p) => *p,
            None => return Vec::new(),
        };
        let stats = match self.world.get::<Stats>(entity) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let max_ap = stats.ap;
        if max_ap == 0 {
            return vec![pos];
        }

        let map = self.world.resource::<TacticalMap>().unwrap();
        let weather = self
            .world
            .resource::<crate::components::Weather>()
            .map(|w| w.current)
            .unwrap_or(crate::components::WeatherType::Sunny);
        let gravity_bonus = crate::systems::floor_modifier_gravity_cost(&self.world);

        let mut occupied = HashSet::new();
        for (e, p) in self.world.query::<Position>() {
            if e != entity && self.world.get::<Team>(e).is_some() {
                occupied.insert(*p);
            }
        }

        // BFS with terrain cost tracking
        let mut reachable = Vec::new();
        let mut best_cost: std::collections::HashMap<Position, i32> =
            std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((pos, 0i32));
        best_cost.insert(pos, 0);

        while let Some((current, cost_so_far)) = queue.pop_front() {
            reachable.push(current);
            for neighbor in current.neighbors4() {
                if neighbor.x < 0
                    || neighbor.x >= map.width as i16
                    || neighbor.y < 0
                    || neighbor.y >= map.height as i16
                {
                    continue;
                }
                if occupied.contains(&neighbor) {
                    continue;
                }
                let tile = map.tile(neighbor.x, neighbor.y);
                if matches!(tile, Tile::Wall) {
                    continue;
                }
                let move_cost = map.movement_cost_with_weather(neighbor, weather) + gravity_bonus;
                let new_cost = cost_so_far + move_cost;
                if new_cost <= max_ap {
                    let entry = best_cost.entry(neighbor).or_insert(i32::MAX);
                    if new_cost < *entry {
                        *entry = new_cost;
                        queue.push_back((neighbor, new_cost));
                    }
                }
            }
        }

        reachable
    }

    pub fn get_path_to(&self, entity: Entity, target: Position) -> Option<Vec<Position>> {
        let pos = *self.world.get::<Position>(entity)?;
        let map = self.world.resource::<TacticalMap>().unwrap();
        let weather = self
            .world
            .resource::<crate::components::Weather>()
            .map(|w| w.current)
            .unwrap_or(crate::components::WeatherType::Sunny);
        let mut occupied = HashSet::new();
        for (e, p) in self.world.query::<Position>() {
            if e != entity && self.world.get::<Team>(e).is_some() {
                occupied.insert(*p);
            }
        }
        map.tiles.shortest_path4_weighted(
            pos,
            target,
            |pt, tile| {
                matches!(
                    tile,
                    Tile::Grass | Tile::Water | Tile::Lava | Tile::Ice | Tile::Stairs | Tile::Mud
                ) && !occupied.contains(&pt)
            },
            |_from, _to, tile| match (tile, weather) {
                (Tile::Water, crate::components::WeatherType::Rainy) => 1,
                (Tile::Ice, crate::components::WeatherType::Snowing) => 0,
                (Tile::Water | Tile::Lava, _) => 2,
                (Tile::Mud, _) => 3,
                _ => 1,
            },
        )
    }

    pub fn cycle_character(&mut self, next: bool) {
        let mut players = Vec::new();
        for (e, _pos, team) in self.world.query2::<Position, Team>() {
            if *team == Team::Player {
                if let Some(stats) = self.world.get::<Stats>(e) {
                    if stats.ap > 0 {
                        players.push(e);
                    }
                }
            }
        }
        if players.is_empty() {
            return;
        }
        players.sort();

        let (next_entity, next_pos) = {
            let state = self.world.resource::<GameState>().unwrap();
            let current_index = state
                .selected_entity
                .and_then(|sel| players.iter().position(|&p| p == sel));

            let next_index = match current_index {
                Some(idx) => {
                    if next {
                        (idx + 1) % players.len()
                    } else {
                        (idx + players.len() - 1) % players.len()
                    }
                }
                None => 0,
            };
            let ent = players[next_index];
            let pos = *self.world.get::<Position>(ent).unwrap();
            (ent, pos)
        };

        {
            let state = self.world.resource_mut::<GameState>().unwrap();
            state.selected_entity = Some(next_entity);
            state.cursor = next_pos;
        }
        let (cx, cy) = self.get_tile_center_pixels(next_pos);
        self.camera.look_at(cx, cy);

        if let Some(class) = self.world.get::<CharacterClass>(next_entity) {
            let name = Self::get_class_name(*class);
            let stats = self.world.get::<Stats>(next_entity).unwrap();
            self.log(format!(
                "Selected {} (AP: {}/{})",
                name, stats.ap, stats.max_ap
            ));
        }
    }

    pub fn try_absorb_echo(&mut self, pos: Position) {
        let mut absorbed = None;
        for (e, p, echo) in self.world.query2::<Position, crate::components::EchoItem>() {
            if *p == pos {
                absorbed = Some((e, echo.class));
                break;
            }
        }
        if let Some((echo_ent, class)) = absorbed {
            self.world.despawn(echo_ent);

            if class == CharacterClass::Boss {
                self.log("Absorbed Blight Sovereign Echo! Echo absorbed successfully.");
                self.last_outcome = ActionOutcome::Absorbed {
                    echo_name: "Blight Sovereign".to_string(),
                };
                if let Some(echoes) = self
                    .world
                    .resource_mut::<crate::components::EquippedEchoes>()
                {
                    let ability = crate::components::EchoAbility::Lifesteal;
                    if !echoes.abilities.contains(&ability) {
                        echoes.abilities.push(ability);
                        self.log("Granted Blight Sovereign's Lifesteal ability!");
                        self.unlock_lore("echo_lore");
                    }
                }
                let floor = self
                    .world
                    .resource::<GameState>()
                    .map(|s| s.floor)
                    .unwrap_or(1);
                if floor == 1 {
                    if let Some(map) = self.world.resource_mut::<TacticalMap>() {
                        map.tiles.set(pos, Tile::Stairs);
                    }
                    self.log(format!(
                        "A staircase has appeared at {},{}. Stand on it and press '>' or type 'stairs' to descend.",
                        pos.x, pos.y
                    ));
                } else {
                    self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Victory;
                    self.log("Victory! Blight Sovereign defeated and Floor 2 conquered!");
                }
            } else {
                let mut ability = crate::components::EchoAbility::Swift;
                let mut name = "Swift";
                if class == CharacterClass::ShadowStalker {
                    ability = crate::components::EchoAbility::Frostbite;
                    name = "Frostbite";
                }

                if let Some(echoes) = self
                    .world
                    .resource_mut::<crate::components::EquippedEchoes>()
                {
                    if !echoes.abilities.contains(&ability) {
                        echoes.abilities.push(ability);
                        self.log(format!("Absorbed {} Echo! Ability granted.", name));
                        self.last_outcome = ActionOutcome::Absorbed {
                            echo_name: name.to_string(),
                        };
                    } else {
                        self.log(format!("Absorbed {} Echo, but already have it.", name));
                        self.last_outcome = ActionOutcome::Absorbed {
                            echo_name: name.to_string(),
                        };
                    }
                }
            }

            // Check if this was the last enemy and last echo
            let enemy_exists = self
                .world
                .query::<Team>()
                .into_iter()
                .any(|(_, team)| *team == Team::Enemy);
            let echo_exists = self
                .world
                .query::<crate::components::EchoItem>()
                .into_iter()
                .next()
                .is_some();
            if !enemy_exists && !echo_exists {
                let floor = self
                    .world
                    .resource::<GameState>()
                    .map(|s| s.floor)
                    .unwrap_or(1);
                if floor == 1 {
                    if let Some(map) = self.world.resource_mut::<TacticalMap>() {
                        map.tiles.set(pos, Tile::Stairs);
                    }
                    self.log(format!(
                        "All enemies defeated on Floor 1! A staircase has appeared at {},{}. Stand on it and press '>' or type 'stairs' to descend.",
                        pos.x, pos.y
                    ));
                } else {
                    self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Victory;
                    self.log("Victory! All enemies defeated and Floor 2 conquered!");
                }
            }

            // Spawn absorption VFX
            let (cx, cy) = self.get_tile_center_pixels(pos);
            self.vfx_mut()
                .particles
                .extend(verryte_terminal::vfx::emit_heal(cx, cy, 30));
            self.vfx_mut()
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(3.0, 0.6));
            self.vfx_mut()
                .flashes
                .push(verryte_terminal::vfx::Flash::full_screen(
                    Color(200, 255, 200),
                    0.3,
                ));
        }
    }

    pub fn handle_defeat(
        &mut self,
        entity: Entity,
        name: &str,
        class: CharacterClass,
        pos: Position,
    ) {
        if class == CharacterClass::Boss {
            let mut phase = crate::components::BossPhase::Phase1;
            if let Some(state) = self.world.resource::<GameState>() {
                phase = state.boss_phase;
            }
            if phase == crate::components::BossPhase::Phase1 {
                let config = self
                    .world
                    .resource::<crate::components::BossConfig>()
                    .cloned()
                    .unwrap_or_default();
                if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                    stats.max_hp = config.phase2_max_hp;
                    stats.hp = config.phase2_max_hp;
                    stats.atk += config.phase2_atk_bonus;
                    stats.def += config.phase2_def_bonus;
                    stats.spd += config.phase2_spd_bonus;
                    stats.max_ap = config.phase2_max_ap;
                    stats.ap = config.phase2_max_ap;
                }

                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.boss_phase = crate::components::BossPhase::Phase2;
                }

                self.log("Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");
                self.last_outcome = ActionOutcome::BossPhaseChanged {
                    phase: "Phase2".to_string(),
                };

                let (bx, by) = self.get_tile_center_pixels(pos);

                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        bx,
                        by,
                        50,
                        Color(255, 0, 0),
                        &['✦', '*', '░', '▓', '¤'],
                    ));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(5.0, 1.0));
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 0, 0),
                        0.5,
                    ));
                return;
            }
        }

        self.log(format!("{} was defeated!", name));
        if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
            log.send(GameEvent::Defeated { entity });
        }

        let defeated_team = self.world.get::<Team>(entity).copied();
        if defeated_team == Some(Team::Player) {
            crate::systems::apply_ally_defeated_morale(&mut self.world, entity);
        } else if defeated_team == Some(Team::Enemy) {
            crate::systems::apply_enemy_defeated_morale(&mut self.world);
        }

        self.world.despawn(entity);

        if class == CharacterClass::Boss {
            self.log(
                "Blight Sovereign dropped an Echo! Move a character to its tile to absorb it.",
            );
            self.world
                .builder()
                .with(pos)
                .with(crate::components::EchoItem { class })
                .build();

            // Visual boss death burst
            let (cx, cy) = self.get_tile_center_pixels(pos);
            self.vfx_mut()
                .particles
                .extend(verryte_terminal::vfx::emit_burst(
                    cx,
                    cy,
                    40,
                    Color(180, 50, 255),
                    &['✦', '✧', '░', '▓', '¤'],
                ));
            self.vfx_mut()
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(4.0, 0.8));
            self.vfx_mut()
                .flashes
                .push(verryte_terminal::vfx::Flash::full_screen(
                    Color(255, 255, 255),
                    0.4,
                ));
        } else {
            let mut drop = false;
            {
                let rng = self.world.resource_mut::<Rng>().unwrap();
                if rng.chance(0.4) {
                    drop = true;
                }
            }
            if drop {
                self.world
                    .builder()
                    .with(pos)
                    .with(crate::components::EchoItem { class })
                    .build();
                self.log(format!("{} dropped an Echo!", name));
            }
            if class == CharacterClass::CursedSentinel || class == CharacterClass::GlacialGolem {
                let kit = self
                    .world
                    .spawn_item("Upgrade Kit", crate::components::ItemEffect::UpgradeKit);
                self.log(format!("{} dropped an Upgrade Kit!", name));
                if let Some(player) = self
                    .world
                    .query2::<Team, crate::components::Inventory>()
                    .iter()
                    .find(|(_, team, _)| **team == Team::Player)
                    .map(|(e, _, _)| *e)
                {
                    if let Some(inventory) =
                        self.world.get_mut::<crate::components::Inventory>(player)
                    {
                        inventory.items.push(kit);
                    }
                }
            }
        }
        self.award_equipment_set_reward(class, name);

        let enemy_exists = self
            .world
            .query::<Team>()
            .into_iter()
            .any(|(_, team)| *team == Team::Enemy);
        let echo_exists = self
            .world
            .query::<crate::components::EchoItem>()
            .into_iter()
            .next()
            .is_some();
        if !enemy_exists && !echo_exists {
            let floor = self
                .world
                .resource::<GameState>()
                .map(|s| s.floor)
                .unwrap_or(1);
            if floor == 1 {
                if let Some(map) = self.world.resource_mut::<TacticalMap>() {
                    map.tiles.set(pos, Tile::Stairs);
                }
                self.log(format!(
                    "All enemies defeated on Floor 1! A staircase has appeared at {},{}. Stand on it and press '>' or type 'stairs' to descend.",
                    pos.x, pos.y
                ));
            } else {
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Victory;
                self.log("Victory! All enemies defeated and Floor 2 conquered!");
            }
        }
    }

    fn award_equipment_set_reward(&mut self, defeated_class: CharacterClass, defeated_name: &str) {
        let Some((hero_class, equipment)) =
            crate::equipment::set_reward_for_defeated_class(defeated_class)
        else {
            return;
        };
        let item_name = equipment.name.clone();
        let Some(hero) = self
            .world
            .query2::<Team, CharacterClass>()
            .iter()
            .find(|(_, team, class)| **team == Team::Player && **class == hero_class)
            .map(|(entity, _, _)| *entity)
        else {
            return;
        };
        if let Some(equipped) = self.world.get_mut::<EquippedItems>(hero) {
            equipped.equip(equipment);
            let hero_name = Game::get_class_name(hero_class).to_string();
            self.log(format!(
                "{} equipped {} from {}.",
                hero_name, item_name, defeated_name
            ));
            self.last_outcome = ActionOutcome::EquipmentRewarded {
                item_name,
                hero: hero_name,
            };
        }
    }

    pub fn build_concert_energy(&mut self, amount: u32) {
        let mut energy = 0;
        if let Some(state) = self.world.resource_mut::<GameState>() {
            state.concert_energy = std::cmp::min(100, state.concert_energy + amount);
            energy = state.concert_energy;
        }
        self.log(format!("Concert Energy: {}/100", energy));
    }

    pub fn check_parry(&mut self, target_pos: Position) {
        let mut target_is_boss = false;
        for (_e, class, pos) in self.world.query2::<CharacterClass, Position>() {
            if *class == CharacterClass::Boss && *pos == target_pos {
                target_is_boss = true;
                break;
            }
        }

        if !target_is_boss {
            return;
        }

        let mut player_in_telegraph = false;
        let mut acting_pos = None;
        if let Some(state) = self.world.resource::<GameState>() {
            if let Some(sel_ent) = state.selected_entity {
                if let Some(pos) = self.world.get::<Position>(sel_ent) {
                    acting_pos = Some(*pos);
                }
            }
        }

        if let Some(telegraph_zone) = self.world.resource::<crate::components::TelegraphZone>() {
            if let Some(pos) = acting_pos {
                if telegraph_zone.tiles.contains(&pos) {
                    player_in_telegraph = true;
                }
            } else {
                for (_e, team, pos) in self.world.query2::<Team, Position>() {
                    if *team == Team::Player && telegraph_zone.tiles.contains(pos) {
                        player_in_telegraph = true;
                        break;
                    }
                }
            }
        }

        if player_in_telegraph {
            self.log("PARRY! Blight Sovereign's telegraphed attack was canceled!");
            if let Some(telegraph_zone) = self
                .world
                .resource_mut::<crate::components::TelegraphZone>()
            {
                telegraph_zone.tiles.clear();
            }

            // Stun the boss
            let mut boss_ent = None;
            for (e, class) in self.world.query::<CharacterClass>() {
                if *class == CharacterClass::Boss {
                    boss_ent = Some(e);
                    break;
                }
            }
            if let Some(be) = boss_ent {
                if let Some(stats) = self.world.get_mut::<Stats>(be) {
                    stats.ap = 0;
                    self.log("Blight Sovereign is STUNNED and loses its action points!");
                }
            }

            self.vfx_mut()
                .flashes
                .push(verryte_terminal::vfx::Flash::full_screen(
                    Color(255, 255, 255),
                    0.25,
                ));
            self.vfx_mut()
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(4.0, 0.5));
            let (cx, cy) = self.get_tile_center_pixels(target_pos);
            self.vfx_mut()
                .particles
                .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
            self.vfx_mut()
                .floating_texts
                .push(verryte_terminal::vfx::FloatingText::new(
                    cx,
                    cy - 2.0,
                    "PARRIED!",
                    Color(255, 255, 100),
                    true,
                ));
        }
    }

    pub fn trigger_qte_swap(&mut self, active_ent: Entity) {
        let mut players = Vec::new();
        for (e, team) in self.world.query::<Team>() {
            if *team == Team::Player {
                players.push(e);
            }
        }
        players.sort();
        if players.len() <= 1 {
            self.log("Need at least 2 player characters on field to QTE swap!");
            return;
        }

        let current_idx = players.iter().position(|&e| e == active_ent).unwrap();
        let next_idx = (current_idx + 1) % players.len();
        let next_ent = players[next_idx];

        let active_pos = *self.world.get::<Position>(active_ent).unwrap();
        let next_pos = *self.world.get::<Position>(next_ent).unwrap();

        // Swap positions
        if let Some(pos) = self.world.get_mut::<Position>(active_ent) {
            *pos = next_pos;
        }
        if let Some(pos) = self.world.get_mut::<Position>(next_ent) {
            *pos = active_pos;
        }

        self.play_spatial_sfx("swap_swoosh", active_pos);

        self.world
            .resource_mut::<GameState>()
            .unwrap()
            .concert_energy = 0;
        self.world
            .resource_mut::<GameState>()
            .unwrap()
            .selected_entity = Some(next_ent);

        if let Some(bstats) = self.world.resource_mut::<BattleStats>() {
            bstats.total_swaps += 1;
        }

        let next_class = *self.world.get::<CharacterClass>(next_ent).unwrap();
        let active_class = *self.world.get::<CharacterClass>(active_ent).unwrap();
        let active_name = Self::get_class_name(active_class);
        let next_name = Self::get_class_name(next_class);

        self.log(format!(
            "[fg:9933FF][b]QTE Swap![/] [fg:E6E600]{}[/] swaps in for [fg:CCCCCC]{}[/] at ({}, {})!",
            next_name, active_name, active_pos.x, active_pos.y
        ));

        let (cx, cy) = self.get_tile_center_pixels(active_pos);

        match next_class {
            CharacterClass::Warrior => {
                self.log("Warrior Intro Skill: Cloud Slasher!");
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_slash(cx, cy, 1.0));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(2.5, 0.4));

                let mut targets = Vec::new();
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Enemy
                        && (p.x - active_pos.x).abs() <= 1
                        && (p.y - active_pos.y).abs() <= 1
                    {
                        targets.push(e);
                    }
                }
                for te in targets {
                    let target_class = *self.world.get::<CharacterClass>(te).unwrap();
                    let target_pos = *self.world.get::<Position>(te).unwrap();
                    let target_name = Self::get_class_name(target_class);
                    let mut final_hp = 0;
                    if let Some(t_stats) = self.world.get_mut::<Stats>(te) {
                        t_stats.hp -= 20;
                        final_hp = t_stats.hp;
                    }
                    self.log(format!(
                        "Cloud Slasher hit {} for 20 damage! (HP: {})",
                        target_name, final_hp
                    ));

                    let (tcx, tcy) = self.get_tile_center_pixels(target_pos);
                    self.vfx_mut()
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            tcx,
                            tcy - 2.0,
                            "-20",
                            Color(255, 80, 50),
                            true,
                        ));
                    if final_hp <= 0 {
                        let name_str = target_name.to_string();
                        self.handle_defeat(te, &name_str, target_class, target_pos);
                    } else {
                        self.check_parry(target_pos);
                    }
                }
            }
            CharacterClass::Mage => {
                self.log("Mage Intro Skill: Lightning Storm!");
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.3));

                let mut targets = Vec::new();
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Enemy {
                        let dx = (p.x - active_pos.x).abs();
                        let dy = (p.y - active_pos.y).abs();
                        if (dx == 0 && dy <= 2) || (dy == 0 && dx <= 2) {
                            targets.push(e);
                        }
                    }
                }
                for te in targets {
                    let target_class = *self.world.get::<CharacterClass>(te).unwrap();
                    let target_pos = *self.world.get::<Position>(te).unwrap();
                    let target_name = Self::get_class_name(target_class);
                    let mut final_hp = 0;
                    if let Some(t_stats) = self.world.get_mut::<Stats>(te) {
                        t_stats.hp -= 30;
                        final_hp = t_stats.hp;
                    }
                    self.log(format!(
                        "Lightning Storm hit {} for 30 damage! (HP: {})",
                        target_name, final_hp
                    ));

                    let (tcx, tcy) = self.get_tile_center_pixels(target_pos);
                    self.vfx_mut()
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            tcx,
                            tcy - 2.0,
                            "-30",
                            Color(255, 80, 50),
                            true,
                        ));
                    if final_hp <= 0 {
                        let name_str = target_name.to_string();
                        self.handle_defeat(te, &name_str, target_class, target_pos);
                    } else {
                        self.check_parry(target_pos);
                    }
                }
            }
            CharacterClass::Healer => {
                self.log("Healer Intro Skill: Holy Aura!");
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_heal(cx, cy, 25));
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(100, 255, 100),
                        0.2,
                    ));

                for pe in players {
                    let mut final_hp = 0;
                    let mut p_class = CharacterClass::Warrior;
                    if let Some(stats) = self.world.get_mut::<Stats>(pe) {
                        stats.hp = std::cmp::min(stats.max_hp, stats.hp + 40);
                        final_hp = stats.hp;
                        p_class = *self.world.get::<CharacterClass>(pe).unwrap();
                    }
                    let p_name = Self::get_class_name(p_class);
                    self.log(format!(
                        "Holy Aura healed {} for 40 HP! (HP: {})",
                        p_name, final_hp
                    ));

                    let p_pos = *self.world.get::<Position>(pe).unwrap();
                    let (pcx, pcy) = self.get_tile_center_pixels(p_pos);
                    self.vfx_mut()
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            pcx,
                            pcy - 2.0,
                            "+40",
                            Color(50, 255, 50),
                            true,
                        ));
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_heal(pcx, pcy, 8));
                }
            }
            _ => {}
        }

        let (cx, cy) = self.get_tile_center_pixels(active_pos);
        self.camera.look_at(cx, cy);
    }

    pub fn execute_combo_skill(
        &mut self,
        skill: crate::components::ComboSkill,
        participants: &[Entity],
        _target_pos: Position,
    ) {
        use crate::components::{ComboSkill, ComboSkillDef};

        let def = ComboSkillDef::for_skill(&skill);
        self.log(format!(
            "[fg:FFD700][b]COMBO SKILL: {}![/] {}[/fg]",
            def.name, def.description
        ));

        let total_atk: i32 = participants
            .iter()
            .filter_map(|&e| self.world.get::<Stats>(e))
            .map(|s| s.atk)
            .sum();

        match skill {
            ComboSkill::BladeStorm => {
                let damage = ((total_atk as f32) * 1.5) as i32;
                let mut center_positions = Vec::new();
                for &e in participants {
                    if let Some(pos) = self.world.get::<Position>(e) {
                        center_positions.push(*pos);
                    }
                }

                let mut targets = Vec::new();
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Enemy {
                        for center in &center_positions {
                            let dist = (p.x - center.x).abs() + (p.y - center.y).abs();
                            if dist <= 2 {
                                targets.push((e, *p));
                                break;
                            }
                        }
                    }
                }

                for (te, t_pos) in &targets {
                    let target_class = self
                        .world
                        .get::<CharacterClass>(*te)
                        .copied()
                        .unwrap_or(CharacterClass::ShadowStalker);
                    let target_name = Self::get_class_name(target_class);
                    let base_dmg = std::cmp::max(
                        1,
                        damage - self.world.get::<Stats>(*te).map(|s| s.def).unwrap_or(0),
                    );
                    let (actual, defeated) = self.resolve_combat_hit(
                        participants[0],
                        *te,
                        base_dmg,
                        "Blade Storm",
                        target_name,
                        *t_pos,
                    );
                    if let Some(events) = self.world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Attacked {
                            attacker: participants[0],
                            target: *te,
                            damage: actual,
                        });
                    }
                    if defeated {
                        let name_str = target_name.to_string();
                        self.handle_defeat(*te, &name_str, target_class, *t_pos);
                    }
                }

                for center in &center_positions {
                    let (cx, cy) = self.get_tile_center_pixels(*center);
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_slash(cx, cy, 2.0));
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
                }
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(4.0, 0.6));
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 255, 100),
                        0.3,
                    ));
            }

            ComboSkill::HolySmite => {
                let damage = total_atk * 2;
                let cursor = self.world.resource::<GameState>().unwrap().cursor;
                let mut found_target = None;
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Enemy && *p == cursor {
                        let dist = participants.iter().any(|&pe| {
                            self.world.get::<Position>(pe).is_some_and(|pp| {
                                (pp.x - cursor.x).abs() + (pp.y - cursor.y).abs() <= 2
                            })
                        });
                        if dist {
                            found_target = Some((e, *p));
                            break;
                        }
                    }
                }

                if let Some((te, t_pos)) = found_target {
                    let target_class = self
                        .world
                        .get::<CharacterClass>(te)
                        .copied()
                        .unwrap_or(CharacterClass::ShadowStalker);
                    let target_name = Self::get_class_name(target_class);
                    let base_dmg = std::cmp::max(
                        1,
                        damage - self.world.get::<Stats>(te).map(|s| s.def).unwrap_or(0),
                    );
                    let (actual, defeated) = self.resolve_combat_hit(
                        participants[0],
                        te,
                        base_dmg,
                        "Holy Smite",
                        target_name,
                        t_pos,
                    );
                    if let Some(events) = self.world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Attacked {
                            attacker: participants[0],
                            target: te,
                            damage: actual,
                        });
                    }

                    let warrior_ent = participants
                        .iter()
                        .find(|&&e| {
                            self.world
                                .get::<CharacterClass>(e)
                                .is_some_and(|c| *c == CharacterClass::Warrior)
                        })
                        .copied();
                    if let Some(we) = warrior_ent {
                        let heal = (actual as f32 * 0.3) as i32;
                        if heal > 0 {
                            if let Some(stats) = self.world.get_mut::<Stats>(we) {
                                stats.hp = (stats.hp + heal).min(stats.max_hp);
                            }
                            let w_name = Self::get_class_name(CharacterClass::Warrior);
                            self.log(format!("Holy Smite healed {} for {} HP!", w_name, heal));
                            let w_pos = *self.world.get::<Position>(we).unwrap();
                            let (wx, wy) = self.get_tile_center_pixels(w_pos);
                            self.vfx_mut().floating_texts.push(
                                verryte_terminal::vfx::FloatingText::new(
                                    wx,
                                    wy - 2.0,
                                    &format!("+{}", heal),
                                    Color(255, 215, 0),
                                    true,
                                ),
                            );
                        }
                    }

                    if defeated {
                        let name_str = target_name.to_string();
                        self.handle_defeat(te, &name_str, target_class, t_pos);
                    }

                    let (cx, cy) = self.get_tile_center_pixels(t_pos);
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_burst(
                            cx,
                            cy,
                            30,
                            Color(255, 215, 0),
                            &['✦', '✧', '+', '*'],
                        ));
                    self.vfx_mut()
                        .flashes
                        .push(verryte_terminal::vfx::Flash::full_screen(
                            Color(255, 215, 0),
                            0.3,
                        ));
                    self.vfx_mut()
                        .shakes
                        .push(verryte_terminal::vfx::ScreenShake::new(3.0, 0.5));
                } else {
                    self.log("Holy Smite: No valid target at cursor position!");
                }
            }

            ComboSkill::ArcaneSanctuary => {
                let shield_amount = total_atk;
                let heal_amount = 15;
                let mut players = Vec::new();
                for (e, team) in self.world.query::<Team>() {
                    if *team == Team::Player {
                        players.push(e);
                    }
                }
                for pe in &players {
                    if let Some(shield) = self
                        .world
                        .get_mut::<crate::components::ElementalShield>(*pe)
                    {
                        shield.amount += shield_amount;
                        shield.max_amount += shield_amount;
                    } else {
                        self.world.insert(
                            *pe,
                            crate::components::ElementalShield {
                                shield_type: crate::components::ShieldType::Physical,
                                amount: shield_amount,
                                max_amount: shield_amount,
                            },
                        );
                    }
                    if let Some(stats) = self.world.get_mut::<Stats>(*pe) {
                        stats.hp = (stats.hp + heal_amount).min(stats.max_hp);
                    }
                    let p_class = self
                        .world
                        .get::<CharacterClass>(*pe)
                        .copied()
                        .unwrap_or(CharacterClass::Warrior);
                    let p_name = Self::get_class_name(p_class);
                    let p_pos = *self.world.get::<Position>(*pe).unwrap();
                    self.log(format!(
                        "Arcane Sanctuary: {} shielded for {} and healed for {}!",
                        p_name, shield_amount, heal_amount
                    ));
                    let (px, py) = self.get_tile_center_pixels(p_pos);
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_heal(px, py, 20));
                    self.vfx_mut()
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            px,
                            py - 2.0,
                            &format!("SHIELD +{}", shield_amount),
                            Color(100, 180, 255),
                            true,
                        ));
                    if let Some(events) = self.world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Healed {
                            healer: participants[0],
                            target: *pe,
                            amount: heal_amount,
                        });
                    }
                }
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(100, 180, 255),
                        0.4,
                    ));
            }

            ComboSkill::TrinityStrike => {
                let damage = total_atk * 3;
                let cursor = self.world.resource::<GameState>().unwrap().cursor;
                let mut found_target = None;
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Enemy && *p == cursor {
                        let in_range = participants.iter().any(|&pe| {
                            self.world.get::<Position>(pe).is_some_and(|pp| {
                                (pp.x - cursor.x).abs() + (pp.y - cursor.y).abs() <= 3
                            })
                        });
                        if in_range {
                            found_target = Some((e, *p));
                            break;
                        }
                    }
                }

                if let Some((te, t_pos)) = found_target {
                    let target_class = self
                        .world
                        .get::<CharacterClass>(te)
                        .copied()
                        .unwrap_or(CharacterClass::ShadowStalker);
                    let target_name = Self::get_class_name(target_class);
                    let base_dmg = std::cmp::max(
                        1,
                        damage - self.world.get::<Stats>(te).map(|s| s.def).unwrap_or(0),
                    );
                    let (actual, defeated) = self.resolve_combat_hit(
                        participants[0],
                        te,
                        base_dmg,
                        "Trinity Strike",
                        target_name,
                        t_pos,
                    );
                    if let Some(events) = self.world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Attacked {
                            attacker: participants[0],
                            target: te,
                            damage: actual,
                        });
                    }

                    let stun_roll = {
                        let rng = self.world.resource_mut::<Rng>().unwrap();
                        rng.next_u32(100)
                    };
                    if stun_roll < 50 && !defeated {
                        self.world
                            .insert(te, crate::components::Stunned { duration: 1 });
                        self.log(format!(
                            "Trinity Strike STUNNED {} for 1 turn!",
                            target_name
                        ));
                    }

                    if defeated {
                        let name_str = target_name.to_string();
                        self.handle_defeat(te, &name_str, target_class, t_pos);
                    }

                    let (cx, cy) = self.get_tile_center_pixels(t_pos);
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_burst(
                            cx,
                            cy,
                            50,
                            Color(255, 255, 255),
                            &['✦', '✧', '*', '░', '▓', '¤'],
                        ));
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_slash(cx, cy, 3.0));
                    self.vfx_mut()
                        .shakes
                        .push(verryte_terminal::vfx::ScreenShake::new(6.0, 0.8));
                    self.vfx_mut()
                        .flashes
                        .push(verryte_terminal::vfx::Flash::full_screen(
                            Color(255, 255, 255),
                            0.5,
                        ));
                    self.vfx_mut()
                        .aoe_rings
                        .push(verryte_terminal::vfx::AoeRing {
                            cx: cx as i32,
                            cy: cy as i32,
                            max_radius: 12.0,
                            current_radius: 1.0,
                            expand_speed: 20.0,
                            color: Color(255, 255, 255),
                            lifetime: 0.6,
                            max_lifetime: 0.6,
                        });
                } else {
                    self.log("Trinity Strike: No valid target at cursor position!");
                }
            }
        }

        for &e in participants {
            if let Some(stats) = self.world.get_mut::<Stats>(e) {
                stats.ap -= def.ap_cost;
            }
        }

        self.build_concert_energy(50);
    }

    pub fn get_skill_info(
        class: CharacterClass,
        skill: crate::components::TargetingMode,
    ) -> Option<(String, i16, i32, bool, i32)> {
        match (class, skill) {
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill1) => {
                Some(("Heavy Slash".to_string(), 1, 2, false, 45))
            }
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                Some(("Dragon Fire".to_string(), 3, 3, true, 50))
            }
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill3) => {
                Some(("Taunt Shield".to_string(), 0, 1, false, 0))
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => {
                Some(("Thunderbolt".to_string(), 3, 2, false, 55))
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                Some(("Glacial Tempest".to_string(), 4, 3, true, 40))
            }
            (CharacterClass::Healer, crate::components::TargetingMode::Skill1) => {
                Some(("Holy Light".to_string(), 2, 2, false, 50))
            }
            (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => {
                Some(("Divine Protection".to_string(), 0, 3, true, 40))
            }
            _ => None,
        }
    }

    pub fn get_skill_aoe(
        class: CharacterClass,
        skill: crate::components::TargetingMode,
        target: Position,
    ) -> Vec<Position> {
        let mut tiles = Vec::new();
        match (class, skill) {
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        tiles.push(Position::new(target.x + dx, target.y + dy));
                    }
                }
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                tiles.push(target);
                for d in 1..=2 {
                    tiles.push(Position::new(target.x + d, target.y));
                    tiles.push(Position::new(target.x - d, target.y));
                    tiles.push(Position::new(target.x, target.y + d));
                    tiles.push(Position::new(target.x, target.y - d));
                }
            }
            (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => {
                // Healer ultimate affects ALL players.
            }
            _ => {
                tiles.push(target);
            }
        }
        tiles
    }

    pub fn execute_skill(
        &mut self,
        caster: Entity,
        class: CharacterClass,
        skill: crate::components::TargetingMode,
        target_pos: Position,
        value: i32,
        is_aoe: bool,
    ) {
        let is_divine_healer = class == CharacterClass::Healer
            && self
                .world
                .get::<crate::components::PrestigeProgress>(caster)
                .is_some_and(|p| {
                    p.class == crate::components::PrestigeClass::DivineHealer && p.promoted
                });

        let is_archmage_aoe = class == CharacterClass::Mage
            && self
                .world
                .get::<crate::components::PrestigeProgress>(caster)
                .is_some_and(|p| {
                    p.class == crate::components::PrestigeClass::Archmage && p.promoted
                });

        let value = if is_divine_healer && class == CharacterClass::Healer {
            value * 2
        } else {
            value
        };

        let (skill_name, _range, _ap, _is_aoe, _power) =
            Self::get_skill_info(class, skill).unwrap_or(("Unknown".to_string(), 0, 0, false, 0));
        let caster_name = Self::get_class_name(class);

        self.log(format!(
            "{} cast {} at ({}, {})!",
            caster_name, skill_name, target_pos.x, target_pos.y
        ));

        let (cx, cy) = self.get_tile_center_pixels(target_pos);

        match (class, skill) {
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill1) => {
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_slash(cx, cy, 1.0));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.3));
            }
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_fire(cx, cy, 25));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(3.5, 0.5));
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 100, 30),
                        0.2,
                    ));
            }
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill3) => {
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 50, 50),
                        0.15,
                    ));
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        cx,
                        cy,
                        30,
                        Color(255, 50, 50),
                        &['!', 'X', '#'],
                    ));
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => {
                let caster_pos = *self.world.get::<Position>(caster).unwrap();
                let (ccx, ccy) = self.get_tile_center_pixels(caster_pos);
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_lightning(ccx, ccy, cx, cy));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(1.5, 0.2));
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_ice(cx, cy, 25));
                self.vfx_mut()
                    .aoe_rings
                    .push(verryte_terminal::vfx::AoeRing {
                        cx: cx as i32,
                        cy: cy as i32,
                        max_radius: 12.0,
                        current_radius: 1.0,
                        expand_speed: 18.0,
                        color: Color(100, 180, 255),
                        lifetime: 0.6,
                        max_lifetime: 0.6,
                    });
            }
            (CharacterClass::Healer, crate::components::TargetingMode::Skill1) => {
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_heal(cx, cy, 20));
            }
            (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => {
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(100, 255, 100),
                        0.3,
                    ));
            }
            _ => {}
        }

        if class == CharacterClass::Healer && skill == crate::components::TargetingMode::Skill2 {
            let mut players = Vec::new();
            for (e, team) in self.world.query::<Team>() {
                if *team == Team::Player {
                    players.push(e);
                }
            }
            for pe in players {
                let mut final_hp = 0;
                let mut p_class = CharacterClass::Warrior;
                if let Some(stats) = self.world.get_mut::<Stats>(pe) {
                    stats.hp = std::cmp::min(stats.max_hp, stats.hp + value);
                    final_hp = stats.hp;
                    p_class = *self.world.get::<CharacterClass>(pe).unwrap();
                }
                let p_name = Self::get_class_name(p_class);
                self.log(format!(
                    "Healed {} for {} HP! (HP: {})",
                    p_name, value, final_hp
                ));

                crate::systems::apply_heal_morale(&mut self.world, pe);

                let p_pos = *self.world.get::<Position>(pe).unwrap();
                let (pcx, pcy) = self.get_tile_center_pixels(p_pos);
                self.vfx_mut()
                    .floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        pcx,
                        pcy - 2.0,
                        &format!("+{}", value),
                        Color(50, 255, 50),
                        true,
                    ));
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_heal(pcx, pcy, 10));

                if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                    log.send(GameEvent::Healed {
                        healer: caster,
                        target: pe,
                        amount: value,
                    });
                }
            }
        } else if class == CharacterClass::Warrior
            && skill == crate::components::TargetingMode::Skill3
        {
            if let Some(threat) = self.world.get_mut::<crate::components::Threat>(caster) {
                threat.value += 100;
                self.log(format!(
                    "{} taunted the enemies! Threat significantly increased (+100).",
                    caster_name
                ));
            }
        } else {
            let mut targets = Vec::new();
            if is_aoe {
                let mut aoe_tiles = Self::get_skill_aoe(class, skill, target_pos);
                if is_archmage_aoe {
                    let extra: Vec<Position> = aoe_tiles
                        .iter()
                        .flat_map(|p| p.neighbors4())
                        .filter(|p| !aoe_tiles.contains(p))
                        .collect();
                    aoe_tiles.extend(extra);
                }
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team
                        == (if class == CharacterClass::Healer {
                            Team::Player
                        } else {
                            Team::Enemy
                        })
                        && aoe_tiles.contains(p)
                    {
                        targets.push((e, *p));
                    }
                }
            } else {
                if let Some((target_ent, target_team, _stats, _class)) =
                    self.get_entity_at(target_pos)
                {
                    if target_team
                        == (if class == CharacterClass::Healer {
                            Team::Player
                        } else {
                            Team::Enemy
                        })
                    {
                        targets.push((target_ent, target_pos));
                    }
                }
            }

            if targets.is_empty() {
                self.log("Skill hit no targets.");
            }

            for (te, t_pos) in targets {
                let target_class = *self.world.get::<CharacterClass>(te).unwrap();
                let target_name = Self::get_class_name(target_class);

                if class == CharacterClass::Healer {
                    let mut final_hp = 0;
                    if let Some(stats) = self.world.get_mut::<Stats>(te) {
                        stats.hp = std::cmp::min(stats.max_hp, stats.hp + value);
                        final_hp = stats.hp;
                    }
                    self.log(format!(
                        "Healed {} for {} HP! (HP: {})",
                        target_name, value, final_hp
                    ));

                    crate::systems::apply_heal_morale(&mut self.world, te);

                    let mut cleansed = false;
                    if is_divine_healer {
                        cleansed = true;
                        self.world
                            .insert(te, crate::components::ElementalStatus::None);
                        self.world.remove::<crate::components::Rooted>(te);
                        self.world.remove::<crate::components::Stunned>(te);
                    } else if self
                        .world
                        .get::<crate::components::CharacterTrait>(caster)
                        .is_some_and(|t| {
                            t.trait_type == crate::components::HeroTrait::PurifyingTouch
                        })
                    {
                        let roll = {
                            let rng = self.world.resource_mut::<Rng>().unwrap();
                            rng.next_u32(100)
                        };
                        if roll < 50 {
                            cleansed = true;
                            self.world
                                .insert(te, crate::components::ElementalStatus::None);
                            self.world.remove::<crate::components::Rooted>(te);
                            self.world.remove::<crate::components::Stunned>(te);
                        }
                    }

                    if cleansed {
                        self.log(format!(
                            "Mira's Purifying Touch cleansed negative statuses from {}!",
                            target_name
                        ));
                        self.vfx_mut()
                            .particles
                            .extend(verryte_terminal::vfx::emit_bloom(cx, cy, 12));
                    }
                    self.vfx_mut()
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            cx,
                            cy - 2.0,
                            &format!("+{}", value),
                            Color(50, 255, 50),
                            true,
                        ));

                    if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                        log.send(GameEvent::Healed {
                            healer: caster,
                            target: te,
                            amount: value,
                        });
                    }
                } else {
                    let base_damage =
                        std::cmp::max(1, value - self.world.get::<Stats>(te).unwrap().def);
                    let (damage, mut defeated) = self.resolve_combat_hit(
                        caster,
                        te,
                        base_damage,
                        caster_name,
                        target_name,
                        t_pos,
                    );

                    if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                        log.send(GameEvent::Attacked {
                            attacker: caster,
                            target: te,
                            damage,
                        });
                    }

                    if !defeated {
                        let skill_element = match (class, skill) {
                            (CharacterClass::Warrior, crate::components::TargetingMode::Skill1) => {
                                crate::components::ElementalStatus::Ice { duration: 3 }
                            }
                            (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => {
                                crate::components::ElementalStatus::Lightning { duration: 3 }
                            }
                            (CharacterClass::Healer, crate::components::TargetingMode::Skill1) => {
                                crate::components::ElementalStatus::Nature { duration: 3 }
                            }
                            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                                crate::components::ElementalStatus::Ice { duration: 3 }
                            }
                            _ => crate::components::ElementalStatus::None,
                        };
                        if skill_element != crate::components::ElementalStatus::None {
                            self.apply_elemental_status(te, skill_element);
                        }
                        if let Some(t_stats) = self.world.get::<Stats>(te) {
                            if t_stats.hp <= 0 {
                                defeated = true;
                            }
                        }
                    }

                    if defeated {
                        let name_str = target_name.to_string();
                        self.handle_defeat(te, &name_str, target_class, t_pos);
                    } else {
                        self.check_parry(t_pos);
                    }
                }
            }
        }

        self.build_concert_energy(30);
    }

    pub fn outcome(&self) -> Outcome {
        self.world.resource::<GameState>().unwrap().outcome
    }

    pub fn check_boss_phase_transition(&mut self) {
        let mut boss_entity = None;
        let mut boss_pos = None;
        for (e, class, pos) in self.world.query2::<CharacterClass, Position>() {
            if *class == CharacterClass::Boss {
                boss_entity = Some(e);
                boss_pos = Some(*pos);
                break;
            }
        }

        if let Some(be) = boss_entity {
            let mut transition = false;
            let current_hp = if let Some(stats) = self.world.get::<Stats>(be) {
                stats.hp
            } else {
                0
            };

            let mut phase = crate::components::BossPhase::Phase1;
            if let Some(state) = self.world.resource::<GameState>() {
                phase = state.boss_phase;
            }

            let config = self
                .world
                .resource::<crate::components::BossConfig>()
                .cloned()
                .unwrap_or_default();

            if phase == crate::components::BossPhase::Phase1
                && current_hp <= config.phase2_hp_threshold
                && current_hp > 0
            {
                transition = true;
            }

            if transition {
                self.boss_transitioned = true;
                if let Some(stats) = self.world.get_mut::<Stats>(be) {
                    stats.max_hp = config.phase2_max_hp;
                    stats.hp = config.phase2_max_hp;
                    stats.atk += config.phase2_atk_bonus;
                    stats.def += config.phase2_def_bonus;
                    stats.spd += config.phase2_spd_bonus;
                    stats.max_ap = config.phase2_max_ap;
                    stats.ap = config.phase2_max_ap;
                }

                // Apply phase 2 shield from BossConfig.
                if config.phase2_shield_amount > 0 {
                    self.world.insert(
                        be,
                        crate::components::ElementalShield {
                            shield_type: config.phase2_shield_type,
                            amount: config.phase2_shield_amount,
                            max_amount: config.phase2_shield_amount,
                        },
                    );
                }

                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.boss_phase = crate::components::BossPhase::Phase2;
                }

                self.log("Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");
                self.last_outcome = ActionOutcome::BossPhaseChanged {
                    phase: "Phase2".to_string(),
                };

                crate::systems::apply_boss_phase_morale(&mut self.world);

                if let Some(dialogue) = self.world.resource_mut::<verryte_terminal::DialogueState>()
                {
                    *dialogue = verryte_terminal::DialogueState::new(
                        "Blight Sovereign",
                        "ENOUGH! You think your mortal sparks can extinguish my eternal shadow? Witness the true power of the Void... CELESTIAL RUIN!"
                    );
                }

                let bp = boss_pos.unwrap_or(Position::new(0, 0));
                self.play_spatial_sfx("boss_phase_transition", bp);
                let (bx, by) = self.get_tile_center_pixels(bp);

                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        bx,
                        by,
                        50,
                        Color(255, 0, 0),
                        &['✦', '*', '░', '▓', '¤'],
                    ));
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_fire(bx, by, 30));
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_shatter(bx, by, 30));

                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(5.0, 1.0));

                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 0, 0),
                        0.5,
                    ));

                self.vfx_mut()
                    .aoe_rings
                    .push(verryte_terminal::vfx::AoeRing {
                        cx: bx as i32,
                        cy: by as i32,
                        max_radius: 8.0,
                        current_radius: 1.0,
                        expand_speed: 15.0,
                        color: Color(255, 0, 0),
                        lifetime: 0.5,
                        max_lifetime: 0.5,
                    });
                self.vfx_mut()
                    .aoe_rings
                    .push(verryte_terminal::vfx::AoeRing {
                        cx: bx as i32,
                        cy: by as i32,
                        max_radius: 12.0,
                        current_radius: 1.0,
                        expand_speed: 20.0,
                        color: Color(0, 255, 0),
                        lifetime: 0.6,
                        max_lifetime: 0.6,
                    });
                self.vfx_mut()
                    .aoe_rings
                    .push(verryte_terminal::vfx::AoeRing {
                        cx: bx as i32,
                        cy: by as i32,
                        max_radius: 16.0,
                        current_radius: 1.0,
                        expand_speed: 25.0,
                        color: Color(255, 0, 0),
                        lifetime: 0.7,
                        max_lifetime: 0.7,
                    });
            }
        }
    }

    pub fn apply_elemental_status(
        &mut self,
        target: Entity,
        new_status: crate::components::ElementalStatus,
    ) {
        if !self.world.is_alive(target) {
            return;
        }

        let old_status = self
            .world
            .get::<crate::components::ElementalStatus>(target)
            .copied()
            .unwrap_or(crate::components::ElementalStatus::None);

        let target_pos = self
            .world
            .get::<Position>(target)
            .copied()
            .unwrap_or(Position::new(0, 0));

        let (t_cx, t_cy) = self.get_tile_center_pixels(target_pos);

        let target_class = self
            .world
            .get::<CharacterClass>(target)
            .copied()
            .unwrap_or(CharacterClass::Warrior);

        let target_name = Self::get_class_name(target_class);

        let mut is_storm_chaser = false;
        if let Some(state) = self.world.resource::<crate::components::GameState>() {
            if let Some(active_hero) = state.selected_entity {
                if self
                    .world
                    .get::<crate::components::CharacterTrait>(active_hero)
                    .is_some_and(|t| t.trait_type == crate::components::HeroTrait::StormChaser)
                {
                    is_storm_chaser = true;
                }
            }
        }

        match (old_status, new_status) {
            // Reaction: Ice + Lightning -> Shatter (or Lightning + Ice -> Shatter)
            (
                crate::components::ElementalStatus::Ice { .. },
                crate::components::ElementalStatus::Lightning { .. },
            )
            | (
                crate::components::ElementalStatus::Lightning { .. },
                crate::components::ElementalStatus::Ice { .. },
            ) => {
                self.log(format!(
                    "[fg:64C8FF][b]Elemental Reaction: SHATTER[/] on {}![/fg]",
                    target_name
                ));
                let bonus_damage = if is_storm_chaser { 40 } else { 30 };
                let mut defeated = false;
                if let Some(stats) = self.world.get_mut::<Stats>(target) {
                    stats.hp -= bonus_damage;
                    if stats.hp <= 0 {
                        defeated = true;
                    }
                }

                // Shatter VFX
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_shatter(t_cx, t_cy, 25));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(3.0, 0.5));
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen_eased(
                        Color(100, 200, 255),
                        0.4,
                        verryte_terminal::vfx::EasingMode::QuadOut,
                    ));
                self.vfx_mut()
                    .floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        t_cx,
                        t_cy - 1.0,
                        &format!("SHATTER! -{}", bonus_damage),
                        Color(100, 200, 255),
                        true,
                    ));

                if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                    log.send(GameEvent::ReactionTriggered {
                        entity: target,
                        reaction: "Shatter".to_owned(),
                        damage: bonus_damage,
                        healing: 0,
                    });
                }
                if let Some(events) = self
                    .world
                    .resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("shatter"));
                }

                if let Some(status) = self
                    .world
                    .get_mut::<crate::components::ElementalStatus>(target)
                {
                    *status = crate::components::ElementalStatus::None;
                }

                if defeated {
                    let name_str = target_name.to_string();
                    self.handle_defeat(target, &name_str, target_class, target_pos);
                }
            }

            // Reaction: Lightning + Nature -> Overgrowth (or Nature + Lightning -> Overgrowth)
            (
                crate::components::ElementalStatus::Lightning { .. },
                crate::components::ElementalStatus::Nature { .. },
            )
            | (
                crate::components::ElementalStatus::Nature { .. },
                crate::components::ElementalStatus::Lightning { .. },
            ) => {
                self.log(format!(
                    "[fg:32DC64][b]Elemental Reaction: OVERGROWTH[/] on {}![/fg]",
                    target_name
                ));
                let bonus_damage = if is_storm_chaser { 20 } else { 10 };
                let mut defeated = false;
                if let Some(stats) = self.world.get_mut::<Stats>(target) {
                    stats.hp -= bonus_damage;
                    if stats.hp <= 0 {
                        defeated = true;
                    }
                }

                // Root target
                self.world
                    .insert(target, crate::components::Rooted { duration: 1 });

                // Overgrowth VFX (Bloom + floating text)
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_bloom(t_cx, t_cy, 15));
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen_eased(
                        Color(50, 220, 100),
                        0.4,
                        verryte_terminal::vfx::EasingMode::QuadOut,
                    ));
                self.vfx_mut()
                    .floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        t_cx,
                        t_cy - 1.0,
                        &format!("OVERGROWTH! -{} [ROOTED]", bonus_damage),
                        Color(50, 220, 100),
                        true,
                    ));

                if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                    log.send(GameEvent::ReactionTriggered {
                        entity: target,
                        reaction: "Overgrowth".to_owned(),
                        damage: bonus_damage,
                        healing: 0,
                    });
                }
                if let Some(events) = self
                    .world
                    .resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("overgrowth"));
                }

                if let Some(status) = self
                    .world
                    .get_mut::<crate::components::ElementalStatus>(target)
                {
                    *status = crate::components::ElementalStatus::None;
                }

                if defeated {
                    let name_str = target_name.to_string();
                    self.handle_defeat(target, &name_str, target_class, target_pos);
                }
            }

            // Reaction: Nature + Ice -> Bloom (or Ice + Nature -> Bloom)
            (
                crate::components::ElementalStatus::Nature { .. },
                crate::components::ElementalStatus::Ice { .. },
            )
            | (
                crate::components::ElementalStatus::Ice { .. },
                crate::components::ElementalStatus::Nature { .. },
            ) => {
                self.log(format!(
                    "[fg:FFD700][b]Elemental Reaction: BLOOM[/] on {}![/fg]",
                    target_name
                ));
                let healing_amount = 20;

                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen_eased(
                        Color(200, 255, 100),
                        0.4,
                        verryte_terminal::vfx::EasingMode::QuadOut,
                    ));

                let mut allies = Vec::new();
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Player {
                        let dist = (p.x - target_pos.x).abs() + (p.y - target_pos.y).abs();
                        if dist <= 1 {
                            allies.push(e);
                        }
                    }
                }

                for ally in allies {
                    let a_class = *self.world.get::<CharacterClass>(ally).unwrap();
                    let a_pos = *self.world.get::<Position>(ally).unwrap();
                    let (a_cx, a_cy) = self.get_tile_center_pixels(a_pos);

                    let mut final_hp = 0;
                    if let Some(stats) = self.world.get_mut::<Stats>(ally) {
                        stats.hp = std::cmp::min(stats.max_hp, stats.hp + healing_amount);
                        final_hp = stats.hp;
                    }
                    self.log(format!(
                        "[fg:32FF32]Bloom healed {} for [b]{} HP![/] (HP: {})[/fg]",
                        Self::get_class_name(a_class),
                        healing_amount,
                        final_hp
                    ));

                    self.vfx_mut()
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            a_cx,
                            a_cy - 2.0,
                            &format!("+{} (Bloom)", healing_amount),
                            Color(50, 255, 50),
                            true,
                        ));
                    self.vfx_mut()
                        .particles
                        .extend(verryte_terminal::vfx::emit_bloom(a_cx, a_cy, 10));
                }

                if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                    log.send(GameEvent::ReactionTriggered {
                        entity: target,
                        reaction: "Bloom".to_owned(),
                        damage: 0,
                        healing: healing_amount,
                    });
                }
                if let Some(events) = self
                    .world
                    .resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("bloom"));
                }

                if let Some(status) = self
                    .world
                    .get_mut::<crate::components::ElementalStatus>(target)
                {
                    *status = crate::components::ElementalStatus::None;
                }
            }

            (_, new) => {
                if let Some(status) = self
                    .world
                    .get_mut::<crate::components::ElementalStatus>(target)
                {
                    *status = new;
                }

                let badge = match new {
                    crate::components::ElementalStatus::Ice { .. } => "Ice",
                    crate::components::ElementalStatus::Lightning { .. } => "Lightning",
                    crate::components::ElementalStatus::Nature { .. } => "Nature",
                    crate::components::ElementalStatus::Poison { .. } => "Poison",
                    crate::components::ElementalStatus::Regen { .. } => "Regen",
                    _ => "None",
                };

                let color_hex = match badge {
                    "Ice" => "64C8FF",
                    "Lightning" => "FFFF64",
                    "Nature" => "32DC64",
                    "Poison" => "A020F0",
                    "Regen" => "32CD32",
                    _ => "FFFFFF",
                };
                self.log(format!(
                    "Applied [fg:{}][b]{}[/] element to {}.",
                    color_hex, badge, target_name
                ));

                if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                    log.send(GameEvent::ElementalApplied {
                        entity: target,
                        status: new,
                    });
                }
            }
        }
    }

    pub fn transition_to_next_floor(&mut self) {
        let mut floor = 1;
        if let Some(state) = self.world.resource_mut::<GameState>() {
            state.floor += 1;
            floor = state.floor;
        }

        self.log(format!("Descending to Floor {}...", floor));
        if floor == 2 {
            self.unlock_lore("descent_into_darkness");
        }

        // 1. Clear VFX
        self.vfx_mut().clear();
        self.vfx_mut().trigger_shake(4.0, 0.8);
        self.vfx_mut().trigger_flash(Color(255, 255, 255), 0.5);
        if let Some(events) = self
            .world
            .resource_mut::<Events<verryte_core::AudioEvent>>()
        {
            events.send(verryte_core::AudioEvent::play("cleanse"));
        }

        // 2. Despawn all old enemies and EchoItems
        let mut to_despawn = Vec::new();
        for (e, team) in self.world.query::<Team>() {
            if *team == Team::Enemy {
                to_despawn.push(e);
            }
        }
        for (e, _) in self.world.query::<crate::components::EchoItem>() {
            to_despawn.push(e);
        }
        for e in to_despawn {
            self.world.despawn(e);
        }

        // 3. Generate a new map! (using BSP algorithm)
        let width = 24;
        let height = 16;
        let mut map = TacticalMap::new(width, height);
        // Generate BSP dungeon
        let seed = 100 + floor as u64;
        let room_centers = map
            .tiles
            .generate_bsp_dungeon(Tile::Wall, Tile::Grass, 4, seed);

        // Ensure map boundaries are set
        map.width = width;
        map.height = height;

        self.world.insert_resource(map);

        // Place Ice terrain patches near GlacialGolem spawn rooms
        {
            let map = self.world.resource_mut::<TacticalMap>().unwrap();
            for (i, &rc) in room_centers.iter().enumerate().skip(1) {
                if i < room_centers.len() - 1 && i % 3 == 2 {
                    for dx in -1i16..=1 {
                        for dy in -1i16..=1 {
                            let x = rc.x + dx;
                            let y = rc.y + dy;
                            if x >= 0
                                && x < width as i16
                                && y >= 0
                                && y < height as i16
                                && map.tile(x, y) == Tile::Grass
                            {
                                map.tiles.set(Position::new(x, y), Tile::Ice);
                            }
                        }
                    }
                }
            }
            map.add_ice_patches(seed + 100, 2);
        }

        // 4. Position players in the first room center
        let player_spawn = room_centers.first().copied().unwrap_or(Position::new(4, 4));
        let mut player_entities = Vec::new();
        for (e, team) in self.world.query::<Team>() {
            if *team == Team::Player {
                player_entities.push(e);
            }
        }
        for (i, p_ent) in player_entities.iter().enumerate() {
            let dx = (i % 2) as i16;
            let dy = (i / 2) as i16;
            if let Some(pos) = self.world.get_mut::<Position>(*p_ent) {
                *pos = Position::new(player_spawn.x + dx, player_spawn.y + dy);
            }
            if let Some(stats) = self.world.get_mut::<Stats>(*p_ent) {
                stats.ap = stats.max_ap;
            }
        }

        // 5. Spawn enemies in other rooms
        for (i, &room_center) in room_centers.iter().enumerate().skip(1) {
            if i == room_centers.len() - 1 {
                self.world
                    .spawn_character(room_center, Team::Enemy, CharacterClass::Boss);
                if let Some(boss_ent) = self
                    .world
                    .query2::<CharacterClass, Team>()
                    .into_iter()
                    .find(|(_, class, team)| {
                        **class == CharacterClass::Boss && **team == Team::Enemy
                    })
                    .map(|(e, _, _)| e)
                {
                    self.world.insert(
                        boss_ent,
                        crate::components::ElementalShield {
                            shield_type: crate::components::ShieldType::Ice,
                            amount: 150,
                            max_amount: 150,
                        },
                    );
                }
            } else {
                if i % 3 == 0 {
                    self.world.spawn_character(
                        room_center,
                        Team::Enemy,
                        CharacterClass::ShadowStalker,
                    );
                } else if i % 3 == 1 {
                    self.world.spawn_character(
                        room_center,
                        Team::Enemy,
                        CharacterClass::CorruptedSpore,
                    );
                } else {
                    self.world.spawn_character(
                        room_center,
                        Team::Enemy,
                        CharacterClass::GlacialGolem,
                    );
                }
            }
        }

        // 5b. Guarantee at least one GlacialGolem near Ice terrain
        let has_golem = self
            .world
            .query2::<CharacterClass, Team>()
            .iter()
            .any(|(_, c, t)| **c == CharacterClass::GlacialGolem && **t == Team::Enemy);
        if !has_golem {
            let map = self.world.resource::<TacticalMap>().unwrap();
            let mut ice_adjacent = None;
            for y in 1..(height as i16 - 1) {
                for x in 1..(width as i16 - 1) {
                    if map.tile(x, y) == Tile::Ice {
                        for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
                            let nx = x + dx;
                            let ny = y + dy;
                            if map.tile(nx, ny) == Tile::Grass {
                                ice_adjacent = Some(Position::new(nx, ny));
                                break;
                            }
                        }
                    }
                    if ice_adjacent.is_some() {
                        break;
                    }
                }
                if ice_adjacent.is_some() {
                    break;
                }
            }
            if let Some(pos) = ice_adjacent {
                self.world
                    .spawn_character(pos, Team::Enemy, CharacterClass::GlacialGolem);
            }
        }

        // 6. Reset GameState parameters for next floor
        if let Some(state) = self.world.resource_mut::<GameState>() {
            state.cursor = player_spawn;
            state.selected_entity = None;
            state.boss_phase = crate::components::BossPhase::Phase1;
            state.turn = 1;
        }

        self.world
            .insert_resource(verryte_map::VisibilityMap::new(width, height));
        let (cx, cy) = self.get_tile_center_pixels(player_spawn);
        self.camera.look_at(cx, cy);

        crate::systems::select_floor_modifiers(&mut self.world);

        self.log(format!(
            "Welcome to Floor {}! Conquer this final level.",
            floor
        ));
    }

    pub fn apply_action(
        &mut self,
        action: Action,
        source: ActionSource,
    ) -> crate::snapshot::StepReport {
        self.world.insert_resource(self.camera.clone());
        let before = self.snapshot();
        let before_log_len = self
            .world
            .resource::<MessageLog>()
            .map(|l| l.len())
            .unwrap_or(0);

        if matches!(action, Action::Undo) {
            let mut undo_stack = self
                .world
                .remove_resource::<crate::components::UndoStack>()
                .unwrap_or_default();
            let mut redo_stack = self
                .world
                .remove_resource::<crate::components::RedoStack>()
                .unwrap_or_default();
            let popped_state = undo_stack.states.pop();

            if let Some(state_bytes) = popped_state {
                let current_state = self.save_state().unwrap_or_default();
                if self.load_state(&state_bytes).is_ok() {
                    redo_stack.states.push(current_state);
                    if redo_stack.states.len() > 10 {
                        redo_stack.states.remove(0);
                    }
                    self.log("Undo successful: Restored previous state.");
                } else {
                    self.log("Failed to load undo state.");
                }
            } else {
                self.log("Nothing to undo!");
            }
            self.world.insert_resource(undo_stack);
            self.world.insert_resource(redo_stack);
            self.last_outcome = ActionOutcome::StateUpdated;
            return crate::snapshot::StepReport {
                action,
                source,
                before,
                after: self.snapshot(),
                events: Vec::new(),
                diagnostics: std::collections::HashMap::new(),
                outcome: ActionOutcome::StateUpdated,
            };
        }

        if matches!(action, Action::Redo) {
            let mut undo_stack = self
                .world
                .remove_resource::<crate::components::UndoStack>()
                .unwrap_or_default();
            let mut redo_stack = self
                .world
                .remove_resource::<crate::components::RedoStack>()
                .unwrap_or_default();
            let popped_state = redo_stack.states.pop();

            if let Some(state_bytes) = popped_state {
                let current_state = self.save_state().unwrap_or_default();
                if self.load_state(&state_bytes).is_ok() {
                    undo_stack.states.push(current_state);
                    if undo_stack.states.len() > 10 {
                        undo_stack.states.remove(0);
                    }
                    self.log("Redo successful: Restored next state.");
                } else {
                    self.log("Failed to load redo state.");
                }
            } else {
                self.log("Nothing to redo!");
            }
            self.world.insert_resource(undo_stack);
            self.world.insert_resource(redo_stack);
            self.last_outcome = ActionOutcome::StateUpdated;
            return crate::snapshot::StepReport {
                action,
                source,
                before,
                after: self.snapshot(),
                events: Vec::new(),
                diagnostics: std::collections::HashMap::new(),
                outcome: ActionOutcome::StateUpdated,
            };
        }

        let is_undoable = !matches!(
            action,
            Action::Undo
                | Action::Redo
                | Action::Save
                | Action::Load
                | Action::Quit
                | Action::ToggleRecording
                | Action::ToggleReplay
                | Action::ToggleReplayAuto
                | Action::StepReplay
                | Action::TogglePerf
                | Action::ToggleMinimap
                | Action::ChangeWeather(_)
                | Action::ViewPrestige
        );

        let phase_current = self
            .world
            .resource::<GameState>()
            .map(|s| s.phase)
            .unwrap_or(TurnPhase::Player);
        if is_undoable && phase_current == TurnPhase::Player {
            let state_bytes = self.save_state().unwrap_or_default();
            if let Some(stack) = self.world.resource_mut::<crate::components::UndoStack>() {
                stack.states.push(state_bytes);
                if stack.states.len() > 10 {
                    stack.states.remove(0);
                }
            }
            if let Some(stack) = self.world.resource_mut::<crate::components::RedoStack>() {
                stack.states.clear();
            }
        }

        if matches!(action, Action::EndTurn) {
            if let Some(stack) = self.world.resource_mut::<crate::components::UndoStack>() {
                stack.states.clear();
            }
            if let Some(stack) = self.world.resource_mut::<crate::components::RedoStack>() {
                stack.states.clear();
            }
        }

        // Record action history
        let (turn, phase, time) = {
            let state = self.world.resource::<GameState>().unwrap();
            let clock = self.world.resource::<GameClock>().unwrap();
            (
                state.turn,
                state.phase,
                clock.elapsed_real_time().as_secs_f32(),
            )
        };
        if let Some(history) = self
            .world
            .resource_mut::<verryte_input::ActionHistory<Action>>()
        {
            let mut record = verryte_input::ActionRecord::new(action, source, time);
            record = record.with_metadata("turn", &turn.to_string());
            record = record.with_metadata("phase", &format!("{:?}", phase));
            history.push(record);
        }

        // Reset outcome for this action.
        self.last_outcome = ActionOutcome::NoOp;
        self.boss_transitioned = false;
        self.apply_action_internal(action);
        self.check_boss_phase_transition();

        // Promote outcome based on observable state changes.
        let after = self.snapshot();
        let outcome = self.compute_outcome(action, &before, &after, phase, before_log_len);

        // Update BattleStats based on events
        let events_vec: Vec<GameEvent> = self
            .world
            .resource::<Events<GameEvent>>()
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default();

        for event in &events_vec {
            match event {
                GameEvent::Attacked {
                    attacker, damage, ..
                } => {
                    if let Some(threat) = self.world.get_mut::<crate::components::Threat>(*attacker)
                    {
                        threat.value += *damage;
                    }
                }
                GameEvent::Healed { healer, amount, .. } => {
                    if let Some(threat) = self.world.get_mut::<crate::components::Threat>(*healer) {
                        threat.value += (*amount as f32 * 1.5) as i32;
                    }
                    if self.world.get::<Team>(*healer) == Some(&Team::Player) {
                        if let Some(progress) = self
                            .world
                            .get_mut::<crate::components::PrestigeProgress>(*healer)
                        {
                            progress.total_healing_done += *amount;
                        }
                    }
                }
                _ => {}
            }
        }

        let mut entity_teams = std::collections::HashMap::new();
        for (entity, team) in self.world.query::<Team>() {
            entity_teams.insert(entity, *team);
        }

        let combo_count = self
            .world
            .resource::<GameState>()
            .map(|s| s.combo_count)
            .unwrap_or(0);

        if let Some(bstats) = self.world.resource_mut::<BattleStats>() {
            for event in &events_vec {
                match event {
                    GameEvent::Attacked {
                        attacker,
                        target,
                        damage,
                    } => {
                        let attacker_is_player = entity_teams.get(attacker) == Some(&Team::Player);
                        let target_is_player = entity_teams.get(target) == Some(&Team::Player);
                        if attacker_is_player {
                            bstats.total_damage_dealt += *damage;
                        }
                        if target_is_player {
                            bstats.total_damage_taken += *damage;
                        }
                    }
                    GameEvent::Healed { healer, amount, .. } => {
                        let healer_is_player = entity_teams.get(healer) == Some(&Team::Player);
                        if healer_is_player {
                            bstats.total_healing_done += *amount;
                        }
                    }
                    GameEvent::Defeated { entity } => {
                        let entity_is_enemy = entity_teams.get(entity) == Some(&Team::Enemy);
                        if entity_is_enemy {
                            bstats.total_kills += 1;
                        }
                    }
                    GameEvent::ReactionTriggered {
                        entity,
                        damage,
                        healing,
                        ..
                    } => {
                        let entity_is_player = entity_teams.get(entity) == Some(&Team::Player);
                        if entity_is_player {
                            bstats.total_damage_taken += *damage;
                            bstats.total_healing_done += *healing;
                        } else {
                            bstats.total_damage_dealt += *damage;
                        }
                    }
                    _ => {}
                }
            }

            if matches!(outcome, ActionOutcome::TurnAdvanced) {
                bstats.total_turns += 1;
            }

            if combo_count > bstats.max_combo_reached {
                bstats.max_combo_reached = combo_count;
            }
        }

        // Reset combo count if action failed or if the action was Wait
        let is_failed = matches!(outcome, ActionOutcome::Failed { .. });
        let is_wait = action == Action::Wait;
        if is_failed || is_wait {
            if let Some(state) = self.world.resource_mut::<GameState>() {
                if state.combo_count > 0 {
                    state.combo_count = 0;
                    if is_failed {
                        self.log("Combo broken by failed action.");
                    } else {
                        self.log("Combo broken by waiting.");
                    }
                }
            }
        }

        self.last_outcome = outcome.clone();

        if let Some(history) = self
            .world
            .resource_mut::<verryte_input::ActionHistory<Action>>()
        {
            if let Some(record) = history.records.last_mut() {
                if let Ok(outcome_str) = serde_json::to_string(&outcome) {
                    record.metadata.insert("outcome".to_string(), outcome_str);
                }
            }
        }

        let mut diagnostics = std::collections::HashMap::new();
        if let Some(diags) = self.world.resource::<verryte_core::Diagnostics>() {
            for (name, metrics) in &diags.systems {
                diagnostics.insert(name.clone(), metrics.last_duration.as_secs_f64() * 1000.0);
            }
        }

        crate::snapshot::StepReport {
            action,
            source,
            before,
            after,
            events: self.take_events(),
            diagnostics,
            outcome,
        }
    }

    /// Derive an `ActionOutcome` summary from the before/after state delta.
    fn compute_outcome(
        &self,
        action: Action,
        before: &crate::snapshot::Snapshot,
        break_after: &crate::snapshot::Snapshot,
        _phase_before: TurnPhase,
        before_log_len: usize,
    ) -> ActionOutcome {
        if matches!(action, Action::Quit) {
            return ActionOutcome::GameOver {
                outcome: break_after.outcome,
            };
        }
        if !matches!(break_after.outcome, Outcome::Playing) {
            return ActionOutcome::GameOver {
                outcome: break_after.outcome,
            };
        }

        let new_log_messages: Vec<String> = if let Some(log) = self.world.resource::<MessageLog>() {
            if log.len() > before_log_len {
                log.messages()
                    .iter()
                    .skip(before_log_len)
                    .map(|m| m.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        for msg in &new_log_messages {
            if msg.starts_with("Not enough AP")
                || msg.starts_with("Target is out of")
                || msg.starts_with("Cannot move")
                || msg.starts_with("Select a character")
                || msg.starts_with("Concert Energy not full")
                || msg.starts_with("No reachable safe")
                || msg.starts_with("Target is out of range")
            {
                return ActionOutcome::Failed {
                    reason: msg.clone(),
                };
            }
        }

        match (&self.last_outcome, action) {
            (ActionOutcome::Failed { .. }, _) => return self.last_outcome.clone(),
            (
                ActionOutcome::ItemUsed { .. },
                Action::UseItem(_) | Action::Skill1 | Action::Skill2 | Action::Skill3,
            )
            | (ActionOutcome::Crafted { .. }, Action::CraftItem(_, _))
            | (ActionOutcome::EquipmentUpgraded { .. }, Action::UpgradeEquipment(_))
            | (ActionOutcome::EquipmentRewarded { .. }, _)
            | (ActionOutcome::Absorbed { .. }, Action::Confirm)
            | (ActionOutcome::BossPhaseChanged { .. }, _)
            | (ActionOutcome::ToggleChanged { .. }, _)
            | (ActionOutcome::StatusViewed { .. }, Action::ViewPrestige)
            | (ActionOutcome::Rested { .. }, Action::Rest)
            | (ActionOutcome::ModifiersRerolled { .. }, Action::RerollModifiers)
            | (ActionOutcome::GameSaved { .. }, Action::Save)
            | (ActionOutcome::GameLoaded { .. }, Action::Load)
            | (ActionOutcome::RecordingChanged { .. }, Action::ToggleRecording)
            | (ActionOutcome::ReplayChanged { .. }, Action::ToggleReplay | Action::StepReplay)
            | (ActionOutcome::ReplayStepped { .. }, Action::StepReplay)
            | (ActionOutcome::ReplayAutoChanged { .. }, Action::ToggleReplayAuto) => {
                return self.last_outcome.clone();
            }
            _ => {}
        }

        if before.floor != break_after.floor {
            return ActionOutcome::FloorTransition {
                from: before.floor,
                to: break_after.floor,
            };
        }

        if matches!(self.boss_phase(), crate::components::BossPhase::Phase2)
            && !matches!(action, Action::EndTurn)
        {
            let is_combat_action = matches!(
                action,
                Action::Confirm
                    | Action::Skill1
                    | Action::Skill2
                    | Action::Skill3
                    | Action::EndTurn
            );
            if is_combat_action && self.boss_just_transitioned() {
                return ActionOutcome::BossPhaseChanged {
                    phase: "Phase2".to_string(),
                };
            }
        }
        if before.phase != break_after.phase {
            return ActionOutcome::PhaseChanged;
        }
        if before.turn != break_after.turn {
            return ActionOutcome::TurnAdvanced;
        }

        for msg in &new_log_messages {
            if msg.contains("crafted") || msg.contains("Crafted") {
                let item_name = msg
                    .split("crafted ")
                    .nth(1)
                    .or_else(|| msg.split("Crafted ").nth(1))
                    .and_then(|s| s.split('!').next())
                    .unwrap_or("Unknown")
                    .to_string();
                return ActionOutcome::Crafted { item_name };
            }
            if let Some(upgraded) = msg.strip_prefix("Upgraded ") {
                let upgraded = upgraded.trim_end_matches('.');
                if let Some((item_name, level)) = upgraded.rsplit_once(" +") {
                    if let (Action::UpgradeEquipment(slot), Ok(level)) =
                        (action, level.parse::<u8>())
                    {
                        return ActionOutcome::EquipmentUpgraded {
                            item_name: item_name.to_string(),
                            slot,
                            level,
                        };
                    }
                }
            }
        }

        if let Some(log) = self.world.resource::<Events<GameEvent>>() {
            for event in log.iter() {
                if let GameEvent::Attacked { damage, target, .. } = event {
                    let _ = target;
                    if *damage > 0 {
                        let was_crit = new_log_messages.iter().any(|m| m.contains("CRITICAL HIT"));
                        let was_blocked = new_log_messages.iter().any(|m| m.contains("BLOCKED"));
                        if was_crit {
                            return ActionOutcome::CritHit { damage: *damage };
                        }
                        if was_blocked {
                            return ActionOutcome::Blocked {
                                damage_reduced: *damage,
                            };
                        }
                        return ActionOutcome::Hit {
                            damage: *damage,
                            target: String::new(),
                            was_critical: false,
                            was_blocked: false,
                        };
                    }
                }
                if let GameEvent::Healed { amount, target, .. } = event {
                    let _ = target;
                    if *amount > 0 {
                        return ActionOutcome::Healed {
                            amount: *amount,
                            target: String::new(),
                        };
                    }
                }
                if let GameEvent::PhaseChanged(_) = event {
                    return ActionOutcome::PhaseChanged;
                }
                if let GameEvent::Moved { entity, to, .. } = event {
                    let entity_name = self
                        .world
                        .get::<CharacterClass>(*entity)
                        .map(|c| Game::get_class_name(*c).to_string())
                        .unwrap_or_default();
                    return ActionOutcome::Moved {
                        entity: entity_name,
                        to: *to,
                    };
                }
                if let GameEvent::ElementalApplied { entity, status } = event {
                    let target_name = self
                        .world
                        .get::<CharacterClass>(*entity)
                        .map(|c| Game::get_class_name(*c).to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    return ActionOutcome::StatusApplied {
                        status: format!("{:?}", status),
                        target: target_name,
                    };
                }
            }
        }
        if matches!(
            action,
            Action::MoveNorth
                | Action::MoveSouth
                | Action::MoveEast
                | Action::MoveWest
                | Action::Inspect(_)
                | Action::ClearCursor
                | Action::NextCharacter
                | Action::PrevCharacter
                | Action::Skill1
                | Action::Skill2
                | Action::Skill3
                | Action::ToggleInventory
                | Action::TogglePerf
                | Action::ToggleMinimap
                | Action::ToggleHelp
                | Action::AutoBattle
                | Action::ToggleBestiary
                | Action::UpgradeEquipment(_)
        ) {
            return ActionOutcome::StateUpdated;
        }
        ActionOutcome::NoOp
    }

    pub fn run_pending_reports(&mut self) -> Vec<crate::snapshot::StepReport> {
        let mut reports = Vec::new();
        while let Some(action) = self.router.pop_action() {
            reports.push(self.apply_action(action.action, action.source));
        }
        reports
    }

    fn apply_action_internal(&mut self, action: Action) {
        if let Some(dialogue) = self.world.resource_mut::<verryte_terminal::DialogueState>() {
            if !dialogue.text.is_empty() && !dialogue.finished {
                match action {
                    Action::Confirm => {
                        if dialogue.is_typing() {
                            dialogue.visible_chars = dialogue.text.len() as f32;
                        } else if !dialogue.choices.is_empty() {
                            let selected = dialogue.selected_choice;
                            dialogue.chosen = Some(selected);
                            dialogue.finished = true;
                            dialogue.text.clear();
                            // Apply dialogue consequences
                            if dialogue.title == "Tactical Focus" {
                                if selected == 0 {
                                    // +5 Attack for Kael
                                    if let Some(kael) = self
                                        .world
                                        .query::<CharacterClass>()
                                        .into_iter()
                                        .find(|(_, class)| **class == CharacterClass::Warrior)
                                        .map(|(e, _)| e)
                                    {
                                        if let Some(stats) = self.world.get_mut::<Stats>(kael) {
                                            stats.atk += 5;
                                        }
                                    }
                                    self.log("Focus selected: Pure Blade! Kael gets +5 ATK.");
                                } else if selected == 1 {
                                    // +5 Concert Energy
                                    if let Some(state) = self.world.resource_mut::<GameState>() {
                                        state.concert_energy = 5;
                                    }
                                    self.log(
                                        "Focus selected: Arcane Synergy! Concert Energy set to 5.",
                                    );
                                }
                            }
                        } else {
                            dialogue.finished = true;
                            dialogue.text.clear();
                        }
                        return;
                    }
                    Action::Cancel => {
                        if dialogue.is_typing() {
                            dialogue.visible_chars = dialogue.text.len() as f32;
                        } else {
                            dialogue.finished = true;
                            dialogue.text.clear();
                        }
                        return;
                    }
                    Action::MoveNorth | Action::MoveWest => {
                        if !dialogue.is_typing() && !dialogue.choices.is_empty() {
                            dialogue.prev_choice();
                        }
                        return;
                    }
                    Action::MoveSouth | Action::MoveEast => {
                        if !dialogue.is_typing() && !dialogue.choices.is_empty() {
                            dialogue.next_choice();
                        }
                        return;
                    }
                    _ => return, // Ignore other actions while dialogue is active
                }
            }
        }

        if self.world.resource::<GameState>().unwrap().ui_state
            == crate::components::UIState::Inventory
        {
            match action {
                Action::Skill1 => {
                    self.apply_action_internal(Action::UseItem(0));
                    return;
                }
                Action::Skill2 => {
                    self.apply_action_internal(Action::UseItem(1));
                    return;
                }
                Action::Skill3 => {
                    self.apply_action_internal(Action::UseItem(2));
                    return;
                }
                Action::SwapCharacter(idx) => {
                    self.apply_action_internal(Action::UseItem(idx + 3));
                    return;
                }
                Action::ToggleInventory | Action::Cancel => {
                    self.world.resource_mut::<GameState>().unwrap().ui_state =
                        crate::components::UIState::Normal;
                    self.log("Inventory closed.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "inventory".to_string(),
                        enabled: false,
                    };
                    return;
                }
                Action::UseItem(_) | Action::Quit => {} // Allow these to fall through
                _ => return,                            // Ignore others in inventory
            }
        }

        if self.world.resource::<GameState>().unwrap().ui_state == crate::components::UIState::Help
        {
            match action {
                Action::Cancel | Action::ToggleHelp => {
                    self.world.resource_mut::<GameState>().unwrap().ui_state =
                        crate::components::UIState::Normal;
                    self.log("Help closed.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "help".to_string(),
                        enabled: false,
                    };
                    return;
                }
                Action::Quit => {} // Allow quit to fall through
                _ => return,       // Ignore others in help overlay
            }
        }

        if self.world.resource::<GameState>().unwrap().ui_state
            == crate::components::UIState::Bestiary
        {
            match action {
                Action::Cancel | Action::ToggleBestiary => {
                    self.world.resource_mut::<GameState>().unwrap().ui_state =
                        crate::components::UIState::Normal;
                    self.log("Bestiary closed.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "bestiary".to_string(),
                        enabled: false,
                    };
                    return;
                }
                Action::Quit => {}
                _ => return,
            }
        }

        if self.outcome() != Outcome::Playing && action != Action::Quit {
            return;
        }

        if let Some(sel) = self.world.resource::<GameState>().unwrap().selected_entity {
            if let Some(morale) = self.world.get::<crate::components::Morale>(sel) {
                if morale.value == 0
                    && !matches!(
                        action,
                        Action::Wait | Action::EndTurn | Action::Rest | Action::Cancel
                    )
                {
                    let name = self
                        .world
                        .get::<CharacterClass>(sel)
                        .map(|c| Game::get_class_name(*c).to_string())
                        .unwrap_or_default();
                    self.log(format!("{} is too demoralized to act!", name));
                    return;
                }
            }
        }

        match action {
            Action::MoveNorth | Action::MoveSouth | Action::MoveEast | Action::MoveWest => {
                let dir = action.direction().unwrap();
                let (width, height) = {
                    let map = self.world.resource::<TacticalMap>().unwrap();
                    (map.width, map.height)
                };
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.cursor = state.cursor.step(dir);
                state.cursor.x = state.cursor.x.clamp(0, width as i16 - 1);
                state.cursor.y = state.cursor.y.clamp(0, height as i16 - 1);
                let target_pos = state.cursor;
                let (cx, cy) = self.get_tile_center_pixels(target_pos);
                self.camera.look_at(cx, cy);
            }
            Action::Confirm => {
                let state_clone = self.world.resource::<GameState>().unwrap().clone();
                let cursor = state_clone.cursor;
                let selected_entity = state_clone.selected_entity;

                // Handle skill casting confirmation
                if state_clone.targeting != crate::components::TargetingMode::None {
                    let sel_entity = selected_entity.unwrap();
                    let caster_pos = *self.world.get::<Position>(sel_entity).unwrap();
                    let caster_class = *self.world.get::<CharacterClass>(sel_entity).unwrap();

                    let (_name, range, ap_cost, is_aoe, damage_or_heal) = Self::get_skill_info(
                        caster_class,
                        state_clone.targeting,
                    )
                    .unwrap_or(("Unknown".to_string(), 1, 1, false, 0));

                    let is_archmage = self
                        .world
                        .get::<crate::components::PrestigeProgress>(sel_entity)
                        .is_some_and(|p| {
                            p.class == crate::components::PrestigeClass::Archmage && p.promoted
                        });
                    let effective_ap_cost = if is_archmage {
                        ap_cost.max(2) - 1
                    } else {
                        ap_cost
                    };

                    let dist = (caster_pos.x - cursor.x).abs() + (caster_pos.y - cursor.y).abs();
                    if range > 0 && dist > range {
                        self.log("Target is out of skill range!");
                        return;
                    }

                    let mut ap_ok = false;
                    if let Some(stats) = self.world.get_mut::<Stats>(sel_entity) {
                        if stats.ap >= effective_ap_cost {
                            stats.ap -= effective_ap_cost;
                            ap_ok = true;
                        }
                    }

                    if !ap_ok {
                        self.log("Not enough AP to cast this skill!");
                        self.world.resource_mut::<GameState>().unwrap().targeting =
                            crate::components::TargetingMode::None;
                        return;
                    }

                    self.execute_skill(
                        sel_entity,
                        caster_class,
                        state_clone.targeting,
                        cursor,
                        damage_or_heal,
                        is_aoe,
                    );

                    let state_mut = self.world.resource_mut::<GameState>().unwrap();
                    state_mut.targeting = crate::components::TargetingMode::None;
                    state_mut.selected_entity = None;
                    self.log("Selection cleared.");
                    return;
                }

                if let Some(sel_entity) = selected_entity {
                    if let Some((target_entity, target_team, target_stats, target_class)) =
                        self.get_entity_at(cursor)
                    {
                        if target_entity == sel_entity {
                            self.world
                                .resource_mut::<GameState>()
                                .unwrap()
                                .selected_entity = None;
                            self.log("Selection cleared.");
                        } else if target_team == Team::Enemy {
                            let sel_pos = *self.world.get::<Position>(sel_entity).unwrap();
                            let sel_class = *self.world.get::<CharacterClass>(sel_entity).unwrap();
                            let range = match sel_class {
                                CharacterClass::Warrior => 1,
                                CharacterClass::Mage => 3,
                                CharacterClass::Healer => 2,
                                _ => 1,
                            };
                            let dist = (sel_pos.x - cursor.x).abs() + (sel_pos.y - cursor.y).abs();
                            if dist <= range {
                                let mut ap_ok = false;
                                let mut atk_val = 0;
                                if let Some(sel_stats) = self.world.get_mut::<Stats>(sel_entity) {
                                    if sel_stats.ap >= 1 {
                                        sel_stats.ap -= 1;
                                        ap_ok = true;
                                        atk_val = sel_stats.atk;
                                    }
                                }
                                if ap_ok {
                                    let base_damage = std::cmp::max(1, atk_val - target_stats.def);
                                    let attacker_name = Self::get_class_name(sel_class);
                                    let target_name = Self::get_class_name(target_class);
                                    let (damage, mut defeated) = self.resolve_combat_hit(
                                        sel_entity,
                                        target_entity,
                                        base_damage,
                                        attacker_name,
                                        target_name,
                                        cursor,
                                    );

                                    if let Some(log) =
                                        self.world.resource_mut::<Events<GameEvent>>()
                                    {
                                        log.send(GameEvent::Attacked {
                                            attacker: sel_entity,
                                            target: target_entity,
                                            damage,
                                        });
                                    }

                                    if !defeated {
                                        let mut attacker_element = match sel_class {
                                            CharacterClass::Warrior => {
                                                crate::components::ElementalStatus::Ice {
                                                    duration: 3,
                                                }
                                            }
                                            CharacterClass::Mage => {
                                                crate::components::ElementalStatus::Lightning {
                                                    duration: 3,
                                                }
                                            }
                                            CharacterClass::Healer => {
                                                crate::components::ElementalStatus::Nature {
                                                    duration: 3,
                                                }
                                            }
                                            _ => crate::components::ElementalStatus::None,
                                        };

                                        // Frostbite Echo: 20% chance to apply Ice regardless of class
                                        if let Some(echoes) =
                                            self.world
                                                .resource::<crate::components::EquippedEchoes>()
                                        {
                                            if echoes.abilities.contains(
                                                &crate::components::EchoAbility::Frostbite,
                                            ) {
                                                let rng = self.world.resource_mut::<Rng>().unwrap();
                                                if rng.chance(0.2) {
                                                    attacker_element =
                                                        crate::components::ElementalStatus::Ice {
                                                            duration: 2,
                                                        };
                                                }
                                            }
                                        }

                                        if attacker_element
                                            != crate::components::ElementalStatus::None
                                        {
                                            self.apply_elemental_status(
                                                target_entity,
                                                attacker_element,
                                            );
                                        }
                                        if let Some(t_stats) =
                                            self.world.get::<Stats>(target_entity)
                                        {
                                            if t_stats.hp <= 0 {
                                                defeated = true;
                                            }
                                        }
                                    }

                                    if defeated {
                                        let name_str = target_name.to_string();
                                        self.handle_defeat(
                                            target_entity,
                                            &name_str,
                                            target_class,
                                            cursor,
                                        );
                                    } else {
                                        self.check_parry(cursor);
                                    }

                                    self.build_concert_energy(20);

                                    self.world
                                        .resource_mut::<GameState>()
                                        .unwrap()
                                        .selected_entity = None;
                                } else {
                                    self.log("Not enough AP to attack!");
                                }
                            } else {
                                self.log("Target is out of range!");
                            }
                        } else if target_team == Team::Player {
                            let sel_class = *self.world.get::<CharacterClass>(sel_entity).unwrap();
                            if sel_class == CharacterClass::Healer {
                                let sel_pos = *self.world.get::<Position>(sel_entity).unwrap();
                                let dist =
                                    (sel_pos.x - cursor.x).abs() + (sel_pos.y - cursor.y).abs();
                                if dist <= 2 {
                                    let mut ap_ok = false;
                                    let mut heal_val = 0;
                                    if let Some(sel_stats) = self.world.get_mut::<Stats>(sel_entity)
                                    {
                                        if sel_stats.ap >= 1 {
                                            sel_stats.ap -= 1;
                                            ap_ok = true;
                                            heal_val = sel_stats.atk * 2;
                                        }
                                    }
                                    if ap_ok {
                                        let mut final_hp = 0;
                                        if let Some(t_stats) =
                                            self.world.get_mut::<Stats>(target_entity)
                                        {
                                            t_stats.hp = std::cmp::min(
                                                t_stats.max_hp,
                                                t_stats.hp + heal_val,
                                            );
                                            final_hp = t_stats.hp;
                                        }
                                        let target_name = Self::get_class_name(target_class);
                                        self.log(format!(
                                            "Mira healed {} for {} HP! (Target HP: {})",
                                            target_name, heal_val, final_hp
                                        ));

                                        crate::systems::apply_heal_morale(
                                            &mut self.world,
                                            target_entity,
                                        );

                                        let (target_cx, target_cy) =
                                            self.get_tile_center_pixels(cursor);
                                        self.vfx_mut().floating_texts.push(
                                            verryte_terminal::vfx::FloatingText::new(
                                                target_cx,
                                                target_cy - 2.0,
                                                &format!("+{}", heal_val),
                                                Color(50, 255, 50),
                                                true,
                                            ),
                                        );
                                        self.vfx_mut().particles.extend(
                                            verryte_terminal::vfx::emit_heal(
                                                target_cx, target_cy, 15,
                                            ),
                                        );

                                        if let Some(log) =
                                            self.world.resource_mut::<Events<GameEvent>>()
                                        {
                                            log.send(GameEvent::Healed {
                                                healer: sel_entity,
                                                target: target_entity,
                                                amount: heal_val,
                                            });
                                        }

                                        self.build_concert_energy(15);

                                        self.world
                                            .resource_mut::<GameState>()
                                            .unwrap()
                                            .selected_entity = None;
                                    } else {
                                        self.log("Not enough AP to heal!");
                                    }
                                } else {
                                    self.log("Target is out of range for healing!");
                                }
                            } else {
                                if target_stats.ap > 0 {
                                    self.world
                                        .resource_mut::<GameState>()
                                        .unwrap()
                                        .selected_entity = Some(target_entity);
                                    let target_name = Self::get_class_name(target_class);
                                    self.log(format!(
                                        "Selected {} (AP: {}/{})",
                                        target_name, target_stats.ap, target_stats.max_ap
                                    ));
                                } else {
                                    self.world
                                        .resource_mut::<GameState>()
                                        .unwrap()
                                        .selected_entity = None;
                                    self.log("Selection cleared.");
                                }
                            }
                        }
                    } else {
                        let mut has_ap = true;
                        if let Some(stats) = self.world.get::<Stats>(sel_entity) {
                            if stats.ap <= 0 {
                                has_ap = false;
                            }
                        }
                        if !has_ap {
                            self.log("Not enough AP to move there!");
                        } else {
                            let reachable = self.get_reachable_tiles(sel_entity);
                            if reachable.contains(&cursor) {
                                if let Some(path) = self.get_path_to(sel_entity, cursor) {
                                    let (total_cost, dest_tile) = {
                                        let map = self.world.resource::<TacticalMap>().unwrap();
                                        let weather = self
                                            .world
                                            .resource::<crate::components::Weather>()
                                            .map(|w| w.current)
                                            .unwrap_or(crate::components::WeatherType::Sunny);
                                        let total_cost: i32 = path
                                            .iter()
                                            .skip(1)
                                            .map(|p| map.movement_cost_with_weather(*p, weather))
                                            .sum();
                                        let dest_tile = map.tile(cursor.x, cursor.y);
                                        (total_cost, dest_tile)
                                    };
                                    let mut ap_ok = false;
                                    if let Some(sel_stats) = self.world.get_mut::<Stats>(sel_entity)
                                    {
                                        if sel_stats.ap >= total_cost {
                                            sel_stats.ap -= total_cost;
                                            ap_ok = true;
                                        }
                                    }
                                    if ap_ok {
                                        let from_pos =
                                            *self.world.get::<Position>(sel_entity).unwrap();

                                        // Ice slide logic
                                        let mut final_dest = cursor;
                                        let occupied: Vec<Position> = self
                                            .world
                                            .query2::<Position, Team>()
                                            .iter()
                                            .filter(|(e, _, _)| *e != sel_entity)
                                            .map(|(_, p, _)| **p)
                                            .collect();
                                        let has_ice_walker = self
                                            .world
                                            .get::<crate::components::CharacterTrait>(sel_entity)
                                            .map(|t| {
                                                t.trait_type
                                                    == crate::components::HeroTrait::IceWalker
                                            })
                                            .unwrap_or(false);
                                        if dest_tile == Tile::Ice
                                            && path.len() >= 2
                                            && !has_ice_walker
                                        {
                                            let p_last = path[path.len() - 1];
                                            let p_prev = path[path.len() - 2];
                                            let dx = p_last.x - p_prev.x;
                                            let dy = p_last.y - p_prev.y;

                                            let mut curr = cursor;
                                            let map = self.world.resource::<TacticalMap>().unwrap();
                                            loop {
                                                let next_pt =
                                                    Position::new(curr.x + dx, curr.y + dy);
                                                if next_pt.x < 0
                                                    || next_pt.x >= map.width as i16
                                                    || next_pt.y < 0
                                                    || next_pt.y >= map.height as i16
                                                {
                                                    break;
                                                }
                                                if occupied.contains(&next_pt) {
                                                    break;
                                                }
                                                let next_tile = map.tile(next_pt.x, next_pt.y);
                                                if next_tile == Tile::Wall {
                                                    break;
                                                }
                                                curr = next_pt;
                                                if next_tile != Tile::Ice {
                                                    break;
                                                }
                                            }
                                            final_dest = curr;
                                        }

                                        if let Some(pos) =
                                            self.world.get_mut::<Position>(sel_entity)
                                        {
                                            *pos = final_dest;
                                        }
                                        let sel_class =
                                            *self.world.get::<CharacterClass>(sel_entity).unwrap();
                                        let char_name = Self::get_class_name(sel_class);
                                        self.log(format!(
                                            "{} moved to ({}, {}) spending {} AP.",
                                            char_name, cursor.x, cursor.y, total_cost
                                        ));

                                        if final_dest != cursor {
                                            self.log(format!(
                                                "Ice slide! Slid to ({}, {}).",
                                                final_dest.x, final_dest.y
                                            ));
                                            let (tcx, tcy) =
                                                self.get_tile_center_pixels(final_dest);
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_burst(
                                                    tcx,
                                                    tcy,
                                                    15,
                                                    Color(150, 220, 255),
                                                    &['*', '✦', '·'],
                                                ),
                                            );
                                        }

                                        // Spawn movement particles
                                        let (tcx, tcy) = self.get_tile_center_pixels(final_dest);
                                        self.vfx_mut()
                                            .particles
                                            .extend(verryte_terminal::vfx::emit_heal(tcx, tcy, 5));

                                        if let Some(log) =
                                            self.world.resource_mut::<Events<GameEvent>>()
                                        {
                                            log.send(GameEvent::Moved {
                                                entity: sel_entity,
                                                from: from_pos,
                                                to: final_dest,
                                            });
                                        }

                                        let map = self.world.resource::<TacticalMap>().unwrap();
                                        let final_dest_tile = map.tile(final_dest.x, final_dest.y);
                                        // Check for Lava damage on player movement
                                        if final_dest_tile == Tile::Lava {
                                            let lava_dmg = {
                                                let w = self
                                                    .world
                                                    .resource::<crate::components::Weather>()
                                                    .map(|w| w.current)
                                                    .unwrap_or(
                                                        crate::components::WeatherType::Sunny,
                                                    );
                                                if w == crate::components::WeatherType::Rainy {
                                                    16
                                                } else {
                                                    20
                                                }
                                            };
                                            let mut final_hp = 0;
                                            let mut defeated = false;
                                            if let Some(stats) =
                                                self.world.get_mut::<Stats>(sel_entity)
                                            {
                                                stats.hp = std::cmp::max(0, stats.hp - lava_dmg);
                                                final_hp = stats.hp;
                                                if stats.hp <= 0 {
                                                    defeated = true;
                                                }
                                            }
                                            self.log(format!(
                                                "{} stepped into LAVA and took {} damage! (HP: {})",
                                                char_name, lava_dmg, final_hp
                                            ));

                                            // Spawn fire/lava particles
                                            let (tcx, tcy) = self.get_tile_center_pixels(cursor);
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_burst(
                                                    tcx,
                                                    tcy,
                                                    15,
                                                    Color(255, 60, 0),
                                                    &['*', '·', '✦'],
                                                ),
                                            );
                                            self.vfx_mut().shakes.push(
                                                verryte_terminal::vfx::ScreenShake::new_eased(
                                                    1.5,
                                                    0.3,
                                                    verryte_terminal::vfx::EasingMode::QuadOut,
                                                ),
                                            );

                                            if defeated {
                                                self.handle_defeat(
                                                    sel_entity, char_name, sel_class, cursor,
                                                );
                                            }
                                        }

                                        // Check for Mud entry effects
                                        if final_dest_tile == Tile::Mud {
                                            self.log(format!("{} trudged through MUD.", char_name));
                                            let (tcx, tcy) =
                                                self.get_tile_center_pixels(final_dest);
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_burst(
                                                    tcx,
                                                    tcy,
                                                    10,
                                                    Color(100, 70, 40),
                                                    &['~', '≈', '·'],
                                                ),
                                            );
                                        }

                                        // Check for Hazard terrain effects (SpikeTrap, PoisonCloud, etc.)
                                        if final_dest_tile != Tile::Lava
                                            && final_dest_tile != Tile::Ice
                                        {
                                            if let Some(hazards) =
                                                self.world
                                                    .resource::<crate::components::ActiveHazards>()
                                            {
                                                if let Some(stats) =
                                                    self.world.get::<Stats>(sel_entity)
                                                {
                                                    let result = crate::hazards::HazardSystem::trigger_hazard(
                                                        hazards, final_dest, stats,
                                                    );
                                                    if let Some(result) = result {
                                                        self.log(&result.message);
                                                        let dmg = result.damage;
                                                        let heal = result.healing;
                                                        let mut hp_after = 0;
                                                        if let Some(stats_mut) =
                                                            self.world.get_mut::<Stats>(sel_entity)
                                                        {
                                                            stats_mut.hp = (stats_mut.hp - dmg
                                                                + heal)
                                                                .clamp(0, stats_mut.max_hp);
                                                            hp_after = stats_mut.hp;
                                                        }
                                                        if result.should_destroy_tile {
                                                            if let Some(map) = self
                                                                .world
                                                                .resource_mut::<TacticalMap>()
                                                            {
                                                                crate::hazards::HazardSystem::process_cracked_floor(
                                                                    map, final_dest,
                                                                );
                                                            }
                                                        }
                                                        if let Some(hazards_mut) = self
                                                            .world
                                                            .resource_mut::<crate::components::ActiveHazards>()
                                                        {
                                                            crate::hazards::HazardSystem::decrement_trigger(
                                                                hazards_mut,
                                                                final_dest,
                                                            );
                                                        }
                                                        let (tcx, tcy) =
                                                            self.get_tile_center_pixels(final_dest);
                                                        if dmg > 0 {
                                                            self.vfx_mut().particles.extend(
                                                                verryte_terminal::vfx::emit_burst(
                                                                    tcx,
                                                                    tcy,
                                                                    8,
                                                                    Color(180, 40, 40),
                                                                    &['*', '·', '✦'],
                                                                ),
                                                            );
                                                        }
                                                        if heal > 0 {
                                                            self.vfx_mut().particles.extend(
                                                                verryte_terminal::vfx::emit_heal(
                                                                    tcx, tcy, 10,
                                                                ),
                                                            );
                                                        }
                                                        if hp_after <= 0 {
                                                            self.handle_defeat(
                                                                sel_entity, char_name, sel_class,
                                                                final_dest,
                                                            );
                                                        }
                                                        if let Some(status) = result.status_effect {
                                                            if self
                                                                .world
                                                                .get::<Stats>(sel_entity)
                                                                .is_some_and(|s| s.hp > 0)
                                                            {
                                                                self.world
                                                                    .insert(sel_entity, status);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        self.try_absorb_echo(cursor);

                                        self.world
                                            .resource_mut::<GameState>()
                                            .unwrap()
                                            .selected_entity = None;
                                    } else {
                                        self.log("Not enough AP to move there!");
                                    }
                                }
                            } else {
                                self.log("Cannot move to that tile!");
                            }
                        }
                    }
                } else {
                    if let Some((target_entity, target_team, target_stats, target_class)) =
                        self.get_entity_at(cursor)
                    {
                        if target_team == Team::Player && target_stats.ap > 0 {
                            self.world
                                .resource_mut::<GameState>()
                                .unwrap()
                                .selected_entity = Some(target_entity);
                            let char_name = Self::get_class_name(target_class);
                            self.log(format!(
                                "Selected {} (AP: {}/{})",
                                char_name, target_stats.ap, target_stats.max_ap
                            ));
                        }
                    }
                    self.try_absorb_echo(cursor);
                }
            }
            Action::Cancel => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                if state.targeting != crate::components::TargetingMode::None {
                    state.targeting = crate::components::TargetingMode::None;
                    self.log("Skill targeting canceled.");
                } else if state.selected_entity.is_some() {
                    state.selected_entity = None;
                    self.log("Selection cleared.");
                }
            }
            Action::NextCharacter => {
                self.cycle_character(true);
            }
            Action::PrevCharacter => {
                self.cycle_character(false);
            }
            Action::SwapCharacter(idx) => {
                let mut players = Vec::new();
                for (e, team) in self.world.query::<Team>() {
                    if *team == Team::Player {
                        players.push(e);
                    }
                }
                players.sort();
                if idx < players.len() {
                    let ent = players[idx];
                    let pos = *self.world.get::<Position>(ent).unwrap();
                    let state = self.world.resource_mut::<GameState>().unwrap();
                    state.selected_entity = Some(ent);
                    state.cursor = pos;
                    let (cx, cy) = self.get_tile_center_pixels(pos);
                    self.camera.look_at(cx, cy);
                    let class = *self.world.get::<CharacterClass>(ent).unwrap();
                    self.log(format!(
                        "Selected character: {}.",
                        Self::get_class_name(class)
                    ));
                } else {
                    self.log("Invalid character index.");
                }
            }
            Action::Skill1 => {
                let state = self.world.resource::<GameState>().unwrap();
                if state.selected_entity.is_some() {
                    let state_mut = self.world.resource_mut::<GameState>().unwrap();
                    state_mut.targeting = crate::components::TargetingMode::Skill1;
                    self.log("Skill 1 targeted! Use cursor to select target and press Confirm.");
                } else {
                    self.log("Select a character first to cast a skill!");
                }
            }
            Action::Skill2 => {
                let state = self.world.resource::<GameState>().unwrap();
                if state.selected_entity.is_some() {
                    let state_mut = self.world.resource_mut::<GameState>().unwrap();
                    state_mut.targeting = crate::components::TargetingMode::Skill2;
                    self.log("Skill 2 targeted! Use cursor to select target and press Confirm.");
                } else {
                    self.log("Select a character first to cast a skill!");
                }
            }
            Action::Skill3 => {
                let state = self.world.resource::<GameState>().unwrap();
                if let Some(active_ent) = state.selected_entity {
                    let class = *self.world.get::<CharacterClass>(active_ent).unwrap();
                    let energy = state.concert_energy;
                    if class == CharacterClass::Warrior && energy < 100 {
                        let state_mut = self.world.resource_mut::<GameState>().unwrap();
                        state_mut.targeting = crate::components::TargetingMode::Skill3;
                        self.log("Taunt Shield targeted! Use cursor to select target (self) and press Confirm.");
                    } else if energy >= 100 {
                        self.trigger_qte_swap(active_ent);
                    } else {
                        self.log(format!("Concert Energy not full ({}/100)!", energy));
                    }
                } else {
                    self.log("Select a character first!");
                }
            }
            Action::UseItem(idx) => {
                let (sel_entity, ui_state) = {
                    let state = self.world.resource::<GameState>().unwrap();
                    (state.selected_entity, state.ui_state)
                };

                if let Some(entity) = sel_entity {
                    if ui_state == crate::components::UIState::Inventory {
                        let item_to_use = {
                            if let Some(inv) =
                                self.world.get_mut::<crate::components::Inventory>(entity)
                            {
                                if idx < inv.items.len() {
                                    Some(inv.items.remove(idx))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        };

                        if let Some(item_ent) = item_to_use {
                            if let Some(item) = self.world.get::<crate::components::Item>(item_ent)
                            {
                                let item_name = item.name.clone();
                                let effect = item.effect.clone();
                                let mut should_consume = true;
                                self.log(format!("Used {}!", item_name));

                                match effect {
                                    crate::components::ItemEffect::Heal(amount) => {
                                        if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                                            stats.hp =
                                                std::cmp::min(stats.max_hp, stats.hp + amount);
                                            self.log(format!("Healed for {} HP.", amount));
                                            let (tcx, tcy) = self.get_tile_center_pixels(
                                                *self.world.get::<Position>(entity).unwrap(),
                                            );
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_heal(tcx, tcy, 20),
                                            );
                                            if let Some(events) = self
                                                .world
                                                .resource_mut::<Events<verryte_core::AudioEvent>>()
                                            {
                                                events.send(verryte_core::AudioEvent::play("heal"));
                                            }
                                        }
                                    }
                                    crate::components::ItemEffect::Combined(heal, ap) => {
                                        if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                                            stats.hp = std::cmp::min(stats.max_hp, stats.hp + heal);
                                            stats.ap = std::cmp::min(stats.max_ap, stats.ap + ap);
                                            self.log(format!("Combined effect! Healed for {} HP and replenished {} AP.", heal, ap));
                                            let (tcx, tcy) = self.get_tile_center_pixels(
                                                *self.world.get::<Position>(entity).unwrap(),
                                            );
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_heal(tcx, tcy, 20),
                                            );
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_burst(
                                                    tcx,
                                                    tcy,
                                                    12,
                                                    Color(255, 255, 100),
                                                    &['+', '⚡'],
                                                ),
                                            );
                                            if let Some(events) = self
                                                .world
                                                .resource_mut::<Events<verryte_core::AudioEvent>>()
                                            {
                                                events.send(verryte_core::AudioEvent::play("heal"));
                                                events.send(verryte_core::AudioEvent::play(
                                                    "replenish_ap",
                                                ));
                                            }
                                        }
                                    }
                                    crate::components::ItemEffect::ReplenishAp(amount) => {
                                        if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                                            stats.ap =
                                                std::cmp::min(stats.max_ap, stats.ap + amount);
                                            self.log(format!("Replenished {} AP.", amount));
                                            let (tcx, tcy) = self.get_tile_center_pixels(
                                                *self.world.get::<Position>(entity).unwrap(),
                                            );
                                            self.vfx_mut().particles.extend(
                                                verryte_terminal::vfx::emit_burst(
                                                    tcx,
                                                    tcy,
                                                    12,
                                                    Color(255, 255, 100),
                                                    &['+', '⚡'],
                                                ),
                                            );
                                            if let Some(events) = self
                                                .world
                                                .resource_mut::<Events<verryte_core::AudioEvent>>()
                                            {
                                                events.send(verryte_core::AudioEvent::play(
                                                    "replenish_ap",
                                                ));
                                            }
                                        }
                                    }
                                    crate::components::ItemEffect::CleanseAndHeal(amount) => {
                                        self.world.insert(
                                            entity,
                                            crate::components::ElementalStatus::None,
                                        );
                                        self.world.remove::<crate::components::Rooted>(entity);
                                        self.world.remove::<crate::components::Stunned>(entity);
                                        let mut final_hp = 0;
                                        if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                                            stats.hp =
                                                std::cmp::min(stats.max_hp, stats.hp + amount);
                                            final_hp = stats.hp;
                                        }
                                        self.log(format!(
                                            "Cleansed and healed for {} HP! (HP: {})",
                                            amount, final_hp
                                        ));
                                        let (tcx, tcy) = self.get_tile_center_pixels(
                                            *self.world.get::<Position>(entity).unwrap(),
                                        );
                                        self.vfx_mut().particles.extend(
                                            verryte_terminal::vfx::emit_bloom(tcx, tcy, 18),
                                        );
                                        self.vfx_mut()
                                            .particles
                                            .extend(verryte_terminal::vfx::emit_heal(tcx, tcy, 15));
                                        self.vfx_mut().trigger_flash(Color(100, 255, 100), 0.25);
                                        if let Some(events) = self
                                            .world
                                            .resource_mut::<Events<verryte_core::AudioEvent>>()
                                        {
                                            events.send(verryte_core::AudioEvent::play("cleanse"));
                                            events.send(verryte_core::AudioEvent::play("heal"));
                                        }
                                    }
                                    crate::components::ItemEffect::Cleanse => {
                                        self.world.insert(
                                            entity,
                                            crate::components::ElementalStatus::None,
                                        );
                                        self.world.remove::<crate::components::Rooted>(entity);
                                        self.world.remove::<crate::components::Stunned>(entity);
                                        self.log("All negative statuses cleansed!");
                                        let (tcx, tcy) = self.get_tile_center_pixels(
                                            *self.world.get::<Position>(entity).unwrap(),
                                        );
                                        self.vfx_mut().particles.extend(
                                            verryte_terminal::vfx::emit_bloom(tcx, tcy, 18),
                                        );
                                        self.vfx_mut().trigger_flash(Color(100, 255, 100), 0.25);
                                        if let Some(events) = self
                                            .world
                                            .resource_mut::<Events<verryte_core::AudioEvent>>()
                                        {
                                            events.send(verryte_core::AudioEvent::play("cleanse"));
                                        }
                                    }
                                    crate::components::ItemEffect::RestoreShield(
                                        shield_type,
                                        amount,
                                    ) => {
                                        self.world.insert(
                                            entity,
                                            crate::components::ElementalShield {
                                                shield_type,
                                                amount,
                                                max_amount: amount,
                                            },
                                        );
                                        self.log(format!(
                                            "Shielded! Created a {} HP {:?} Shield.",
                                            amount, shield_type
                                        ));
                                        let (tcx, tcy) = self.get_tile_center_pixels(
                                            *self.world.get::<Position>(entity).unwrap(),
                                        );
                                        self.vfx_mut()
                                            .particles
                                            .extend(verryte_terminal::vfx::emit_ice(tcx, tcy, 15));
                                        if let Some(events) = self
                                            .world
                                            .resource_mut::<Events<verryte_core::AudioEvent>>()
                                        {
                                            events.send(verryte_core::AudioEvent::play("shield"));
                                        }
                                    }
                                    crate::components::ItemEffect::UpgradeKit => {
                                        should_consume = false;
                                        self.log(
                                            "Upgrade Kit must be used with equip_upgrade:<slot>.",
                                        );
                                        self.last_outcome =
                                            crate::snapshot::ActionOutcome::Failed {
                                                reason: "Upgrade Kit requires an equipment slot"
                                                    .to_string(),
                                            };
                                    }
                                }

                                // Close inventory after use
                                self.world.resource_mut::<GameState>().unwrap().ui_state =
                                    crate::components::UIState::Normal;

                                if should_consume {
                                    self.last_outcome = crate::snapshot::ActionOutcome::ItemUsed {
                                        name: item_name,
                                    };
                                    // Consumed item entity is gone
                                    self.world.despawn(item_ent);
                                } else if let Some(inv) =
                                    self.world.get_mut::<crate::components::Inventory>(entity)
                                {
                                    inv.items.insert(idx.min(inv.items.len()), item_ent);
                                }
                            }
                        } else {
                            self.log("Invalid item slot!");
                            self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                                reason: "Invalid item slot".to_string(),
                            };
                        }
                    } else {
                        self.log("Inventory must be open to use items!");
                        self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                            reason: "Inventory must be open to use items".to_string(),
                        };
                    }
                } else {
                    self.log("Select a character first to use an item!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "Select a character first".to_string(),
                    };
                }
            }
            Action::ToggleInventory => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                if state.selected_entity.is_some() {
                    if state.ui_state == crate::components::UIState::Inventory {
                        state.ui_state = crate::components::UIState::Normal;
                        self.log("Inventory closed.");
                        self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                            name: "inventory".to_string(),
                            enabled: false,
                        };
                    } else {
                        state.ui_state = crate::components::UIState::Inventory;
                        self.log("Inventory opened. Press [1-9] to use item.");
                        self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                            name: "inventory".to_string(),
                            enabled: true,
                        };
                    }
                } else {
                    self.log("Select a character first to view their inventory!");
                }
            }
            Action::ToggleHelp => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                if state.ui_state == crate::components::UIState::Help {
                    state.ui_state = crate::components::UIState::Normal;
                    self.log("Help closed.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "help".to_string(),
                        enabled: false,
                    };
                } else {
                    state.ui_state = crate::components::UIState::Help;
                    self.log("Help overlay opened. Press [?] or [Esc] to close.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "help".to_string(),
                        enabled: true,
                    };
                }
            }
            Action::EndTurn => {
                let phase = self.world.resource::<GameState>().unwrap().phase;
                if phase == TurnPhase::Player {
                    self.world
                        .resource_mut::<crate::components::TurnTransition>()
                        .unwrap()
                        .request_end = true;
                }
            }
            Action::Inspect(point) => {
                let (width, height) = {
                    let map = self.world.resource::<TacticalMap>().unwrap();
                    (map.width, map.height)
                };
                if point.x >= 0 && point.x < width as i16 && point.y >= 0 && point.y < height as i16
                {
                    let state = self.world.resource_mut::<GameState>().unwrap();
                    state.cursor = point;
                    let (cx, cy) = self.get_tile_center_pixels(point);
                    self.camera.look_at(cx, cy);
                }
            }
            Action::ClearCursor => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.selected_entity = None;
                self.log("Selection cleared.");
            }
            Action::Quit => {
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Quit;
            }
            Action::Save => {
                let base_path = saves_dir();
                let _ = std::fs::create_dir_all(base_path);

                if let Ok(state) = self.save_state() {
                    let filename = "quicksave.json";
                    let path = format!("{}/{}", base_path, filename);
                    match std::fs::write(&path, &state) {
                        Ok(()) => {
                            self.log(format!("Game saved to {}", path));
                            self.last_outcome = ActionOutcome::GameSaved { path: path.clone() };

                            // Also save a timestamped version for manual play history.
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let ts_path = format!("{}/save_{}.json", base_path, now);
                            let _ = std::fs::write(&ts_path, &state);
                        }
                        Err(err) => {
                            let reason = format!("Failed to save game: {}", err);
                            self.log(reason.clone());
                            self.last_outcome = ActionOutcome::Failed { reason };
                        }
                    }
                } else {
                    let reason = "Failed to serialize save state".to_string();
                    self.log(reason.clone());
                    self.last_outcome = ActionOutcome::Failed { reason };
                }
            }
            Action::Load => {
                let base_path = saves_dir();
                let path = format!("{}/quicksave.json", base_path);
                if let Ok(state_str) = std::fs::read_to_string(&path) {
                    if self.load_state(&state_str).is_ok() {
                        self.log(format!("Game loaded from {}", path));
                        self.last_outcome = ActionOutcome::GameLoaded { path };
                    } else {
                        let reason = "Failed to load game state".to_string();
                        self.log(reason.clone());
                        self.last_outcome = ActionOutcome::Failed { reason };
                    }
                } else {
                    let reason = "No save file found".to_string();
                    self.log(reason.clone());
                    self.last_outcome = ActionOutcome::Failed { reason };
                }
            }
            Action::TogglePerf => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.show_perf = !state.show_perf;
                self.last_outcome = ActionOutcome::ToggleChanged {
                    name: "performance_overlay".to_string(),
                    enabled: state.show_perf,
                };
            }
            Action::ToggleMinimap => {
                let show = {
                    let state = self.world.resource_mut::<GameState>().unwrap();
                    state.show_minimap = !state.show_minimap;
                    state.show_minimap
                };
                if show {
                    self.log("Minimap enabled.");
                } else {
                    self.log("Minimap disabled.");
                }
                self.last_outcome = ActionOutcome::ToggleChanged {
                    name: "minimap".to_string(),
                    enabled: show,
                };
            }
            Action::AutoBattle => {
                let current = self.world.resource::<GameState>().unwrap().auto_battle;
                self.world.resource_mut::<GameState>().unwrap().auto_battle = !current;
                if !current {
                    self.log("Auto-Battle ENABLED.");
                } else {
                    self.log("Auto-Battle DISABLED.");
                }
                self.last_outcome = ActionOutcome::ToggleChanged {
                    name: "auto_battle".to_string(),
                    enabled: !current,
                };
            }
            Action::StepToSafety => {
                self.execute_step_to_safety();
            }
            Action::ToggleRecording => {
                if self.router.is_recording() {
                    let records = self
                        .world
                        .resource::<verryte_input::ActionHistory<Action>>()
                        .map(|history| history.len())
                        .unwrap_or(0);
                    self.router.stop_recording();
                    self.world.resource_mut::<GameState>().unwrap().is_recording = false;
                    self.log("Action recording STOPPED.");
                    let base_path = saves_dir();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if let Some(history) = self
                        .world
                        .resource::<verryte_input::ActionHistory<Action>>()
                    {
                        let path = format!("{}/trace_{}.json", base_path, now);
                        let _ = history.save_to_file(&path);
                        let last_path = format!("{}/last_recording.json", base_path);
                        if history.save_to_file(&last_path).is_ok() {
                            self.log(format!("Action history saved to {}", last_path));
                        } else {
                            self.log("Failed to save action history!");
                        }
                    }
                    self.last_outcome = ActionOutcome::RecordingChanged {
                        enabled: false,
                        records,
                    };
                } else {
                    self.router.clear_history();
                    if let Some(history) = self
                        .world
                        .resource_mut::<verryte_input::ActionHistory<Action>>()
                    {
                        history.clear();
                    }
                    let base_path = saves_dir();
                    let path = format!("{}/last_recording.json", base_path);
                    self.router.start_recording(path);
                    self.world.resource_mut::<GameState>().unwrap().is_recording = true;
                    self.log("Action recording STARTED.");
                    self.last_outcome = ActionOutcome::RecordingChanged {
                        enabled: true,
                        records: 0,
                    };
                }
            }

            Action::ToggleReplay => {
                let (active, actions, errors, msg) = {
                    let replay = self
                        .world
                        .resource_mut::<crate::components::ReplayState>()
                        .unwrap();
                    if replay.active {
                        let actions = replay.trace.steps().len();
                        let errors = replay.verification_errors.len();
                        replay.active = false;
                        (false, actions, errors, "Replay mode DISABLED.".to_string())
                    } else {
                        // Try to load last_recording.json
                        let base_path = saves_dir();
                        let path = format!("{}/last_recording.json", base_path);
                        if let Ok(history) =
                            verryte_input::ActionHistory::<Action>::load_from_file(&path)
                        {
                            let mut expected = Vec::new();
                            for record in history.iter() {
                                let outcome = if let Some(outcome_str) =
                                    record.metadata.get("outcome")
                                {
                                    serde_json::from_str(outcome_str).unwrap_or(ActionOutcome::NoOp)
                                } else {
                                    ActionOutcome::NoOp
                                };
                                expected.push(outcome);
                            }
                            replay.expected_outcomes = expected;
                            replay.verification_errors.clear();
                            replay.trace =
                                verryte_input::ActionTrace::from_records(&history.records);
                            replay.active = true;
                            replay.next_index = 0;
                            replay.auto = false;
                            let actions = replay.trace.steps().len();
                            (
                                true,
                                actions,
                                0,
                                format!("Replay mode ENABLED. Trace loaded ({} actions).", actions),
                            )
                        } else {
                            (
                                false,
                                0,
                                0,
                                "No last_recording.json found to replay!".to_string(),
                            )
                        }
                    }
                };

                self.log(msg);
                if active {
                    self.log("Press F12 to step through replay.");
                    self.last_outcome = ActionOutcome::ReplayChanged {
                        enabled: true,
                        actions,
                        errors,
                    };
                } else if actions > 0 || errors > 0 {
                    self.last_outcome = ActionOutcome::ReplayChanged {
                        enabled: false,
                        actions,
                        errors,
                    };
                } else {
                    self.last_outcome = ActionOutcome::Failed {
                        reason: "No last_recording.json found to replay".to_string(),
                    };
                }
            }
            Action::StepReplay => {
                let next_step = {
                    let replay = self
                        .world
                        .resource_mut::<crate::components::ReplayState>()
                        .unwrap();
                    if replay.active {
                        if let Some(step) = replay.trace.steps().get(replay.next_index) {
                            let action = step.action;
                            let source = step.source;
                            let index = replay.next_index;
                            replay.next_index += 1;
                            Some((index, action, source))
                        } else {
                            replay.active = false;
                            None
                        }
                    } else {
                        None
                    }
                };

                match next_step {
                    Some((index, action, source)) => {
                        self.log(format!("Replaying action {}: {:?}", index, action));
                        let report = self.apply_action(action, source);
                        let mut verified = true;
                        let expected = {
                            let replay = self
                                .world
                                .resource::<crate::components::ReplayState>()
                                .unwrap();
                            replay.expected_outcomes.get(index).cloned()
                        };
                        if let Some(expected) = expected {
                            if expected != report.outcome {
                                verified = false;
                                let err = format!(
                                    "Replay step {} outcome mismatch: expected {:?}, got {:?}",
                                    index, expected, report.outcome
                                );
                                self.log(format!("[fg:FF5555]Validation Error: {}[/fg]", err));
                                let replay = self
                                    .world
                                    .resource_mut::<crate::components::ReplayState>()
                                    .unwrap();
                                replay.verification_errors.push(err);
                            }
                        }
                        self.last_outcome = ActionOutcome::ReplayStepped {
                            index,
                            action: format!("{:?}", action),
                            verified,
                        };
                    }

                    None => {
                        let (was_active, actions, errors) = {
                            let replay = self
                                .world
                                .resource::<crate::components::ReplayState>()
                                .unwrap();
                            (
                                replay.next_index >= replay.trace.steps().len()
                                    && !replay.trace.steps().is_empty(),
                                replay.trace.steps().len(),
                                replay.verification_errors.len(),
                            )
                        };
                        if was_active {
                            self.log("End of replay trace reached.");
                            self.world
                                .resource_mut::<crate::components::ReplayState>()
                                .unwrap()
                                .active = false;
                            self.last_outcome = ActionOutcome::ReplayChanged {
                                enabled: false,
                                actions,
                                errors,
                            };
                        } else {
                            self.log("Enable Replay mode first (F11)!");
                            self.last_outcome = ActionOutcome::Failed {
                                reason: "Enable Replay mode first".to_string(),
                            };
                        }
                    }
                }
            }

            Action::ToggleReplayAuto => {
                let msg = {
                    let replay = self
                        .world
                        .resource_mut::<crate::components::ReplayState>()
                        .unwrap();
                    if replay.active {
                        replay.auto = !replay.auto;
                        if replay.auto {
                            Some("Replay AUTO-PLAY enabled.")
                        } else {
                            Some("Replay AUTO-PLAY disabled.")
                        }
                    } else {
                        None
                    }
                };
                if let Some(m) = msg {
                    self.log(m);
                    self.last_outcome = ActionOutcome::ReplayAutoChanged {
                        enabled: m.contains("enabled"),
                    };
                } else {
                    self.last_outcome = ActionOutcome::Failed {
                        reason: "Replay mode must be active to toggle auto-play".to_string(),
                    };
                }
            }
            Action::NextFloor => {
                let sel_entity = self.world.resource::<GameState>().unwrap().selected_entity;
                if let Some(entity) = sel_entity {
                    let pos = *self.world.get::<Position>(entity).unwrap();
                    let tile = self
                        .world
                        .resource::<TacticalMap>()
                        .unwrap()
                        .tile(pos.x, pos.y);
                    if tile == Tile::Stairs {
                        self.transition_to_next_floor();
                    } else {
                        self.log("You must stand on a staircase to descend!");
                        self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                            reason: "You must stand on a staircase to descend".to_string(),
                        };
                    }
                } else {
                    self.log("Select a character first!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "Select a character first".to_string(),
                    };
                }
            }
            Action::ChangeWeather(weather_type) => {
                if let Some(w) = self.world.resource_mut::<crate::components::Weather>() {
                    w.current = weather_type;
                }
                self.update_weather_ambient(weather_type);
                self.log(format!("Weather changed to {:?}.", weather_type));
            }
            Action::CraftItem(idx1, idx2) => {
                let sel_entity = self.world.resource::<GameState>().unwrap().selected_entity;
                if let Some(entity) = sel_entity {
                    let items_to_craft = {
                        if let Some(inv) = self.world.get::<crate::components::Inventory>(entity) {
                            if idx1 < inv.items.len() && idx2 < inv.items.len() && idx1 != idx2 {
                                Some((inv.items[idx1], inv.items[idx2]))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some((item1_ent, item2_ent)) = items_to_craft {
                        let item1_name = self
                            .world
                            .get::<crate::components::Item>(item1_ent)
                            .map(|i| i.name.clone());
                        let item2_name = self
                            .world
                            .get::<crate::components::Item>(item2_ent)
                            .map(|i| i.name.clone());
                        if let (Some(name1), Some(name2)) = (item1_name, item2_name) {
                            let (matched, new_name, new_effect) = match (&name1[..], &name2[..]) {
                                ("Healing Potion", "Healing Potion") => (
                                    true,
                                    "Mega Potion".to_string(),
                                    crate::components::ItemEffect::Heal(75),
                                ),
                                ("Energy Elixir", "Energy Elixir") => (
                                    true,
                                    "Mega Energy Elixir".to_string(),
                                    crate::components::ItemEffect::ReplenishAp(4),
                                ),
                                ("Healing Potion", "Energy Elixir")
                                | ("Energy Elixir", "Healing Potion") => (
                                    true,
                                    "Elixir of Life".to_string(),
                                    crate::components::ItemEffect::Combined(40, 2),
                                ),
                                ("Healing Potion", "Cleanse Remedy")
                                | ("Cleanse Remedy", "Healing Potion") => (
                                    true,
                                    "Aegis Elixir".to_string(),
                                    crate::components::ItemEffect::RestoreShield(
                                        crate::components::ShieldType::Physical,
                                        30,
                                    ),
                                ),
                                ("Cleanse Remedy", "Mega Potion")
                                | ("Mega Potion", "Cleanse Remedy") => (
                                    true,
                                    "Divine Remedy".to_string(),
                                    crate::components::ItemEffect::CleanseAndHeal(80),
                                ),
                                ("Mega Potion", "Mega Energy Elixir")
                                | ("Mega Energy Elixir", "Mega Potion") => (
                                    true,
                                    "Elixir of the Gods".to_string(),
                                    crate::components::ItemEffect::Combined(100, 4),
                                ),
                                _ => (false, String::new(), crate::components::ItemEffect::Cleanse),
                            };

                            if matched {
                                if let Some(inv) =
                                    self.world.get_mut::<crate::components::Inventory>(entity)
                                {
                                    inv.items.retain(|&e| e != item1_ent && e != item2_ent);
                                }
                                self.world.despawn(item1_ent);
                                self.world.despawn(item2_ent);

                                let new_item_ent = self.world.spawn_item(&new_name, new_effect);
                                if let Some(inv) =
                                    self.world.get_mut::<crate::components::Inventory>(entity)
                                {
                                    inv.items.push(new_item_ent);
                                }

                                self.log(format!(
                                    "Alchemy success! Crafted {} from {} and {}.",
                                    new_name, name1, name2
                                ));
                                self.last_outcome = crate::snapshot::ActionOutcome::Crafted {
                                    item_name: new_name.clone(),
                                };

                                let (tcx, tcy) = self.get_tile_center_pixels(
                                    *self.world.get::<Position>(entity).unwrap(),
                                );
                                self.vfx_mut()
                                    .particles
                                    .extend(verryte_terminal::vfx::emit_bloom(tcx, tcy, 20));
                                if let Some(pos) = self.world.get::<Position>(entity).copied() {
                                    self.play_spatial_sfx("item_craft", pos);
                                }
                            } else {
                                self.log("No valid recipe for those items!");
                                self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                                    reason: "No valid recipe for those items".to_string(),
                                };
                            }
                        }
                    } else {
                        self.log("Invalid crafting slots chosen!");
                        self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                            reason: "Invalid crafting slots chosen".to_string(),
                        };
                    }
                } else {
                    self.log("Select a character first!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "Select a character first".to_string(),
                    };
                }
            }
            Action::UpgradeEquipment(slot) => {
                let selected = self.world.resource::<GameState>().unwrap().selected_entity;
                let Some(entity) = selected else {
                    self.log("Select a character first to upgrade equipment!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "Select a character first".to_string(),
                    };
                    return;
                };

                let upgrade_kit = self
                    .world
                    .get::<crate::components::Inventory>(entity)
                    .and_then(|inventory| {
                        inventory.items.iter().copied().find(|item_entity| {
                            self.world
                                .get::<crate::components::Item>(*item_entity)
                                .is_some_and(|item| {
                                    item.effect == crate::components::ItemEffect::UpgradeKit
                                })
                        })
                    });
                let Some(upgrade_kit) = upgrade_kit else {
                    self.log("No Upgrade Kit available!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "No Upgrade Kit available".to_string(),
                    };
                    return;
                };

                let mut upgraded_name = None;
                if let Some(equipped) = self.world.get_mut::<EquippedItems>(entity) {
                    let item = match slot {
                        crate::components::EquipmentSlot::Weapon => equipped.weapon.as_mut(),
                        crate::components::EquipmentSlot::Armor => equipped.armor.as_mut(),
                        crate::components::EquipmentSlot::Accessory => equipped.accessory.as_mut(),
                    };
                    if let Some(item) = item {
                        if crate::equipment::upgrade_equipment(item) {
                            upgraded_name = Some(format!("{} +{}", item.name, item.upgrade_level));
                        }
                    }
                }

                let Some(upgraded_name) = upgraded_name else {
                    self.log("No upgradeable equipment in that slot!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "No upgradeable equipment in that slot".to_string(),
                    };
                    return;
                };

                if let Some(inventory) = self.world.get_mut::<crate::components::Inventory>(entity)
                {
                    inventory.items.retain(|&item| item != upgrade_kit);
                }
                self.world.despawn(upgrade_kit);
                self.log(format!("Upgraded {}.", upgraded_name));
                self.last_outcome = crate::snapshot::ActionOutcome::StateUpdated;
            }
            Action::RerollModifiers => {
                let cost = 1i32;
                let sel_entity = self.world.resource::<GameState>().unwrap().selected_entity;
                if let Some(entity) = sel_entity {
                    let ap = self.world.get::<Stats>(entity).map(|s| s.ap).unwrap_or(0);
                    if ap >= cost {
                        if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                            stats.ap -= cost;
                        }
                        self.log("[fg:FF00FF]Rerolling floor modifiers (-1 AP)...[/fg]");
                        crate::systems::select_floor_modifiers(&mut self.world);
                        let modifiers = self
                            .world
                            .resource::<crate::components::ActiveFloorModifiers>()
                            .map(|active| {
                                active
                                    .modifiers
                                    .iter()
                                    .map(|modifier| modifier.display_name().to_string())
                                    .collect()
                            })
                            .unwrap_or_default();
                        self.last_outcome =
                            crate::snapshot::ActionOutcome::ModifiersRerolled { modifiers };
                    } else {
                        self.log("Not enough AP to reroll modifiers! (Costs 1 AP)");
                        self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                            reason: "Not enough AP to reroll modifiers".to_string(),
                        };
                    }
                } else {
                    self.log("Select a character first to reroll modifiers!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "Select a character first".to_string(),
                    };
                }
            }
            Action::Rest => {
                let sel_entity = self.world.resource::<GameState>().unwrap().selected_entity;
                if let Some(entity) = sel_entity {
                    let before_fatigue = self
                        .world
                        .get::<crate::components::Fatigue>(entity)
                        .map(|fatigue| fatigue.value)
                        .unwrap_or_default();
                    let before_morale = self
                        .world
                        .get::<crate::components::Morale>(entity)
                        .map(|morale| morale.value)
                        .unwrap_or_default();
                    if let Some(fatigue) = self.world.get_mut::<crate::components::Fatigue>(entity)
                    {
                        fatigue.value = (fatigue.value - 20).max(0);
                    }
                    if let Some(morale) = self.world.get_mut::<crate::components::Morale>(entity) {
                        morale.value = (morale.value + 5).min(morale.max);
                    }
                    let name = self
                        .world
                        .get::<CharacterClass>(entity)
                        .map(|c| Game::get_class_name(*c).to_string())
                        .unwrap_or_default();
                    self.log(format!("{} rests and recovers stamina.", name));
                    self.world
                        .resource_mut::<GameState>()
                        .unwrap()
                        .selected_entity = None;
                    let after_fatigue = self
                        .world
                        .get::<crate::components::Fatigue>(entity)
                        .map(|fatigue| fatigue.value)
                        .unwrap_or_default();
                    let after_morale = self
                        .world
                        .get::<crate::components::Morale>(entity)
                        .map(|morale| morale.value)
                        .unwrap_or_default();
                    self.last_outcome = crate::snapshot::ActionOutcome::Rested {
                        entity: name,
                        fatigue_recovered: before_fatigue - after_fatigue,
                        morale_gained: after_morale - before_morale,
                    };
                } else {
                    self.log("Select a character first to rest!");
                    self.last_outcome = crate::snapshot::ActionOutcome::Failed {
                        reason: "Select a character first".to_string(),
                    };
                }
            }
            Action::ComboSkill(skill) => {
                let available = self
                    .world
                    .resource::<crate::components::AvailableCombos>()
                    .cloned()
                    .unwrap_or_default();
                let found = available.combos.iter().find(|(s, _)| *s == skill).cloned();
                if let Some((_skill, participants)) = found {
                    let def = crate::components::ComboSkillDef::for_skill(&skill);
                    let all_have_ap = participants.iter().all(|&e| {
                        self.world
                            .get::<Stats>(e)
                            .is_some_and(|s| s.ap >= def.ap_cost)
                    });
                    if !all_have_ap {
                        self.log("Not enough AP from all participants for this combo!");
                    } else {
                        let cursor = self.world.resource::<GameState>().unwrap().cursor;
                        self.execute_combo_skill(skill, &participants, cursor);
                    }
                } else {
                    self.log("That combo skill is not currently available!");
                }
            }
            Action::ToggleBestiary => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                if state.ui_state == crate::components::UIState::Bestiary {
                    state.ui_state = crate::components::UIState::Normal;
                    self.log("Bestiary closed.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "bestiary".to_string(),
                        enabled: false,
                    };
                } else {
                    state.ui_state = crate::components::UIState::Bestiary;
                    self.log("Bestiary opened. Press [J] or [Esc] to close.");
                    self.last_outcome = crate::snapshot::ActionOutcome::ToggleChanged {
                        name: "bestiary".to_string(),
                        enabled: true,
                    };
                }
            }
            Action::ViewPrestige => {
                let mut prestige_data: Vec<(String, String, String)> = Vec::new();
                let entity_classes: Vec<(Entity, CharacterClass)> = self
                    .world
                    .query2::<CharacterClass, crate::components::PrestigeProgress>()
                    .iter()
                    .filter(|(e, _, _)| {
                        self.world.get::<Team>(*e).copied().unwrap_or(Team::Enemy) == Team::Player
                    })
                    .map(|(e, class, _)| (*e, *(*class)))
                    .collect::<Vec<_>>();
                for (e, class) in entity_classes {
                    if let Some(progress) = self.world.get::<crate::components::PrestigeProgress>(e)
                    {
                        let name = Self::get_class_name(class).to_string();
                        let prestige_name = if progress.promoted {
                            progress.class.display_name().to_string()
                        } else {
                            "Not yet promoted".to_string()
                        };
                        let req = match class {
                            CharacterClass::Warrior => format!("Kills: {}/10", progress.kill_count),
                            CharacterClass::Mage => {
                                format!("Damage: {}/500", progress.total_damage_dealt)
                            }
                            CharacterClass::Healer => {
                                format!("Healing: {}/300", progress.total_healing_done)
                            }
                            _ => String::new(),
                        };
                        prestige_data.push((name, prestige_name, req));
                    }
                }
                self.log("[fg:FFD700][b]--- Prestige Status ---[/][/fg]");
                for (name, prestige_name, req) in prestige_data {
                    self.log(format!("{}: [b]{}[/] ({})", name, prestige_name, req));
                }
                self.log("[fg:FFD700]-----------------------[/fg]");
                self.last_outcome = crate::snapshot::ActionOutcome::StatusViewed {
                    name: "prestige".to_string(),
                };
            }
            _ => {}
        }

        let is_skill = matches!(action, Action::Skill1 | Action::Skill2 | Action::Skill3);
        let is_wait = action == Action::Wait;
        let is_rest = action == Action::Rest;
        if (is_skill || is_wait || matches!(action, Action::Confirm)) && !is_rest {
            if let Some(sel) = self.world.resource::<GameState>().unwrap().selected_entity {
                crate::systems::increment_fatigue_on_action(
                    &mut self.world,
                    sel,
                    is_skill,
                    is_wait,
                );
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        if let Some(clock) = self.world.resource_mut::<GameClock>() {
            clock.tick();
        }
        if let Some(vfx) = self
            .world
            .resource_mut::<verryte_terminal::vfx::VfxSystem>()
        {
            vfx.update(dt);
        }
        if let Some(registry) = self
            .world
            .resource_mut::<verryte_terminal::VisualRegistry>()
        {
            registry.tick();
        }
        if let Some(dialogue) = self.world.resource_mut::<verryte_terminal::DialogueState>() {
            let newly_typed = dialogue.update(dt, 30.0);
            if newly_typed > 0 {
                if let Some(events) = self
                    .world
                    .resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("dialogue_blip"));
                }
            }
        }
        let mut rng = *self.world.resource::<Rng>().unwrap();
        self.camera.tick(&mut rng);
        self.world.insert_resource(self.camera.clone());
        self.world.insert_resource(rng);

        // Replay auto-step
        let mut replay_step = false;
        if let Some(replay) = self.world.resource_mut::<crate::components::ReplayState>() {
            if replay.active && replay.auto {
                replay_step = true;
            }
        }
        if replay_step {
            self.apply_action(Action::StepReplay, ActionSource::Agent);
        }

        // Auto-battle logic
        let (auto, phase, outcome) = {
            let state = self.world.resource::<GameState>().unwrap();
            (state.auto_battle, state.phase, state.outcome)
        };
        if auto && phase == TurnPhase::Player && outcome == Outcome::Playing {
            self.tick_auto_battle();
        }

        // Run systems
        self.schedule.run(&mut self.world);
        verryte_audio::audio_system(&mut self.world);
    }

    fn tick_auto_battle(&mut self) {
        // Simple AI: pick the first player character with AP and do something
        let player_entities: Vec<Entity> = self
            .world
            .query2::<Team, Stats>()
            .iter()
            .filter(|(_, team, stats)| **team == Team::Player && stats.ap > 0)
            .map(|(e, _, _)| *e)
            .collect();

        if player_entities.is_empty() {
            // No more actions possible, end turn
            self.apply_action(Action::EndTurn, ActionSource::Agent);
            return;
        }

        // Pick one (deterministic for now)
        let entity = player_entities[0];
        let pos = *self.world.get::<Position>(entity).unwrap();
        let class = *self.world.get::<CharacterClass>(entity).unwrap();

        // Check for enemies in range
        let enemies: Vec<(Entity, Position)> = self
            .world
            .query2::<Team, Position>()
            .iter()
            .filter(|(_, team, _)| **team == Team::Enemy)
            .map(|(e, _, p)| (*e, **p))
            .collect();

        let range = match class {
            CharacterClass::Warrior => 1,
            CharacterClass::Mage => 3,
            CharacterClass::Healer => 2,
            _ => 1,
        };

        let mut target: Option<(Entity, Position)> = None;
        for (e_entity, e_pos) in enemies {
            let dist = (pos.x - e_pos.x).abs() + (pos.y - e_pos.y).abs();
            if dist <= range {
                target = Some((e_entity, e_pos));
                break;
            }
        }

        if let Some((_, e_pos)) = target {
            // Move cursor to enemy and confirm attack
            let state = self.world.resource_mut::<GameState>().unwrap();
            state.selected_entity = Some(entity);
            state.cursor = e_pos;
            self.apply_action(Action::Confirm, ActionSource::Agent);
        } else {
            // Move towards nearest enemy
            let mut nearest_enemy: Option<Position> = None;
            let mut min_dist = i32::MAX;
            for (_, _team, e_pos) in self
                .world
                .query2::<Team, Position>()
                .iter()
                .filter(|(_, team, _)| **team == Team::Enemy)
            {
                let dist = (pos.x - e_pos.x).abs() as i32 + (pos.y - e_pos.y).abs() as i32;
                if dist < min_dist {
                    min_dist = dist;
                    nearest_enemy = Some(**e_pos);
                }
            }

            if let Some(target_pos) = nearest_enemy {
                let path = self.get_path_to(entity, target_pos);
                if let Some(path) = path {
                    if path.len() > 1 {
                        let next_step = path[1];
                        // Select character and move
                        let state = self.world.resource_mut::<GameState>().unwrap();
                        state.selected_entity = Some(entity);
                        state.cursor = next_step;
                        self.apply_action(Action::Confirm, ActionSource::Agent);
                        return;
                    }
                }
            }
            // If no path or stuck, just wait
            self.apply_action(Action::Wait, ActionSource::Agent);
        }
    }

    fn execute_step_to_safety(&mut self) {
        let (sel_entity, pos) = {
            let state = self.world.resource::<GameState>().unwrap();
            let sel = state.selected_entity;
            if sel.is_none() {
                self.log("Select a character first!");
                return;
            }
            let pos = self.world.get::<Position>(sel.unwrap()).copied().unwrap();
            (sel.unwrap(), pos)
        };

        let telegraph_zone = self
            .world
            .resource::<crate::components::TelegraphZone>()
            .unwrap();
        if telegraph_zone.tiles.is_empty() {
            self.log("No danger zones telegraphed.");
            return;
        }

        if !telegraph_zone.tiles.contains(&pos) {
            self.log("Character is already in a safe position.");
            return;
        }

        // Find nearest safe tile
        let reachable = self.get_reachable_tiles(sel_entity);
        let mut best_safe: Option<Position> = None;
        let mut min_dist = i32::MAX;

        for r_pos in reachable {
            if !telegraph_zone.tiles.contains(&r_pos) {
                let dist = (r_pos.x - pos.x).abs() as i32 + (r_pos.y - pos.y).abs() as i32;
                if dist < min_dist {
                    min_dist = dist;
                    best_safe = Some(r_pos);
                }
            }
        }

        if let Some(safe_pos) = best_safe {
            self.log(format!(
                "Stepping to safety at {},{}...",
                safe_pos.x, safe_pos.y
            ));
            let mut ap_ok = false;
            if let Some(stats) = self.world.get_mut::<Stats>(sel_entity) {
                if stats.ap >= 1 {
                    stats.ap -= 1;
                    ap_ok = true;
                }
            }

            if ap_ok {
                if let Some(p) = self.world.get_mut::<Position>(sel_entity) {
                    *p = safe_pos;
                }
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.cursor = safe_pos;
                let (cx, cy) = self.get_tile_center_pixels(safe_pos);
                self.camera.look_at(cx, cy);
            } else {
                self.log("Not enough AP to step to safety!");
            }
        } else {
            self.log("No reachable safe tiles found!");
        }
    }

    pub fn render(&self) -> Grid {
        let map = self.world.resource::<TacticalMap>().unwrap();
        let state = self.world.resource::<GameState>().unwrap();
        let registry = self.world.resource::<VisualRegistry>().unwrap();
        let visibility = self.world.resource::<verryte_map::VisibilityMap>().unwrap();
        let clock = self.world.resource::<GameClock>().unwrap();

        // Determine resolution tier and tile dimensions dynamically
        let (term_w, term_h) = verryte_tty::terminal_size();
        let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
        let (tile_w, tile_h) = tier.tile_dimensions();

        // Screen layout
        let hud_h = 6;
        let board_h = term_h.saturating_sub(hud_h);
        let mut screen = Grid::new(term_w, term_h);

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

        // 1. Render Tiles (culled by visibility)
        let (start_x, start_y, end_x, end_y) = viewport.visible_tiles(map.width, map.height);

        for ty in start_y..end_y {
            for tx in start_x..end_x {
                let pos = Position::new(tx, ty);
                let vis = visibility.get(pos);
                if matches!(vis, verryte_map::Visibility::Hidden) {
                    continue;
                }

                let tile = map.tile(tx, ty);
                let mut color = match tile {
                    Tile::Grass => Color(30, 80, 30),
                    Tile::Wall => Color(60, 60, 60),
                    Tile::Water => Color(30, 30, 100),
                    Tile::Lava => Color(120, 20, 10),
                    Tile::Ice => Color(100, 180, 200),
                    Tile::Stairs => Color(160, 120, 40),
                    Tile::Mud => Color(80, 50, 30),
                    Tile::SpikeTrap => Color(180, 30, 30),
                    Tile::PoisonCloud => Color(80, 30, 120),
                    Tile::HealingSpring => Color(30, 160, 80),
                    Tile::CrackedFloor => Color(90, 70, 50),
                    Tile::PressurePlate => Color(140, 140, 60),
                    Tile::ThornBush => Color(50, 100, 20),
                };

                if matches!(vis, verryte_map::Visibility::Explored) {
                    color = verryte_terminal::vfx::blend_color(color, Color::BLACK, 0.6);
                }

                let (sx, sy) = viewport.world_to_screen(tx as f32, ty as f32);
                let ticks = clock.elapsed_ticks();

                // Draw tile background/border
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        let tx_abs = sx + dx as i32;
                        let ty_abs = sy + dy as i32;

                        if viewport.rect.contains(tx_abs as u16, ty_abs as u16) {
                            let glyph = match tile {
                                Tile::Water => {
                                    let phase =
                                        ((ticks + (tx as u64) * 3 + (ty as u64) * 7) / 10) % 3;
                                    match phase {
                                        0 => '~',
                                        1 => '≈',
                                        _ => '∽',
                                    }
                                }
                                Tile::Lava => {
                                    let phase =
                                        ((ticks + (tx as u64) * 3 + (ty as u64) * 7) / 8) % 3;
                                    match phase {
                                        0 => '^',
                                        1 => 'v',
                                        _ => '*',
                                    }
                                }
                                _ => {
                                    let is_ice = matches!(tile, Tile::Ice);
                                    let is_stairs = matches!(tile, Tile::Stairs);
                                    let is_mud = matches!(tile, Tile::Mud);
                                    if dx == 0 || dy == 0 {
                                        if is_ice {
                                            '-'
                                        } else if is_stairs {
                                            '>'
                                        } else if is_mud {
                                            '='
                                        } else {
                                            '·'
                                        }
                                    } else {
                                        ' '
                                    }
                                }
                            };
                            let mut fg = match tile {
                                Tile::Water => Color(80, 80, 180),
                                Tile::Lava => Color(240, 100, 20),
                                Tile::Ice => Color(200, 240, 255),
                                Tile::Stairs => Color(255, 215, 0),
                                Tile::Mud => Color(140, 90, 50),
                                _ => Color(40, 40, 40),
                            };
                            if matches!(vis, verryte_map::Visibility::Explored) {
                                fg = verryte_terminal::vfx::blend_color(fg, Color::BLACK, 0.6);
                            }
                            screen.put(
                                tx_abs as u16,
                                ty_abs as u16,
                                Cell::new(glyph).with_fg(fg).with_bg(color),
                            );
                        }
                    }
                }
            }
        }

        // 2. Overlays (Range, Path, Telegraphs)
        if state.targeting == crate::components::TargetingMode::None {
            if let Some(sel_entity) = state.selected_entity {
                let reachable = self.get_reachable_tiles(sel_entity);
                for pos in &reachable {
                    let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                    for dy in 0..tile_h {
                        for dx in 0..tile_w {
                            let tx = sx + dx as i32;
                            let ty = sy + dy as i32;
                            if viewport.rect.contains(tx as u16, ty as u16) {
                                let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                                cell.bg = verryte_terminal::vfx::blend_color(
                                    cell.bg,
                                    Color(0, 100, 150),
                                    0.35,
                                );
                            }
                        }
                    }
                }

                // Draw path preview
                if reachable.contains(&state.cursor) {
                    if let Some(path) = self.get_path_to(sel_entity, state.cursor) {
                        for pos in path {
                            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                            for dy in 0..tile_h {
                                for dx in 0..tile_w {
                                    let tx = sx + dx as i32;
                                    let ty = sy + dy as i32;
                                    if viewport.rect.contains(tx as u16, ty as u16) {
                                        let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                                        cell.bg = verryte_terminal::vfx::blend_color(
                                            cell.bg,
                                            Color(0, 150, 220),
                                            0.4,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Targeting mode range
            if let Some(sel_entity) = state.selected_entity {
                if let (Some(caster_pos), Some(class)) = (
                    self.world.get::<Position>(sel_entity),
                    self.world.get::<CharacterClass>(sel_entity),
                ) {
                    let (_name, range, _ap, _aoe, _power) = Self::get_skill_info(
                        *class,
                        state.targeting,
                    )
                    .unwrap_or(("Unknown".to_string(), 0, 0, false, 0));

                    for ty in 0..map.height {
                        for tx in 0..map.width {
                            let target = Position::new(tx as i16, ty as i16);
                            let dist =
                                (caster_pos.x - target.x).abs() + (caster_pos.y - target.y).abs();
                            if dist <= range {
                                let (sx, sy) = viewport.world_to_screen(tx as f32, ty as f32);
                                for dy in 0..tile_h {
                                    for dx in 0..tile_w {
                                        let tx_abs = sx + dx as i32;
                                        let ty_abs = sy + dy as i32;
                                        if viewport.rect.contains(tx_abs as u16, ty_abs as u16) {
                                            let cell = screen
                                                .get_mut(tx_abs as u16, ty_abs as u16)
                                                .unwrap();
                                            cell.bg = verryte_terminal::vfx::blend_color(
                                                cell.bg,
                                                Color(50, 150, 50),
                                                0.35,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // AoE Preview under cursor
                    let dist = (caster_pos.x - state.cursor.x).abs()
                        + (caster_pos.y - state.cursor.y).abs();
                    if dist <= range {
                        let aoe_tiles = Self::get_skill_aoe(*class, state.targeting, state.cursor);
                        for pos in aoe_tiles {
                            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                            for dy in 0..tile_h {
                                for dx in 0..tile_w {
                                    let tx = sx + dx as i32;
                                    let ty = sy + dy as i32;
                                    if viewport.rect.contains(tx as u16, ty as u16) {
                                        if let Some(cell) = screen.get_mut(tx as u16, ty as u16) {
                                            cell.bg = verryte_terminal::vfx::blend_color(
                                                cell.bg,
                                                Color(200, 200, 50),
                                                0.5,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Telegraph Zones
        let telegraph_zone = self
            .world
            .resource::<crate::components::TelegraphZone>()
            .unwrap();
        for pos in &telegraph_zone.tiles {
            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
            for dy in 0..tile_h {
                for dx in 0..tile_w {
                    let tx = sx + dx as i32;
                    let ty = sy + dy as i32;
                    if viewport.rect.contains(tx as u16, ty as u16) {
                        let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                        cell.bg =
                            verryte_terminal::vfx::blend_color(cell.bg, Color(150, 0, 75), 0.4);
                    }
                }
            }
        }

        // 3b. Weather Danger Zones
        crate::ui::render_weather_danger_zones(
            &mut screen,
            &self.world,
            &viewport,
            tile_w,
            tile_h,
            clock.elapsed_ticks(),
        );

        // 4. Cursor
        let pulse = ((clock.elapsed_ticks() as f32 * 0.1).sin() * 0.5 + 0.5) * 0.6 + 0.2; // 0.2 to 0.8
        let (csx, csy) = viewport.world_to_screen(state.cursor.x as f32, state.cursor.y as f32);
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                let tx = csx + dx as i32;
                let ty = csy + dy as i32;
                if viewport.rect.contains(tx as u16, ty as u16) {
                    let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                    let is_border = dx == 0 || dy == 0 || dx == tile_w - 1 || dy == tile_h - 1;
                    if is_border {
                        cell.bg =
                            verryte_terminal::vfx::blend_color(cell.bg, Color(255, 255, 0), pulse);
                    } else {
                        cell.bg = verryte_terminal::vfx::blend_color(
                            cell.bg,
                            Color(150, 150, 0),
                            pulse * 0.5,
                        );
                    }
                }
            }
        }

        // 5. Render Entities
        for (entity, pos, team, class) in self.world.query3::<Position, Team, CharacterClass>() {
            // Check visibility
            let vis = visibility.get(*pos);
            if *team != Team::Player && !matches!(vis, verryte_map::Visibility::Visible) {
                continue;
            }

            let key = match class {
                CharacterClass::Warrior => "kael",
                CharacterClass::Mage => "lyra",
                CharacterClass::Healer => "mira",
                CharacterClass::Boss => "blight-sovereign",
                CharacterClass::ShadowStalker => "lyra",
                CharacterClass::CorruptedSpore => "blight-sovereign",
                CharacterClass::CursedSentinel => "kael",
                CharacterClass::PlagueWraith => "lyra",
                CharacterClass::GlacialGolem => "blight-sovereign",
                CharacterClass::EnemyCleric => "mira",
            };

            if let Some(asset) = registry.get(key) {
                let sprite_grid = asset.render(tier);
                viewport.blit_sprite(&mut screen, pos.x as f32, pos.y as f32, sprite_grid);

                // HP Bar
                if let Some(stats) = self.world.get::<Stats>(entity) {
                    let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                    let bar_w = tile_w.min(10);
                    let hp_ratio = stats.hp as f32 / stats.max_hp as f32;
                    let fill_w = (bar_w as f32 * hp_ratio).round() as u16;

                    let bar_x = sx + (tile_w as i32 - bar_w as i32) / 2;
                    let bar_y = sy + tile_h as i32 - 1;

                    for i in 0..bar_w {
                        let tx = viewport.rect.x as i32 + bar_x + i as i32;
                        let ty = viewport.rect.y as i32 + bar_y;
                        if viewport.rect.contains(tx as u16, ty as u16) {
                            let color = if i < fill_w {
                                Color(0, 255, 0)
                            } else {
                                Color(100, 0, 0)
                            };
                            screen.put(tx as u16, ty as u16, Cell::new('=').with_fg(color));
                        }
                    }

                    // Render Shield Bar right above HP Bar
                    if let Some(shield) =
                        self.world.get::<crate::components::ElementalShield>(entity)
                    {
                        if shield.amount > 0 {
                            let sh_ratio = shield.amount as f32 / shield.max_amount as f32;
                            let sh_fill_w = (bar_w as f32 * sh_ratio).round() as u16;
                            let sh_color = match shield.shield_type {
                                crate::components::ShieldType::Ice => Color(100, 200, 255),
                                crate::components::ShieldType::Lightning => Color(255, 255, 0),
                                crate::components::ShieldType::Nature => Color(0, 255, 100),
                                crate::components::ShieldType::Physical => Color(200, 200, 200),
                            };
                            let sh_y = sy + tile_h as i32 - 2;
                            for i in 0..bar_w {
                                let tx = viewport.rect.x as i32 + bar_x + i as i32;
                                let ty = viewport.rect.y as i32 + sh_y;
                                if viewport.rect.contains(tx as u16, ty as u16) {
                                    let color = if i < sh_fill_w {
                                        sh_color
                                    } else {
                                        Color(50, 50, 50)
                                    };
                                    screen.put(tx as u16, ty as u16, Cell::new('-').with_fg(color));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Render Echo items
        for (_e, pos, _echo) in self.world.query2::<Position, crate::components::EchoItem>() {
            let vis = visibility.get(*pos);
            if matches!(vis, verryte_map::Visibility::Hidden) {
                continue;
            }
            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
            let tx = sx + tile_w as i32 / 2;
            let ty = sy + tile_h as i32 / 2;
            if viewport.rect.contains(tx as u16, ty as u16) {
                screen.put(
                    tx as u16,
                    ty as u16,
                    Cell::new('Ω')
                        .with_fg(Color(180, 50, 255))
                        .with_bg(Color::BLACK)
                        .with_attrs(verryte_terminal::CellAttrs::NONE.bold()),
                );
            }
        }

        // 6. VFX
        self.vfx().render_world(&mut screen, &viewport);

        // 7. Dialogue
        if let Some(dialogue) = self.world.resource::<verryte_terminal::DialogueState>() {
            if !dialogue.text.is_empty() {
                let dialog_w = term_w.min(60);
                let dialog_h = 8;
                let dialog_x = (term_w - dialog_w) / 2;
                let dialog_y = (term_h - dialog_h) / 2;

                let theme = match dialogue.title.as_str() {
                    "Blight Sovereign" => verryte_terminal::DialogueTheme::Blood,
                    "Kael" => verryte_terminal::DialogueTheme::Frost,
                    "Lyra" => verryte_terminal::DialogueTheme::Arcane,
                    "Mira" => verryte_terminal::DialogueTheme::Forest,
                    _ => verryte_terminal::DialogueTheme::Dungeon,
                };
                let box_widget = verryte_terminal::DialogueBox::new(verryte_terminal::Rect::new(
                    dialog_x, dialog_y, dialog_w, dialog_h,
                ))
                .with_theme(theme);

                box_widget.render(
                    &mut screen,
                    &dialogue.title,
                    &dialogue.text,
                    dialogue.visible_chars as usize,
                    None,
                    &dialogue.choices,
                    if dialogue.choices.is_empty() {
                        None
                    } else {
                        Some(dialogue.selected_choice)
                    },
                );
            }
        }

        // 8. Minimap
        if state.show_minimap {
            crate::ui::render_minimap(&mut screen, &self.world, board_h);
        }

        // 9. HUD
        crate::ui::render_hud(&mut screen, &self.world, term_w, term_h);

        // 9. Screen Flash
        self.vfx().render_flash(&mut screen, term_w, term_h);

        // 10. Performance Overlay
        if state.show_perf {
            if let Some(diagnostics) = self
                .world
                .resource::<verryte_core::diagnostics::Diagnostics>()
            {
                let perf_widget = verryte_terminal::widgets::PerformanceOverlay::new(
                    verryte_terminal::Rect::new(term_w.saturating_sub(30), 0, 30, 10),
                );
                perf_widget.render(&mut screen, diagnostics);
            }
        }

        screen
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

    /// Snapshot boss phase directly (helper for the outcome derivation).
    fn boss_phase(&self) -> crate::components::BossPhase {
        self.world
            .resource::<GameState>()
            .map(|s| s.boss_phase)
            .unwrap_or(crate::components::BossPhase::Phase1)
    }

    /// True iff the boss crossed into Phase2 during the most recent action.
    fn boss_just_transitioned(&self) -> bool {
        self.boss_transitioned
    }

    pub fn update_weather_ambient(&mut self, weather: crate::components::WeatherType) {
        use crate::components::WeatherType;

        // Emit ambient audio events
        if let Some(events) = self
            .world
            .resource_mut::<Events<verryte_core::AudioEvent>>()
        {
            let (ambient_name, volume) = match weather {
                WeatherType::Rainy => ("ambient_rain", 0.4),
                WeatherType::LightningStorm => ("ambient_thunder", 0.5),
                WeatherType::Sunny => ("ambient_birds", 0.3),
                WeatherType::Snowing => ("ambient_wind", 0.35),
            };
            events.send(verryte_core::AudioEvent::loop_music(ambient_name).with_volume(volume));
        }

        // VFX effects
        match weather {
            WeatherType::Rainy => {
                let (tw, th) = self.get_tile_dimensions();
                let cols = 24u16;
                let rows = 16u16;
                let screen_w = cols * tw;
                let screen_h = rows * th;
                let mut rain = Vec::new();
                for _ in 0..30 {
                    let rx = (screen_w as f32) * 0.5;
                    let ry = (screen_h as f32) * 0.5;
                    rain.extend(verryte_terminal::vfx::emit_burst(
                        rx,
                        ry,
                        1,
                        Color(100, 140, 220),
                        &['│', '┃', '¦'],
                    ));
                }
                self.vfx_mut().particles.extend(rain);
            }
            WeatherType::LightningStorm => {
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 255, 200),
                        0.15,
                    ));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.2));
            }
            WeatherType::Sunny => {
                let (tw, th) = self.get_tile_dimensions();
                let cols = 24u16;
                let rows = 16u16;
                let screen_w = cols * tw;
                let screen_h = rows * th;
                let cx = screen_w as f32 * 0.5;
                let cy = screen_h as f32 * 0.5;
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        cx,
                        cy,
                        12,
                        Color(255, 230, 100),
                        &['·', '°', '∘'],
                    ));
            }
            WeatherType::Snowing => {
                let (tw, th) = self.get_tile_dimensions();
                let cols = 24u16;
                let rows = 16u16;
                let screen_w = cols * tw;
                let screen_h = rows * th;
                let cx = screen_w as f32 * 0.5;
                let cy = screen_h as f32 * 0.5;
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        cx,
                        cy,
                        20,
                        Color(220, 230, 255),
                        &['*', '·', '❄'],
                    ));
            }
        }
    }

    pub fn snapshot(&self) -> crate::snapshot::Snapshot {
        let state = self.world.resource::<GameState>().unwrap();

        let mut player_team = crate::snapshot::TeamSummary {
            count: 0,
            total_hp: 0,
            max_hp: 0,
        };
        let mut enemy_team = crate::snapshot::TeamSummary {
            count: 0,
            total_hp: 0,
            max_hp: 0,
        };

        for (_, team, stats) in self.world.query2::<Team, Stats>() {
            let summary = match team {
                Team::Player => &mut player_team,
                Team::Enemy => &mut enemy_team,
            };
            summary.count += 1;
            summary.total_hp += stats.hp;
            summary.max_hp += stats.max_hp;
        }

        let (reachable_tiles, targetable_tiles, selected_can_act) =
            if let Some(sel) = state.selected_entity {
                let reachable = self.get_reachable_tiles(sel);
                let attack_range = self
                    .world
                    .get::<CharacterClass>(sel)
                    .map(|c| match c {
                        CharacterClass::Warrior => 1,
                        CharacterClass::Mage => 3,
                        CharacterClass::Healer => 2,
                        _ => 1,
                    })
                    .unwrap_or(1);
                let can_act = self
                    .world
                    .get::<Stats>(sel)
                    .map(|s| s.ap > 0)
                    .unwrap_or(false);
                let mut targets: Vec<Position> = Vec::new();
                for (_, team, pos) in self.world.query2::<Team, Position>() {
                    if team == &Team::Enemy {
                        let dist = (pos.x - state.cursor.x).abs() + (pos.y - state.cursor.y).abs();
                        if dist <= attack_range {
                            targets.push(*pos);
                        }
                    }
                }
                (reachable, targets, can_act)
            } else {
                (Vec::new(), Vec::new(), false)
            };

        let battle_stats = self
            .world
            .resource::<BattleStats>()
            .cloned()
            .unwrap_or_default();

        crate::snapshot::Snapshot {
            turn: state.turn,
            phase: state.phase,
            outcome: state.outcome,
            cursor: state.cursor,
            player_team,
            enemy_team,
            reachable_tiles,
            targetable_tiles,
            selected_can_act,
            combo_count: state.combo_count,
            battle_stats,
            floor: state.floor,
            weather: self
                .world
                .resource::<crate::components::Weather>()
                .map(|w| w.current)
                .unwrap_or(crate::components::WeatherType::Sunny),
            turn_order: crate::battle_preview::BattlePreview::calculate_turn_order(
                &self.world,
                state.selected_entity,
            )
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{}[{}]{}",
                    e.name,
                    e.spd,
                    if e.is_current { "*" } else { "" }
                )
            })
            .collect(),
            enemy_intents: {
                let default_map = crate::map::TacticalMap::new(24, 16);
                let default_telegraph = crate::components::TelegraphZone::default();
                let map_ref = self
                    .world
                    .resource::<crate::map::TacticalMap>()
                    .unwrap_or(&default_map);
                let telegraph_ref = self
                    .world
                    .resource::<crate::components::TelegraphZone>()
                    .unwrap_or(&default_telegraph);
                crate::battle_preview::BattlePreview::predict_enemy_intents(
                    &self.world,
                    map_ref,
                    telegraph_ref,
                )
                .intents
                .iter()
                .map(|i| i.description.clone())
                .collect()
            },
            damage_preview: state.selected_entity.and_then(|sel| {
                let cursor = state.cursor;
                self.get_entity_at(cursor)
                    .and_then(|(target, team, _stats, _class)| {
                        if team == Team::Enemy {
                            if let (Some(atk_stats), Some(tgt_stats)) = (
                                self.world.get::<Stats>(sel),
                                self.world.get::<Stats>(target),
                            ) {
                                let mut preview =
                                    crate::battle_preview::BattlePreview::calculate_damage_preview(
                                        atk_stats.atk,
                                        atk_stats.level,
                                        tgt_stats.def,
                                        tgt_stats.level,
                                        1.0,
                                        20,
                                    );
                                preview.can_kill = preview.max_damage >= tgt_stats.hp;
                                Some(preview)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
            }),
            aoe_preview: Vec::new(),
            available_combos: self
                .world
                .resource::<crate::components::AvailableCombos>()
                .map(|ac| {
                    ac.combos
                        .iter()
                        .map(|(s, _)| crate::components::ComboSkillDef::for_skill(s).name)
                        .collect()
                })
                .unwrap_or_default(),
            bestiary_discovered: self
                .world
                .resource::<crate::components::Bestiary>()
                .map(|b| b.entries.iter().filter(|e| e.encountered).count() as u32)
                .unwrap_or(0),
            bestiary_total: self
                .world
                .resource::<crate::components::Bestiary>()
                .map(|b| b.entries.len() as u32)
                .unwrap_or(0),
            lore_discovered: self
                .world
                .resource::<crate::components::LoreJournal>()
                .map(|j| j.entries.iter().filter(|e| e.discovered).count() as u32)
                .unwrap_or(0),
            lore_total: self
                .world
                .resource::<crate::components::LoreJournal>()
                .map(|j| j.entries.len() as u32)
                .unwrap_or(0),
            active_modifiers: self
                .world
                .resource::<crate::components::ActiveFloorModifiers>()
                .map(|m| {
                    m.modifiers
                        .iter()
                        .map(|fm| fm.display_name().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            active_set_bonuses: self
                .world
                .query2::<Team, crate::components::EquippedItems>()
                .into_iter()
                .filter(|(_, t, _)| **t == Team::Player)
                .filter_map(|(_, _, eq)| {
                    let stats = crate::equipment::check_set_bonuses(eq);
                    if stats.active_sets.is_empty() {
                        None
                    } else {
                        Some(
                            stats
                                .active_sets
                                .iter()
                                .map(|s| s.display_name().to_string())
                                .collect::<Vec<_>>()
                                .join(", "),
                        )
                    }
                })
                .collect(),
        }
    }

    pub fn take_events(&mut self) -> Vec<GameEvent> {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .unwrap()
            .drain()
            .collect()
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

    /// Apply version-by-version migrations to bring an old save up to date.
    fn migrate_save_state(
        mut state: crate::snapshot::FullSaveState,
    ) -> Result<crate::snapshot::FullSaveState, Box<dyn std::error::Error>> {
        // Migration v1 -> v2: add migrations_applied field (already handled
        // by serde default) and stamp the migration.
        if state.version == 1 {
            state
                .migrations_applied
                .push("v1_to_v2: added migrations_applied tracking".to_string());
            state.version = 2;
        }
        // Future migrations would go here:
        // if state.version == 2 { ... state.version = 3; }

        Ok(state)
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
                self.apply_action(Action::Inspect(point), ActionSource::Terminal);
                return true;
            }
        }
        false
    }
}
