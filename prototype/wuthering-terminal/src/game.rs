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
        });
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
            ("kael", "kael.png", 12, 12),
            ("lyra", "lyra.png", 12, 12),
            ("mira", "mira.png", 12, 12),
            ("blight-sovereign", "blight-sovereign.png", 16, 16),
        ];

        let chroma_key = Color(255, 255, 255); // White background
        let tolerance = 30;

        for (key, filename, w, h) in assets {
            let path = format!("prototype/wuthering-terminal/assets/{}", filename);
            let img_res = image::io::Reader::open(&path)
                .map_err(|e| e.to_string())
                .and_then(|r| r.with_guessed_format().map_err(|e| e.to_string()))
                .and_then(|r| r.decode().map_err(|e| e.to_string()));

            if let Ok(img) = img_res {
                let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
                let grid = verryte_terminal::image_to_grid_with_chroma_key(
                    &resized, chroma_key, tolerance,
                );
                registry.register(key, VisualAsset::BlockSprite(grid));
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
            if e != except && *p == pos {
                return true;
            }
        }
        false
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
            if e != entity {
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
            if e != entity {
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
            let mut state = self.world.resource_mut::<GameState>().unwrap();
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
        {
            let mut state = self.world.resource_mut::<GameState>().unwrap();
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
            let mut state = self.world.resource_mut::<GameState>().unwrap();
            state.phase = TurnPhase::Player;
            state.turn += 1;
        }
        let turn_num = self.world.resource::<GameState>().unwrap().turn;
        self.log(format!("Player Phase starts! Turn {}", turn_num));

        // Replenish AP
        let mut players = Vec::new();
        for (e, team) in self.world.query::<Team>() {
            if *team == Team::Player {
                players.push(e);
            }
        }
        for e in players {
            if let Some(stats) = self.world.get_mut::<Stats>(e) {
                stats.ap = stats.max_ap;
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
                    let mut ap_ok = false;
                    if let Some(stats) = self.world.get_mut::<Stats>(enemy_entity) {
                        if stats.ap >= 1 {
                            stats.ap -= 1;
                            ap_ok = true;
                        }
                    }
                    if ap_ok {
                        let damage = std::cmp::max(1, enemy_stats.atk - player_stats.def);
                        let mut defeated = false;
                        let mut final_hp = 0;
                        if let Some(stats) = self.world.get_mut::<Stats>(player_entity) {
                            stats.hp -= damage;
                            final_hp = stats.hp;
                            if stats.hp <= 0 {
                                defeated = true;
                            }
                        }
                        let enemy_name = Self::get_class_name(enemy_class);
                        let player_name = Self::get_class_name(player_class);
                        self.log(format!(
                            "{} attacked {} for {} damage! (Target HP: {})",
                            enemy_name, player_name, damage, final_hp
                        ));

                        if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                            log.send(GameEvent::Attacked {
                                attacker: enemy_entity,
                                target: player_entity,
                                damage,
                            });
                        }

                        if defeated {
                            self.log(format!("{} was defeated!", player_name));
                            if let Some(log) = self.world.resource_mut::<Events<GameEvent>>() {
                                log.send(GameEvent::Defeated {
                                    entity: player_entity,
                                });
                            }
                            self.world.despawn(player_entity);

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

    pub fn apply_action(&mut self, action: Action, _source: ActionSource) {
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
                self.camera
                    .look_at(state.cursor.x as f32, state.cursor.y as f32);
            }
            Action::Confirm => {
                let cursor = self.world.resource::<GameState>().unwrap().cursor;
                let selected_entity = self.world.resource::<GameState>().unwrap().selected_entity;

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
                                    let damage = std::cmp::max(1, atk_val - target_stats.def);
                                    let mut defeated = false;
                                    let mut final_hp = 0;
                                    if let Some(t_stats) =
                                        self.world.get_mut::<Stats>(target_entity)
                                    {
                                        t_stats.hp -= damage;
                                        final_hp = t_stats.hp;
                                        if t_stats.hp <= 0 {
                                            defeated = true;
                                        }
                                    }
                                    let attacker_name = Self::get_class_name(sel_class);
                                    let target_name = Self::get_class_name(target_class);
                                    self.log(format!(
                                        "{} attacked {} for {} damage! (Target HP: {})",
                                        attacker_name, target_name, damage, final_hp
                                    ));

                                    if let Some(log) =
                                        self.world.resource_mut::<Events<GameEvent>>()
                                    {
                                        log.send(GameEvent::Attacked {
                                            attacker: sel_entity,
                                            target: target_entity,
                                            damage,
                                        });
                                    }

                                    if defeated {
                                        self.log(format!("{} was defeated!", target_name));
                                        if let Some(log) =
                                            self.world.resource_mut::<Events<GameEvent>>()
                                        {
                                            log.send(GameEvent::Defeated {
                                                entity: target_entity,
                                            });
                                        }
                                        self.world.despawn(target_entity);

                                        let mut enemy_exists = false;
                                        for (_e, team) in self.world.query::<Team>() {
                                            if *team == Team::Enemy {
                                                enemy_exists = true;
                                                break;
                                            }
                                        }
                                        if !enemy_exists {
                                            self.world
                                                .resource_mut::<GameState>()
                                                .unwrap()
                                                .outcome = Outcome::Victory;
                                            self.log("Victory! All enemies defeated.");
                                        }
                                    }
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

                                        if let Some(log) =
                                            self.world.resource_mut::<Events<GameEvent>>()
                                        {
                                            log.send(GameEvent::Healed {
                                                healer: sel_entity,
                                                target: target_entity,
                                                amount: heal_val,
                                            });
                                        }
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

                                    if let Some(log) =
                                        self.world.resource_mut::<Events<GameEvent>>()
                                    {
                                        log.send(GameEvent::Moved {
                                            entity: sel_entity,
                                            from: from_pos,
                                            to: cursor,
                                        });
                                    }

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
                }
            }
            Action::Cancel => {
                let mut state = self.world.resource_mut::<GameState>().unwrap();
                if state.selected_entity.is_some() {
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
            Action::EndTurn => {
                let phase = self.world.resource::<GameState>().unwrap().phase;
                if phase == TurnPhase::Player {
                    self.end_player_turn();
                }
            }
            Action::Quit => {
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Quit;
            }
            _ => {}
        }
        self.camera.tick();
    }

    pub fn render(&self) -> Grid {
        let map = self.world.resource::<TacticalMap>().unwrap();
        let state = self.world.resource::<GameState>().unwrap();
        let registry = self.world.resource::<VisualRegistry>().unwrap();

        let tile_w = 8;
        let tile_h = 4;
        let mut grid = Grid::new(map.width * tile_w, map.height * tile_h + 6);

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
                        grid.put(
                            rx + dx,
                            ry + dy,
                            Cell::new(glyph).with_fg(Color(40, 40, 40)).with_bg(color),
                        );
                    }
                }
            }
        }

        // Draw movement range overlay if a player character is selected
        if let Some(sel_entity) = state.selected_entity {
            let reachable = self.get_reachable_tiles(sel_entity);
            for pos in reachable {
                let rx = pos.x as u16 * tile_w;
                let ry = pos.y as u16 * tile_h;
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        if let Some(cell) = grid.get_mut(rx + dx, ry + dy) {
                            cell.bg = verryte_terminal::vfx::blend_color(
                                cell.bg,
                                Color(0, 100, 150),
                                0.35,
                            );
                        }
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
                let sprite_grid = asset.render(verryte_terminal::ResolutionTier::TINY);
                let sw = sprite_grid.width();
                let sh = sprite_grid.height();

                let rx = (pos.x as u16 * tile_w) + (tile_w - sw) / 2;
                let ry = (pos.y as u16 * tile_h) + (tile_h - sh) / 2;

                grid.blit(&sprite_grid, rx as i32, ry as i32);
            }
        }

        // Render cursor
        let cx = state.cursor.x as u16 * tile_w;
        let cy = state.cursor.y as u16 * tile_h;
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                if let Some(cell) = grid.get_mut(cx + dx, cy + dy) {
                    cell.bg = verryte_terminal::vfx::blend_color(cell.bg, Color(150, 150, 0), 0.3);
                }
            }
        }

        // Draw HUD
        let hud_y = map.height * tile_h;
        let hud_w = map.width * tile_w;

        let border_fg = Color(100, 100, 100);
        let border_bg = Color::BLACK;
        let border_line = "═".repeat(hud_w as usize);
        grid.write_str(0, hud_y, &border_line, border_fg, border_bg);

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
                selection_str = format!(
                    "Selected: {} (HP: {}/{}, AP: {}/{})",
                    name, stats.hp, stats.max_hp, stats.ap, stats.max_ap
                );
            }
        }

        // Draw HUD line 1 components
        grid.write_str(
            2,
            hud_y + 1,
            &format!("TURN: {:02} | ", state.turn),
            Color::WHITE,
            Color::BLACK,
        );
        grid.write_str(
            13,
            hud_y + 1,
            &format!("PHASE: {:<12}", phase_str),
            phase_color,
            Color::BLACK,
        );
        grid.write_str(
            31,
            hud_y + 1,
            &format!(" | {}", selection_str),
            Color::WHITE,
            Color::BLACK,
        );

        let hovered_tile = map.tile(state.cursor.x, state.cursor.y);
        let tile_type_str = match hovered_tile {
            Tile::Grass => "Grass",
            Tile::Wall => "Wall",
            Tile::Water => "Water",
        };

        let hovered_str = if let Some((_, target_team, target_stats, target_class)) =
            self.get_entity_at(state.cursor)
        {
            let name = Self::get_class_name(target_class);
            let team_str = match target_team {
                Team::Player => "Player",
                Team::Enemy => "Enemy",
            };
            format!(
                "Tile: {} | Entity: {} (HP: {}/{}, AP: {}/{}, Team: {})",
                tile_type_str,
                name,
                target_stats.hp,
                target_stats.max_hp,
                target_stats.ap,
                target_stats.max_ap,
                team_str
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
            Color::BLACK,
        );
        grid.write_str(20, hud_y + 2, &hovered_str, Color::WHITE, Color::BLACK);

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
                    Color::BLACK,
                );
            }
        }

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
}
