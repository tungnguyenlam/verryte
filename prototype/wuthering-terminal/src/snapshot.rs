//! State snapshots for Wuthering Terminal.

use crate::action::Action;
use crate::components::{
    CharacterClass, EchoItem, ElementalShield, ElementalStatus, GameEvent, GameState, Inventory,
    Item, Outcome, Position, Rooted, Stats, Stunned, Team, TelegraphZone, TurnPhase,
};
use verryte_core::snapshot::{WorldRegistry, WorldSnapshot};
use verryte_input::ActionSource;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TeamSummary {
    pub count: usize,
    pub total_hp: i32,
    pub max_hp: i32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub turn: u32,
    pub phase: TurnPhase,
    pub outcome: Outcome,
    pub cursor: Position,
    pub player_team: TeamSummary,
    pub enemy_team: TeamSummary,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StepReport {
    pub action: Action,
    pub source: ActionSource,
    pub before: Snapshot,
    pub after: Snapshot,
    pub events: Vec<GameEvent>,
    pub diagnostics: std::collections::HashMap<String, f64>, // ms
}

pub fn create_registry() -> WorldRegistry {
    let mut reg = WorldRegistry::new();

    // Components
    reg.register_component::<CharacterClass>("CharacterClass");
    reg.register_component::<Team>("Team");
    reg.register_component::<Stats>("Stats");
    reg.register_component::<Position>("Position");
    reg.register_component::<ElementalStatus>("ElementalStatus");
    reg.register_component::<ElementalShield>("ElementalShield");
    reg.register_component::<Rooted>("Rooted");
    reg.register_component::<Stunned>("Stunned");
    reg.register_component::<Inventory>("Inventory");
    reg.register_component::<Item>("Item");
    reg.register_component::<EchoItem>("EchoItem");

    // Resources
    reg.register_resource::<GameState>("GameState");
    reg.register_resource::<TelegraphZone>("TelegraphZone");
    reg.register_resource::<verryte_core::MessageLog>("MessageLog");
    reg.register_resource::<verryte_core::GameClock>("GameClock");
    reg.register_resource::<verryte_core::Rng>("Rng");
    reg.register_resource::<crate::map::TacticalMap>("TacticalMap");
    reg.register_resource::<verryte_terminal::Camera>("Camera");
    reg.register_resource::<verryte_input::ActionHistory<Action>>("ActionHistory");
    // reg.register_resource::<verryte_terminal::vfx::VfxSystem>("VfxSystem"); // Skip if not serializable
    reg.register_resource::<crate::components::EquippedEchoes>("EquippedEchoes");
    reg.register_resource::<crate::components::TurnTransition>("TurnTransition");
    reg.register_resource::<verryte_map::VisibilityMap>("VisibilityMap");
    reg.register_resource::<verryte_core::Events<GameEvent>>("GameEvents");

    reg
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FullSaveState {
    pub world: WorldSnapshot,
}
