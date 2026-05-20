use verryte_core::{Entity, Events, GameClock, MessageLog, Rng, Schedule, World};
use verryte_input::{ActionSource, Bindings, InputEvent, InputRouter};
use verryte_map::{Direction, Point, TileGrid};
use verryte_terminal::{Cell, ColorPalette, Grid, Rect};

use crate::action::{default_bindings, Action};
use crate::components::{Chaser, GameEvent, GameState, Hazard, Outcome, Package, Player, Position};
use crate::map::{Map, Tile};
use crate::snapshot::{ActionResult, Snapshot, StepReport};
use crate::systems::resolve_tile_system;

pub struct Game {
    pub world: World,
    pub schedule: Schedule,
    pub router: InputRouter<Action>,
    player: Entity,
}

impl Game {
    /// Build a game using the default starting map and keymap.
    pub fn new() -> Self {
        Self::from_layout(DEFAULT_MAP, default_bindings()).expect("default map is well-formed")
    }

    /// Build a game with a specific RNG seed for reproducible behavior.
    pub fn with_seed(seed: u64) -> Self {
        Self::from_layout_with_seed(DEFAULT_MAP, default_bindings(), seed)
            .expect("default map is well-formed")
    }

    /// Build a game from an ASCII map layout.
    ///
    /// Map symbols:
    /// * `#` — wall
    /// * `.` — floor
    /// * `@` — player spawn (floor underneath)
    /// * `p` — package on floor
    /// * `h` — hazard on floor
    /// * `G` — goal tile
    pub fn from_layout(rows: &[&str], bindings: Bindings<Action>) -> Result<Self, MapError> {
        Self::from_layout_with_seed(rows, bindings, 1)
    }

    /// Build a game from an ASCII map layout with a specific RNG seed.
    pub fn from_layout_with_seed(
        rows: &[&str],
        bindings: Bindings<Action>,
        seed: u64,
    ) -> Result<Self, MapError> {
        let height = rows.len() as u16;
        if height == 0 {
            return Err(MapError::Empty);
        }
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;
        if width == 0 {
            return Err(MapError::Empty);
        }
        let mut map = Map::new(width, height);

        let mut world = World::new();
        let mut player_spawn: Option<Position> = None;

        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let (tile, entity_kind) = match ch {
                    '#' => (Tile::Wall, None),
                    '.' => (Tile::Floor, None),
                    '@' => (Tile::Floor, Some(SpawnKind::Player)),
                    'p' => (Tile::Floor, Some(SpawnKind::Package)),
                    'h' => (Tile::Floor, Some(SpawnKind::Hazard)),
                    'c' => (Tile::Floor, Some(SpawnKind::Chaser)),
                    'G' => (Tile::Goal, None),
                    ' ' => (Tile::Wall, None), // treat padding as wall
                    other => return Err(MapError::UnknownGlyph(other)),
                };
                map.set(x as u16, y as u16, tile);
                if let Some(kind) = entity_kind {
                    let pos = Position {
                        x: x as i16,
                        y: y as i16,
                    };
                    match kind {
                        SpawnKind::Player => {
                            if player_spawn.is_some() {
                                return Err(MapError::DuplicatePlayer);
                            }
                            player_spawn = Some(pos);
                        }
                        SpawnKind::Package => {
                            world.builder().with(pos).with(Package).build();
                        }
                        SpawnKind::Hazard => {
                            world.builder().with(pos).with(Hazard).build();
                        }
                        SpawnKind::Chaser => {
                            world.builder().with(pos).with(Hazard).with(Chaser).build();
                        }
                    }
                }
            }
        }

        let player_spawn = player_spawn.ok_or(MapError::NoPlayer)?;
        let player = world.builder().with(player_spawn).with(Player).build();

        world.insert_resource(map);
        world.insert_resource(GameState::default());
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(seed));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(MessageLog::with_max(50));

        let mut schedule = Schedule::new();
        schedule.add_named("chaser", crate::systems::chaser_system);
        schedule.add_named("resolve", resolve_tile_system);
        schedule.add_named("messages", crate::systems::message_system);

        Ok(Self {
            world,
            schedule,
            router: InputRouter::new(bindings),
            player,
        })
    }

    /// Build a game on a procedurally generated cave map.
    ///
    /// Uses cellular automata to carve an organic cave, then places the player,
    /// a package, a goal, and hazards at reachable positions. The `seed`
    /// controls map generation and RNG; the `width`/`height` set the grid
    /// dimensions (minimum 10×10).
    pub fn from_cave(width: u16, height: u16, seed: u64) -> Self {
        let width = width.max(10);
        let height = height.max(10);
        let mut grid = TileGrid::new(width, height, crate::map::Tile::Wall);
        grid.cellular_automata_cave(
            crate::map::Tile::Wall,
            crate::map::Tile::Floor,
            0.42,
            5,
            4,
            seed,
        );

        // Collect walkable tiles (floor only, not wall or goal).
        let walkable: Vec<Point> = grid
            .iter()
            .filter(|(_, tile)| matches!(tile, crate::map::Tile::Floor))
            .map(|(p, _)| p)
            .collect();
        assert!(
            walkable.len() >= 4,
            "cave must have at least 4 walkable tiles"
        );

        Self::from_generated_grid(grid, walkable, width, height, seed)
    }

    /// Build a game on a procedurally generated BSP dungeon map.
    ///
    /// Uses binary space partitioning to create a structured room-and-corridor
    /// layout, then places the player, a package, a goal, and hazards at room
    /// centers or reachable positions. The `seed` controls generation.
    /// The `width`/`height` set the grid dimensions (minimum 10×10).
    pub fn from_bsp(width: u16, height: u16, seed: u64) -> Self {
        let width = width.max(10);
        let height = height.max(10);
        let mut grid = TileGrid::new(width, height, crate::map::Tile::Wall);
        let centers =
            grid.generate_bsp_dungeon(crate::map::Tile::Wall, crate::map::Tile::Floor, 3, seed);

        if centers.is_empty() {
            // Fallback: generate a cave instead.
            return Self::from_cave(width, height, seed);
        }

        // Collect walkable tiles.
        let walkable: Vec<Point> = grid
            .iter()
            .filter(|(_, tile)| matches!(tile, crate::map::Tile::Floor))
            .map(|(p, _)| p)
            .collect();
        assert!(
            walkable.len() >= 4,
            "BSP dungeon must have at least 4 walkable tiles"
        );

        Self::from_generated_grid(grid, walkable, width, height, seed)
    }

    /// Shared helper: place entities on a generated grid and wire up the ECS.
    fn from_generated_grid(
        mut grid: TileGrid<crate::map::Tile>,
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
        grid.set(goal_pt, crate::map::Tile::Goal);

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

        let map = crate::map::Map {
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

        world.insert_resource(map);
        world.insert_resource(GameState::default());
        world.insert_resource(GameClock::new());
        world.insert_resource(Rng::seed(seed));
        world.insert_resource(Events::<GameEvent>::with_capacity(16));
        world.insert_resource(MessageLog::with_max(50));

        let mut schedule = Schedule::new();
        schedule.add_named("chaser", crate::systems::chaser_system);
        schedule.add_named("resolve", resolve_tile_system);
        schedule.add_named("messages", crate::systems::message_system);

        Self {
            world,
            schedule,
            router: InputRouter::new(default_bindings()),
            player,
        }
    }

    /// Reset the game to its initial state using the default map.
    ///
    /// This is the agent-ready restart path: reuse the same `Game` struct
    /// (and its `InputRouter`) while resetting all world state to a fresh
    /// default game. Pending router actions are also cleared.
    pub fn reset(&mut self) {
        let fresh = Self::new();
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.router.clear();
    }

    /// Reset the game using a specific layout.
    pub fn reset_from_layout(&mut self, rows: &[&str]) -> Result<(), MapError> {
        let fresh = Self::from_layout(rows, default_bindings())?;
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.router.clear();
        Ok(())
    }

    /// Reset the game using a specific layout and seed.
    pub fn reset_from_layout_with_seed(
        &mut self,
        rows: &[&str],
        seed: u64,
    ) -> Result<(), MapError> {
        let fresh = Self::from_layout_with_seed(rows, default_bindings(), seed)?;
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.router.clear();
        Ok(())
    }

    /// Reset the game to a new procedurally generated cave.
    pub fn reset_from_cave(&mut self, width: u16, height: u16, seed: u64) {
        let fresh = Self::from_cave(width, height, seed);
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.router.clear();
    }

    /// Reset the game to a new procedurally generated BSP dungeon.
    pub fn reset_from_bsp(&mut self, width: u16, height: u16, seed: u64) {
        let fresh = Self::from_bsp(width, height, seed);
        self.world = fresh.world;
        self.schedule = fresh.schedule;
        self.player = fresh.player;
        self.router.clear();
    }

    pub fn player_entity(&self) -> Entity {
        self.player
    }

    pub fn state(&self) -> &GameState {
        self.world
            .resource::<GameState>()
            .expect("game state resource")
    }

    pub fn clock(&self) -> &GameClock {
        self.world
            .resource::<GameClock>()
            .expect("game clock resource")
    }

    pub fn map(&self) -> &Map {
        self.world.resource::<Map>().expect("map resource")
    }

    pub fn player_position(&self) -> Position {
        *self
            .world
            .get::<Position>(self.player)
            .expect("player has position")
    }

    pub fn outcome(&self) -> Outcome {
        self.state().outcome
    }

    pub fn is_over(&self) -> bool {
        !matches!(self.outcome(), Outcome::Playing)
    }

    /// Drain the router and apply each action. Stops if the game ends.
    /// Returns the number of actions consumed.
    pub fn run_pending(&mut self) -> usize {
        self.run_pending_reports().len()
    }

    /// Drain pending actions and keep a report for each applied action.
    pub fn run_pending_reports(&mut self) -> Vec<StepReport> {
        if self.is_over() {
            let queued_quit = matches!(
                self.router.peek(),
                Some(queued) if queued.action == Action::Quit
            );
            if queued_quit {
                let queued = self.router.next_queued().expect("peek returned Some");
                let report = self.step_from(queued.action, queued.source);
                self.router.clear();
                return vec![report];
            }
            self.router.clear();
            return Vec::new();
        }
        let mut reports = Vec::new();
        while let Some(queued) = self.router.next_queued() {
            reports.push(self.step_from(queued.action, queued.source));
            if self.is_over() {
                self.router.clear();
                break;
            }
        }
        reports
    }

    /// Feed a terminal event in. Same downstream path as `inject`/scripts.
    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        self.router.handle(event)
    }

    /// Feed a terminal event in with a custom translation hook, falling back to
    /// bindings when the hook returns `None`.
    pub fn handle_event_with<F>(&mut self, event: InputEvent, translate: F) -> bool
    where
        F: FnOnce(InputEvent) -> Option<Action>,
    {
        self.router.handle_with(event, translate)
    }

    /// Inject one action and apply it immediately.
    pub fn inject_apply(&mut self, action: Action) {
        self.router.inject(action);
        self.run_pending();
    }

    /// Apply one action and return the state delta an agent or script runner
    /// can inspect.
    pub fn step(&mut self, action: Action) -> StepReport {
        self.step_from(action, ActionSource::Test)
    }

    /// Apply one sourced action and return the state delta an agent or script
    /// runner can inspect. The source is report metadata only; behavior is
    /// still entirely controlled by [`Self::apply`].
    pub fn step_from(&mut self, action: Action, source: ActionSource) -> StepReport {
        let before = self.snapshot();
        let result = self.apply(action);
        let events = self.drain_events();
        let after = self.snapshot();
        StepReport {
            action,
            source,
            result,
            changed: before != after,
            turn_advanced: after.turn > before.turn,
            before,
            after,
            events,
        }
    }

    /// Apply a single action against the game state.
    ///
    /// This is the spine of `terminal event -> game action -> game system ->
    /// observable state`. Interactive frontends, scripts, tests, and agents
    /// all converge here.
    pub fn apply(&mut self, action: Action) -> ActionResult {
        self.clear_events();
        let result = match action {
            Action::Quit => {
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Quit;
                self.send_event(GameEvent::OutcomeChanged(Outcome::Quit));
                ActionResult::Ended(Outcome::Quit)
            }
            _ if self.is_over() => ActionResult::IgnoredGameOver,
            Action::Wait => {
                self.send_event(GameEvent::Waited {
                    at: self.player_position(),
                });
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::Scan => {
                let visible_tiles = self.visible_tiles();
                let visible_hazards = self.visible_hazards_in(&visible_tiles);
                self.world.resource_mut::<GameState>().unwrap().scans += 1;
                self.send_event(GameEvent::Scanned {
                    at: self.player_position(),
                    visible_tiles: visible_tiles.len(),
                    visible_hazards: visible_hazards.len(),
                });
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::ScanRadius(radius) => {
                let visible_tiles = self.map().visible_from(self.player_position(), radius);
                let visible_hazards = self.visible_hazards_in(&visible_tiles);
                self.world.resource_mut::<GameState>().unwrap().scans += 1;
                self.send_event(GameEvent::Scanned {
                    at: self.player_position(),
                    visible_tiles: visible_tiles.len(),
                    visible_hazards: visible_hazards.len(),
                });
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::Inspect(point) => {
                if !self.map().in_bounds(point) {
                    ActionResult::NoOp
                } else {
                    let tile = self.map().tile(point.x, point.y);
                    self.world.resource_mut::<GameState>().unwrap().cursor = Some(point);
                    self.send_event(GameEvent::Inspected { at: point, tile });
                    self.schedule
                        .run_system_by_name("messages", &mut self.world);
                    ActionResult::Updated
                }
            }
            Action::ClearCursor => {
                let cursor = self
                    .world
                    .resource_mut::<GameState>()
                    .unwrap()
                    .cursor
                    .take();
                if let Some(cursor) = cursor {
                    self.send_event(GameEvent::CursorCleared { at: cursor });
                    self.schedule
                        .run_system_by_name("messages", &mut self.world);
                    ActionResult::Updated
                } else {
                    ActionResult::NoOp
                }
            }
            Action::PickUp => {
                if self.try_pick_up() {
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }
            Action::Drop => {
                if self.try_drop_package() {
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }
            Action::StepToPackage => {
                let packages = self
                    .world
                    .query2::<Position, Package>()
                    .into_iter()
                    .map(|(_, pos, _)| *pos)
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
            mv => mv.direction().map_or(ActionResult::NoOp, |direction| {
                if self.try_move(direction) {
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }),
        };

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

    pub fn messages(&self) -> Vec<String> {
        self.world
            .resource::<MessageLog>()
            .unwrap()
            .messages()
            .to_vec()
    }

    fn try_move(&mut self, direction: Direction) -> bool {
        let current = self.player_position();
        let target = current.step(direction);
        match self.map().tile(target.x, target.y) {
            Tile::Wall => {
                self.send_event(GameEvent::Blocked {
                    from: current,
                    to: target,
                });
                false
            }
            Tile::Floor | Tile::Goal => {
                *self.world.get_mut::<Position>(self.player).unwrap() = target;
                self.send_event(GameEvent::Moved {
                    from: current,
                    to: target,
                });
                true
            }
        }
    }

    fn try_pick_up(&mut self) -> bool {
        let pos = self.player_position();
        let found = self
            .world
            .query2::<Position, Package>()
            .into_iter()
            .find_map(|(e, package_pos, _)| (*package_pos == pos).then_some(e));
        if let Some(entity) = found {
            self.world.despawn(entity);
            self.world.resource_mut::<GameState>().unwrap().has_package = true;
            self.send_event(GameEvent::PickedUp { at: pos });
            true
        } else {
            false
        }
    }

    fn try_drop_package(&mut self) -> bool {
        if !self.state().has_package {
            return false;
        }

        let pos = self.player_position();
        let entity = self.world.spawn();
        self.world.insert(entity, pos);
        self.world.insert(entity, Package);
        self.world.resource_mut::<GameState>().unwrap().has_package = false;
        self.send_event(GameEvent::Dropped { at: pos });
        true
    }

    fn advance_turn(&mut self) {
        self.world.resource_mut::<GameClock>().unwrap().tick();
        self.world.resource_mut::<GameState>().unwrap().turn += 1;
    }

    fn clear_events(&mut self) {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .expect("game event resource")
            .clear();
    }

    fn send_event(&mut self, event: GameEvent) {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .expect("game event resource")
            .send(event);
    }

    fn drain_events(&mut self) -> Vec<GameEvent> {
        self.world
            .resource_mut::<Events<GameEvent>>()
            .expect("game event resource")
            .drain()
            .collect()
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

    fn goal_positions(&self) -> Vec<Position> {
        self.map()
            .tiles
            .points()
            .filter(|point| matches!(self.map().tile(point.x, point.y), Tile::Goal))
            .collect()
    }

    fn shortest_path_to_any(&self, from: Position, targets: &[Position]) -> Option<Vec<Position>> {
        self.map()
            .nearest_walkable_path(from, targets.iter().copied())
    }

    fn shortest_distance_to_any(&self, from: Position, targets: &[Position]) -> Option<u16> {
        self.map()
            .nearest_walkable_distance(from, targets.iter().copied())
    }

    fn next_step_direction_toward_any(&self, targets: &[Position]) -> Option<Direction> {
        let from = self.player_position();
        let path = self.shortest_path_to_any(from, targets)?;
        let next = *path.get(1)?;
        self.map().tiles.direction_to(from, next)
    }

    fn safety_step_direction(&self) -> Option<Direction> {
        let player = self.player_position();
        let hazards = self
            .world
            .query2::<Position, Hazard>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .collect::<Vec<_>>();
        let next = *self.safer_neighbors_from(player, &hazards).first()?;
        self.map().tiles.direction_to(player, next)
    }

    fn safer_neighbors_from(&self, from: Position, hazards: &[Position]) -> Vec<Position> {
        self.map()
            .tiles
            .safer_neighbors4(from, hazards.iter().copied(), |_, tile| {
                matches!(tile, Tile::Floor | Tile::Goal)
            })
    }

    /// Render the current state to a [`Grid`].
    pub fn render(&self) -> Grid {
        self.render_with_palette(&ColorPalette::dark_dungeon())
    }

    /// Render the current state to a [`Grid`] using the specified palette.
    pub fn render_with_palette(&self, palette: &ColorPalette) -> Grid {
        let map = self.map();
        let mut grid = Grid::new(map.width, map.height);
        for y in 0..map.height {
            for x in 0..map.width {
                let cell = match map.tile(x as i16, y as i16) {
                    Tile::Wall => Cell::new('#').with_fg(palette.wall),
                    Tile::Floor => Cell::new('.').with_fg(palette.floor),
                    Tile::Goal => Cell::new('G').with_fg(palette.goal),
                };
                grid.put(x, y, cell);
            }
        }
        // Layer order: hazards, packages, player on top.
        for (_, pos, _) in self.world.query2::<Position, Hazard>() {
            grid.put(
                pos.x as u16,
                pos.y as u16,
                Cell::new('h').with_fg(palette.hazard),
            );
        }
        for (_, pos, _) in self.world.query2::<Position, Chaser>() {
            grid.put(
                pos.x as u16,
                pos.y as u16,
                Cell::new('c').with_fg(palette.player),
            );
        }
        for (_, pos, _) in self.world.query2::<Position, Package>() {
            grid.put(
                pos.x as u16,
                pos.y as u16,
                Cell::new('p').with_fg(palette.item),
            );
        }
        let player_pos = self.player_position();
        let player_color = if self.state().has_package {
            palette.player
        } else {
            palette.foreground
        };
        grid.put(
            player_pos.x as u16,
            player_pos.y as u16,
            Cell::new('@').with_fg(player_color),
        );
        if let Some(cursor) = self.state().cursor {
            if self.map().in_bounds(cursor) {
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
        let origin = self.viewport_origin(width, height);
        let x = origin.x.max(0) as u16;
        let y = origin.y.max(0) as u16;
        frame.viewport(Rect::new(
            x,
            y,
            width.min(frame.width()),
            height.min(frame.height()),
        ))
    }

    /// Top-left map coordinate for a viewport centered on the player.
    pub fn viewport_origin(&self, width: u16, height: u16) -> Position {
        let map = self.map();
        let player = self.player_position();
        Position {
            x: centered_origin(player.x, width, map.width) as i16,
            y: centered_origin(player.y, height, map.height) as i16,
        }
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
        let visible_tiles = self.visible_tiles();
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
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

enum SpawnKind {
    Player,
    Package,
    Hazard,
    Chaser,
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
