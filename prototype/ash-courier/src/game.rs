use verryte_core::{Entity, Events, GameClock, MessageLog, Rng, Schedule, World};
use verryte_input::{ActionSource, Bindings, InputEvent, InputRouter};
use verryte_map::{Direction, Point, TileGrid, Visibility, VisibilityMap};
use verryte_terminal::{Cell, Color, ColorPalette, Grid, Rect};

use crate::action::{default_bindings, Action};
use crate::components::{
    Battery, BatteryPack, Chaser, GameEvent, GameState, Hazard, Outcome, Package, Player, Position,
    RechargeStation,
};
use crate::map::{Map, Tile};
use crate::snapshot::{ActionResult, Snapshot, StepReport};
use crate::systems::resolve_tile_system;

#[derive(serde::Serialize, serde::Deserialize)]
struct SaveState {
    clock_ticks: u64,
    rng_state: u64,
    game_state: GameState,
    messages: Vec<String>,
    map: Map,
    fov: VisibilityMap,
    camera: verryte_terminal::Camera,
    entities: Vec<EntityData>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EntityData {
    kind: String,
    pos: Position,
    battery: Option<Battery>,
    charges: Option<u32>,
}

pub struct Game {
    pub world: World,
    pub schedule: Schedule,
    pub router: InputRouter<Action>,
    pub fov: VisibilityMap,
    pub camera: verryte_terminal::Camera,
    player: Entity,
}

impl Game {
    /// Build a game using the default starting map and keymap.
    pub fn new() -> Self {
        Self::from_layout(DEFAULT_MAP, default_bindings()).expect("default map is well-formed")
    }

    /// Build a game with a specific RNG seed for reproducible behavior.
    pub fn with_seed(seed: u64) -> Self {
        Self::from_layout_with_seed(DEFAULT_MAP, default_bindings(), seed).unwrap()
    }

    /// Build a game using a specific layout and keymap.
    pub fn from_layout(rows: &[&str], bindings: Bindings<Action>) -> Result<Self, MapError> {
        Self::from_layout_with_seed(rows, bindings, 1)
    }

    pub fn from_layout_with_seed(
        rows: &[&str],
        bindings: Bindings<Action>,
        seed: u64,
    ) -> Result<Self, MapError> {
        let mut world = World::new();

        let height = rows.len() as u16;
        if height == 0 {
            return Err(MapError::Empty);
        }
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u16;

        let mut map = Map::new(width, height);
        let mut player_pos_opt = None;
        let mut player_entity_opt = None;

        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let pt = Point::new(x as i16, y as i16);
                match ch {
                    '@' => {
                        if player_pos_opt.is_some() {
                            return Err(MapError::DuplicatePlayer);
                        }
                        player_pos_opt = Some(pt);
                        map.set(x as u16, y as u16, Tile::Floor);
                        let e = world
                            .builder()
                            .with(pt)
                            .with(Player)
                            .with(Battery {
                                current: 100,
                                max: 100,
                            })
                            .build();
                        player_entity_opt = Some(e);
                    }
                    '#' => map.set(x as u16, y as u16, Tile::Wall),
                    'G' => {
                        map.set(x as u16, y as u16, Tile::Goal);
                    }
                    'p' => {
                        map.set(x as u16, y as u16, Tile::Floor);
                        world.builder().with(pt).with(Package).build();
                    }
                    'h' => {
                        map.set(x as u16, y as u16, Tile::Floor);
                        world.builder().with(pt).with(Hazard).build();
                    }
                    'c' => {
                        map.set(x as u16, y as u16, Tile::Floor);
                        world.builder().with(pt).with(Hazard).with(Chaser).build();
                    }
                    'b' => {
                        map.set(x as u16, y as u16, Tile::Floor);
                        world.builder().with(pt).with(BatteryPack).build();
                    }
                    'R' => {
                        map.set(x as u16, y as u16, Tile::Floor);
                        world
                            .builder()
                            .with(pt)
                            .with(RechargeStation { charges: 3 })
                            .build();
                    }
                    _ => map.set(x as u16, y as u16, Tile::Floor),
                }
            }
        }

        let player = player_entity_opt.ok_or(MapError::NoPlayer)?;
        let player_pos = player_pos_opt.unwrap();

        world.insert_resource(map);
        world.insert_resource(GameState::default());
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(seed));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(MessageLog::with_max(50));
        world.insert_resource(default_visual_registry());

        let mut schedule = Schedule::new();
        schedule.add_named("chaser", crate::systems::chaser_system);
        schedule.add_named("resolve", resolve_tile_system);
        schedule.add_named("messages", crate::systems::message_system);

        let mut game = Self {
            world,
            schedule,
            router: InputRouter::new(bindings),
            fov: VisibilityMap::new(width, height),
            camera: verryte_terminal::Camera::new(player_pos.x as f32, player_pos.y as f32),
            player,
        };
        game.update_fov();
        game.update_camera();
        Ok(game)
    }

    pub fn from_cave(width: u16, height: u16, seed: u64) -> Self {
        let width = width.max(10);
        let height = height.max(10);
        let mut grid = TileGrid::new(width, height, Tile::Wall);
        grid.cellular_automata_cave(Tile::Wall, Tile::Floor, 0.42, 5, 4, seed);

        // Collect walkable tiles (floor only, not wall or goal).
        let walkable: Vec<Point> = grid
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Floor))
            .map(|(p, _)| p)
            .collect();
        assert!(
            walkable.len() >= 4,
            "cave must have at least 4 walkable tiles"
        );

        Self::from_generated_grid(grid, walkable, width, height, seed)
    }

    pub fn from_bsp(width: u16, height: u16, seed: u64) -> Self {
        let width = width.max(10);
        let height = height.max(10);
        let mut grid = TileGrid::new(width, height, Tile::Wall);
        let centers = grid.generate_bsp_dungeon(Tile::Wall, Tile::Floor, 3, seed);

        if centers.is_empty() {
            // Fallback: generate a cave instead.
            return Self::from_cave(width, height, seed);
        }

        // Collect walkable tiles.
        let walkable: Vec<Point> = grid
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Floor))
            .map(|(p, _)| p)
            .collect();
        assert!(
            walkable.len() >= 4,
            "BSP dungeon must have at least 4 walkable tiles"
        );

        Self::from_generated_grid(grid, walkable, width, height, seed)
    }

    fn from_generated_grid(
        mut grid: TileGrid<Tile>,
        mut walkable: Vec<Point>,
        width: u16,
        height: u16,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::seed(seed.wrapping_add(1));

        // Pick player spawn.
        let player_idx = rng.pick_index(walkable.len()).unwrap();
        let player_pt = walkable.remove(player_idx);

        // Pick goal (far from player if possible).
        let goal_idx = rng.pick_index(walkable.len()).unwrap();
        let goal_pt = walkable.remove(goal_idx);
        grid.set(goal_pt, Tile::Goal);

        // Pick package.
        let pkg_idx = rng.pick_index(walkable.len()).unwrap();
        let pkg_pt = walkable.remove(pkg_idx);

        // Pick hazard.
        let haz_idx = rng.pick_index(walkable.len()).unwrap();
        let haz_pt = walkable.remove(haz_idx);

        // Pick chaser (if enough walkable tiles remain).
        let chaser_pt = if !walkable.is_empty() {
            let chaser_idx = rng.pick_index(walkable.len()).unwrap();
            Some(walkable.remove(chaser_idx))
        } else {
            None
        };

        // Pick battery pack.
        let bat_pt = if !walkable.is_empty() {
            let bat_idx = rng.pick_index(walkable.len()).unwrap();
            Some(walkable.remove(bat_idx))
        } else {
            None
        };

        let map = Map {
            width,
            height,
            tiles: grid,
        };

        let mut world = World::new();

        let player = world
            .builder()
            .with(Position {
                x: player_pt.x,
                y: player_pt.y,
            })
            .with(Player)
            .with(Battery {
                current: 100,
                max: 100,
            })
            .build();

        world
            .builder()
            .with(Position {
                x: pkg_pt.x,
                y: pkg_pt.y,
            })
            .with(Package)
            .build();

        world
            .builder()
            .with(Position {
                x: haz_pt.x,
                y: haz_pt.y,
            })
            .with(Hazard)
            .build();

        if let Some(chaser_pos) = chaser_pt {
            world
                .builder()
                .with(Position {
                    x: chaser_pos.x,
                    y: chaser_pos.y,
                })
                .with(Hazard)
                .with(Chaser)
                .build();
        }

        if let Some(bat_pos) = bat_pt {
            world
                .builder()
                .with(Position {
                    x: bat_pos.x,
                    y: bat_pos.y,
                })
                .with(BatteryPack)
                .build();
        }

        world.insert_resource(map);
        world.insert_resource(GameState::default());
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(seed));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(MessageLog::with_max(50));
        world.insert_resource(default_visual_registry());

        let mut schedule = Schedule::new();
        schedule.add_named("chaser", crate::systems::chaser_system);
        schedule.add_named("resolve", resolve_tile_system);
        schedule.add_named("messages", crate::systems::message_system);

        let mut game = Self {
            world,
            schedule,
            router: InputRouter::new(default_bindings()),
            fov: VisibilityMap::new(width, height),
            camera: verryte_terminal::Camera::new(player_pt.x as f32, player_pt.y as f32),
            player,
        };
        game.update_fov();
        game.update_camera();
        game
    }

    pub fn reset(&mut self) {
        let fresh = Self::new();
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.fov = fresh.fov;
        self.camera = fresh.camera;
        self.router.clear();
    }

    pub fn reset_from_layout(&mut self, rows: &[&str]) -> Result<(), MapError> {
        let fresh = Self::from_layout(rows, default_bindings())?;
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.fov = fresh.fov;
        self.camera = fresh.camera;
        self.router.clear();
        Ok(())
    }

    pub fn reset_from_layout_with_seed(
        &mut self,
        rows: &[&str],
        seed: u64,
    ) -> Result<(), MapError> {
        let fresh = Self::from_layout_with_seed(rows, default_bindings(), seed)?;
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.fov = fresh.fov;
        self.camera = fresh.camera;
        self.router.clear();
        Ok(())
    }

    pub fn reset_from_cave(&mut self, width: u16, height: u16, seed: u64) {
        let fresh = Self::from_cave(width, height, seed);
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.fov = fresh.fov;
        self.camera = fresh.camera;
        self.router.clear();
    }

    pub fn reset_from_bsp(&mut self, width: u16, height: u16, seed: u64) {
        let fresh = Self::from_bsp(width, height, seed);
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.fov = fresh.fov;
        self.camera = fresh.camera;
        self.router.clear();
    }

    pub fn player_entity(&self) -> Entity {
        self.player
    }

    pub fn player_position(&self) -> Position {
        *self
            .world
            .get::<Position>(self.player)
            .expect("player has position")
    }

    pub fn state(&self) -> &GameState {
        self.world
            .resource::<GameState>()
            .expect("game state resource")
    }

    pub fn state_mut(&mut self) -> &mut GameState {
        self.world
            .resource_mut::<GameState>()
            .expect("game state resource")
    }

    pub fn map(&self) -> &Map {
        self.world.resource::<Map>().expect("map resource")
    }

    pub fn clock(&self) -> &GameClock {
        self.world.resource::<GameClock>().expect("clock resource")
    }

    pub fn messages(&self) -> Vec<String> {
        self.world
            .resource::<MessageLog>()
            .map(|log| log.messages().to_vec())
            .unwrap_or_default()
    }

    pub fn outcome(&self) -> Outcome {
        self.state().outcome
    }

    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        self.router.handle_event(event)
    }

    pub fn handle_event_with<F>(&mut self, event: InputEvent, f: F) -> bool
    where
        F: FnOnce(InputEvent) -> Option<Action>,
    {
        self.router.handle_with(event, f)
    }

    pub fn is_over(&self) -> bool {
        !matches!(self.outcome(), Outcome::Playing)
    }

    pub fn inject_apply(&mut self, action: Action) {
        self.router.inject(action);
        self.run_pending();
    }

    pub fn run_pending(&mut self) {
        while let Some(action) = self.router.pop_action() {
            self.apply_action(action.action, action.source);
        }
    }

    pub fn run_pending_reports(&mut self) -> Vec<StepReport> {
        let mut reports = Vec::new();
        while let Some(action) = self.router.pop_action() {
            reports.push(self.apply_action(action.action, action.source));
        }
        reports
    }

    pub fn inject_script(&mut self, script: &str, source: ActionSource) -> Result<usize, String> {
        self.router
            .inject_script(&crate::action::default_commands(), script, source)
            .map_err(|e| e.to_string())
    }

    /// Apply one action with a default ActionSource::Test source.
    pub fn step(&mut self, action: Action) -> StepReport {
        self.apply_action(action, ActionSource::Test)
    }

    pub fn apply_action(&mut self, action: Action, source: ActionSource) -> StepReport {
        let before = self.snapshot();
        let old_turn = before.turn;

        let result = self.apply_action_internal(action);

        let after = self.snapshot();
        let turn_advanced = after.turn > old_turn;
        let changed = before != after;

        StepReport {
            action,
            source,
            result,
            before,
            after,
            changed,
            turn_advanced,
            events: self.take_events(),
        }
    }

    fn apply_action_internal(&mut self, action: Action) -> ActionResult {
        if self.outcome() != Outcome::Playing && action != Action::Quit {
            return ActionResult::IgnoredGameOver;
        }

        let result = match action {
            Action::MoveNorth | Action::MoveSouth | Action::MoveEast | Action::MoveWest => {
                let direction = action.direction().unwrap();
                if self.try_move(direction) {
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }
            Action::Wait => {
                let pos = self.player_position();
                self.send_event(GameEvent::Waited { at: pos });
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::Scan => {
                let pos = self.player_position();
                let visible_tiles = self.visible_tiles();
                let visible_hazards = self.visible_hazards_in(&visible_tiles);
                self.world.resource_mut::<GameState>().unwrap().scans += 1;
                self.send_event(GameEvent::Scanned {
                    at: pos,
                    visible_tiles: visible_tiles.len(),
                    visible_hazards: visible_hazards.len(),
                });
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::ScanRadius(radius) => {
                let pos = self.player_position();
                let visible_tiles = self.map().visible_from(self.player_position(), radius);
                let visible_hazards = self.visible_hazards_in(&visible_tiles);
                self.world.resource_mut::<GameState>().unwrap().scans += 1;
                self.send_event(GameEvent::Scanned {
                    at: pos,
                    visible_tiles: visible_tiles.len(),
                    visible_hazards: visible_hazards.len(),
                });
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::Inspect(point) => {
                if self.map().in_bounds(point) {
                    let state = self.world.resource_mut::<GameState>().unwrap();
                    state.cursor = Some(point);
                    let tile = self.map().tile(point.x, point.y);
                    self.send_event(GameEvent::Inspected { at: point, tile });
                    ActionResult::Updated
                } else {
                    ActionResult::NoOp
                }
            }
            Action::ClearCursor => {
                let mut state = self.world.resource_mut::<GameState>().unwrap();
                if let Some(at) = state.cursor.take() {
                    self.send_event(GameEvent::CursorCleared { at });
                    ActionResult::Updated
                } else {
                    ActionResult::NoOp
                }
            }
            Action::PickUp => {
                let pos = self.player_position();
                let item = self
                    .world
                    .query2::<Position, Package>()
                    .into_iter()
                    .find(|(_, p, _)| **p == pos);
                if let Some((e, _, _)) = item {
                    self.world.despawn(e);
                    self.world.resource_mut::<GameState>().unwrap().has_package = true;
                    self.send_event(GameEvent::PickedUp { at: pos });
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    let battery = self
                        .world
                        .query2::<Position, BatteryPack>()
                        .into_iter()
                        .find(|(_, p, _)| **p == pos);
                    if let Some((e, _, _)) = battery {
                        self.world.despawn(e);
                        if let Some(b) = self.world.get_mut::<Battery>(self.player) {
                            b.current = (b.current + 25).min(b.max);
                        }
                        self.send_event(GameEvent::PickedUpBattery {
                            at: pos,
                            amount: 25,
                        });
                        self.advance_turn();
                        ActionResult::Advanced
                    } else {
                        ActionResult::NoOp
                    }
                }
            }
            Action::Drop => {
                if self.state().has_package {
                    let pos = self.player_position();
                    self.world.builder().with(pos).with(Package).build();
                    self.world.resource_mut::<GameState>().unwrap().has_package = false;
                    self.send_event(GameEvent::Dropped { at: pos });
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }
            Action::Quit => {
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Quit;
                self.send_event(GameEvent::OutcomeChanged(Outcome::Quit));
                ActionResult::Ended(Outcome::Quit)
            }
            Action::StepToPackage => {
                let packages = self
                    .world
                    .query2::<Position, Package>()
                    .into_iter()
                    .map(|(_, p, _)| *p)
                    .collect::<Vec<_>>();
                self.next_step_direction_toward_any(&packages).map_or(
                    ActionResult::NoOp,
                    |direction| {
                        if self.try_move(direction) {
                            self.advance_turn();
                            ActionResult::Advanced
                        } else {
                            ActionResult::NoOp
                        }
                    },
                )
            }
            Action::StepToGoal => self
                .next_step_direction_toward_any(&self.goal_positions())
                .map_or(ActionResult::NoOp, |direction| {
                    if self.try_move(direction) {
                        self.advance_turn();
                        ActionResult::Advanced
                    } else {
                        ActionResult::NoOp
                    }
                }),
            Action::StepToCursor => {
                let cursor = self
                    .state()
                    .cursor
                    .filter(|point| self.map().in_bounds(*point));
                cursor
                    .and_then(|point| {
                        self.next_step_direction_toward_any(std::slice::from_ref(&point))
                    })
                    .map_or(ActionResult::NoOp, |direction| {
                        if self.try_move(direction) {
                            self.advance_turn();
                            ActionResult::Advanced
                        } else {
                            ActionResult::NoOp
                        }
                    })
            }
            Action::StepToSafety => {
                self.safety_step_direction()
                    .map_or(ActionResult::NoOp, |direction| {
                        if self.try_move(direction) {
                            self.advance_turn();
                            ActionResult::Advanced
                        } else {
                            ActionResult::NoOp
                        }
                    })
            }
            Action::ZoomCamera(amount) => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.camera_zoom = (state.camera_zoom + amount).clamp(-5, 5);
                if amount > 0 {
                    self.camera.zoom *= 0.8;
                } else if amount < 0 {
                    self.camera.zoom *= 1.25;
                }
                self.camera.zoom = self.camera.zoom.clamp(0.2, 5.0);
                ActionResult::Updated
            }
            Action::ToggleLog => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.show_log = !state.show_log;
                ActionResult::Updated
            }
            Action::ToggleHighFidelity => {
                let state = self.world.resource_mut::<GameState>().unwrap();
                state.high_fidelity = !state.high_fidelity;
                state.tier = match state.tier {
                    verryte_terminal::ResolutionTier::TINY => {
                        verryte_terminal::ResolutionTier::SMALL
                    }
                    verryte_terminal::ResolutionTier::SMALL => {
                        verryte_terminal::ResolutionTier::MEDIUM
                    }
                    verryte_terminal::ResolutionTier::MEDIUM => {
                        verryte_terminal::ResolutionTier::LARGE
                    }
                    verryte_terminal::ResolutionTier::LARGE => {
                        verryte_terminal::ResolutionTier::XLARGE
                    }
                    verryte_terminal::ResolutionTier::XLARGE => {
                        verryte_terminal::ResolutionTier::ULTRA
                    }
                    verryte_terminal::ResolutionTier::ULTRA => {
                        verryte_terminal::ResolutionTier::TINY
                    }
                };
                ActionResult::Updated
            }
            mv => mv.direction().map_or(ActionResult::NoOp, |direction| {
                if self.try_move(direction) {
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }),
        };

        if matches!(result, ActionResult::Advanced) {
            let battery_cost = match action {
                Action::Scan | Action::ScanRadius(_) => 2,
                Action::Wait => 1,
                Action::MoveNorth
                | Action::MoveSouth
                | Action::MoveEast
                | Action::MoveWest
                | Action::StepToPackage
                | Action::StepToGoal
                | Action::StepToSafety
                | Action::StepToCursor => 1,
                _ => 0,
            };
            if battery_cost > 0 {
                if let Some(battery) = self.world.get_mut::<Battery>(self.player) {
                    battery.current = battery.current.saturating_sub(battery_cost);
                    if battery.current == 0 {
                        self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Lost;
                        self.send_event(GameEvent::OutcomeChanged(Outcome::Lost));
                    }
                }
            }
            self.check_recharge_station();
        }

        if matches!(result, ActionResult::Advanced)
            || matches!(result, ActionResult::Ended(_))
            || matches!(
                action,
                Action::MoveNorth | Action::MoveSouth | Action::MoveEast | Action::MoveWest
            )
        {
            self.schedule.run(&mut self.world);
            if matches!(result, ActionResult::Advanced) && self.is_over() {
                return ActionResult::Ended(self.outcome());
            }
        }

        result
    }

    fn try_move(&mut self, direction: Direction) -> bool {
        let pos = self.player_position();
        let next = pos.step(direction);
        if self.map().is_walkable(next) {
            *self.world.get_mut::<Position>(self.player).unwrap() = next;
            self.send_event(GameEvent::Moved {
                from: pos,
                to: next,
            });
            self.check_battery_pickup(next);
            true
        } else {
            self.send_event(GameEvent::Blocked {
                from: pos,
                to: next,
            });
            false
        }
    }

    fn check_battery_pickup(&mut self, pos: Position) {
        let found = self
            .world
            .query2::<Position, BatteryPack>()
            .into_iter()
            .find_map(|(e, pack_pos, _)| (*pack_pos == pos).then_some(e));
        if let Some(entity) = found {
            self.world.despawn(entity);
            if let Some(battery) = self.world.get_mut::<Battery>(self.player) {
                battery.current = (battery.current + 25).min(battery.max);
            }
            self.send_event(GameEvent::PickedUpBattery {
                at: pos,
                amount: 25,
            });
        }
    }

    fn advance_turn(&mut self) {
        self.world.resource_mut::<GameClock>().unwrap().tick();
        self.world.resource_mut::<GameState>().unwrap().turn += 1;
        self.update_fov();
        self.update_camera();
    }

    pub fn update_camera(&mut self) {
        let player_pos = self.player_position();
        self.camera
            .look_at(player_pos.x as f32, player_pos.y as f32);
        self.camera.tick();
    }

    pub fn update_fov(&mut self) {
        let player_pos = self.player_position();
        let visible = self.map().visible_from(player_pos, 5);
        self.fov.clear_visible();
        for pt in visible {
            self.fov.set_visible(pt);
        }
    }

    fn clear_events(&mut self) {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .expect("game event resource")
            .clear();
    }

    fn take_events(&mut self) -> Vec<GameEvent> {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .expect("game event resource")
            .drain()
            .collect()
    }

    fn send_event(&mut self, event: GameEvent) {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .expect("game event resource")
            .send(event);
    }

    fn goal_positions(&self) -> Vec<Position> {
        self.map()
            .tiles
            .points_matching(|_, t| matches!(t, Tile::Goal))
    }

    fn shortest_path_to_any(&self, start: Position, targets: &[Position]) -> Option<Vec<Position>> {
        self.map()
            .tiles
            .nearest_path4(start, targets.iter().copied(), |_, t| {
                matches!(t, Tile::Floor | Tile::Goal)
            })
    }

    fn shortest_distance_to_any(&self, start: Position, targets: &[Position]) -> Option<u16> {
        self.map()
            .tiles
            .distance_to_nearest4(start, targets.iter().copied(), |_, t| {
                matches!(t, Tile::Floor | Tile::Goal)
            })
    }

    fn next_step_direction_toward_any(&self, targets: &[Position]) -> Option<Direction> {
        let pos = self.player_position();
        let path = self.shortest_path_to_any(pos, targets)?;
        if path.len() > 1 {
            self.map().tiles.direction_to(pos, path[1])
        } else {
            None
        }
    }

    fn safety_step_direction(&self) -> Option<Direction> {
        let player = self.player_position();
        let hazards = self
            .world
            .query2::<Position, Hazard>()
            .into_iter()
            .map(|(_, p, _)| *p)
            .collect::<Vec<_>>();
        let chasers = self
            .world
            .query2::<Position, Chaser>()
            .into_iter()
            .map(|(_, p, _)| *p)
            .collect::<Vec<_>>();

        let mut all_threats = hazards;
        all_threats.extend(chasers);

        let next = *self.safer_neighbors_from(player, &all_threats).first()?;
        self.map().tiles.direction_to(player, next)
    }

    fn safer_neighbors_from(&self, from: Position, threats: &[Position]) -> Vec<Position> {
        self.map()
            .tiles
            .safer_neighbors4(from, threats.iter().copied(), |_, tile| {
                matches!(tile, Tile::Floor | Tile::Goal)
            })
    }

    fn check_recharge_station(&mut self) {
        if self.is_over() {
            return;
        }
        let pos = self.player_position();
        let station_data = self
            .world
            .query2::<Position, RechargeStation>()
            .into_iter()
            .find(|(_, p, _)| **p == pos)
            .map(|(e, _, s)| (e, s.charges));
        if let Some((e, charges)) = station_data {
            let mut recharged = false;
            if let Some(b) = self.world.get_mut::<Battery>(self.player) {
                if b.current < b.max {
                    b.current = (b.current + 25).min(b.max);
                    recharged = true;
                }
            }
            let next_charges = charges.saturating_sub(1);
            if next_charges == 0 {
                self.world.despawn(e);
            } else {
                if let Some(station_mut) = self.world.get_mut::<RechargeStation>(e) {
                    station_mut.charges = next_charges;
                }
            }
            if recharged {
                self.send_event(GameEvent::PickedUpBattery {
                    at: pos,
                    amount: 25,
                });
            }
        }
    }

    fn visible_tiles(&self) -> Vec<Position> {
        self.map().visible_from(self.player_position(), 5)
    }

    fn visible_hazards_in(&self, visible_tiles: &[Position]) -> Vec<Position> {
        self.world
            .query2::<Position, Hazard>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .filter(|pos| visible_tiles.contains(pos))
            .collect()
    }

    pub fn save_to_string(&self) -> String {
        let mut entities = Vec::new();

        // Player
        let player_pos = self.player_position();
        let battery = self.world.get::<Battery>(self.player).cloned();
        entities.push(EntityData {
            kind: "Player".to_string(),
            pos: player_pos,
            battery,
            charges: None,
        });

        // Package
        for (_, pos, _) in self.world.query2::<Position, Package>() {
            entities.push(EntityData {
                kind: "Package".to_string(),
                pos: *pos,
                battery: None,
                charges: None,
            });
        }

        // Chasers (which are also Hazards)
        let chaser_entities = self
            .world
            .query2::<Position, Chaser>()
            .into_iter()
            .map(|(e, _, _)| e)
            .collect::<std::collections::HashSet<_>>();

        // Hazards (excluding Chasers)
        for (e, pos, _) in self.world.query2::<Position, Hazard>() {
            if !chaser_entities.contains(&e) {
                entities.push(EntityData {
                    kind: "Hazard".to_string(),
                    pos: *pos,
                    battery: None,
                    charges: None,
                });
            }
        }

        // Chasers
        for (_, pos, _) in self.world.query2::<Position, Chaser>() {
            entities.push(EntityData {
                kind: "Chaser".to_string(),
                pos: *pos,
                battery: None,
                charges: None,
            });
        }

        // BatteryPack
        for (_, pos, _) in self.world.query2::<Position, BatteryPack>() {
            entities.push(EntityData {
                kind: "BatteryPack".to_string(),
                pos: *pos,
                battery: None,
                charges: None,
            });
        }

        // RechargeStation
        for (_, pos, s) in self.world.query2::<Position, RechargeStation>() {
            entities.push(EntityData {
                kind: "RechargeStation".to_string(),
                pos: *pos,
                battery: None,
                charges: Some(s.charges),
            });
        }

        let save = SaveState {
            clock_ticks: self.clock().elapsed_ticks(),
            rng_state: self.world.resource::<Rng>().unwrap().seed_value(),
            game_state: *self.state(),
            messages: self.messages(),
            map: self.map().clone(),
            fov: self.fov.clone(),
            camera: self.camera.clone(),
            entities,
        };

        serde_json::to_string_pretty(&save).unwrap_or_default()
    }

    pub fn load_from_string(&mut self, s: &str) -> Result<(), String> {
        let save: SaveState = serde_json::from_str(s).map_err(|e| format!("json error: {}", e))?;

        let mut new_world = World::new();

        let mut clock = GameClock::new();
        clock.set_elapsed_ticks(save.clock_ticks);
        new_world.insert_resource(clock);
        new_world.insert_resource(Rng::seed(save.rng_state));
        new_world.insert_resource(save.game_state);
        new_world.insert_resource(Events::<GameEvent>::with_capacity(16));

        let mut log = MessageLog::with_max(50);
        for msg in save.messages {
            log.push(msg);
        }
        new_world.insert_resource(log);
        new_world.insert_resource(save.map);
        new_world.insert_resource(default_visual_registry());

        let mut player_entity_opt = None;
        for ent in save.entities {
            match ent.kind.as_str() {
                "Player" => {
                    let player = new_world.builder().with(ent.pos).with(Player);
                    let player = if let Some(battery) = ent.battery {
                        player.with(battery)
                    } else {
                        player
                    };
                    let player = player.build();
                    player_entity_opt = Some(player);
                }
                "Package" => {
                    new_world.builder().with(ent.pos).with(Package).build();
                }
                "Hazard" => {
                    new_world.builder().with(ent.pos).with(Hazard).build();
                }
                "Chaser" => {
                    new_world
                        .builder()
                        .with(ent.pos)
                        .with(Hazard)
                        .with(Chaser)
                        .build();
                }
                "BatteryPack" => {
                    new_world.builder().with(ent.pos).with(BatteryPack).build();
                }
                "RechargeStation" => {
                    let charges = ent.charges.unwrap_or(3);
                    new_world
                        .builder()
                        .with(ent.pos)
                        .with(RechargeStation { charges })
                        .build();
                }
                _ => return Err(format!("unknown entity kind: {}", ent.kind)),
            }
        }

        let player =
            player_entity_opt.ok_or_else(|| "missing Player entity in save state".to_string())?;

        self.world = new_world;
        self.player = player;
        self.fov = save.fov;
        self.camera = save.camera;

        Ok(())
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let content = self.save_to_string();
        std::fs::write(path, content)
    }

    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read save file: {}", e))?;
        self.load_from_string(&content)
    }

    /// Render the current state to a [`Grid`].
    pub fn render(&self) -> Grid {
        self.render_with_palette(&ColorPalette::dark_dungeon())
    }

    /// Render the current state to a [`Grid`] using the specified palette.
    pub fn render_with_palette(&self, palette: &ColorPalette) -> Grid {
        let map = self.map();
        let mut grid = Grid::new(map.width, map.height);

        let registry = self.world.resource::<verryte_terminal::VisualRegistry>();
        let tier = self.state().tier;

        // Render terrain
        for y in 0..map.height {
            for x in 0..map.width {
                let pt = Point::new(x as i16, y as i16);
                let visibility = self.fov.get(pt);
                if visibility == Visibility::Hidden {
                    grid.put(
                        x,
                        y,
                        Cell::new(' ').with_fg(Color::BLACK).with_bg(Color::BLACK),
                    );
                    continue;
                }

                let is_explored = visibility == Visibility::Explored;

                let cell = if !self.state().high_fidelity {
                    match map.tile(x as i16, y as i16) {
                        Tile::Wall => Cell::new('#').with_fg(palette.primary),
                        Tile::Floor => Cell::new('.').with_fg(palette.secondary),
                        Tile::Goal => Cell::new('G').with_fg(palette.success),
                    }
                } else if let Some(reg) = registry {
                    let key = match map.tile(x as i16, y as i16) {
                        Tile::Wall => "wall",
                        Tile::Floor => "floor",
                        Tile::Goal => "goal",
                    };
                    if let Some(asset) = reg.get(key) {
                        *asset.render(tier).get(0, 0).unwrap_or(&Cell::EMPTY)
                    } else {
                        match map.tile(x as i16, y as i16) {
                            Tile::Wall => Cell::new('#').with_fg(palette.primary),
                            Tile::Floor => Cell::new('.').with_fg(palette.secondary),
                            Tile::Goal => Cell::new('G').with_fg(palette.success),
                        }
                    }
                } else {
                    match map.tile(x as i16, y as i16) {
                        Tile::Wall => Cell::new('#').with_fg(palette.primary),
                        Tile::Floor => Cell::new('.').with_fg(palette.secondary),
                        Tile::Goal => Cell::new('G').with_fg(palette.success),
                    }
                };

                let cell = if is_explored {
                    let mut dimmed = cell;
                    dimmed.fg = Color(dimmed.fg.0 / 3, dimmed.fg.1 / 3, dimmed.fg.2 / 3);
                    dimmed.bg = Color(dimmed.bg.0 / 3, dimmed.bg.1 / 3, dimmed.bg.2 / 3);
                    dimmed
                } else {
                    cell
                };
                grid.put(x, y, cell);
            }
        }

        // Helper to render entities
        let mut render_entity =
            |key: &str, pos: Position, fallback_glyph: char, fallback_color: Color| {
                if !self.fov.is_visible(pos) {
                    return;
                }
                let cell = if !self.state().high_fidelity {
                    Cell::new(fallback_glyph).with_fg(fallback_color)
                } else if let Some(reg) = registry {
                    if let Some(asset) = reg.get(key) {
                        *asset.render(tier).get(0, 0).unwrap_or(&Cell::EMPTY)
                    } else {
                        Cell::new(fallback_glyph).with_fg(fallback_color)
                    }
                } else {
                    Cell::new(fallback_glyph).with_fg(fallback_color)
                };
                grid.put(pos.x as u16, pos.y as u16, cell);
            };

        for (_, pos, _) in self.world.query2::<Position, Hazard>() {
            render_entity("hazard", *pos, 'h', palette.danger);
        }

        for (_, pos, _) in self.world.query2::<Position, Chaser>() {
            render_entity("chaser", *pos, 'c', palette.info);
        }

        for (_, pos, _) in self.world.query2::<Position, Package>() {
            render_entity("package", *pos, 'p', palette.accent);
        }

        for (_, pos, _) in self.world.query2::<Position, BatteryPack>() {
            render_entity("battery_pack", *pos, 'b', palette.accent);
        }

        for (_, pos, _) in self.world.query2::<Position, RechargeStation>() {
            render_entity("recharge_station", *pos, 'R', palette.accent);
        }

        let player_pos = self.player_position();
        let player_color = if self.state().has_package {
            palette.info
        } else {
            palette.foreground
        };
        render_entity("player", player_pos, '@', player_color);

        if let Some(cursor) = self.state().cursor {
            if self.map().in_bounds(cursor) && self.fov.is_explored(cursor) {
                let x = cursor.x as u16;
                let y = cursor.y as u16;
                if let Some(cell) = grid.get(x, y).copied() {
                    grid.put(x, y, cell.with_bg(palette.ui_highlight));
                }
            }
        }
        grid
    }

    /// Render a clipped viewport centered as closely as possible on the player.
    pub fn render_viewport(&self, width: u16, height: u16) -> Grid {
        let frame = self.render();
        let rect = self.camera.viewport_rect(width, height);
        frame.viewport(rect)
    }

    /// Top-left map coordinate for a viewport centered on the player.
    pub fn viewport_origin(&self, width: u16, height: u16) -> Position {
        let rect = self.camera.viewport_rect(width, height);
        Position::new(rect.x as i16, rect.y as i16)
    }

    pub fn snapshot(&self) -> Snapshot {
        let player = self.player_position();
        let mut packages = self
            .world
            .query2::<Position, Package>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .collect::<Vec<_>>();
        packages.sort_unstable();
        let mut hazards = self
            .world
            .query2::<Position, Hazard>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .collect::<Vec<_>>();
        hazards.sort_unstable();
        let mut chasers = self
            .world
            .query2::<Position, Chaser>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .collect::<Vec<_>>();
        chasers.sort_unstable();
        let map = self.map();
        let state = self.state();
        let mut visible_tiles = Vec::new();
        for pt in map.tiles.points() {
            if self.fov.is_visible(pt) {
                visible_tiles.push(pt);
            }
        }
        let visible_hazards = self.visible_hazards_in(&visible_tiles);
        let reachable_tiles = map.reachable_from(player);
        let goals = self.goal_positions();
        let cursor = state.cursor.filter(|point| map.in_bounds(*point));
        let cursor_tile = cursor.map(|point| map.tile(point.x, point.y));
        let path_to_cursor = cursor.and_then(|point| map.shortest_walkable_path(player, point));
        let distance_to_cursor =
            cursor.and_then(|point| map.nearest_walkable_distance(player, std::iter::once(point)));
        let path_to_nearest_package = self.shortest_path_to_any(player, &packages);
        let path_to_goal = self.shortest_path_to_any(player, &goals);
        let path_to_nearest_hazard = self.shortest_path_to_any(player, &hazards);
        let path_to_nearest_chaser = self.shortest_path_to_any(player, &chasers);
        let distance_to_nearest_package = self.shortest_distance_to_any(player, &packages);
        let distance_to_goal = self.shortest_distance_to_any(player, &goals);
        let distance_to_nearest_hazard = self.shortest_distance_to_any(player, &hazards);
        let distance_to_nearest_chaser = self.shortest_distance_to_any(player, &chasers);
        let safer_neighbors = self.safer_neighbors_from(player, &hazards);

        let chebyshev_to_goal = goals.iter().map(|g| player.chebyshev_distance(*g)).min();
        let chebyshev_to_nearest_hazard =
            hazards.iter().map(|h| player.chebyshev_distance(*h)).min();
        let chebyshev_to_nearest_package =
            packages.iter().map(|p| player.chebyshev_distance(*p)).min();
        let chebyshev_to_nearest_chaser =
            chasers.iter().map(|c| player.chebyshev_distance(*c)).min();

        let euclidean_to_goal = goals
            .iter()
            .map(|g| player.euclidean_distance(*g))
            .min_by(|a, b| a.partial_cmp(b).unwrap());
        let euclidean_to_nearest_hazard = hazards
            .iter()
            .map(|h| player.euclidean_distance(*h))
            .min_by(|a, b| a.partial_cmp(b).unwrap());
        let euclidean_to_nearest_package = packages
            .iter()
            .map(|p| player.euclidean_distance(*p))
            .min_by(|a, b| a.partial_cmp(b).unwrap());
        let euclidean_to_nearest_chaser = chasers
            .iter()
            .map(|c| player.euclidean_distance(*c))
            .min_by(|a, b| a.partial_cmp(b).unwrap());

        let battery = self
            .world
            .get::<Battery>(self.player)
            .map(|b| (b.current, b.max));

        Snapshot {
            turn: state.turn,
            outcome: state.outcome,
            has_package: state.has_package,
            scans: state.scans,
            player,
            packages,
            hazards,
            chasers,
            visible_tiles,
            visible_hazards,
            reachable_tiles,
            map_width: map.width,
            map_height: map.height,
            tile_under_player: map.tile(player.x, player.y),
            walkable_neighbors: map.walkable_neighbors(player),
            cursor,
            cursor_tile,
            path_to_cursor,
            distance_to_cursor,
            path_to_nearest_package,
            path_to_goal,
            path_to_nearest_hazard,
            path_to_nearest_chaser,
            distance_to_nearest_package,
            distance_to_goal,
            distance_to_nearest_hazard,
            distance_to_nearest_chaser,
            safer_neighbors,
            entity_count: self.world.entity_count(),
            frame: self.render().to_plain_string(),
            local_frame: self.render_viewport(7, 7).to_plain_string(),
            battery,
            chebyshev_to_goal,
            chebyshev_to_nearest_hazard,
            chebyshev_to_nearest_package,
            chebyshev_to_nearest_chaser,
            euclidean_to_goal,
            euclidean_to_nearest_hazard,
            euclidean_to_nearest_package,
            euclidean_to_nearest_chaser,
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

fn centered_origin(center: i16, span: u16, limit: u16) -> u16 {
    if span >= limit {
        return 0;
    }
    let half = (span / 2) as i16;
    let max_origin = (limit - span) as i16;
    center.saturating_sub(half).clamp(0, max_origin) as u16
}

#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    Empty,
    NoPlayer,
    DuplicatePlayer,
    UnknownGlyph(char),
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::Empty => write!(f, "map layout is empty"),
            MapError::NoPlayer => write!(f, "map layout has no '@' player spawn"),
            MapError::DuplicatePlayer => write!(f, "map layout has more than one '@'"),
            MapError::UnknownGlyph(c) => write!(f, "map layout has unknown glyph '{c}'"),
        }
    }
}

impl std::error::Error for MapError {}

/// The default map shipped with the prototype. Small, fits in a terminal, has
/// every interesting tile: walls, floors, a goal, a package, and a hazard.
pub const DEFAULT_MAP: &[&str] = &[
    "##########",
    "#@.......#",
    "#.##.###.#",
    "#.#....#.#",
    "#.#.p..#.#",
    "#.#....#.#",
    "#.######.#",
    "#..h.....#",
    "#.......G#",
    "##########",
];

pub fn default_visual_registry() -> verryte_terminal::VisualRegistry {
    use verryte_terminal::{Cell, CellAttrs, Color, Grid, VisualAsset, VisualRegistry};

    let mut registry = VisualRegistry::new();

    let make_custom_asset = |glyph: char, fg: Color, bg: Color| {
        let mut g = Grid::new(1, 1);
        g.put(
            0,
            0,
            Cell {
                glyph,
                fg,
                bg,
                attrs: CellAttrs::NONE,
            },
        );
        VisualAsset::BlockSprite(g)
    };

    let floor_bg = Color(15, 15, 20);

    // Walls (solid visual block)
    registry.register(
        "wall",
        make_custom_asset('█', Color(70, 70, 85), Color(35, 35, 45)),
    );
    registry.register_single_cell("wall_fallback", '#', Color::GREY, Color::BLACK);

    // Floors (clean center dot dotting)
    registry.register("floor", make_custom_asset('·', Color(50, 50, 60), floor_bg));
    registry.register_single_cell("floor_fallback", '.', Color::DARK_GREY, Color::BLACK);

    // Goal (four-pointed gold star)
    registry.register("goal", make_custom_asset('✦', Color(255, 215, 0), floor_bg));
    registry.register_single_cell("goal_fallback", 'G', Color::YELLOW, Color::BLACK);

    // Player without package (glowing cyan hexagon)
    registry.register(
        "player",
        make_custom_asset('⬢', Color(0, 255, 255), floor_bg),
    );
    registry.register_single_cell("player_fallback", '@', Color::CYAN, Color::BLACK);

    // Player with package (glowing gold hexagon)
    registry.register(
        "player_carrying",
        make_custom_asset('⬢', Color(255, 165, 0), floor_bg),
    );
    registry.register_single_cell("player_carrying_fallback", '@', Color::YELLOW, Color::BLACK);

    // Hazard (warning red spike)
    registry.register(
        "hazard",
        make_custom_asset('▲', Color(220, 20, 60), floor_bg),
    );
    registry.register_single_cell("hazard_fallback", 'h', Color::RED, Color::BLACK);

    // Chaser (magenta skull)
    registry.register(
        "chaser",
        make_custom_asset('☠', Color(255, 0, 255), floor_bg),
    );
    registry.register_single_cell("chaser_fallback", 'c', Color::MAGENTA, Color::BLACK);

    // Package (lime box/cargo block)
    registry.register(
        "package",
        make_custom_asset('■', Color(50, 205, 50), floor_bg),
    );
    registry.register_single_cell("package_fallback", 'p', Color::GREEN, Color::BLACK);

    // Battery pack (yellow/lime lightning bolt)
    registry.register(
        "battery_pack",
        make_custom_asset('⚡', Color(173, 255, 47), floor_bg),
    );
    registry.register_single_cell("battery_pack_fallback", 'b', Color::GREEN, Color::BLACK);

    // Recharge station (blue energy junction/diamond circular)
    registry.register(
        "recharge_station",
        make_custom_asset('⛯', Color(30, 144, 255), floor_bg),
    );
    registry.register_single_cell("recharge_station_fallback", 'R', Color::BLUE, Color::BLACK);

    registry
}
