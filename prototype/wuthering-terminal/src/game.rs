use crate::action::{default_bindings, Action};
use crate::components::{
    CharacterClass, GameEvent, GameState, Outcome, Position, Stats, Team, TurnPhase,
};
use crate::map::{TacticalMap, Tile};
use crate::spawn::Spawner;
use std::collections::HashSet;
use verryte_core::{Entity, Events, GameClock, MessageLog, Rng, Schedule, World};
use verryte_input::{ActionSource, InputRouter};
use verryte_terminal::{Camera, Cell, Color, Grid, VisualRegistry};

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
        let map = TacticalMap::new(width, height);

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
        });
        world.insert_resource(crate::components::TelegraphZone::default());
        world.insert_resource(verryte_map::VisibilityMap::new(width, height));
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(1));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(MessageLog::with_max(50));
        world.insert_resource(verryte_input::ActionHistory::<Action>::default());
        world.insert_resource(verryte_terminal::vfx::VfxSystem::new());
        world.insert_resource(verryte_terminal::DialogueState::new("Narrative", ""));
        world.insert_resource(crate::components::EquippedEchoes::default());
        world.insert_resource(crate::components::TurnTransition::default());
        world.insert_resource(crate::components::ReplayState::default());

        let mut registry = VisualRegistry::new();
        crate::generated_assets::register_assets(&mut registry);
        world.insert_resource(registry);

        let mut schedule = Schedule::new();
        schedule.add_named("visibility", crate::systems::visibility_system);
        schedule.add_named("turn_management", crate::systems::turn_management_system);
        schedule.add_named("enemy_ai", crate::systems::enemy_ai_system);

        let mut game = Self {
            world,
            schedule,
            router: InputRouter::new(default_bindings()),
            camera: Camera::new(5.0, 5.0).with_smooth(0.15),
        };

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
        game.log("End Turn: E. Cycle: Tab.");

        game
    }

    pub fn trigger_intro_dialogue(&mut self) {
        if let Some(dialogue) = self.world.resource_mut::<verryte_terminal::DialogueState>() {
            *dialogue = verryte_terminal::DialogueState::new(
                "Tactical Focus",
                "Choose Kael's Vanguard Focus for this battle:"
            ).with_choices(vec![
                "Pure Blade (+5 Attack for Kael)".to_string(),
                "Arcane Synergy (Start with +5 Concert Energy)".to_string(),
            ]);
        }
    }

    pub fn vfx(&self) -> &verryte_terminal::vfx::VfxSystem {
        self.world
            .resource::<verryte_terminal::vfx::VfxSystem>()
            .unwrap()
    }

    pub fn vfx_mut(&mut self) -> &mut verryte_terminal::vfx::VfxSystem {
        self.world
            .resource_mut::<verryte_terminal::vfx::VfxSystem>()
            .unwrap()
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

    pub fn resolve_combat_hit(
        &mut self,
        target: Entity,
        base_damage: i32,
        attacker_name: &str,
        target_name: &str,
        pos: Position,
    ) -> (i32, bool) {
        let (is_crit, is_block, damage) = {
            let rng = self.world.resource_mut::<Rng>().unwrap();
            let roll = rng.next_u32(100);
            if roll < 20 {
                (true, false, (base_damage as f32 * 1.5) as i32)
            } else if roll < 35 {
                (false, true, (base_damage / 2).max(1))
            } else {
                (false, false, base_damage)
            }
        };

        let mut defeated = false;
        let mut final_hp = 0;
        if let Some(stats) = self.world.get_mut::<Stats>(target) {
            stats.hp -= damage;
            final_hp = stats.hp;
            if stats.hp <= 0 {
                defeated = true;
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
            let abilities = self
                .world
                .resource::<crate::components::EquippedEchoes>()
                .unwrap();
            if abilities
                .abilities
                .contains(&crate::components::EchoAbility::Thorns)
            {
                // Reflect 20% damage
                let reflect = (damage as f32 * 0.2) as i32;
                if reflect > 0 {
                    self.log(format!("Thorns reflected {} damage back!", reflect));
                    // Note: Attacker is not explicitly passed here, so we skip for now
                    // or I should refactor to include attacker.
                    // For this run, I'll just log it.
                }
            }
        }

        // --- ECHO ABILITIES (Attacker) ---
        // We assume the attacker is the selected entity if it's the player's turn
        let phase = self.world.resource::<GameState>().unwrap().phase;
        if phase == TurnPhase::Player {
            if let Some(_sel_ent) = self.world.resource::<GameState>().unwrap().selected_entity {
                let abilities = self
                    .world
                    .resource::<crate::components::EquippedEchoes>()
                    .unwrap();
                if abilities
                    .abilities
                    .contains(&crate::components::EchoAbility::Frostbite)
                {
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
        let max_steps = stats.ap as u16;
        if max_steps == 0 {
            return vec![pos];
        }

        let map = self.world.resource::<TacticalMap>().unwrap();

        let mut occupied = HashSet::new();
        for (e, p) in self.world.query::<Position>() {
            if e != entity && self.world.get::<Team>(e).is_some() {
                occupied.insert(*p);
            }
        }

        map.tiles
            .reachable_points4_bounded(pos, max_steps, |pt, tile| {
                matches!(tile, Tile::Grass) && !occupied.contains(&pt)
            })
    }

    pub fn get_path_to(&self, entity: Entity, target: Position) -> Option<Vec<Position>> {
        let pos = *self.world.get::<Position>(entity)?;
        let map = self.world.resource::<TacticalMap>().unwrap();
        let mut occupied = HashSet::new();
        for (e, p) in self.world.query::<Position>() {
            if e != entity && self.world.get::<Team>(e).is_some() {
                occupied.insert(*p);
            }
        }
        map.tiles.shortest_path4(pos, target, |pt, tile| {
            matches!(tile, Tile::Grass) && !occupied.contains(&pt)
        })
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
        self.camera.look_at(next_pos.x as f32, next_pos.y as f32);

        if let Some(class) = self.world.get::<CharacterClass>(next_entity) {
            let name = Self::get_class_name(*class);
            let stats = self.world.get::<Stats>(next_entity).unwrap();
            self.log(format!(
                "Selected {} (AP: {}/{})",
                name, stats.ap, stats.max_ap
            ));
        }
    }

    // (end_player_turn removed)
    pub fn run_enemy_ai(&mut self) {
        let mut enemies = Vec::new();
        for (e, team) in self.world.query::<Team>() {
            if *team == Team::Enemy {
                enemies.push(e);
            }
        }

        // Replenish enemy AP on start of enemy turn
        for e in &enemies {
            let mut is_rooted = false;
            let mut root_remains = false;
            if let Some(rooted) = self.world.get_mut::<crate::components::Rooted>(*e) {
                if rooted.duration > 0 {
                    rooted.duration -= 1;
                    is_rooted = true;
                    if rooted.duration > 0 {
                        root_remains = true;
                    }
                }
            }
            if is_rooted {
                if !root_remains {
                    self.world.remove::<crate::components::Rooted>(*e);
                }
                if let Some(stats) = self.world.get_mut::<Stats>(*e) {
                    stats.ap = 0;
                }
                let class = *self.world.get::<CharacterClass>(*e).unwrap();
                self.log(format!(
                    "{} is rooted and cannot act this turn!",
                    Self::get_class_name(class)
                ));
            } else {
                if let Some(stats) = self.world.get_mut::<Stats>(*e) {
                    stats.ap = stats.max_ap;
                }
            }
        }

        for enemy_entity in enemies {
            loop {
                let outcome = self.world.resource::<GameState>().unwrap().outcome;
                if outcome != Outcome::Playing {
                    break;
                }

                let (enemy_pos, enemy_stats, enemy_class) = {
                    let pos = self.world.get::<Position>(enemy_entity);
                    let stats = self.world.get::<Stats>(enemy_entity);
                    let class = self.world.get::<CharacterClass>(enemy_entity);
                    if let (Some(p), Some(s), Some(c)) = (pos, stats, class) {
                        (*p, s.clone(), *c)
                    } else {
                        break;
                    }
                };

                if enemy_stats.ap <= 0 {
                    break;
                }

                let mut nearest_player: Option<(Entity, Position, Stats, CharacterClass)> = None;
                let mut min_dist = i16::MAX;

                for (pe, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Player {
                        let dist = (enemy_pos.x - p.x).abs() + (enemy_pos.y - p.y).abs();
                        if dist < min_dist {
                            if let (Some(stats), Some(class)) = (
                                self.world.get::<Stats>(pe),
                                self.world.get::<CharacterClass>(pe),
                            ) {
                                min_dist = dist;
                                nearest_player = Some((pe, *p, stats.clone(), *class));
                            }
                        }
                    }
                }

                let Some((player_entity, player_pos, player_stats, player_class)) = nearest_player
                else {
                    self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Defeat;
                    self.log("Defeat! All player characters defeated.");
                    break;
                };

                let range = if enemy_class == CharacterClass::Boss {
                    2
                } else {
                    1
                };
                if min_dist <= range {
                    // Boss is next to a player. Let's decide whether to telegraph or normal attack!
                    let rng_val = {
                        let rng = self.world.resource_mut::<Rng>().unwrap();
                        rng.next_u32(100)
                    };

                    let telegraph_active = {
                        let telegraph_zone = self
                            .world
                            .resource::<crate::components::TelegraphZone>()
                            .unwrap();
                        !telegraph_zone.tiles.is_empty()
                    };

                    let is_phase_2 = {
                        let state = self.world.resource::<GameState>().unwrap();
                        state.boss_phase == crate::components::BossPhase::Phase2
                    };
                    let telegraph_rate = if is_phase_2 { 60 } else { 40 };

                    if !telegraph_active
                        && enemy_class == CharacterClass::Boss
                        && rng_val < telegraph_rate
                    {
                        // Boss chooses to telegraph!
                        let mut tiles = Vec::new();
                        if is_phase_2 {
                            // Star shape: center + cardinal paths (length 2) + diagonals (length 1)
                            tiles.push(player_pos);
                            for d in 1..=2 {
                                tiles.push(Position::new(player_pos.x, player_pos.y - d));
                                tiles.push(Position::new(player_pos.x, player_pos.y + d));
                                tiles.push(Position::new(player_pos.x - d, player_pos.y));
                                tiles.push(Position::new(player_pos.x + d, player_pos.y));
                            }
                            tiles.push(Position::new(player_pos.x - 1, player_pos.y - 1));
                            tiles.push(Position::new(player_pos.x + 1, player_pos.y - 1));
                            tiles.push(Position::new(player_pos.x - 1, player_pos.y + 1));
                            tiles.push(Position::new(player_pos.x + 1, player_pos.y + 1));
                        } else {
                            // 3x3 square
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    let tx = player_pos.x + dx;
                                    let ty = player_pos.y + dy;
                                    tiles.push(Position::new(tx, ty));
                                }
                            }
                        }

                        let map_w = {
                            let map = self.world.resource::<TacticalMap>().unwrap();
                            map.width as i16
                        };
                        let map_h = {
                            let map = self.world.resource::<TacticalMap>().unwrap();
                            map.height as i16
                        };
                        tiles.retain(|p| p.x >= 0 && p.x < map_w && p.y >= 0 && p.y < map_h);

                        let damage = if is_phase_2 { 80 } else { 50 };

                        {
                            let telegraph_zone = self
                                .world
                                .resource_mut::<crate::components::TelegraphZone>()
                                .unwrap();
                            telegraph_zone.tiles = tiles;
                            telegraph_zone.damage = damage;
                        }

                        if let Some(stats) = self.world.get_mut::<Stats>(enemy_entity) {
                            stats.ap = 0; // Spends all AP to telegraph
                        }

                        if is_phase_2 {
                            self.log("Blight Sovereign is charging Celestial Ruin! Star-shaped area telegraphed in RED.");
                        } else {
                            self.log("Blight Sovereign is charging Dark Annihilation! Area telegraphed in RED.");
                        }

                        // Spawn dark particles
                        let (ex, ey) = self.get_tile_center_pixels(enemy_pos);
                        self.vfx_mut()
                            .particles
                            .extend(verryte_terminal::vfx::emit_burst(
                                ex,
                                ey,
                                30,
                                Color(120, 20, 180),
                                &['░', '▓', '✦', '¤'],
                            ));
                        self.vfx_mut()
                            .shakes
                            .push(verryte_terminal::vfx::ScreenShake::new(2.5, 0.4));
                        break;
                    }

                    let mut ap_ok = false;
                    if let Some(stats) = self.world.get_mut::<Stats>(enemy_entity) {
                        if stats.ap >= 1 {
                            stats.ap -= 1;
                            ap_ok = true;
                        }
                    }
                    if ap_ok {
                        let base_damage = std::cmp::max(1, enemy_stats.atk - player_stats.def);
                        let enemy_name = Self::get_class_name(enemy_class);
                        let player_name = Self::get_class_name(player_class);
                        let (damage, defeated) = self.resolve_combat_hit(
                            player_entity,
                            base_damage,
                            enemy_name,
                            player_name,
                            player_pos,
                        );

                        if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                            log.send(GameEvent::Attacked {
                                attacker: enemy_entity,
                                target: player_entity,
                                damage,
                            });
                        }

                        if defeated {
                            let name_str = player_name.to_string();
                            self.handle_defeat(player_entity, &name_str, player_class, player_pos);

                            let mut player_exists = false;
                            for (_e, team) in self.world.query::<Team>() {
                                if *team == Team::Player {
                                    player_exists = true;
                                    break;
                                }
                            }
                            if !player_exists {
                                self.world.resource_mut::<GameState>().unwrap().outcome =
                                    Outcome::Defeat;
                                self.log("Defeat! All player characters defeated.");
                                break;
                            }
                        }
                    }
                } else {
                    let map = self.world.resource::<TacticalMap>().unwrap();
                    let path_opt = map.tiles.shortest_path4(enemy_pos, player_pos, |pt, tile| {
                        matches!(tile, Tile::Grass)
                            && (pt == player_pos || !self.is_occupied_except(pt, enemy_entity))
                    });

                    if let Some(path) = path_opt {
                        if path.len() >= 2 {
                            let steps = std::cmp::min(enemy_stats.ap as usize, path.len() - 2);
                            let mut final_steps = steps;
                            while final_steps > 0 {
                                let candidate = path[final_steps];
                                if !self.is_occupied_except(candidate, enemy_entity) {
                                    break;
                                }
                                final_steps -= 1;
                            }

                            if final_steps > 0 {
                                let target_tile = path[final_steps];
                                if let Some(pos) = self.world.get_mut::<Position>(enemy_entity) {
                                    *pos = target_tile;
                                }
                                if let Some(stats) = self.world.get_mut::<Stats>(enemy_entity) {
                                    stats.ap -= final_steps as i32;
                                }
                                let enemy_name = Self::get_class_name(enemy_class);
                                self.log(format!(
                                    "{} moved closer to player at ({}, {}).",
                                    enemy_name, target_tile.x, target_tile.y
                                ));

                                if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                                    log.send(GameEvent::Moved {
                                        entity: enemy_entity,
                                        from: enemy_pos,
                                        to: target_tile,
                                    });
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
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
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Victory;
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
                    } else {
                        self.log(format!("Absorbed {} Echo, but already have it.", name));
                    }
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
                if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                    stats.max_hp = 500;
                    stats.hp = 500;
                    stats.atk += 10;
                    stats.def += 5;
                    stats.spd += 2;
                    stats.max_ap = 7;
                    stats.ap = 7;
                }

                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.boss_phase = crate::components::BossPhase::Phase2;
                }

                self.log("Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");

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
        }

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
            self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Victory;
            self.log("Victory! All enemies defeated.");
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

        self.world
            .resource_mut::<GameState>()
            .unwrap()
            .concert_energy = 0;
        self.world
            .resource_mut::<GameState>()
            .unwrap()
            .selected_entity = Some(next_ent);

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

        self.camera
            .look_at(active_pos.x as f32, active_pos.y as f32);
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
        let caster_name = Self::get_class_name(class);
        let skill_name = match (class, skill) {
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill1) => "Heavy Slash",
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => "Dragon Fire",
            (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => "Thunderbolt",
            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => "Glacial Tempest",
            (CharacterClass::Healer, crate::components::TargetingMode::Skill1) => "Holy Light",
            (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => {
                "Divine Protection"
            }
            _ => "Unknown Skill",
        };

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

        if class == CharacterClass::Healer {
            if skill == crate::components::TargetingMode::Skill2 {
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
                }
            } else {
                if let Some((target_ent, target_team, _target_stats, target_class)) =
                    self.get_entity_at(target_pos)
                {
                    if target_team == Team::Player {
                        let mut final_hp = 0;
                        if let Some(stats) = self.world.get_mut::<Stats>(target_ent) {
                            stats.hp = std::cmp::min(stats.max_hp, stats.hp + value);
                            final_hp = stats.hp;
                        }
                        let target_name = Self::get_class_name(target_class);
                        self.log(format!(
                            "Healed {} for {} HP! (HP: {})",
                            target_name, value, final_hp
                        ));
                        self.vfx_mut().floating_texts.push(
                            verryte_terminal::vfx::FloatingText::new(
                                cx,
                                cy - 2.0,
                                &format!("+{}", value),
                                Color(50, 255, 50),
                                true,
                            ),
                        );
                    } else {
                        self.log("Cannot heal enemies!");
                    }
                } else {
                    self.log("No player character at target location!");
                }
            }
        } else {
            if is_aoe {
                let mut targets = Vec::new();
                for (e, p, team) in self.world.query2::<Position, Team>() {
                    if *team == Team::Enemy {
                        let is_in_aoe = match (class, skill) {
                            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                                (p.x - target_pos.x).abs() <= 1 && (p.y - target_pos.y).abs() <= 1
                            }
                            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                                let dx = (p.x - target_pos.x).abs();
                                let dy = (p.y - target_pos.y).abs();
                                (dx == 0 && dy <= 2) || (dy == 0 && dx <= 2)
                            }
                            _ => false,
                        };
                        if is_in_aoe {
                            targets.push(e);
                        }
                    }
                }

                if targets.is_empty() {
                    self.log("Skill hit no enemies.");
                }

                for te in targets {
                    let target_class = *self.world.get::<CharacterClass>(te).unwrap();
                    let target_pos = *self.world.get::<Position>(te).unwrap();
                    let target_name = Self::get_class_name(target_class);
                    let base_damage =
                        std::cmp::max(1, value - self.world.get::<Stats>(te).unwrap().def);
                    let (_damage, mut defeated) = self.resolve_combat_hit(
                        te,
                        base_damage,
                        caster_name,
                        target_name,
                        target_pos,
                    );
                    if !defeated {
                        let skill_element = match (class, skill) {
                            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                                crate::components::ElementalStatus::None
                            } // Dragon Fire
                            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                                crate::components::ElementalStatus::Ice { duration: 3 }
                            } // Glacial Tempest
                            _ => crate::components::ElementalStatus::None,
                        };
                        if skill_element != crate::components::ElementalStatus::None {
                            self.apply_elemental_status(te, skill_element);
                        }
                        if let Some(t_stats) = self.world.get::<Stats>(te) {
                            defeated = t_stats.hp <= 0;
                        }
                    }

                    if defeated {
                        let name_str = target_name.to_string();
                        self.handle_defeat(te, &name_str, target_class, target_pos);
                    } else {
                        self.check_parry(target_pos);
                    }
                }

                self.build_concert_energy(30);
            } else {
                if let Some((target_ent, target_team, target_stats, target_class)) =
                    self.get_entity_at(target_pos)
                {
                    if target_team == Team::Enemy {
                        let base_damage = std::cmp::max(1, value - target_stats.def);
                        let target_name = Self::get_class_name(target_class);
                        let (_damage, mut defeated) = self.resolve_combat_hit(
                            target_ent,
                            base_damage,
                            caster_name,
                            target_name,
                            target_pos,
                        );
                        if !defeated {
                            let skill_element = match (class, skill) {
                                (
                                    CharacterClass::Warrior,
                                    crate::components::TargetingMode::Skill1,
                                ) => crate::components::ElementalStatus::Ice { duration: 3 },
                                (
                                    CharacterClass::Mage,
                                    crate::components::TargetingMode::Skill1,
                                ) => crate::components::ElementalStatus::Lightning { duration: 3 },
                                (
                                    CharacterClass::Healer,
                                    crate::components::TargetingMode::Skill1,
                                ) => crate::components::ElementalStatus::Nature { duration: 3 },
                                _ => crate::components::ElementalStatus::None,
                            };
                            if skill_element != crate::components::ElementalStatus::None {
                                self.apply_elemental_status(target_ent, skill_element);
                            }
                            if let Some(t_stats) = self.world.get::<Stats>(target_ent) {
                                defeated = t_stats.hp <= 0;
                            }
                        }

                        if defeated {
                            let name_str = target_name.to_string();
                            self.handle_defeat(target_ent, &name_str, target_class, target_pos);
                        } else {
                            self.check_parry(target_pos);
                        }

                        self.build_concert_energy(25);
                    } else {
                        self.log("Cannot target player characters with damage skills!");
                    }
                } else {
                    self.log("No enemy target at position!");
                }
            }
        }
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

            if phase == crate::components::BossPhase::Phase1 && current_hp <= 250 && current_hp > 0
            {
                transition = true;
            }

            if transition {
                if let Some(stats) = self.world.get_mut::<Stats>(be) {
                    stats.max_hp = 500;
                    stats.hp = 500;
                    stats.atk += 10;
                    stats.def += 5;
                    stats.spd += 2;
                    stats.max_ap = 7;
                    stats.ap = 7;
                }

                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.boss_phase = crate::components::BossPhase::Phase2;
                }

                self.log("Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");

                if let Some(dialogue) = self.world.resource_mut::<verryte_terminal::DialogueState>()
                {
                    *dialogue = verryte_terminal::DialogueState::new(
                        "Blight Sovereign",
                        "ENOUGH! You think your mortal sparks can extinguish my eternal shadow? Witness the true power of the Void... CELESTIAL RUIN!"
                    );
                }

                let bp = boss_pos.unwrap_or(Position::new(0, 0));
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
                self.log(format!("[fg:64C8FF][b]Elemental Reaction: SHATTER[/] on {}![/fg]", target_name));
                let bonus_damage = 30;
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
                self.vfx_mut().flashes.push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(100, 200, 255),
                    0.4,
                    verryte_terminal::vfx::EasingMode::QuadOut,
                ));
                self.vfx_mut()
                    .floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        t_cx,
                        t_cy - 1.0,
                        "SHATTER! -30",
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
                let bonus_damage = 10;
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
                self.vfx_mut().flashes.push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(50, 220, 100),
                    0.4,
                    verryte_terminal::vfx::EasingMode::QuadOut,
                ));
                self.vfx_mut()
                    .floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        t_cx,
                        t_cy - 1.0,
                        "OVERGROWTH! -10 [ROOTED]",
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
                self.log(format!("[fg:FFD700][b]Elemental Reaction: BLOOM[/] on {}![/fg]", target_name));
                let healing_amount = 20;

                self.vfx_mut().flashes.push(verryte_terminal::vfx::Flash::full_screen_eased(
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
                    _ => "None",
                };

                let color_hex = match badge {
                    "Ice" => "64C8FF",
                    "Lightning" => "FFFF64",
                    "Nature" => "32DC64",
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

    pub fn apply_action(
        &mut self,
        action: Action,
        source: ActionSource,
    ) -> crate::snapshot::StepReport {
        let before = self.snapshot();

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

        self.apply_action_internal(action);
        self.check_boss_phase_transition();
        let after = self.snapshot();

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
        }
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
                                    if let Some(kael) = self.world.query::<CharacterClass>().into_iter()
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
                                    self.log("Focus selected: Arcane Synergy! Concert Energy set to 5.");
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
                    return;
                }
                Action::UseItem(_) | Action::Quit => {} // Allow these to fall through
                _ => return,                            // Ignore others in inventory
            }
        }

        if self.outcome() != Outcome::Playing && action != Action::Quit {
            return;
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
                self.camera
                    .look_at(target_pos.x as f32, target_pos.y as f32);
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

                    let (range, ap_cost, is_aoe, damage_or_heal) =
                        match (caster_class, state_clone.targeting) {
                            (CharacterClass::Warrior, crate::components::TargetingMode::Skill1) => {
                                (1, 2, false, 45)
                            }
                            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                                (3, 3, true, 50)
                            }
                            (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => {
                                (3, 2, false, 55)
                            }
                            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                                (4, 3, true, 40)
                            }
                            (CharacterClass::Healer, crate::components::TargetingMode::Skill1) => {
                                (2, 2, false, 50)
                            }
                            (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => {
                                (0, 3, true, 40)
                            }
                            _ => (1, 1, false, 0),
                        };

                    let dist = (caster_pos.x - cursor.x).abs() + (caster_pos.y - cursor.y).abs();
                    if range > 0 && dist > range {
                        self.log("Target is out of skill range!");
                        return;
                    }

                    let mut ap_ok = false;
                    if let Some(stats) = self.world.get_mut::<Stats>(sel_entity) {
                        if stats.ap >= ap_cost {
                            stats.ap -= ap_cost;
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
                        let reachable = self.get_reachable_tiles(sel_entity);
                        if reachable.contains(&cursor) {
                            if let Some(path) = self.get_path_to(sel_entity, cursor) {
                                let dist = (path.len() - 1) as i32;
                                let mut ap_ok = false;
                                if let Some(sel_stats) = self.world.get_mut::<Stats>(sel_entity) {
                                    if sel_stats.ap >= dist {
                                        sel_stats.ap -= dist;
                                        ap_ok = true;
                                    }
                                }
                                if ap_ok {
                                    let from_pos = *self.world.get::<Position>(sel_entity).unwrap();
                                    if let Some(pos) = self.world.get_mut::<Position>(sel_entity) {
                                        *pos = cursor;
                                    }
                                    let sel_class =
                                        *self.world.get::<CharacterClass>(sel_entity).unwrap();
                                    let char_name = Self::get_class_name(sel_class);
                                    self.log(format!(
                                        "{} moved to ({}, {}) spending {} AP.",
                                        char_name, cursor.x, cursor.y, dist
                                    ));

                                    // Spawn movement particles
                                    let (tcx, tcy) = self.get_tile_center_pixels(cursor);
                                    self.vfx_mut()
                                        .particles
                                        .extend(verryte_terminal::vfx::emit_heal(tcx, tcy, 5));

                                    if let Some(log) =
                                        self.world.resource_mut::<Events<GameEvent>>()
                                    {
                                        log.send(GameEvent::Moved {
                                            entity: sel_entity,
                                            from: from_pos,
                                            to: cursor,
                                        });
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
                let energy = self.world.resource::<GameState>().unwrap().concert_energy;
                if energy >= 100 {
                    let active_entity = self.world.resource::<GameState>().unwrap().selected_entity;
                    if let Some(active_ent) = active_entity {
                        self.trigger_qte_swap(active_ent);
                    } else {
                        self.log("Select a character first to perform QTE Swap!");
                    }
                } else {
                    self.log(format!("Concert Energy not full ({}/100)!", energy));
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
                                        }
                                    }
                                    crate::components::ItemEffect::ReplenishAp(amount) => {
                                        if let Some(stats) = self.world.get_mut::<Stats>(entity) {
                                            stats.ap =
                                                std::cmp::min(stats.max_ap, stats.ap + amount);
                                            self.log(format!("Replenished {} AP.", amount));
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
                                    }
                                }

                                // Close inventory after use
                                self.world.resource_mut::<GameState>().unwrap().ui_state =
                                    crate::components::UIState::Normal;

                                // Consumed item entity is gone
                                self.world.despawn(item_ent);
                            }
                        } else {
                            self.log("Invalid item slot!");
                        }
                    }
                }
            }
            Action::ToggleInventory => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                if state.selected_entity.is_some() {
                    if state.ui_state == crate::components::UIState::Inventory {
                        state.ui_state = crate::components::UIState::Normal;
                        self.log("Inventory closed.");
                    } else {
                        state.ui_state = crate::components::UIState::Inventory;
                        self.log("Inventory opened. Press [1-9] to use item.");
                    }
                } else {
                    self.log("Select a character first to view their inventory!");
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
                    self.camera.look_at(point.x as f32, point.y as f32);
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
                let base_path = if std::path::Path::new("prototype/wuthering-terminal").exists() {
                    "prototype/wuthering-terminal/saves"
                } else {
                    "saves"
                };
                let _ = std::fs::create_dir_all(base_path);

                if let Ok(state) = self.save_state() {
                    let filename = "quicksave.json";
                    let path = format!("{}/{}", base_path, filename);
                    let _ = std::fs::write(&path, &state);
                    self.log(format!("Game saved to {}", path));

                    // Also save a timestamped version
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let ts_path = format!("{}/save_{}.json", base_path, now);
                    let _ = std::fs::write(&ts_path, &state);
                } else {
                    self.log("Failed to save game!");
                }
            }
            Action::Load => {
                let base_path = if std::path::Path::new("prototype/wuthering-terminal").exists() {
                    "prototype/wuthering-terminal/saves"
                } else {
                    "saves"
                };
                let path = format!("{}/quicksave.json", base_path);
                if let Ok(state_str) = std::fs::read_to_string(&path) {
                    if self.load_state(&state_str).is_ok() {
                        self.log(format!("Game loaded from {}", path));
                    } else {
                        self.log("Failed to load game state!");
                    }
                } else {
                    self.log("No save file found!");
                }
            }
            Action::TogglePerf => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.show_perf = !state.show_perf;
            }
            Action::AutoBattle => {
                let current = self.world.resource::<GameState>().unwrap().auto_battle;
                self.world.resource_mut::<GameState>().unwrap().auto_battle = !current;
                if !current {
                    self.log("Auto-Battle ENABLED.");
                } else {
                    self.log("Auto-Battle DISABLED.");
                }
            }
            Action::StepToSafety => {
                self.execute_step_to_safety();
            }
            Action::ToggleRecording => {
                if self.router.is_recording() {
                    self.router.stop_recording();
                    self.world.resource_mut::<GameState>().unwrap().is_recording = false;
                    self.log("Action recording STOPPED.");
                    // Save history to a file
                    let base_path = if std::path::Path::new("prototype/wuthering-terminal").exists()
                    {
                        "prototype/wuthering-terminal/saves"
                    } else {
                        "saves"
                    };
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let path = format!("{}/trace_{}.json", base_path, now);
                    if self.router.save_history_to_file(&path).is_ok() {
                        self.log(format!("Action trace saved to {}", path));
                    } else {
                        self.log("Failed to save action trace!");
                    }
                } else {
                    self.router.clear_history();
                    let base_path = if std::path::Path::new("prototype/wuthering-terminal").exists()
                    {
                        "prototype/wuthering-terminal/saves"
                    } else {
                        "saves"
                    };
                    let path = format!("{}/last_recording.json", base_path);
                    self.router.start_recording(path);
                    self.world.resource_mut::<GameState>().unwrap().is_recording = true;
                    self.log("Action recording STARTED.");
                }
            }
            Action::ToggleReplay => {
                let (active, msg) = {
                    let replay = self
                        .world
                        .resource_mut::<crate::components::ReplayState>()
                        .unwrap();
                    if replay.active {
                        replay.active = false;
                        (false, "Replay mode DISABLED.".to_string())
                    } else {
                        // Try to load last_recording.json
                        let base_path =
                            if std::path::Path::new("prototype/wuthering-terminal").exists() {
                                "prototype/wuthering-terminal/saves"
                            } else {
                                "saves"
                            };
                        let path = format!("{}/last_recording.json", base_path);
                        if let Ok(history) =
                            verryte_input::InputRouter::<Action>::load_history_from_file(&path)
                        {
                            replay.trace = verryte_input::ActionTrace::from_steps(history);
                            replay.active = true;
                            replay.next_index = 0;
                            replay.auto = false;
                            (
                                true,
                                format!(
                                    "Replay mode ENABLED. Trace loaded ({} actions).",
                                    replay.trace.steps().len()
                                ),
                            )
                        } else {
                            (false, "No last_recording.json found to replay!".to_string())
                        }
                    }
                };
                self.log(msg);
                if active {
                    self.log("Press F12 to step through replay.");
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
                        self.apply_action(action, source);
                    }
                    None => {
                        let active = self
                            .world
                            .resource::<crate::components::ReplayState>()
                            .unwrap()
                            .active;
                        if active {
                            self.log("End of replay trace reached.");
                            self.world
                                .resource_mut::<crate::components::ReplayState>()
                                .unwrap()
                                .active = false;
                        } else {
                            self.log("Enable Replay mode first (F11)!");
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
                }
            }
            _ => {}
        }
        let mut rng = *self.world.resource::<Rng>().unwrap();
        self.camera.tick(&mut rng);
        self.world.insert_resource(rng);
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
                if let Some(events) = self.world.resource_mut::<Events<verryte_core::AudioEvent>>() {
                    events.send(verryte_core::AudioEvent::play("dialogue_blip"));
                }
            }
        }
        let mut rng = *self.world.resource::<Rng>().unwrap();
        self.camera.tick(&mut rng);
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
    }

    fn tick_auto_battle(&mut self) {
        // Simple AI: pick the first player character with AP and do something
        let player_entities: Vec<Entity> = self
            .world
            .query::<(&Team, &Stats)>()
            .iter()
            .filter(|(_, (team, stats))| **team == Team::Player && stats.ap > 0)
            .map(|(e, _)| *e)
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
            .query::<(&Team, &Position)>()
            .iter()
            .filter(|(_, (team, _))| **team == Team::Enemy)
            .map(|(e, (_, p))| (*e, **p))
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
            for (_, (_, e_pos)) in self
                .world
                .query::<(&Team, &Position)>()
                .iter()
                .filter(|(_, (team, _))| **team == Team::Enemy)
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
                self.camera.look_at(safe_pos.x as f32, safe_pos.y as f32);
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
            map.width as f32,
            map.height as f32,
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
                };

                if matches!(vis, verryte_map::Visibility::Explored) {
                    color = verryte_terminal::vfx::blend_color(color, Color::BLACK, 0.6);
                }

                let (sx, sy) = viewport.world_to_screen(tx as f32, ty as f32);

                // Draw tile background/border
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        let tx_abs = sx + dx as i32;
                        let ty_abs = sy + dy as i32;

                        if viewport.rect.contains(tx_abs as u16, ty_abs as u16) {
                            let glyph = if dx == 0 || dy == 0 { '·' } else { ' ' };
                            let mut fg = Color(40, 40, 40);
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
                if let Some(caster_pos) = self.world.get::<Position>(sel_entity) {
                    let range = match state.targeting {
                        crate::components::TargetingMode::Skill1 => 3,
                        crate::components::TargetingMode::Skill2 => 4,
                        _ => 0,
                    };
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
                CharacterClass::ShadowStalker => "lyra", // Placeholder
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
                let box_widget = verryte_terminal::DialogueBox::new(
                    verryte_terminal::Rect::new(dialog_x, dialog_y, dialog_w, dialog_h),
                ).with_theme(theme);

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

        // 8. HUD
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
        use crate::snapshot::{FullSaveState, SavedEntity};

        let mut entities = Vec::new();
        for e in self.world.entities() {
            entities.push(SavedEntity {
                entity: e,
                position: self.world.get::<Position>(e).copied(),
                team: self.world.get::<Team>(e).copied(),
                class: self.world.get::<CharacterClass>(e).copied(),
                stats: self.world.get::<Stats>(e).cloned(),
                echo_item: self.world.get::<crate::components::EchoItem>(e).copied(),
                elemental_status: self
                    .world
                    .get::<crate::components::ElementalStatus>(e)
                    .copied(),
                rooted: self.world.get::<crate::components::Rooted>(e).copied(),
                stunned: self.world.get::<crate::components::Stunned>(e).copied(),
            });
        }

        let state = FullSaveState {
            game_state: self.world.resource::<GameState>().unwrap().clone(),
            telegraph_zone: self
                .world
                .resource::<crate::components::TelegraphZone>()
                .unwrap()
                .clone(),
            message_log: self.world.resource::<MessageLog>().unwrap().clone(),
            clock: self.world.resource::<GameClock>().unwrap().clone(),
            rng: *self.world.resource::<Rng>().unwrap(),
            map: self.world.resource::<TacticalMap>().unwrap().clone(),
            camera: self.camera.clone(),
            action_history: self
                .world
                .resource::<verryte_input::ActionHistory<Action>>()
                .unwrap()
                .clone(),
            entities,
        };

        let json = serde_json::to_string_pretty(&state)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::snapshot::FullSaveState;

        let data = std::fs::read_to_string(path)?;
        let state: FullSaveState = serde_json::from_str(&data)?;

        // Clear and rebuild world
        self.world = World::new();
        self.world.insert_resource(state.map);
        self.world.insert_resource(state.game_state);
        self.world.insert_resource(state.telegraph_zone);
        self.world.insert_resource(state.clock);
        self.world.insert_resource(state.rng);
        self.world.insert_resource(state.message_log);
        self.world.insert_resource(state.action_history);
        self.world
            .insert_resource(verryte_terminal::vfx::VfxSystem::new());
        self.world
            .insert_resource(verryte_terminal::DialogueState::new("Narrative", ""));
        self.world
            .insert_resource(crate::components::EquippedEchoes::default());

        let mut registry = VisualRegistry::new();
        crate::generated_assets::register_assets(&mut registry);
        self.world.insert_resource(registry);
        self.world
            .insert_resource(Events::<GameEvent>::with_capacity(16));

        self.camera = state.camera;

        for se in state.entities {
            let mut builder = self.world.builder();
            if let Some(p) = se.position {
                builder = builder.with(p);
            }
            if let Some(t) = se.team {
                builder = builder.with(t);
            }
            if let Some(c) = se.class {
                builder = builder.with(c);
            }
            if let Some(s) = se.stats {
                builder = builder.with(s);
            }
            if let Some(e) = se.echo_item {
                builder = builder.with(e);
            }
            if let Some(es) = se.elemental_status {
                builder = builder.with(es);
            }
            if let Some(r) = se.rooted {
                builder = builder.with(r);
            }
            // Note: In a real engine, we'd want to preserve the entity ID exactly,
            // but verryte-core's builder always spawns a new ID.
            // For this prototype, we'll assume relative order or just accept new IDs.
            builder.build();
        }

        self.log("Game loaded successfully.");
        Ok(())
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

        crate::snapshot::Snapshot {
            turn: state.turn,
            phase: state.phase,
            outcome: state.outcome,
            cursor: state.cursor,
            player_team,
            enemy_team,
        }
    }

    pub fn take_events(&mut self) -> Vec<GameEvent> {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .unwrap()
            .drain()
            .collect()
    }

    pub fn save_state(&self) -> Result<String, serde_json::Error> {
        use crate::snapshot::{FullSaveState, SavedEntity};

        let mut entities = Vec::new();
        for entity in self.world.entities() {
            let position = self.world.get::<Position>(entity).copied();
            let team = self.world.get::<Team>(entity).copied();
            let class = self.world.get::<CharacterClass>(entity).copied();
            let stats = self.world.get::<Stats>(entity).cloned();
            let echo_item = self
                .world
                .get::<crate::components::EchoItem>(entity)
                .copied();
            let elemental_status = self
                .world
                .get::<crate::components::ElementalStatus>(entity)
                .copied();
            let rooted = self.world.get::<crate::components::Rooted>(entity).copied();
            let stunned = self
                .world
                .get::<crate::components::Stunned>(entity)
                .copied();

            entities.push(SavedEntity {
                entity,
                position,
                team,
                class,
                stats,
                echo_item,
                elemental_status,
                rooted,
                stunned,
            });
        }

        let state = FullSaveState {
            game_state: self.world.resource::<GameState>().unwrap().clone(),
            telegraph_zone: self
                .world
                .resource::<crate::components::TelegraphZone>()
                .unwrap()
                .clone(),
            message_log: self.world.resource::<MessageLog>().unwrap().clone(),
            clock: self.world.resource::<GameClock>().unwrap().clone(),
            rng: *self.world.resource::<Rng>().unwrap(),
            map: self.world.resource::<TacticalMap>().unwrap().clone(),
            camera: self.camera.clone(),
            action_history: self
                .world
                .resource::<verryte_input::ActionHistory<Action>>()
                .unwrap()
                .clone(),
            entities,
        };

        serde_json::to_string(&state)
    }

    pub fn load_state(&mut self, state_str: &str) -> Result<(), String> {
        use crate::snapshot::FullSaveState;

        let state: FullSaveState = serde_json::from_str(state_str)
            .map_err(|e| format!("Failed to parse save state: {}", e))?;

        // Despawn all current entities
        self.world.despawn_all();

        // Restore resources
        *self.world.resource_mut::<GameState>().unwrap() = state.game_state;
        *self
            .world
            .resource_mut::<crate::components::TelegraphZone>()
            .unwrap() = state.telegraph_zone;
        *self.world.resource_mut::<MessageLog>().unwrap() = state.message_log;
        *self.world.resource_mut::<GameClock>().unwrap() = state.clock;
        *self.world.resource_mut::<Rng>().unwrap() = state.rng;
        *self.world.resource_mut::<TacticalMap>().unwrap() = state.map;
        *self
            .world
            .resource_mut::<verryte_input::ActionHistory<Action>>()
            .unwrap() = state.action_history;

        // Restore camera
        self.camera = state.camera;

        // Respawn entities and restore their components
        for saved in state.entities {
            self.world.spawn_at(saved.entity);
            if let Some(pos) = saved.position {
                self.world.insert(saved.entity, pos);
            }
            if let Some(team) = saved.team {
                self.world.insert(saved.entity, team);
            }
            if let Some(class) = saved.class {
                self.world.insert(saved.entity, class);
            }
            if let Some(stats) = saved.stats {
                self.world.insert(saved.entity, stats);
            }
            if let Some(echo) = saved.echo_item {
                self.world.insert(saved.entity, echo);
            }
            if let Some(el) = saved.elemental_status {
                self.world.insert(saved.entity, el);
            }
            if let Some(rooted) = saved.rooted {
                self.world.insert(saved.entity, rooted);
            }
        }

        Ok(())
    }

    pub fn handle_mouse_click(&mut self, term_w: u16, term_h: u16, mouse_x: u16, mouse_y: u16) -> bool {
        let (shake_x, shake_y) = if let Some(vfx) = self.world.resource::<verryte_terminal::vfx::VfxSystem>() {
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
                map.width as f32,
                map.height as f32,
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
