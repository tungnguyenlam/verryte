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
    /// Tiles the currently selected character can reach with movement.
    #[serde(default)]
    pub reachable_tiles: Vec<Position>,
    /// Tiles the currently selected character can attack (within attack range).
    #[serde(default)]
    pub targetable_tiles: Vec<Position>,
    /// True iff there is a character selected who still has AP to act.
    #[serde(default)]
    pub selected_can_act: bool,
    #[serde(default)]
    pub combo_count: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StepReport {
    pub action: Action,
    pub source: ActionSource,
    pub before: Snapshot,
    pub after: Snapshot,
    pub events: Vec<GameEvent>,
    pub diagnostics: std::collections::HashMap<String, f64>, // ms
    pub outcome: ActionOutcome,
}

/// Summary of what an action actually did, for agent and script observability.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionOutcome {
    /// No-op or non-advancing action (cursor move, state update).
    #[default]
    NoOp,
    /// The player turn advanced (turn counter incremented).
    TurnAdvanced,
    /// The phase changed (Player -> Enemy or Enemy -> Player).
    PhaseChanged,
    /// A combat hit was applied; carries damage dealt and target name.
    Hit {
        damage: i32,
        target: String,
        was_critical: bool,
        was_blocked: bool,
    },
    /// A heal was applied.
    Healed { amount: i32, target: String },
    /// The action moved an entity between tiles.
    Moved { entity: String, to: Position },
    /// The action used a consumable item.
    ItemUsed { name: String },
    /// The action triggered a boss phase transition.
    BossPhaseChanged { phase: String },
    /// The action triggered a state-only change (selection, cursor, inventory).
    StateUpdated,
    /// The action failed (e.g. out of AP, out of range, invalid target).
    Failed { reason: String },
    /// The action ended the game.
    GameOver { outcome: Outcome },
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
