use crate::action::Action;
use crate::components::{GameEvent, Outcome, Position};
use crate::map::Tile;
use verryte_input::ActionSource;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub turn: u32,
    pub outcome: Outcome,
    pub has_package: bool,
    pub scans: u32,
    pub player: Position,
    pub packages: Vec<Position>,
    pub hazards: Vec<Position>,
    pub chasers: Vec<Position>,
    pub visible_tiles: Vec<Position>,
    pub visible_hazards: Vec<Position>,
    pub reachable_tiles: Vec<Position>,
    pub map_width: u16,
    pub map_height: u16,
    pub tile_under_player: Tile,
    pub walkable_neighbors: Vec<Position>,
    pub path_to_nearest_package: Option<Vec<Position>>,
    pub path_to_goal: Option<Vec<Position>>,
    pub path_to_nearest_hazard: Option<Vec<Position>>,
    pub path_to_nearest_chaser: Option<Vec<Position>>,
    pub distance_to_nearest_package: Option<u16>,
    pub distance_to_goal: Option<u16>,
    pub distance_to_nearest_hazard: Option<u16>,
    pub distance_to_nearest_chaser: Option<u16>,
    pub safer_neighbors: Vec<Position>,
    /// Count of live entities in the world.
    pub entity_count: usize,
    /// Plain-text rendering of the current frame.
    pub frame: String,
    /// Plain-text camera-sized view centered near the player.
    pub local_frame: String,
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
    pub events: Vec<GameEvent>,
}

/// The immediate game-level result of applying one action.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ActionResult {
    NoOp,
    Advanced,
    Ended(Outcome),
    IgnoredGameOver,
}
