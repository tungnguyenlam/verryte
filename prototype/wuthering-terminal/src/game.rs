use crate::action::{default_bindings, Action};
use crate::components::{
    CharacterClass, GameEvent, GameState, Outcome, Position, Stats, Team, TurnPhase,
};
use crate::map::{TacticalMap, Tile};
use std::collections::HashSet;
use verryte_core::{Entity, Events, GameClock, MessageLog, Rng, Schedule, World};
use verryte_input::{ActionSource, InputRouter};
use verryte_terminal::{Camera, Cell, Color, Grid, VisualAsset, VisualRegistry};

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
    pub vfx: verryte_terminal::vfx::VfxSystem,
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
        });
        world.insert_resource(crate::components::TelegraphZone::default());
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(1));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(MessageLog::with_max(50));

        let mut registry = VisualRegistry::new();
        Self::load_sprites(&mut registry);
        world.insert_resource(registry);

        let mut game = Self {
            world,
            schedule: Schedule::new(),
            router: InputRouter::new(default_bindings()),
            camera: Camera::new(5.0, 5.0),
            vfx: verryte_terminal::vfx::VfxSystem::new(),
        };

        game.spawn_character(Position::new(4, 4), Team::Player, CharacterClass::Warrior);
        game.spawn_character(Position::new(4, 8), Team::Player, CharacterClass::Mage);
        game.spawn_character(Position::new(4, 12), Team::Player, CharacterClass::Healer);
        game.spawn_character(Position::new(18, 8), Team::Enemy, CharacterClass::Boss);

        game.log("Wuthering Terminal Tactical RPG Initialized.");
        game.log("Move cursor: Arrows/WASD. Confirm: Enter. Cancel: Esc.");
        game.log("End Turn: E. Cycle: Tab.");

        game
    }

    fn load_sprites(registry: &mut VisualRegistry) {
        let assets = [
            ("kael", "kael.png", false),
            ("lyra", "lyra.png", false),
            ("mira", "mira.png", false),
            ("blight-sovereign", "blight-sovereign.png", true),
        ];

        let chroma_key = Color(255, 255, 255); // White background
        let tolerance = 30;

        let base_dir = if std::path::Path::new("prototype/wuthering-terminal").exists() {
            "prototype/wuthering-terminal/assets"
        } else {
            "assets"
        };

        for (key, filename, is_boss) in assets {
            let path = format!("{}/{}", base_dir, filename);
            let img_res = image::io::Reader::open(&path)
                .map_err(|e| e.to_string())
                .and_then(|r| r.with_guessed_format().map_err(|e| e.to_string()))
                .and_then(|r| r.decode().map_err(|e| e.to_string()));

            if let Ok(img) = img_res {
                let mut sprite = verryte_terminal::Sprite::new(key, vec![]);
                for tier in verryte_terminal::ResolutionTier::ALL {
                    let (cols, rows) = tier.sprite_size();
                    let (w, h) = if is_boss {
                        (
                            ((cols as f32 * 1.33) as u32).max(1),
                            ((rows as f32 * 2.0 * 1.33) as u32).max(1),
                        )
                    } else {
                        (cols as u32, rows as u32 * 2)
                    };
                    let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
                    let grid = verryte_terminal::image_to_grid_with_chroma_key(
                        &resized, chroma_key, tolerance,
                    );
                    let frame = verryte_terminal::Frame { grid, duration: 1 };
                    sprite = sprite.with_tier(tier, vec![frame]);
                }
                registry.register(key, VisualAsset::Animated(sprite));
            }
        }
    }

    fn spawn_character(&mut self, pos: Position, team: Team, class: CharacterClass) -> Entity {
        let stats = match class {
            CharacterClass::Warrior => Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
            },
            CharacterClass::Mage => Stats {
                hp: 60,
                max_hp: 60,
                atk: 35,
                def: 5,
                spd: 4,
                ap: 3,
                max_ap: 3,
            },
            CharacterClass::Healer => Stats {
                hp: 70,
                max_hp: 70,
                atk: 10,
                def: 8,
                spd: 6,
                ap: 3,
                max_ap: 3,
            },
            CharacterClass::Boss => Stats {
                hp: 500,
                max_hp: 500,
                atk: 40,
                def: 20,
                spd: 3,
                ap: 5,
                max_ap: 5,
            },
        };

        self.world
            .builder()
            .with(pos)
            .with(team)
            .with(class)
            .with(stats)
            .with(crate::components::ElementalStatus::None)
            .build()
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
        match tier {
            verryte_terminal::ResolutionTier::TINY => (6, 3),
            verryte_terminal::ResolutionTier::SMALL => (8, 4),
            verryte_terminal::ResolutionTier::MEDIUM => (12, 6),
            verryte_terminal::ResolutionTier::LARGE => (16, 8),
            verryte_terminal::ResolutionTier::XLARGE => (20, 10),
            verryte_terminal::ResolutionTier::ULTRA => (28, 14),
        }
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

        self.vfx
            .floating_texts
            .push(verryte_terminal::vfx::FloatingText::new(
                tcx,
                tcy - 2.0,
                &float_text,
                float_color,
                is_crit,
            ));

        self.vfx.particles.extend(verryte_terminal::vfx::emit_slash(
            tcx,
            tcy,
            if is_crit { 2.0 } else { 1.0 },
        ));

        let shake_intensity = if is_crit { 3.5 } else { 1.5 };
        let shake_duration = if is_crit { 0.4 } else { 0.25 };
        self.vfx
            .shakes
            .push(verryte_terminal::vfx::ScreenShake::new(
                shake_intensity,
                shake_duration,
            ));

        (damage, defeated)
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

    pub fn end_player_turn(&mut self) {
        // Execute any telegraphed attacks first!
        let telegraph_tiles = {
            let telegraph_zone = self
                .world
                .resource_mut::<crate::components::TelegraphZone>()
                .unwrap();
            let tiles = telegraph_zone.tiles.clone();
            telegraph_zone.tiles.clear();
            tiles
        };

        if !telegraph_tiles.is_empty() {
            self.log("Blight Sovereign releases Dark Annihilation!");

            // VFX feedback!
            self.vfx
                .flashes
                .push(verryte_terminal::vfx::Flash::full_screen(
                    Color(120, 0, 180),
                    0.3,
                ));
            self.vfx
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(4.5, 0.6));

            let mut hit_count = 0;
            // Check all player characters standing in telegraph tiles
            let mut players = Vec::new();
            for (e, p, team) in self.world.query2::<Position, Team>() {
                if *team == Team::Player && telegraph_tiles.contains(p) {
                    players.push(e);
                }
            }

            for pe in players {
                let target_class = *self.world.get::<CharacterClass>(pe).unwrap();
                let target_pos = *self.world.get::<Position>(pe).unwrap();
                let target_name = Self::get_class_name(target_class);
                let mut final_hp = 0;
                if let Some(stats) = self.world.get_mut::<Stats>(pe) {
                    stats.hp -= 50; // Fixed high damage
                    final_hp = stats.hp;
                }
                self.log(format!(
                    "Dark Annihilation hit {} for 50 damage! (HP: {})",
                    target_name, final_hp
                ));
                hit_count += 1;

                let (cx, cy) = self.get_tile_center_pixels(target_pos);
                self.vfx
                    .floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        cx,
                        cy - 2.0,
                        "-50",
                        Color(255, 20, 20),
                        true,
                    ));
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_fire(cx, cy, 15));

                if final_hp <= 0 {
                    let name_str = target_name.to_string();
                    self.handle_defeat(pe, &name_str, target_class, target_pos);
                }
            }

            if hit_count == 0 {
                self.log("Dark Annihilation missed everyone!");
            }
        }

        {
            let state = self.world.resource_mut::<GameState>().unwrap();
            state.phase = TurnPhase::Enemy;
            state.selected_entity = None;
        }
        self.log("Enemy Phase starts!");
        if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
            log.send(GameEvent::PhaseChanged(TurnPhase::Enemy));
            log.send(GameEvent::TurnEnded);
        }

        self.run_enemy_ai();

        let outcome = self.world.resource::<GameState>().unwrap().outcome;
        if outcome != Outcome::Playing {
            return;
        }

        {
            let state = self.world.resource_mut::<GameState>().unwrap();
            state.phase = TurnPhase::Player;
            state.turn += 1;
        }
        let turn_num = self.world.resource::<GameState>().unwrap().turn;
        self.log(format!("Player Phase starts! Turn {}", turn_num));

        // Decrement elemental statuses
        let mut status_entities = Vec::new();
        for (e, _status) in self.world.query::<crate::components::ElementalStatus>() {
            status_entities.push(e);
        }
        for e in status_entities {
            if let Some(status) = self.world.get_mut::<crate::components::ElementalStatus>(e) {
                *status = match *status {
                    crate::components::ElementalStatus::Ice { duration } if duration > 1 => {
                        crate::components::ElementalStatus::Ice {
                            duration: duration - 1,
                        }
                    }
                    crate::components::ElementalStatus::Lightning { duration } if duration > 1 => {
                        crate::components::ElementalStatus::Lightning {
                            duration: duration - 1,
                        }
                    }
                    crate::components::ElementalStatus::Nature { duration } if duration > 1 => {
                        crate::components::ElementalStatus::Nature {
                            duration: duration - 1,
                        }
                    }
                    _ => crate::components::ElementalStatus::None,
                };
            }
        }

        // Replenish AP
        let mut players = Vec::new();
        for (e, team) in self.world.query::<Team>() {
            if *team == Team::Player {
                players.push(e);
            }
        }
        for e in players {
            let mut is_rooted = false;
            let mut root_remains = false;
            if let Some(rooted) = self.world.get_mut::<crate::components::Rooted>(e) {
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
                    self.world.remove::<crate::components::Rooted>(e);
                }
                if let Some(stats) = self.world.get_mut::<Stats>(e) {
                    stats.ap = 0;
                }
                let class = *self.world.get::<CharacterClass>(e).unwrap();
                self.log(format!(
                    "{} is rooted and cannot act this turn!",
                    Self::get_class_name(class)
                ));
            } else {
                if let Some(stats) = self.world.get_mut::<Stats>(e) {
                    stats.ap = stats.max_ap;
                }
            }
        }

        if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
            log.send(GameEvent::ApReplenished);
            log.send(GameEvent::PhaseChanged(TurnPhase::Player));
        }
    }

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

                let range = 2; // Boss attack range
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
                        self.vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                            ex,
                            ey,
                            30,
                            Color(120, 20, 180),
                            &['░', '▓', '✦', '¤'],
                        ));
                        self.vfx
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
        let mut echo_to_absorb = None;
        for (e, p, _echo) in self.world.query2::<Position, crate::components::EchoItem>() {
            if *p == pos {
                echo_to_absorb = Some(e);
                break;
            }
        }
        if let Some(echo_ent) = echo_to_absorb {
            self.world.despawn(echo_ent);
            self.log("Absorbed Blight Sovereign Echo! Echo absorbed successfully.");
            self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Victory;

            // Spawn absorption VFX
            self.vfx.particles.extend(verryte_terminal::vfx::emit_heal(
                self.get_tile_center_pixels(pos).0,
                self.get_tile_center_pixels(pos).1,
                30,
            ));
            self.vfx
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(3.0, 0.6));
            self.vfx
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
                    stats.max_ap += 2;
                    stats.ap = stats.max_ap;
                }

                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.boss_phase = crate::components::BossPhase::Phase2;
                }

                self.log("Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");

                let (bx, by) = self.get_tile_center_pixels(pos);

                self.vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                    bx,
                    by,
                    50,
                    Color(255, 0, 0),
                    &['✦', '*', '░', '▓', '¤'],
                ));
                self.vfx
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(5.0, 1.0));
                self.vfx
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
            self.vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                self.get_tile_center_pixels(pos).0,
                self.get_tile_center_pixels(pos).1,
                40,
                Color(180, 50, 255),
                &['✦', '✧', '░', '▓', '¤'],
            ));
            self.vfx
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(4.0, 0.8));
            self.vfx
                .flashes
                .push(verryte_terminal::vfx::Flash::full_screen(
                    Color(255, 255, 255),
                    0.4,
                ));
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

            self.vfx
                .flashes
                .push(verryte_terminal::vfx::Flash::full_screen(
                    Color(255, 255, 255),
                    0.25,
                ));
            self.vfx
                .shakes
                .push(verryte_terminal::vfx::ScreenShake::new(4.0, 0.5));
            let (cx, cy) = self.get_tile_center_pixels(target_pos);
            self.vfx
                .particles
                .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
            self.vfx
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
            "QTE Swap! {} swaps in for {} at ({}, {})!",
            next_name, active_name, active_pos.x, active_pos.y
        ));

        let (cx, cy) = self.get_tile_center_pixels(active_pos);

        match next_class {
            CharacterClass::Warrior => {
                self.log("Warrior Intro Skill: Cloud Slasher!");
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_slash(cx, cy, 1.0));
                self.vfx
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
                    self.vfx
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
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
                self.vfx
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
                    self.vfx
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
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_heal(cx, cy, 25));
                self.vfx
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
                    self.vfx
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            pcx,
                            pcy - 2.0,
                            "+40",
                            Color(50, 255, 50),
                            true,
                        ));
                    self.vfx
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
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_slash(cx, cy, 1.0));
                self.vfx
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.3));
            }
            (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => {
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_fire(cx, cy, 25));
                self.vfx
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(3.5, 0.5));
                self.vfx
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 100, 30),
                        0.2,
                    ));
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => {
                let caster_pos = *self.world.get::<Position>(caster).unwrap();
                let (ccx, ccy) = self.get_tile_center_pixels(caster_pos);
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_lightning(ccx, ccy, cx, cy));
                self.vfx
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(1.5, 0.2));
            }
            (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => {
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_ice(cx, cy, 25));
                self.vfx.aoe_rings.push(verryte_terminal::vfx::AoeRing {
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
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_heal(cx, cy, 20));
            }
            (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => {
                self.vfx
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
                    self.vfx
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            pcx,
                            pcy - 2.0,
                            &format!("+{}", value),
                            Color(50, 255, 50),
                            true,
                        ));
                    self.vfx
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
                        self.vfx
                            .floating_texts
                            .push(verryte_terminal::vfx::FloatingText::new(
                                cx,
                                cy - 2.0,
                                &format!("+{}", value),
                                Color(50, 255, 50),
                                true,
                            ));
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
                    stats.max_ap += 2;
                    stats.ap = stats.max_ap;
                }

                if let Some(state) = self.world.resource_mut::<GameState>() {
                    state.boss_phase = crate::components::BossPhase::Phase2;
                }

                self.log("Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");

                let bp = boss_pos.unwrap_or(Position::new(0, 0));
                let (bx, by) = self.get_tile_center_pixels(bp);

                self.vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                    bx,
                    by,
                    50,
                    Color(255, 0, 0),
                    &['✦', '*', '░', '▓', '¤'],
                ));
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_fire(bx, by, 30));
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_shatter(bx, by, 30));

                self.vfx
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(5.0, 1.0));

                self.vfx
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 0, 0),
                        0.5,
                    ));

                self.vfx.aoe_rings.push(verryte_terminal::vfx::AoeRing {
                    cx: bx as i32,
                    cy: by as i32,
                    max_radius: 8.0,
                    current_radius: 1.0,
                    expand_speed: 15.0,
                    color: Color(255, 0, 0),
                    lifetime: 0.5,
                    max_lifetime: 0.5,
                });
                self.vfx.aoe_rings.push(verryte_terminal::vfx::AoeRing {
                    cx: bx as i32,
                    cy: by as i32,
                    max_radius: 12.0,
                    current_radius: 1.0,
                    expand_speed: 20.0,
                    color: Color(0, 255, 0),
                    lifetime: 0.6,
                    max_lifetime: 0.6,
                });
                self.vfx.aoe_rings.push(verryte_terminal::vfx::AoeRing {
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
                self.log(format!("Elemental Reaction: SHATTER on {}!", target_name));
                let bonus_damage = 30;
                let mut defeated = false;
                if let Some(stats) = self.world.get_mut::<Stats>(target) {
                    stats.hp -= bonus_damage;
                    if stats.hp <= 0 {
                        defeated = true;
                    }
                }

                // Shatter VFX
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_shatter(t_cx, t_cy, 25));
                self.vfx
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(3.0, 0.5));
                self.vfx
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
                    "Elemental Reaction: OVERGROWTH on {}!",
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
                self.vfx
                    .particles
                    .extend(verryte_terminal::vfx::emit_bloom(t_cx, t_cy, 15));
                self.vfx
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
                self.log(format!("Elemental Reaction: BLOOM on {}!", target_name));
                let healing_amount = 20;

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
                        "Bloom healed {} for {} HP! (HP: {})",
                        Self::get_class_name(a_class),
                        healing_amount,
                        final_hp
                    ));

                    self.vfx
                        .floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            a_cx,
                            a_cy - 2.0,
                            &format!("+{} (Bloom)", healing_amount),
                            Color(50, 255, 50),
                            true,
                        ));
                    self.vfx
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

                self.log(format!("Applied {} element to {}.", badge, target_name));

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
        self.apply_action_internal(action);
        self.check_boss_phase_transition();
        let after = self.snapshot();
        crate::snapshot::StepReport {
            action,
            source,
            before,
            after,
            events: self.take_events(),
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
                                        let attacker_element = match sel_class {
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
                                        self.vfx.floating_texts.push(
                                            verryte_terminal::vfx::FloatingText::new(
                                                target_cx,
                                                target_cy - 2.0,
                                                &format!("+{}", heal_val),
                                                Color(50, 255, 50),
                                                true,
                                            ),
                                        );
                                        self.vfx.particles.extend(
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
                                    self.vfx
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
            Action::EndTurn => {
                let phase = self.world.resource::<GameState>().unwrap().phase;
                if phase == TurnPhase::Player {
                    self.end_player_turn();
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
            _ => {}
        }
        self.camera.tick();
    }

    pub fn update(&mut self, dt: f32) {
        self.vfx.update(dt);
        self.camera.tick();
    }

    pub fn render(&self) -> Grid {
        let map = self.world.resource::<TacticalMap>().unwrap();
        let state = self.world.resource::<GameState>().unwrap();
        let registry = self.world.resource::<VisualRegistry>().unwrap();

        // Determine resolution tier and tile dimensions dynamically
        let (term_w, term_h) = verryte_tty::terminal_size();
        let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
        let (tile_w, tile_h) = match tier {
            verryte_terminal::ResolutionTier::TINY => (6, 3),
            verryte_terminal::ResolutionTier::SMALL => (8, 4),
            verryte_terminal::ResolutionTier::MEDIUM => (12, 6),
            verryte_terminal::ResolutionTier::LARGE => (16, 8),
            verryte_terminal::ResolutionTier::XLARGE => (20, 10),
            verryte_terminal::ResolutionTier::ULTRA => (28, 14),
        };

        let map_w = map.width * tile_w;
        let map_h = map.height * tile_h;

        // Render everything onto a giant temporary board canvas
        let mut board_canvas = Grid::new(map_w, map_h);

        // Render tiles
        for ty in 0..map.height {
            for tx in 0..map.width {
                let tile = map.tile(tx as i16, ty as i16);
                let color = match tile {
                    Tile::Grass => Color(30, 80, 30),
                    Tile::Wall => Color(60, 60, 60),
                    Tile::Water => Color(30, 30, 100),
                };

                let rx = tx * tile_w;
                let ry = ty * tile_h;

                // Draw tile background/border
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        let glyph = if dx == 0 || dy == 0 { '·' } else { ' ' };
                        board_canvas.put(
                            rx + dx,
                            ry + dy,
                            Cell::new(glyph).with_fg(Color(40, 40, 40)).with_bg(color),
                        );
                    }
                }
            }
        }

        // Draw movement range overlay if a player character is selected and not in targeting mode
        if state.targeting == crate::components::TargetingMode::None {
            if let Some(sel_entity) = state.selected_entity {
                let reachable = self.get_reachable_tiles(sel_entity);
                for pos in &reachable {
                    let rx = pos.x as u16 * tile_w;
                    let ry = pos.y as u16 * tile_h;
                    for dy in 0..tile_h {
                        for dx in 0..tile_w {
                            if let Some(cell) = board_canvas.get_mut(rx + dx, ry + dy) {
                                cell.bg = verryte_terminal::vfx::blend_color(
                                    cell.bg,
                                    Color(0, 100, 150),
                                    0.35,
                                );
                            }
                        }
                    }
                }

                // Draw path preview if cursor is on a reachable tile
                if reachable.contains(&state.cursor) {
                    if let Some(path) = self.get_path_to(sel_entity, state.cursor) {
                        for pos in path {
                            let rx = pos.x as u16 * tile_w;
                            let ry = pos.y as u16 * tile_h;
                            for dy in 0..tile_h {
                                for dx in 0..tile_w {
                                    if let Some(cell) = board_canvas.get_mut(rx + dx, ry + dy) {
                                        cell.bg = verryte_terminal::vfx::blend_color(
                                            cell.bg,
                                            Color(0, 150, 220),
                                            0.4,
                                        );
                                    }
                                }
                            }
                            board_canvas.put(
                                rx + tile_w / 2,
                                ry + tile_h / 2,
                                Cell::new('•')
                                    .with_fg(Color(100, 220, 255))
                                    .with_bg(Color::BLACK),
                            );
                        }
                    }
                }
            }
        } else {
            // Draw skill targeting range overlay
            if let Some(sel_entity) = state.selected_entity {
                if let (Some(caster_pos), Some(caster_class)) = (
                    self.world.get::<Position>(sel_entity),
                    self.world.get::<CharacterClass>(sel_entity),
                ) {
                    let range = match (*caster_class, state.targeting) {
                        (CharacterClass::Warrior, crate::components::TargetingMode::Skill1) => 1,
                        (CharacterClass::Warrior, crate::components::TargetingMode::Skill2) => 3,
                        (CharacterClass::Mage, crate::components::TargetingMode::Skill1) => 3,
                        (CharacterClass::Mage, crate::components::TargetingMode::Skill2) => 4,
                        (CharacterClass::Healer, crate::components::TargetingMode::Skill1) => 2,
                        (CharacterClass::Healer, crate::components::TargetingMode::Skill2) => 0,
                        _ => 0,
                    };
                    for ty in 0..map.height {
                        for tx in 0..map.width {
                            let target = Position::new(tx as i16, ty as i16);
                            let dist =
                                (caster_pos.x - target.x).abs() + (caster_pos.y - target.y).abs();
                            if dist <= range {
                                let rx = target.x as u16 * tile_w;
                                let ry = target.y as u16 * tile_h;
                                for dy in 0..tile_h {
                                    for dx in 0..tile_w {
                                        if let Some(cell) = board_canvas.get_mut(rx + dx, ry + dy) {
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

        // Draw telegraphed tiles in dark red/magenta overlay
        let telegraph_zone = self
            .world
            .resource::<crate::components::TelegraphZone>()
            .unwrap();
        for pos in &telegraph_zone.tiles {
            let rx = pos.x as u16 * tile_w;
            let ry = pos.y as u16 * tile_h;
            for dy in 0..tile_h {
                for dx in 0..tile_w {
                    if let Some(cell) = board_canvas.get_mut(rx + dx, ry + dy) {
                        cell.bg =
                            verryte_terminal::vfx::blend_color(cell.bg, Color(150, 0, 75), 0.4);
                    }
                }
            }
        }

        // Render entities
        for (_e, pos, _team, class) in self.world.query3::<Position, Team, CharacterClass>() {
            let key = match class {
                CharacterClass::Warrior => "kael",
                CharacterClass::Mage => "lyra",
                CharacterClass::Healer => "mira",
                CharacterClass::Boss => "blight-sovereign",
            };

            if let Some(asset) = registry.get(key) {
                let sprite_grid = asset.render(tier);
                let sw = sprite_grid.width();
                let sh = sprite_grid.height();

                let rx = (pos.x as i32 * tile_w as i32) + (tile_w as i32 - sw as i32) / 2;
                let ry = (pos.y as i32 * tile_h as i32) + (tile_h as i32 - sh as i32) / 2;

                board_canvas.blit(&sprite_grid, rx, ry);
            }
        }

        // Render Echo items
        for (_e, pos, _echo) in self.world.query2::<Position, crate::components::EchoItem>() {
            let rx = pos.x as u16 * tile_w + tile_w / 2;
            let ry = pos.y as u16 * tile_h + tile_h / 2;
            board_canvas.put(
                rx,
                ry,
                Cell::new('Ω')
                    .with_fg(Color(180, 50, 255))
                    .with_bg(Color::BLACK)
                    .with_attrs(verryte_terminal::CellAttrs::NONE.bold()),
            );
        }

        // Render cursor
        let cx = state.cursor.x as u16 * tile_w;
        let cy = state.cursor.y as u16 * tile_h;
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                if let Some(cell) = board_canvas.get_mut(cx + dx, cy + dy) {
                    cell.bg = verryte_terminal::vfx::blend_color(cell.bg, Color(150, 150, 0), 0.3);
                }
            }
        }

        // Render VFX (particles, floating text, AoE rings) directly onto the board canvas
        self.vfx.render(&mut board_canvas, map_w, map_h);

        // --- Camera Viewport Crop logic ---
        let viewport_w = term_w;
        let viewport_h = term_h.saturating_sub(6).max(1);

        // Center on the camera position
        let pixel_cx = self.camera.center_x * tile_w as f32 + (tile_w as f32 / 2.0);
        let pixel_cy = self.camera.center_y * tile_h as f32 + (tile_h as f32 / 2.0);

        let mut view_x = (pixel_cx - (viewport_w as f32 / 2.0)) as i32;
        let mut view_y = (pixel_cy - (viewport_h as f32 / 2.0)) as i32;

        view_x = view_x.clamp(0, (map_w as i32 - viewport_w as i32).max(0));
        view_y = view_y.clamp(0, (map_h as i32 - viewport_h as i32).max(0));

        let mut grid = Grid::new(term_w, term_h);
        for dy in 0..viewport_h {
            for dx in 0..viewport_w {
                let sx = view_x + dx as i32;
                let sy = view_y + dy as i32;
                if sx >= 0 && sy >= 0 && (sx as u16) < map_w && (sy as u16) < map_h {
                    if let Some(cell) = board_canvas.get(sx as u16, sy as u16) {
                        grid.put(dx, dy, *cell);
                    }
                }
            }
        }

        // Draw HUD stationary at the bottom
        let hud_y = term_h.saturating_sub(6);
        let hud_w = term_w;
        let hud_h = 6u16;

        let hud_bg = Color(10, 10, 15);
        for dy in 0..hud_h {
            for dx in 0..hud_w {
                grid.put(dx, hud_y + dy, Cell::new(' ').with_bg(hud_bg));
            }
        }

        // Draw a rounded border around the HUD panel
        let hud_rect = verryte_terminal::Rect::new(0, hud_y, hud_w, hud_h);
        grid.draw_border_styled(
            hud_rect,
            verryte_terminal::BorderStyle::Rounded,
            Color(80, 120, 160),
            hud_bg,
        );
        // Drop shadow for visual depth
        grid.draw_shadow(hud_rect);

        let phase_str = match state.phase {
            TurnPhase::Player => "PLAYER PHASE",
            TurnPhase::Enemy => "ENEMY PHASE",
        };
        let phase_color = match state.phase {
            TurnPhase::Player => Color::GREEN,
            TurnPhase::Enemy => Color::RED,
        };

        let mut selection_str = "Selected: None".to_string();
        if let Some(sel_entity) = state.selected_entity {
            if let (Some(class), Some(stats)) = (
                self.world.get::<CharacterClass>(sel_entity),
                self.world.get::<Stats>(sel_entity),
            ) {
                let name = Self::get_class_name(*class);
                let elemental = self
                    .world
                    .get::<crate::components::ElementalStatus>(sel_entity)
                    .copied()
                    .unwrap_or(crate::components::ElementalStatus::None);
                let status_badge = match elemental {
                    crate::components::ElementalStatus::Ice { duration } => {
                        format!(" [ICE:{}]", duration)
                    }
                    crate::components::ElementalStatus::Lightning { duration } => {
                        format!(" [LIGHTNING:{}]", duration)
                    }
                    crate::components::ElementalStatus::Nature { duration } => {
                        format!(" [NATURE:{}]", duration)
                    }
                    _ => "".to_string(),
                };
                let is_rooted = self
                    .world
                    .get::<crate::components::Rooted>(sel_entity)
                    .is_some();
                let root_badge = if is_rooted { " [ROOTED]" } else { "" };
                selection_str = format!(
                    "Selected: {} (HP: {}/{}, AP: {}/{}){}{}",
                    name, stats.hp, stats.max_hp, stats.ap, stats.max_ap, status_badge, root_badge
                );
            }
        }

        // Draw HUD line 1 components
        grid.write_str(
            2,
            hud_y + 1,
            &format!("TURN: {:02} | ", state.turn),
            Color::WHITE,
            hud_bg,
        );
        grid.write_str(
            13,
            hud_y + 1,
            &format!("PHASE: {:<12}", phase_str),
            phase_color,
            hud_bg,
        );
        grid.write_str(
            31,
            hud_y + 1,
            &format!(" | {}", selection_str),
            Color::WHITE,
            hud_bg,
        );

        let ce_pct = (state.concert_energy as f32 / 100.0).clamp(0.0, 1.0);
        let bar_len = 10;
        let filled_len = (ce_pct * bar_len as f32).round() as usize;
        let empty_len = bar_len - filled_len;
        let bar_str = format!("[{}{}]", "█".repeat(filled_len), "░".repeat(empty_len));
        let ce_display = format!("CONCERT: {}/100 {}", state.concert_energy, bar_str);
        let ce_color = if state.concert_energy >= 100 {
            Color(255, 215, 0) // Gold
        } else {
            Color(100, 200, 255) // Cyan-ish
        };
        grid.write_str(
            90,
            hud_y + 1,
            &format!(" | {}", ce_display),
            ce_color,
            hud_bg,
        );

        let hovered_tile = map.tile(state.cursor.x, state.cursor.y);
        let tile_type_str = match hovered_tile {
            Tile::Grass => "Grass",
            Tile::Wall => "Wall",
            Tile::Water => "Water",
        };

        let hovered_str = if let Some((target_entity, target_team, target_stats, target_class)) =
            self.get_entity_at(state.cursor)
        {
            let name = Self::get_class_name(target_class);
            let team_str = match target_team {
                Team::Player => "Player",
                Team::Enemy => "Enemy",
            };
            let elemental = self
                .world
                .get::<crate::components::ElementalStatus>(target_entity)
                .copied()
                .unwrap_or(crate::components::ElementalStatus::None);
            let status_badge = match elemental {
                crate::components::ElementalStatus::Ice { duration } => {
                    format!(" [ICE:{}]", duration)
                }
                crate::components::ElementalStatus::Lightning { duration } => {
                    format!(" [LIGHTNING:{}]", duration)
                }
                crate::components::ElementalStatus::Nature { duration } => {
                    format!(" [NATURE:{}]", duration)
                }
                _ => "".to_string(),
            };
            let is_rooted = self
                .world
                .get::<crate::components::Rooted>(target_entity)
                .is_some();
            let root_badge = if is_rooted { " [ROOTED]" } else { "" };
            format!(
                "Tile: {} | Entity: {} (HP: {}/{}, AP: {}/{}, Team: {}){}{}",
                tile_type_str,
                name,
                target_stats.hp,
                target_stats.max_hp,
                target_stats.ap,
                target_stats.max_ap,
                team_str,
                status_badge,
                root_badge
            )
        } else {
            format!("Tile: {} | Entity: None", tile_type_str)
        };

        // Draw HUD line 2 components
        grid.write_str(
            2,
            hud_y + 2,
            &format!("CURSOR: ({:02}, {:02}) | ", state.cursor.x, state.cursor.y),
            Color::CYAN,
            hud_bg,
        );
        grid.write_str(20, hud_y + 2, &hovered_str, Color::WHITE, hud_bg);

        if let Some(log) = self.world.resource::<MessageLog>() {
            let tail = log.tail(3);
            for i in 0..3 {
                let msg = if i < tail.len() { &tail[i] } else { "" };
                let hud_line_log = format!("  > {}", msg);
                grid.write_str(
                    0,
                    hud_y + 3 + i as u16,
                    &hud_line_log,
                    Color::YELLOW,
                    hud_bg,
                );
            }
        }

        // Render screen flash overlay onto the final viewport grid
        self.vfx.render_flash(&mut grid, term_w, term_h);

        grid
    }

    pub fn snapshot(&self) -> crate::snapshot::Snapshot {
        let state = self.world.resource::<GameState>().unwrap();
        crate::snapshot::Snapshot {
            turn: state.turn,
            phase: state.phase,
            outcome: state.outcome,
            cursor: state.cursor,
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

            entities.push(SavedEntity {
                entity,
                position,
                team,
                class,
                stats,
                echo_item,
                elemental_status,
                rooted,
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
}
