//! Ash Courier — the proving game for the Verryte engine.
//!
//! The point of this crate is *not* to be a deep roguelike. It exists to put
//! real pressure on the engine surface: an ECS world holds the player, packages,
//! and hazards; the map lives as a resource; player intent flows from terminal
//! events or scripted injections through the same [`InputRouter`] queue; a
//! single `apply` function turns each action into observable state changes; and
//! [`Snapshot`] is the structured view that tests, scripts, and agents read.
//!
//! If something in here had to reach behind the engine's back, that points at an
//! engine gap, not a game requirement.

use verryte_core::{Entity, World};
use verryte_input::{ActionSource, Bindings, CommandBindings, InputEvent, InputRouter, Key};
use verryte_map::{Direction, Point, TileGrid};
use verryte_terminal::{Cell, Color, Grid};

pub use verryte_input;
pub use verryte_map;

// ----------------------------------------------------------------------------
// Actions — the action vocabulary Ash Courier exposes through the router.
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    MoveNorth,
    MoveSouth,
    MoveEast,
    MoveWest,
    Wait,
    PickUp,
    Quit,
}

impl Action {
    /// Returns the (dx, dy) for movement actions; `None` for non-movement.
    pub fn movement_delta(self) -> Option<(i16, i16)> {
        self.direction().map(Direction::delta)
    }

    /// Returns the direction for movement actions; `None` for non-movement.
    pub fn direction(self) -> Option<Direction> {
        match self {
            Action::MoveNorth => Some(Direction::North),
            Action::MoveSouth => Some(Direction::South),
            Action::MoveEast => Some(Direction::East),
            Action::MoveWest => Some(Direction::West),
            _ => None,
        }
    }
}

/// Default keymap. Games can rebuild this at will — the engine doesn't care.
pub fn default_bindings() -> Bindings<Action> {
    let mut b = Bindings::new();
    b.bind(Key::Up, Action::MoveNorth);
    b.bind(Key::Char('w'), Action::MoveNorth);
    b.bind(Key::Char('k'), Action::MoveNorth);
    b.bind(Key::Down, Action::MoveSouth);
    b.bind(Key::Char('s'), Action::MoveSouth);
    b.bind(Key::Char('j'), Action::MoveSouth);
    b.bind(Key::Left, Action::MoveWest);
    b.bind(Key::Char('a'), Action::MoveWest);
    b.bind(Key::Char('h'), Action::MoveWest);
    b.bind(Key::Right, Action::MoveEast);
    b.bind(Key::Char('d'), Action::MoveEast);
    b.bind(Key::Char('l'), Action::MoveEast);
    b.bind(Key::Char('.'), Action::Wait);
    b.bind(Key::Space, Action::Wait);
    b.bind(Key::Char('g'), Action::PickUp);
    b.bind(Key::Char(','), Action::PickUp);
    b.bind(Key::Char('q'), Action::Quit);
    b.bind(Key::Esc, Action::Quit);
    b
}

/// Default script/agent command map. Parsed actions are still injected into the
/// same [`InputRouter`] queue used by terminal events.
pub fn default_commands() -> CommandBindings<Action> {
    let mut c = CommandBindings::new();
    c.bind_name("north", Action::MoveNorth);
    c.bind_name("move_north", Action::MoveNorth);
    c.bind_name("south", Action::MoveSouth);
    c.bind_name("move_south", Action::MoveSouth);
    c.bind_name("east", Action::MoveEast);
    c.bind_name("move_east", Action::MoveEast);
    c.bind_name("west", Action::MoveWest);
    c.bind_name("move_west", Action::MoveWest);
    c.bind_name("wait", Action::Wait);
    c.bind_name("pickup", Action::PickUp);
    c.bind_name("pick_up", Action::PickUp);
    c.bind_name("quit", Action::Quit);

    for glyph in ['n', 'N'] {
        c.bind_glyph(glyph, Action::MoveNorth);
    }
    for glyph in ['s', 'S'] {
        c.bind_glyph(glyph, Action::MoveSouth);
    }
    for glyph in ['e', 'E'] {
        c.bind_glyph(glyph, Action::MoveEast);
    }
    for glyph in ['w', 'W'] {
        c.bind_glyph(glyph, Action::MoveWest);
    }
    c.bind_glyph('.', Action::Wait);
    c.bind_glyph(',', Action::PickUp);
    for glyph in ['q', 'Q'] {
        c.bind_glyph(glyph, Action::Quit);
    }
    c
}

// ----------------------------------------------------------------------------
// Map — a tile resource. Walls and goals live here; entities live in the world.
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    Goal,
}

#[derive(Clone, Debug)]
pub struct Map {
    pub width: u16,
    pub height: u16,
    tiles: TileGrid<Tile>,
}

impl Map {
    pub fn tile(&self, x: i16, y: i16) -> Tile {
        self.tiles
            .get(Point { x, y })
            .copied()
            .unwrap_or(Tile::Wall)
    }

    pub fn is_walkable(&self, point: Point) -> bool {
        matches!(self.tile(point.x, point.y), Tile::Floor | Tile::Goal)
    }

    pub fn walkable_neighbors(&self, point: Point) -> Vec<Point> {
        self.tiles
            .neighbors4(point)
            .into_iter()
            .filter_map(|(neighbor, tile)| {
                matches!(tile, Tile::Floor | Tile::Goal).then_some(neighbor)
            })
            .collect()
    }

    fn set(&mut self, x: u16, y: u16, tile: Tile) {
        self.tiles.set(Point::new(x as i16, y as i16), tile);
    }
}

// ----------------------------------------------------------------------------
// Components — Position is the spatial anchor; the rest are marker tags.
// ----------------------------------------------------------------------------

pub type Position = Point;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Player;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Package;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hazard;

// ----------------------------------------------------------------------------
// Resources — game-level state that systems read and write through the world.
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Playing,
    Won,
    Lost,
    Quit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub turn: u32,
    pub outcome: Outcome,
    pub has_package: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            turn: 0,
            outcome: Outcome::Playing,
            has_package: false,
        }
    }
}

// ----------------------------------------------------------------------------
// Snapshot — the structured state surface tests / scripts / agents read.
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub turn: u32,
    pub outcome: Outcome,
    pub has_package: bool,
    pub player: Position,
    pub packages: Vec<Position>,
    pub hazards: Vec<Position>,
    pub map_width: u16,
    pub map_height: u16,
    pub tile_under_player: Tile,
    pub walkable_neighbors: Vec<Position>,
    /// Plain-text rendering of the current frame.
    pub frame: String,
}

/// One applied action and the observable state before/after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepReport {
    pub action: Action,
    pub source: ActionSource,
    pub result: ActionResult,
    pub before: Snapshot,
    pub after: Snapshot,
    pub changed: bool,
    pub turn_advanced: bool,
}

/// The immediate game-level result of applying one action.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActionResult {
    NoOp,
    Advanced,
    Ended(Outcome),
    IgnoredGameOver,
}

// ----------------------------------------------------------------------------
// Game — composition root. Owns the world, map, and shared input router.
// ----------------------------------------------------------------------------

pub struct Game {
    pub world: World,
    pub router: InputRouter<Action>,
    player: Entity,
}

impl Game {
    /// Build a game using the default starting map and keymap.
    pub fn new() -> Self {
        Self::from_layout(DEFAULT_MAP, default_bindings()).expect("default map is well-formed")
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
        let height = rows.len() as u16;
        if height == 0 {
            return Err(MapError::Empty);
        }
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;
        if width == 0 {
            return Err(MapError::Empty);
        }
        let tiles = TileGrid::new(width, height, Tile::Wall);
        let mut map = Map {
            width,
            height,
            tiles,
        };

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
                            let e = world.spawn();
                            world.insert(e, pos);
                            world.insert(e, Package);
                        }
                        SpawnKind::Hazard => {
                            let e = world.spawn();
                            world.insert(e, pos);
                            world.insert(e, Hazard);
                        }
                    }
                }
            }
        }

        let player_spawn = player_spawn.ok_or(MapError::NoPlayer)?;
        let player = world.spawn();
        world.insert(player, player_spawn);
        world.insert(player, Player);

        world.insert_resource(map);
        world.insert_resource(GameState::default());

        Ok(Self {
            world,
            router: InputRouter::new(bindings),
            player,
        })
    }

    pub fn player_entity(&self) -> Entity {
        self.player
    }

    pub fn state(&self) -> &GameState {
        self.world
            .resource::<GameState>()
            .expect("game state resource")
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
        let after = self.snapshot();
        StepReport {
            action,
            source,
            result,
            changed: before != after,
            turn_advanced: after.turn > before.turn,
            before,
            after,
        }
    }

    /// Apply a single action against the game state.
    ///
    /// This is the spine of `terminal event -> game action -> game system ->
    /// observable state`. Interactive frontends, scripts, tests, and agents
    /// all converge here.
    pub fn apply(&mut self, action: Action) -> ActionResult {
        if self.is_over() {
            return ActionResult::IgnoredGameOver;
        }
        match action {
            Action::Quit => {
                self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Quit;
                ActionResult::Ended(Outcome::Quit)
            }
            Action::Wait => {
                self.advance_turn();
                ActionResult::Advanced
            }
            Action::PickUp => {
                if self.try_pick_up() {
                    self.advance_turn();
                    ActionResult::Advanced
                } else {
                    ActionResult::NoOp
                }
            }
            mv => {
                if let Some(direction) = mv.direction() {
                    if self.try_move(direction) {
                        self.advance_turn();
                        self.resolve_tile();
                        if self.is_over() {
                            ActionResult::Ended(self.outcome())
                        } else {
                            ActionResult::Advanced
                        }
                    } else {
                        ActionResult::NoOp
                    }
                } else {
                    ActionResult::NoOp
                }
            }
        }
    }

    fn try_move(&mut self, direction: Direction) -> bool {
        let current = self.player_position();
        let target = current.step(direction);
        match self.map().tile(target.x, target.y) {
            Tile::Wall => false,
            Tile::Floor | Tile::Goal => {
                *self.world.get_mut::<Position>(self.player).unwrap() = target;
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
            true
        } else {
            false
        }
    }

    fn resolve_tile(&mut self) {
        let pos = self.player_position();
        let on_hazard = self
            .world
            .query2::<Position, Hazard>()
            .into_iter()
            .any(|(_, hazard_pos, _)| *hazard_pos == pos);
        if on_hazard {
            self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Lost;
            return;
        }
        let on_goal = matches!(self.map().tile(pos.x, pos.y), Tile::Goal);
        let has_pkg = self.state().has_package;
        if on_goal && has_pkg {
            self.world.resource_mut::<GameState>().unwrap().outcome = Outcome::Won;
        }
    }

    fn advance_turn(&mut self) {
        self.world.resource_mut::<GameState>().unwrap().turn += 1;
    }

    /// Render the current state to a [`Grid`].
    pub fn render(&self) -> Grid {
        let map = self.map();
        let mut grid = Grid::new(map.width, map.height);
        for y in 0..map.height {
            for x in 0..map.width {
                let cell = match map.tile(x as i16, y as i16) {
                    Tile::Wall => Cell::new('#').with_fg(Color::DARK_GREY),
                    Tile::Floor => Cell::new('.').with_fg(Color::GREY),
                    Tile::Goal => Cell::new('G').with_fg(Color::GREEN),
                };
                grid.put(x, y, cell);
            }
        }
        // Layer order: hazards, packages, player on top.
        for (_, pos, _) in self.world.query2::<Position, Hazard>() {
            grid.put(
                pos.x as u16,
                pos.y as u16,
                Cell::new('h').with_fg(Color::RED),
            );
        }
        for (_, pos, _) in self.world.query2::<Position, Package>() {
            grid.put(
                pos.x as u16,
                pos.y as u16,
                Cell::new('p').with_fg(Color::YELLOW),
            );
        }
        let player_pos = self.player_position();
        let player_color = if self.state().has_package {
            Color::CYAN
        } else {
            Color::WHITE
        };
        grid.put(
            player_pos.x as u16,
            player_pos.y as u16,
            Cell::new('@').with_fg(player_color),
        );
        grid
    }

    pub fn snapshot(&self) -> Snapshot {
        let player = self.player_position();
        let packages = self
            .world
            .query2::<Position, Package>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .collect::<Vec<_>>();
        let hazards = self
            .world
            .query2::<Position, Hazard>()
            .into_iter()
            .map(|(_, pos, _)| *pos)
            .collect::<Vec<_>>();
        let map = self.map();
        let state = self.state();
        Snapshot {
            turn: state.turn,
            outcome: state.outcome,
            has_package: state.has_package,
            player,
            packages,
            hazards,
            map_width: map.width,
            map_height: map.height,
            tile_under_player: map.tile(player.x, player.y),
            walkable_neighbors: map.walkable_neighbors(player),
            frame: self.render().to_plain_string(),
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

// ----------------------------------------------------------------------------
// Tests — drive the game through the same path scripts/agents use.
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Game {
        Game::new()
    }

    #[test]
    fn default_map_spawns_player_at_top_left() {
        let g = fresh();
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.outcome(), Outcome::Playing);
        assert!(!g.state().has_package);
    }

    #[test]
    fn walls_block_movement_and_do_not_advance_turn() {
        let mut g = fresh();
        let start_turn = g.state().turn;
        let report = g.step(Action::MoveNorth); // wall directly above (1, 0) is '#'
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.state().turn, start_turn);
        assert_eq!(report.result, ActionResult::NoOp);
    }

    #[test]
    fn wait_advances_turn_without_moving() {
        let mut g = fresh();
        let pos_before = g.player_position();
        g.inject_apply(Action::Wait);
        assert_eq!(g.player_position(), pos_before);
        assert_eq!(g.state().turn, 1);
    }

    #[test]
    fn movement_advances_turn() {
        let mut g = fresh();
        g.inject_apply(Action::MoveEast);
        assert_eq!(g.player_position(), Position { x: 2, y: 1 });
        assert_eq!(g.state().turn, 1);
    }

    #[test]
    fn picking_up_package_sets_has_package_and_removes_entity() {
        // Custom map where the player starts adjacent to a package.
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        assert_eq!(g.snapshot().packages.len(), 1);
        g.router.inject_all([Action::MoveEast, Action::PickUp]);
        g.run_pending();
        assert!(g.state().has_package);
        assert!(g.snapshot().packages.is_empty());
    }

    #[test]
    fn reaching_goal_with_package_wins() {
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.router.inject_all([
            Action::MoveEast,
            Action::PickUp,
            Action::MoveEast,
            Action::MoveSouth,
        ]);
        g.run_pending();
        assert_eq!(g.outcome(), Outcome::Won);
        assert!(g.is_over());
    }

    #[test]
    fn reaching_goal_without_package_does_not_win() {
        let layout = &["#####", "#@..#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.router
            .inject_all([Action::MoveEast, Action::MoveEast, Action::MoveSouth]);
        g.run_pending();
        assert_eq!(g.outcome(), Outcome::Playing);
        assert_eq!(g.player_position(), Position { x: 3, y: 2 });
    }

    #[test]
    fn stepping_on_hazard_loses() {
        let layout = &["#####", "#@h.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        let report = g.step(Action::MoveEast);
        assert_eq!(g.outcome(), Outcome::Lost);
        assert_eq!(report.result, ActionResult::Ended(Outcome::Lost));
        assert!(g.is_over());
    }

    #[test]
    fn quit_action_ends_the_game() {
        let mut g = fresh();
        let report = g.step(Action::Quit);
        assert_eq!(g.outcome(), Outcome::Quit);
        assert_eq!(report.result, ActionResult::Ended(Outcome::Quit));
        assert!(g.is_over());
    }

    #[test]
    fn pickup_on_empty_tile_is_noop_and_does_not_advance_turn() {
        let mut g = fresh();
        g.inject_apply(Action::PickUp);
        assert_eq!(g.state().turn, 0);
        assert!(!g.state().has_package);
    }

    #[test]
    fn actions_after_game_over_are_ignored() {
        let layout = &["#####", "#@h.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.inject_apply(Action::MoveEast); // lose
        let pos = g.player_position();
        g.inject_apply(Action::MoveWest);
        assert_eq!(g.player_position(), pos);
        assert_eq!(g.outcome(), Outcome::Lost);
    }

    #[test]
    fn terminal_event_and_script_share_the_same_path() {
        // Drive one move via a Key event, the next via an injected action,
        // and assert the world cannot tell them apart.
        let mut g = fresh();
        g.handle_event(InputEvent::Key(Key::Right));
        g.run_pending();
        assert_eq!(g.player_position(), Position { x: 2, y: 1 });

        g.router.inject(Action::MoveEast);
        g.run_pending();
        assert_eq!(g.player_position(), Position { x: 3, y: 1 });
    }

    #[test]
    fn unbound_key_does_nothing() {
        let mut g = fresh();
        let pos = g.player_position();
        g.handle_event(InputEvent::Key(Key::Char('z')));
        g.run_pending();
        assert_eq!(g.player_position(), pos);
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn script_commands_parse_and_drive_the_shared_router() {
        let mut g = fresh();
        let actions = default_commands()
            .parse_words("east east wait")
            .expect("known commands");
        g.router.inject_all(actions);
        let reports = g.run_pending_reports();

        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].action, Action::MoveEast);
        assert_eq!(reports[0].source, ActionSource::Script);
        assert!(reports[0].changed);
        assert!(reports[0].turn_advanced);
        assert_eq!(g.player_position(), Position { x: 3, y: 1 });
        assert_eq!(g.state().turn, 3);
    }

    #[test]
    fn compact_glyph_scripts_use_engine_command_bindings() {
        let parsed = default_commands().parse_glyphs("e . W").unwrap();
        assert_eq!(
            parsed,
            vec![Action::MoveEast, Action::Wait, Action::MoveWest]
        );
    }

    #[test]
    fn default_map_can_be_won_from_a_script() {
        let mut g = fresh();
        let actions = default_commands()
            .parse_script("eeesss,nnneeeesssssss")
            .expect("default win script should parse");
        g.router.inject_all(actions);
        let reports = g.run_pending_reports();

        assert_eq!(g.outcome(), Outcome::Won);
        assert!(g.state().has_package);
        assert_eq!(g.player_position(), Position { x: 8, y: 8 });
        assert!(reports
            .iter()
            .any(|report| report.result == ActionResult::Ended(Outcome::Won)));
    }

    #[test]
    fn step_report_records_noop_actions() {
        let mut g = fresh();
        let report = g.step(Action::MoveNorth);
        assert_eq!(report.action, Action::MoveNorth);
        assert_eq!(report.source, ActionSource::Test);
        assert_eq!(report.result, ActionResult::NoOp);
        assert_eq!(report.before.player, report.after.player);
        assert_eq!(report.before.turn, report.after.turn);
        assert!(!report.changed);
        assert!(!report.turn_advanced);
    }

    #[test]
    fn snapshot_includes_rendered_frame_and_outcome() {
        let mut g = fresh();
        let snap = g.snapshot();
        assert_eq!(snap.outcome, Outcome::Playing);
        assert_eq!(snap.player, Position { x: 1, y: 1 });
        assert_eq!(snap.tile_under_player, Tile::Floor);
        assert_eq!(
            snap.walkable_neighbors,
            vec![Position { x: 1, y: 2 }, Position { x: 2, y: 1 }]
        );
        // Frame must contain a player glyph.
        assert!(snap.frame.contains('@'));
        // ...and have the right number of rows.
        assert_eq!(snap.frame.lines().count() as u16, snap.map_height);
        // Forward progress reflects in the snapshot.
        g.inject_apply(Action::MoveEast);
        let snap2 = g.snapshot();
        assert_eq!(snap2.player, Position { x: 2, y: 1 });
        assert_eq!(snap2.turn, 1);
    }

    #[test]
    fn map_error_on_missing_player() {
        let err = Game::from_layout(&["#####", "#...#", "#####"], default_bindings());
        assert_eq!(err.err(), Some(MapError::NoPlayer));
    }

    #[test]
    fn shorter_layout_rows_are_padded_as_walls() {
        let layout = &["#####", "#@", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.inject_apply(Action::MoveEast);
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn step_reports_preserve_terminal_and_agent_sources() {
        let mut g = fresh();
        g.handle_event(InputEvent::Key(Key::Right));
        g.router.inject_from(Action::Wait, ActionSource::Agent);

        let reports = g.run_pending_reports();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].action, Action::MoveEast);
        assert_eq!(reports[0].source, ActionSource::Terminal);
        assert_eq!(reports[1].action, Action::Wait);
        assert_eq!(reports[1].source, ActionSource::Agent);
        assert_eq!(g.state().turn, 2);
    }
}
