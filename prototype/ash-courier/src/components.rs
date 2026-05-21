use verryte_map::Point;

pub type Position = Point;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Player;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Package;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hazard;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Chaser;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Battery {
    pub current: u32,
    pub max: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BatteryPack;

/// Tracks the chaser's position from the previous tick to avoid backtracking.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PreviousPosition(pub Position);

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
    pub scans: u32,
    pub cursor: Option<Position>,
    pub camera_zoom: i16,
    pub show_log: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            turn: 0,
            outcome: Outcome::Playing,
            has_package: false,
            scans: 0,
            cursor: None,
            camera_zoom: 0,
            show_log: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    Moved {
        from: Position,
        to: Position,
    },
    Blocked {
        from: Position,
        to: Position,
    },
    Waited {
        at: Position,
    },
    PickedUp {
        at: Position,
    },
    Dropped {
        at: Position,
    },
    Scanned {
        at: Position,
        visible_tiles: usize,
        visible_hazards: usize,
    },
    Inspected {
        at: Position,
        tile: crate::map::Tile,
    },
    CursorCleared {
        at: Position,
    },
    ChaserMoved {
        from: Position,
        to: Position,
    },
    PickedUpBattery {
        at: Position,
        amount: u32,
    },
    OutcomeChanged(Outcome),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChaserBehavior {
    Patrol {
        waypoints: Vec<Position>,
        current: usize,
    },
    ScentTracker {
        max_scent_age: usize,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScentTrail {
    pub positions: Vec<Position>,
}
