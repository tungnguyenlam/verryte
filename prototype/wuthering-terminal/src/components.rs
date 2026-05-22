use verryte_map::Point;

pub type Position = Point;

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Team {
    Player,
    Enemy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CharacterClass {
    Warrior, // Kael
    Mage,    // Lyra
    Healer,  // Mira
    Boss,    // Blight Sovereign
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub spd: i32,
    pub ap: i32, // Action Points
    pub max_ap: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    Playing,
    Victory,
    Defeat,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub turn: u32,
    pub phase: TurnPhase,
    pub outcome: Outcome,
    pub cursor: Position,
    pub selected_entity: Option<verryte_core::Entity>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TurnPhase {
    Player,
    Enemy,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameEvent {
    Moved {
        entity: verryte_core::Entity,
        from: Position,
        to: Position,
    },
    Attacked {
        attacker: verryte_core::Entity,
        target: verryte_core::Entity,
        damage: i32,
    },
    Healed {
        healer: verryte_core::Entity,
        target: verryte_core::Entity,
        amount: i32,
    },
    Defeated {
        entity: verryte_core::Entity,
    },
    PhaseChanged(TurnPhase),
    TurnEnded,
    ApReplenished,
}
