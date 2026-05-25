use verryte_input::{Bindings, CommandBindings, Key};
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
    ToggleInventory,
    UseItem(usize),
    Save,
    Load,
    TogglePerf,
    AutoBattle,
    StepToSafety,
    ToggleRecording,
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
    b.bind(Key::Char('i'), Action::ToggleInventory);
    b.bind(Key::Char('I'), Action::ToggleInventory);

    // Skills
    b.bind(Key::Char('1'), Action::Skill1);
    b.bind(Key::Char('2'), Action::Skill2);
    b.bind(Key::Char('3'), Action::Skill3);

    // Save/Load
    b.bind(Key::F(5), Action::Save);
    b.bind(Key::F(9), Action::Load);

    // Perf
    b.bind(Key::F(3), Action::TogglePerf);

    // Recording
    b.bind(Key::F(10), Action::ToggleRecording);

    // AI
    b.bind(Key::Char('b'), Action::AutoBattle);
    b.bind(Key::Char('B'), Action::AutoBattle);
    b.bind(Key::Char('r'), Action::StepToSafety);
    b.bind(Key::Char('R'), Action::StepToSafety);

    b
}

pub fn default_commands() -> CommandBindings<Action> {
    let mut c = CommandBindings::new();
    c.bind_name("north", Action::MoveNorth);
    c.bind_name("south", Action::MoveSouth);
    c.bind_name("east", Action::MoveEast);
    c.bind_name("west", Action::MoveWest);
    c.bind_name("wait", Action::Wait);
    c.bind_name("confirm", Action::Confirm);
    c.bind_name("cancel", Action::Cancel);
    c.bind_name("next", Action::NextCharacter);
    c.bind_name("prev", Action::PrevCharacter);
    c.bind_name("skill1", Action::Skill1);
    c.bind_name("skill2", Action::Skill2);
    c.bind_name("skill3", Action::Skill3);
    c.bind_name("quit", Action::Quit);
    c.bind_name("end", Action::EndTurn);
    c.bind_name("autobattle", Action::AutoBattle);
    c.bind_name("safety", Action::StepToSafety);

    c.bind_glyph('n', Action::MoveNorth);
    c.bind_glyph('s', Action::MoveSouth);
    c.bind_glyph('e', Action::MoveEast);
    c.bind_glyph('w', Action::MoveWest);
    c.bind_glyph('.', Action::Wait);
    c.bind_glyph('c', Action::Confirm);
    c.bind_glyph('x', Action::Cancel);
    c.bind_glyph('>', Action::NextCharacter);
    c.bind_glyph('<', Action::PrevCharacter);
    c.bind_glyph('1', Action::Skill1);
    c.bind_glyph('2', Action::Skill2);
    c.bind_glyph('3', Action::Skill3);
    c.bind_glyph('q', Action::Quit);
    c.bind_glyph('e', Action::EndTurn);
    c.bind_glyph('i', Action::ToggleInventory);
    c.bind_glyph(',', Action::ClearCursor);
    c.bind_glyph('b', Action::AutoBattle);
    c.bind_glyph('r', Action::StepToSafety);

    c
}

pub fn resolve_command_token(token: &str) -> Option<Action> {
    let inspect = token
        .strip_prefix("inspect:")
        .or_else(|| token.strip_prefix("look:"))
        .or_else(|| token.strip_prefix("cursor:"))
        .and_then(parse_point);
    if let Some(point) = inspect {
        return Some(Action::Inspect(point));
    }
    None
}

fn parse_point(raw: &str) -> Option<Point> {
    let (x, y) = raw.split_once(',')?;
    let x = x.trim().parse::<i16>().ok()?;
    let y = y.trim().parse::<i16>().ok()?;
    Some(Point::new(x, y))
}
