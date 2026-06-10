//! State snapshots for Wuthering Terminal.

use crate::action::Action;
use crate::components::{
    BattleStats, CharacterClass, DamagePreview, EchoItem, ElementalShield, ElementalStatus,
    GameEvent, GameState, Inventory, Item, Outcome, Position, Rooted, Stats, Stunned, Team,
    TelegraphZone, TurnPhase, WeatherType,
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
    #[serde(default)]
    pub battle_stats: BattleStats,
    #[serde(default = "default_floor_one")]
    pub floor: u32,
    #[serde(default)]
    pub weather: WeatherType,
    #[serde(default)]
    pub turn_order: Vec<String>,
    #[serde(default)]
    pub enemy_intents: Vec<String>,
    #[serde(default)]
    pub damage_preview: Option<DamagePreview>,
    #[serde(default)]
    pub aoe_preview: Vec<Position>,
    #[serde(default)]
    pub available_combos: Vec<String>,
    #[serde(default)]
    pub bestiary_discovered: u32,
    #[serde(default)]
    pub bestiary_total: u32,
    #[serde(default)]
    pub lore_discovered: u32,
    #[serde(default)]
    pub lore_total: u32,
    #[serde(default)]
    pub active_modifiers: Vec<String>,
    #[serde(default)]
    pub active_set_bonuses: Vec<String>,
}

fn default_floor_one() -> u32 {
    1
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
    /// A critical hit landed.
    CritHit { damage: i32 },
    /// Damage was blocked by a shield.
    Blocked { damage_reduced: i32 },
    /// A heal was applied.
    Healed { amount: i32, target: String },
    /// The action moved an entity between tiles.
    Moved { entity: String, to: Position },
    /// The action used a consumable item.
    ItemUsed { name: String },
    /// The action triggered a boss phase transition.
    BossPhaseChanged { phase: String },
    /// An echo ability was absorbed from a defeated enemy.
    Absorbed { echo_name: String },
    /// An item was crafted via alchemy.
    Crafted { item_name: String },
    /// The player moved between dungeon floors.
    FloorTransition { from: u32, to: u32 },
    /// An elemental status was applied to a target.
    StatusApplied { status: String, target: String },
    /// A combo chain was extended.
    ComboExtended { combo_count: u32 },
    /// The action triggered a state-only change (selection, cursor, inventory).
    StateUpdated,
    /// The action failed (e.g. out of AP, out of range, invalid target).
    Failed { reason: String },
    /// The action ended the game.
    GameOver { outcome: Outcome },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FailureCategory {
    OutOfAP,
    OutOfRange,
    TileBlocked,
    NoSelection,
    Other,
}

impl ActionOutcome {
    pub fn is_failed(&self) -> bool {
        matches!(self, ActionOutcome::Failed { .. })
    }

    pub fn failure_category(&self) -> Option<FailureCategory> {
        match self {
            ActionOutcome::Failed { reason } => {
                if reason.starts_with("Not enough AP") {
                    Some(FailureCategory::OutOfAP)
                } else if reason.starts_with("Target is out of") {
                    Some(FailureCategory::OutOfRange)
                } else if reason.starts_with("Cannot move") {
                    Some(FailureCategory::TileBlocked)
                } else if reason.starts_with("Select a character") {
                    Some(FailureCategory::NoSelection)
                } else {
                    Some(FailureCategory::Other)
                }
            }
            _ => None,
        }
    }
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
    reg.register_component::<crate::components::PrestigeProgress>("PrestigeProgress");
    reg.register_component::<crate::components::Morale>("Morale");
    reg.register_component::<crate::components::Fatigue>("Fatigue");

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
    reg.register_resource::<crate::components::BattleStats>("BattleStats");
    reg.register_resource::<crate::components::Weather>("Weather");
    reg.register_resource::<crate::components::Bestiary>("Bestiary");
    reg.register_resource::<crate::components::LoreJournal>("LoreJournal");
    reg.register_resource::<crate::components::ActiveFloorModifiers>("ActiveFloorModifiers");

    reg
}

pub const CURRENT_SAVE_VERSION: u32 = 2;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FullSaveState {
    pub magic: String,
    pub version: u32,
    pub timestamp: String,
    pub world: WorldSnapshot,
    #[serde(default)]
    pub migrations_applied: Vec<String>,
}

/// Per-character summary for diagnostics.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CharacterDiag {
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub ap: i32,
    pub max_ap: i32,
    pub status: String,
    pub alive: bool,
    #[serde(default)]
    pub prestige: String,
    #[serde(default)]
    pub morale: i32,
    #[serde(default)]
    pub morale_state: String,
    #[serde(default)]
    pub fatigue: i32,
}

/// A snapshot of diagnostic information about the game state, useful for
/// agents, CI verification, and replay debugging.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GameDiagnostics {
    pub alive_entities: usize,
    pub dead_entities: usize,
    pub current_phase: TurnPhase,
    pub weather: String,
    pub floor: u32,
    pub turn: u32,
    pub characters: Vec<CharacterDiag>,
    pub combo_count: u32,
    pub concert_energy: u32,
}
