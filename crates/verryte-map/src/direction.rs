/// Cardinal movement on a 2D grid.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::South,
        Direction::East,
        Direction::West,
    ];

    pub fn delta(self) -> (i16, i16) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }

    /// Rotate this direction 90 degrees clockwise.
    ///
    /// North -> East -> South -> West -> North
    pub fn rotate_cw(self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
        }
    }

    /// Rotate this direction 90 degrees counter-clockwise.
    ///
    /// North -> West -> South -> East -> North
    pub fn rotate_ccw(self) -> Self {
        match self {
            Direction::North => Direction::West,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::East => Direction::North,
        }
    }

    /// Convert a unit offset `(dx, dy)` into a `Direction`.
    ///
    /// Returns `None` for non-unit or diagonal offsets.
    pub fn from_offset(dx: i16, dy: i16) -> Option<Self> {
        match (dx, dy) {
            (0, -1) => Some(Direction::North),
            (0, 1) => Some(Direction::South),
            (1, 0) => Some(Direction::East),
            (-1, 0) => Some(Direction::West),
            _ => None,
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::North => write!(f, "North"),
            Direction::South => write!(f, "South"),
            Direction::East => write!(f, "East"),
            Direction::West => write!(f, "West"),
        }
    }
}

impl From<Direction> for Direction8 {
    fn from(d: Direction) -> Self {
        Direction8::from_direction(d)
    }
}

/// Eight-directional movement on a 2D grid (cardinal + diagonal).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Direction8 {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Direction8 {
    pub const ALL: [Direction8; 8] = [
        Direction8::North,
        Direction8::NorthEast,
        Direction8::East,
        Direction8::SouthEast,
        Direction8::South,
        Direction8::SouthWest,
        Direction8::West,
        Direction8::NorthWest,
    ];

    /// Returns only the four cardinal directions.
    pub const CARDINAL: [Direction8; 4] = [
        Direction8::North,
        Direction8::East,
        Direction8::South,
        Direction8::West,
    ];

    /// Returns only the four diagonal directions.
    pub const DIAGONAL: [Direction8; 4] = [
        Direction8::NorthEast,
        Direction8::SouthEast,
        Direction8::SouthWest,
        Direction8::NorthWest,
    ];

    pub fn delta(self) -> (i16, i16) {
        match self {
            Direction8::North => (0, -1),
            Direction8::NorthEast => (1, -1),
            Direction8::East => (1, 0),
            Direction8::SouthEast => (1, 1),
            Direction8::South => (0, 1),
            Direction8::SouthWest => (-1, 1),
            Direction8::West => (-1, 0),
            Direction8::NorthWest => (-1, -1),
        }
    }

    pub fn opposite(self) -> Direction8 {
        match self {
            Direction8::North => Direction8::South,
            Direction8::NorthEast => Direction8::SouthWest,
            Direction8::East => Direction8::West,
            Direction8::SouthEast => Direction8::NorthWest,
            Direction8::South => Direction8::North,
            Direction8::SouthWest => Direction8::NorthEast,
            Direction8::West => Direction8::East,
            Direction8::NorthWest => Direction8::SouthEast,
        }
    }

    /// Returns `true` if this is a cardinal (non-diagonal) direction.
    pub fn is_cardinal(self) -> bool {
        matches!(
            self,
            Direction8::North | Direction8::East | Direction8::South | Direction8::West
        )
    }

    /// Convert to a cardinal `Direction` if applicable.
    pub fn to_direction(self) -> Option<Direction> {
        match self {
            Direction8::North => Some(Direction::North),
            Direction8::East => Some(Direction::East),
            Direction8::South => Some(Direction::South),
            Direction8::West => Some(Direction::West),
            _ => None,
        }
    }

    /// Build a `Direction8` from a cardinal `Direction`.
    pub fn from_direction(d: Direction) -> Self {
        match d {
            Direction::North => Direction8::North,
            Direction::East => Direction8::East,
            Direction::South => Direction8::South,
            Direction::West => Direction8::West,
        }
    }

    /// Convert an offset `(dx, dy)` into a `Direction8`.
    ///
    /// Each component should be -1, 0, or 1. Returns `None` for zero offsets
    /// or offsets with components outside [-1, 1].
    pub fn from_offset(dx: i16, dy: i16) -> Option<Self> {
        match (dx, dy) {
            (0, -1) => Some(Direction8::North),
            (1, -1) => Some(Direction8::NorthEast),
            (1, 0) => Some(Direction8::East),
            (1, 1) => Some(Direction8::SouthEast),
            (0, 1) => Some(Direction8::South),
            (-1, 1) => Some(Direction8::SouthWest),
            (-1, 0) => Some(Direction8::West),
            (-1, -1) => Some(Direction8::NorthWest),
            _ => None,
        }
    }
}

impl std::fmt::Display for Direction8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction8::North => write!(f, "N"),
            Direction8::NorthEast => write!(f, "NE"),
            Direction8::East => write!(f, "E"),
            Direction8::SouthEast => write!(f, "SE"),
            Direction8::South => write!(f, "S"),
            Direction8::SouthWest => write!(f, "SW"),
            Direction8::West => write!(f, "W"),
            Direction8::NorthWest => write!(f, "NW"),
        }
    }
}

impl TryFrom<Direction8> for Direction {
    type Error = ();
    fn try_from(d: Direction8) -> Result<Self, ()> {
        d.to_direction().ok_or(())
    }
}
