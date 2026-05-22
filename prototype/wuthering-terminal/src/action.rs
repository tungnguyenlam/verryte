use verryte_input::{Bindings, Key};
use verryte_map::{Direction, Point};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Action {
    MoveNorth,
    MoveSouth,
    MoveEast,
    MoveWest,
    Wait,
    Inspect(Point),
    ClearCursor,
    Confirm,
    Cancel,
    NextCharacter,
    PrevCharacter,
    Skill1,
    Skill2,
    Skill3,
    Quit,
    EndTurn,
}

impl Action {
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

pub fn default_bindings() -> Bindings<Action> {
    let mut b = Bindings::new();

    // Arrows
    b.bind(Key::Up, Action::MoveNorth);
    b.bind(Key::Down, Action::MoveSouth);
    b.bind(Key::Left, Action::MoveWest);
    b.bind(Key::Right, Action::MoveEast);

    // WASD
    b.bind(Key::Char('w'), Action::MoveNorth);
    b.bind(Key::Char('s'), Action::MoveSouth);
    b.bind(Key::Char('a'), Action::MoveWest);
    b.bind(Key::Char('d'), Action::MoveEast);
    b.bind(Key::Char('W'), Action::MoveNorth);
    b.bind(Key::Char('S'), Action::MoveSouth);
    b.bind(Key::Char('A'), Action::MoveWest);
    b.bind(Key::Char('D'), Action::MoveEast);

    // Other
    b.bind(Key::Char(' '), Action::Wait);
    b.bind(Key::Enter, Action::Confirm);
    b.bind(Key::Esc, Action::Cancel);
    b.bind(Key::Tab, Action::NextCharacter);
    b.bind(Key::Char('q'), Action::Quit);
    b.bind(Key::Char('Q'), Action::Quit);
    b.bind(Key::Char('e'), Action::EndTurn);
    b.bind(Key::Char('E'), Action::EndTurn);

    // Skills
    b.bind(Key::Char('1'), Action::Skill1);
    b.bind(Key::Char('2'), Action::Skill2);
    b.bind(Key::Char('3'), Action::Skill3);

    b
}
