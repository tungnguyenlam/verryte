use verryte_input::{Bindings, CommandBindings, Key, MouseButton};
use verryte_map::Direction;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    MoveNorth,
    MoveSouth,
    MoveEast,
    MoveWest,
    StepToPackage,
    StepToGoal,
    StepToSafety,
    Wait,
    Scan,
    ScanRadius(u16),
    PickUp,
    Drop,
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
    b.bind(Key::Char('x'), Action::Scan);
    b.bind(Key::Char('1'), Action::ScanRadius(1));
    b.bind(Key::Char('2'), Action::ScanRadius(2));
    b.bind(Key::Char('3'), Action::ScanRadius(3));
    b.bind(Key::Char('4'), Action::ScanRadius(4));
    b.bind(Key::Char('5'), Action::ScanRadius(5));
    b.bind(Key::Char('p'), Action::StepToPackage);
    b.bind(Key::Char('P'), Action::StepToPackage);
    b.bind(Key::Char('o'), Action::StepToGoal);
    b.bind(Key::Char('O'), Action::StepToGoal);
    b.bind(Key::Char('r'), Action::StepToSafety);
    b.bind(Key::Char('R'), Action::StepToSafety);
    b.bind(Key::Char('g'), Action::PickUp);
    b.bind(Key::Char(','), Action::PickUp);
    b.bind(Key::Char('D'), Action::Drop);
    b.bind(Key::Char('q'), Action::Quit);
    b.bind(Key::Esc, Action::Quit);
    b.bind_mouse(MouseButton::Right, true, Action::Scan);
    b.bind_mouse(MouseButton::Middle, true, Action::Wait);
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
    c.bind_name("scan", Action::Scan);
    c.bind_name("step_package", Action::StepToPackage);
    c.bind_name("step_goal", Action::StepToGoal);
    c.bind_name("step_safety", Action::StepToSafety);
    c.bind_name("pickup", Action::PickUp);
    c.bind_name("drop", Action::Drop);
    c.bind_name("quit", Action::Quit);

    c.bind_glyph('n', Action::MoveNorth);
    c.bind_glyph('N', Action::MoveNorth);
    c.bind_glyph('s', Action::MoveSouth);
    c.bind_glyph('S', Action::MoveSouth);
    c.bind_glyph('e', Action::MoveEast);
    c.bind_glyph('E', Action::MoveEast);
    c.bind_glyph('w', Action::MoveWest);
    c.bind_glyph('W', Action::MoveWest);
    c.bind_glyph('.', Action::Wait);
    c.bind_glyph('x', Action::Scan);
    c.bind_glyph('p', Action::StepToPackage);
    c.bind_glyph('o', Action::StepToGoal);
    c.bind_glyph('v', Action::StepToSafety);
    c.bind_glyph('r', Action::StepToSafety);
    c.bind_glyph(',', Action::PickUp);
    c.bind_glyph('!', Action::Drop);
    c.bind_glyph('D', Action::Drop);
    c.bind_glyph('q', Action::Quit);
    c
}

/// Resolve parameterized script tokens that are not fixed command names.
///
/// Supported forms:
/// * `scan:<radius>` (for example `scan:3`)
/// * `scan<radius>` (for example `scan5`)
/// * `x<radius>` (for example `x2`)
pub fn resolve_command_token(token: &str) -> Option<Action> {
    let radius = token
        .strip_prefix("scan:")
        .or_else(|| token.strip_prefix("scan"))
        .or_else(|| token.strip_prefix('x'))
        .and_then(|digits| (!digits.is_empty()).then_some(digits))
        .and_then(|digits| digits.parse::<u16>().ok())
        .filter(|radius| *radius > 0)?;
    Some(Action::ScanRadius(radius))
}
