use super::Game;
use crate::action::{default_bindings, Action};
use crate::components::{
    BattleStats, CharacterClass, GameEvent, GameState, Outcome, Position, Stats, Team, TurnPhase,
};
use crate::map::{TacticalMap, Tile};
use crate::spawn::Spawner;
use verryte_core::{Entity, Events, GameClock, MessageLog, Rng, Schedule, World};
use verryte_input::InputRouter;
use verryte_terminal::{Camera, Color, VisualRegistry};

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
            log_scroll_offset: 0,
            selected_save_slot: 0,
            show_threat_map: false,
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
        world.insert_resource(crate::components::DynamicFloorEvents::default());
        world.insert_resource(crate::components::IncursionAttackTelegraphs::default());
        world.insert_resource(Self::create_initial_bestiary());
        world.insert_resource(Self::create_initial_lore_journal());
        world.insert_resource(verryte_input::TextInput::new());

        let mut registry = VisualRegistry::new();
        crate::generated_assets::register_assets(&mut registry);
        world.insert_resource(registry);

        let mut schedule = Schedule::new();
        schedule.add_named("visibility", crate::systems::visibility_system);
        schedule.add_named("turn_management", crate::systems::turn_management_system);
        schedule.add_named(
            "dynamic_floor_events",
            crate::systems::dynamic_floor_event_system,
        );
        schedule.add_named("floor_modifier", crate::systems::floor_modifier_system);
        schedule.add_named("auto_battle", crate::systems::auto_battle_system);
        schedule.add_named("enemy_ai", crate::systems::enemy_ai_system);
        schedule.add_named("weather_cycle", crate::systems::weather_cycle_system);
        schedule.add_named("combo_detection", crate::systems::combo_detection_system);
        schedule.add_named("prestige", crate::systems::prestige_system);
        schedule.add_named("morale_fatigue", crate::systems::morale_fatigue_system);
        schedule.add_named("weather_ambient", crate::systems::weather_ambient_system);
        schedule.add_named("sturdy_immunity", crate::systems::sturdy_immunity_system);

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
            camera: Camera::new(0.0, 0.0)
                .with_smooth(0.15)
                .with_dead_zone(4.0, 2.0),
            camera_locked: true,
            last_outcome: crate::snapshot::ActionOutcome::NoOp,
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
            .spawn_character(Position::new(6, 8), Team::Player, CharacterClass::Rogue);
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

        // Spawn some explosive barrels
        game.world.spawn_barrel(Position::new(10, 5));
        game.world.spawn_barrel(Position::new(15, 10));
        game.world.spawn_barrel(Position::new(8, 12));

        // Scan tactical map for Tile::ExplodingBarrel and spawn barrel entities
        let map_clone = game.world.resource::<TacticalMap>().unwrap().clone();
        for y in 0..map_clone.height {
            for x in 0..map_clone.width {
                let pos = Position::new(x as i16, y as i16);
                if map_clone.tile(pos.x, pos.y) == crate::map::Tile::ExplodingBarrel {
                    game.world.spawn_barrel(pos);
                }
            }
        }
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

    pub fn create_initial_bestiary() -> crate::components::Bestiary {
        use crate::components::BestiaryEntry;
        crate::components::Bestiary {
            entries: vec![
                BestiaryEntry { class: CharacterClass::ShadowStalker, name: "Shadow Stalker".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Created from the shadows of the Sovereign's domain, these silent hunters vanish when struck. Area attacks are the only reliable way to pin them down.".to_string(), drop_table: vec!["Frostbite Echo".to_string(), "Shadow Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::CorruptedSpore, name: "Corrupted Spore".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Once benign forest spores, corrupted by the Blight into volatile living mines. They explode on proximity, dealing devastating area damage.".to_string(), drop_table: vec!["Nature Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::CursedSentinel, name: "Cursed Sentinel".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Ancient guardians turned to serve darkness. They prefer ranged attacks and retreat when approached.".to_string(), drop_table: vec!["Sentinel Core".to_string()] },
                BestiaryEntry { class: CharacterClass::PlagueWraith, name: "Plague Wraith".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Spirits of plague victims, bound to spread decay. Their touch applies Nature status.".to_string(), drop_table: vec!["Wraith Shard".to_string(), "Plague Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::VoidTerror, name: "Void Terror".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "A monstrous manifestation of pure corruption. It tears through the fabric of reality, devastating those unfortunate enough to be near.".to_string(), drop_table: vec!["Void Core".to_string(), "Dark Essence".to_string()] },
                BestiaryEntry { class: CharacterClass::GlacialGolem, name: "Glacial Golem".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Forged from eternal ice in the Sovereign's forge. Attacks apply Ice status, freezing victims in place.".to_string(), drop_table: vec!["Frost Core".to_string(), "Ice Walker Echo".to_string()] },
                BestiaryEntry { class: CharacterClass::EnemyCleric, name: "Dark Cleric".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Fallen healers who chose darkness over light. They prioritize healing wounded allies.".to_string(), drop_table: vec!["Dark Blessing".to_string()] },
                BestiaryEntry { class: CharacterClass::FrozenSentinel, name: "Frozen Sentinel".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "Heavily armored constructs designed to protect the Sovereign's inner sanctum. They focus on defending vulnerable allies.".to_string(), drop_table: vec!["Frozen Core".to_string()] },
                BestiaryEntry { class: CharacterClass::Boss, name: "Blight Sovereign".to_string(), encountered: false, defeated_count: 0, times_killed_by: 0, hits_taken: 0, known_weakness: None, known_resistance: None, lore_text: "The source of all corruption. A multi-phase abomination. In Phase 2, it unleashes Celestial Ruin with telegraphed attacks that can be parried.".to_string(), drop_table: vec!["Sovereign's Echo".to_string(), "Blight Crystal".to_string()] },
            ],
        }
    }

    pub fn create_initial_lore_journal() -> crate::components::LoreJournal {
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

        // 5. Spawn enemies in other rooms (scaled by floor)
        for (i, &room_center) in room_centers.iter().enumerate().skip(1) {
            if i == room_centers.len() - 1 {
                self.world.spawn_character_scaled(
                    room_center,
                    Team::Enemy,
                    CharacterClass::Boss,
                    floor,
                );
                if let Some(boss_ent) = self
                    .world
                    .query2::<CharacterClass, Team>()
                    .into_iter()
                    .find(|(_, class, team)| {
                        **class == CharacterClass::Boss && **team == Team::Enemy
                    })
                    .map(|(e, _, _)| e)
                {
                    let shield_amount = 150 + (floor as i32 - 1) * 50;
                    self.world.insert(
                        boss_ent,
                        crate::components::ElementalShield {
                            shield_type: crate::components::ShieldType::Ice,
                            amount: shield_amount,
                            max_amount: shield_amount,
                        },
                    );
                }
            } else {
                // Floor 3+: add elite enemy variants with higher frequency
                let class = if (floor >= 3 && i % 6 == 0) || (floor >= 2 && i % 5 == 0) {
                    CharacterClass::VoidTerror
                } else if floor >= 3 && i % 7 == 0 {
                    CharacterClass::GlacialGolem
                } else {
                    match i % 5 {
                        0 => CharacterClass::ShadowStalker,
                        1 => CharacterClass::CorruptedSpore,
                        2 => CharacterClass::GlacialGolem,
                        3 => CharacterClass::EnemyCleric,
                        4 => CharacterClass::FrozenSentinel,
                        _ => CharacterClass::CursedSentinel,
                    }
                };
                self.world
                    .spawn_character_scaled(room_center, Team::Enemy, class, floor);
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
                self.world.spawn_character_scaled(
                    pos,
                    Team::Enemy,
                    CharacterClass::GlacialGolem,
                    floor,
                );
            }
        }

        // 6. Reset GameState parameters for next floor
        if let Some(state) = self.world.resource_mut::<GameState>() {
            state.cursor = player_spawn;
            state.selected_entity = None;
            state.boss_phase = crate::components::BossPhase::Phase1;
            state.turn = 1;
        }
        if let Some(events) = self
            .world
            .resource_mut::<crate::components::DynamicFloorEvents>()
        {
            events.next_event_turn = 3;
            events.pending = None;
        }
        if let Some(attacks) = self
            .world
            .resource_mut::<crate::components::IncursionAttackTelegraphs>()
        {
            attacks.attacks.clear();
        }

        // 7. Grant bonus items on deeper floors
        if floor > 1 {
            let bonus_items: Vec<(&str, crate::components::ItemEffect)> = if floor >= 4 {
                vec![
                    ("Mega Potion", crate::components::ItemEffect::Heal(60)),
                    (
                        "Elixir of the Gods",
                        crate::components::ItemEffect::Heal(100),
                    ),
                ]
            } else if floor >= 3 {
                vec![("Greater Potion", crate::components::ItemEffect::Heal(45))]
            } else {
                vec![("Healing Potion", crate::components::ItemEffect::Heal(30))]
            };

            let first_player: Option<Entity> = self
                .world
                .query::<Team>()
                .iter()
                .find(|(_, t)| **t == Team::Player)
                .map(|(e, _)| *e);

            if let Some(p_ent) = first_player {
                let mut item_entities = Vec::new();
                for (name, effect) in &bonus_items {
                    let item = self.world.spawn_item(name, effect.clone());
                    item_entities.push(item);
                }
                if let Some(inv) = self.world.get_mut::<crate::components::Inventory>(p_ent) {
                    for item_ent in item_entities {
                        inv.items.push(item_ent);
                    }
                }
            }

            if !bonus_items.is_empty() {
                self.log(format!(
                    "[fg:32CD32]Found {} bonus item(s) for Floor {}![/fg]",
                    bonus_items.len(),
                    floor
                ));
            }
        }

        self.world
            .insert_resource(verryte_map::VisibilityMap::new(width, height));
        let (cx, cy) = self.get_tile_center_pixels(player_spawn);
        if self.camera_locked {
            self.camera.look_at(cx, cy);
        }

        crate::systems::select_floor_modifiers(&mut self.world);

        self.log(format!(
            "Welcome to Floor {}! Enemies grow stronger the deeper you go.",
            floor
        ));
    }
}
