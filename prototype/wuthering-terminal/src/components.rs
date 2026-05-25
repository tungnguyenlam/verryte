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
    ShadowStalker,
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
    pub level: u32,
    pub xp: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    Playing,
    Victory,
    Defeat,
    Quit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TargetingMode {
    None,
    Skill1,
    Skill2,
    Skill3,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BossPhase {
    Phase1,
    Phase2,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UIState {
    Normal,
    Inventory,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub turn: u32,
    pub phase: TurnPhase,
    pub outcome: Outcome,
    pub cursor: Position,
    pub selected_entity: Option<verryte_core::Entity>,
    pub concert_energy: u32,
    pub targeting: TargetingMode,
    pub boss_phase: BossPhase,
    pub ui_state: UIState,
    pub show_perf: bool,
    pub auto_battle: bool,
    pub is_recording: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TurnPhase {
    Player,
    Enemy,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TelegraphZone {
    pub tiles: Vec<Position>,
    pub damage: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ElementalStatus {
    None,
    Ice { duration: u32 },
    Lightning { duration: u32 },
    Nature { duration: u32 },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rooted {
    pub duration: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EchoItem {
    pub class: CharacterClass,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EchoAbility {
    Swift,     // +1 Max AP
    Thorns,    // Reflect damage
    Frostbite, // 20% chance to apply Ice
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EquippedEchoes {
    pub abilities: Vec<EchoAbility>,
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
    ElementalApplied {
        entity: verryte_core::Entity,
        status: ElementalStatus,
    },
    ReactionTriggered {
        entity: verryte_core::Entity,
        reaction: String,
        damage: i32,
        healing: i32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemEffect {
    Heal(i32),
    ReplenishAp(i32),
    Cleanse,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub name: String,
    pub effect: ItemEffect,
    pub consumed: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub items: Vec<verryte_core::Entity>,
}
