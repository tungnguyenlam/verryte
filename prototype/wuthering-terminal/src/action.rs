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
    SwapCharacter(usize),
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
    ToggleReplay,
    ToggleReplayAuto,
    StepReplay,
    ToggleMinimap,
    Undo,
    Redo,
    NextFloor,
    CraftItem(usize, usize),
    ChangeWeather(crate::components::WeatherType),
    ToggleHelp,
    UpgradeSkill(crate::components::SkillSlot, u8),
    ToggleSkillTree,
    RerollModifiers,
    ComboSkill(crate::components::ComboSkill),
    ViewPrestige,
    Rest,
    ToggleBestiary,
    UpgradeEquipment(crate::components::EquipmentSlot),
    PanCamera(Direction),
    ToggleCameraLock,
    ZoomIn,
    ZoomOut,
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
    b.bind(Key::Char('4'), Action::SwapCharacter(0));
    b.bind(Key::Char('5'), Action::SwapCharacter(1));
    b.bind(Key::Char('6'), Action::SwapCharacter(2));

    // Save/Load
    b.bind(Key::F(5), Action::Save);
    b.bind(Key::F(9), Action::Load);

    // Perf
    b.bind(Key::F(3), Action::TogglePerf);

    // Recording
    b.bind(Key::F(10), Action::ToggleRecording);
    b.bind(Key::F(11), Action::ToggleReplay);
    b.bind(Key::F(12), Action::StepReplay);
    b.bind(Key::Char('p'), Action::ToggleReplayAuto);

    // AI
    b.bind(Key::Char('b'), Action::AutoBattle);
    b.bind(Key::Char('B'), Action::AutoBattle);
    b.bind(Key::Char('r'), Action::StepToSafety);
    b.bind(Key::Char('R'), Action::StepToSafety);
    b.bind(Key::Char('m'), Action::ToggleMinimap);
    b.bind(Key::Char('M'), Action::ToggleMinimap);
    b.bind(Key::Char('u'), Action::Undo);
    b.bind(Key::Char('U'), Action::Undo);
    b.bind(Key::Char('y'), Action::Redo);
    b.bind(Key::Char('Y'), Action::Redo);
    b.bind(Key::Char('>'), Action::NextFloor);

    // Help
    b.bind(Key::Char('?'), Action::ToggleHelp);
    b.bind(Key::Char('h'), Action::ToggleHelp);
    b.bind(Key::Char('H'), Action::ToggleHelp);

    // Camera
    b.bind(Key::Char('c'), Action::ToggleCameraLock);
    b.bind(Key::Char('C'), Action::ToggleCameraLock);
    b.bind(
        Key::modified('↑', false, false, true),
        Action::PanCamera(Direction::North),
    );
    b.bind(
        Key::modified('↓', false, false, true),
        Action::PanCamera(Direction::South),
    );
    b.bind(
        Key::modified('←', false, false, true),
        Action::PanCamera(Direction::West),
    );
    b.bind(
        Key::modified('→', false, false, true),
        Action::PanCamera(Direction::East),
    );
    b.bind(Key::Char('='), Action::ZoomIn);
    b.bind(Key::Char('+'), Action::ZoomIn);
    b.bind(Key::Char('-'), Action::ZoomOut);
    b.bind(Key::Char('_'), Action::ZoomOut);

    // Skill Tree
    b.bind(Key::Char('t'), Action::ToggleSkillTree);
    b.bind(Key::Char('T'), Action::ToggleSkillTree);

    // Reroll Modifiers
    b.bind(Key::Char('x'), Action::RerollModifiers);
    b.bind(Key::Char('X'), Action::RerollModifiers);

    // Prestige
    b.bind(Key::Char('v'), Action::ViewPrestige);
    b.bind(Key::Char('V'), Action::ViewPrestige);

    // Rest
    b.bind(Key::Char('z'), Action::Rest);
    b.bind(Key::Char('Z'), Action::Rest);

    // Bestiary
    b.bind(Key::Char('j'), Action::ToggleBestiary);
    b.bind(Key::Char('J'), Action::ToggleBestiary);

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
    c.bind_name("swap1", Action::SwapCharacter(0));
    c.bind_name("swap2", Action::SwapCharacter(1));
    c.bind_name("swap3", Action::SwapCharacter(2));
    c.bind_name("skill1", Action::Skill1);
    c.bind_name("skill2", Action::Skill2);
    c.bind_name("skill3", Action::Skill3);
    c.bind_name("quit", Action::Quit);
    c.bind_name("end", Action::EndTurn);
    c.bind_name("save", Action::Save);
    c.bind_name("load", Action::Load);
    c.bind_name("record", Action::ToggleRecording);
    c.bind_name("replay", Action::ToggleReplay);
    c.bind_name("replay_auto", Action::ToggleReplayAuto);
    c.bind_name("step_replay", Action::StepReplay);
    c.bind_name("autobattle", Action::AutoBattle);
    c.bind_name("safety", Action::StepToSafety);
    c.bind_name("undo", Action::Undo);
    c.bind_name("redo", Action::Redo);
    c.bind_name("stairs", Action::NextFloor);

    c.bind_name("next_floor", Action::NextFloor);
    c.bind_name("help", Action::ToggleHelp);
    c.bind_name("skill_tree", Action::ToggleSkillTree);
    c.bind_name("upgrades", Action::ToggleSkillTree);
    c.bind_name("prestige", Action::ViewPrestige);
    c.bind_name("reroll", Action::RerollModifiers);
    c.bind_name("rest", Action::Rest);
    c.bind_name("bestiary", Action::ToggleBestiary);
    c.bind_name("lore", Action::ToggleBestiary);

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
    c.bind_glyph('m', Action::ToggleMinimap);
    c.bind_glyph('u', Action::Undo);
    c.bind_glyph('y', Action::Redo);
    c.bind_glyph('?', Action::ToggleHelp);
    c.bind_glyph('h', Action::ToggleHelp);
    c.bind_glyph('t', Action::ToggleSkillTree);
    c.bind_glyph('v', Action::ViewPrestige);
    c.bind_glyph('j', Action::ToggleBestiary);

    c
}

pub fn weather_display_name(w: crate::components::WeatherType) -> &'static str {
    match w {
        crate::components::WeatherType::Sunny => "Sunny",
        crate::components::WeatherType::Rainy => "Rainy",
        crate::components::WeatherType::LightningStorm => "Lightning Storm",
        crate::components::WeatherType::Snowing => "Snowing",
    }
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

    if token == "inventory" || token == "items" {
        return Some(Action::ToggleInventory);
    }

    if token == "minimap" || token == "map" {
        return Some(Action::ToggleMinimap);
    }

    if token == "help" {
        return Some(Action::ToggleHelp);
    }

    if token == "undo" {
        return Some(Action::Undo);
    }

    if token == "redo" {
        return Some(Action::Redo);
    }

    if token == "save" || token == "quicksave" {
        return Some(Action::Save);
    }

    if token == "load" || token == "quickload" {
        return Some(Action::Load);
    }

    if token == "record" || token == "recording" {
        return Some(Action::ToggleRecording);
    }

    if token == "replay" {
        return Some(Action::ToggleReplay);
    }

    if token == "replay_auto" || token == "auto_replay" {
        return Some(Action::ToggleReplayAuto);
    }

    if token == "step_replay" || token == "replay_step" {
        return Some(Action::StepReplay);
    }

    if token == "stairs" || token == "next_floor" {
        return Some(Action::NextFloor);
    }

    if token == "skill_tree" || token == "upgrades" {
        return Some(Action::ToggleSkillTree);
    }

    if token == "reroll" {
        return Some(Action::RerollModifiers);
    }

    if token == "prestige" || token == "view_prestige" {
        return Some(Action::ViewPrestige);
    }

    if token == "rest" {
        return Some(Action::Rest);
    }

    if token == "bestiary" || token == "lore" {
        return Some(Action::ToggleBestiary);
    }

    if let Some(upgrade_str) = token.strip_prefix("upgrade:") {
        if let Some((slot_str, tier_str)) = upgrade_str.split_once('_') {
            if let Ok(tier) = tier_str.parse::<u8>() {
                let slot = match slot_str {
                    "s1" => Some(crate::components::SkillSlot::Skill1),
                    "s2" => Some(crate::components::SkillSlot::Skill2),
                    "s3" => Some(crate::components::SkillSlot::Skill3),
                    "passive" => Some(crate::components::SkillSlot::Passive),
                    _ => None,
                };
                if let Some(slot) = slot {
                    return Some(Action::UpgradeSkill(slot, tier));
                }
            }
        }
    }

    if let Some(craft_str) = token.strip_prefix("craft:") {
        if let Some((idx1_str, idx2_str)) = craft_str.split_once(',') {
            if let (Ok(idx1), Ok(idx2)) = (idx1_str.parse::<usize>(), idx2_str.parse::<usize>()) {
                return Some(Action::CraftItem(
                    idx1.saturating_sub(1),
                    idx2.saturating_sub(1),
                ));
            }
        }
    }

    if let Some(idx_str) = token.strip_prefix("use:") {
        if let Ok(idx) = idx_str.parse::<usize>() {
            return Some(Action::UseItem(idx.saturating_sub(1)));
        }
    }

    if let Some(combo_str) = token.strip_prefix("combo:") {
        let combo = match combo_str.to_lowercase().as_str() {
            "bladestorm" => Some(crate::components::ComboSkill::BladeStorm),
            "holysmite" => Some(crate::components::ComboSkill::HolySmite),
            "arcanesanctuary" => Some(crate::components::ComboSkill::ArcaneSanctuary),
            "trinitystrike" => Some(crate::components::ComboSkill::TrinityStrike),
            _ => None,
        };
        if let Some(skill) = combo {
            return Some(Action::ComboSkill(skill));
        }
    }

    if let Some(upgrade_slot_str) = token.strip_prefix("equip_upgrade:") {
        let slot = match upgrade_slot_str.to_lowercase().as_str() {
            "weapon" => Some(crate::components::EquipmentSlot::Weapon),
            "armor" => Some(crate::components::EquipmentSlot::Armor),
            "accessory" => Some(crate::components::EquipmentSlot::Accessory),
            _ => None,
        };
        if let Some(slot) = slot {
            return Some(Action::UpgradeEquipment(slot));
        }
    }

    if token == "camera_lock" || token == "lock" {
        return Some(Action::ToggleCameraLock);
    }

    if token == "zoom_in" || token == "zoomin" || token == "+" {
        return Some(Action::ZoomIn);
    }

    if token == "zoom_out" || token == "zoomout" || token == "-" {
        return Some(Action::ZoomOut);
    }

    if let Some(pan_str) = token.strip_prefix("pan:") {
        let dir = match pan_str.to_lowercase().as_str() {
            "n" | "north" | "up" => Some(Direction::North),
            "s" | "south" | "down" => Some(Direction::South),
            "e" | "east" | "right" => Some(Direction::East),
            "w" | "west" | "left" => Some(Direction::West),
            _ => None,
        };
        if let Some(d) = dir {
            return Some(Action::PanCamera(d));
        }
    }

    if let Some(weather_str) = token.strip_prefix("weather:") {
        let w = match weather_str.to_lowercase().as_str() {
            "sunny" => Some(crate::components::WeatherType::Sunny),
            "rainy" => Some(crate::components::WeatherType::Rainy),
            "lightning" | "lightningstorm" | "storm" => {
                Some(crate::components::WeatherType::LightningStorm)
            }
            "snowing" | "snow" => Some(crate::components::WeatherType::Snowing),
            _ => None,
        };
        if let Some(wt) = w {
            return Some(Action::ChangeWeather(wt));
        }
    }
    None
}

fn parse_point(raw: &str) -> Option<Point> {
    let (x, y) = raw.split_once(',')?;
    let x = x.trim().parse::<i16>().ok()?;
    let y = y.trim().parse::<i16>().ok()?;
    Some(Point::new(x, y))
}
