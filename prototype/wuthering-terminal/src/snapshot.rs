//! State snapshots for Wuthering Terminal.

use crate::action::Action;
use crate::components::{
    BattleStats, CharacterClass, CharacterElement, DamagePreview, EchoItem, ElementalShield,
    ElementalStatus, GameEvent, GameState, Inventory, Item, Outcome, Position, Rooted, Stats,
    Stunned, Team, TelegraphZone, TurnPhase, WeatherType,
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
    /// Remaining turns for each entry in `active_modifiers`, by index.
    #[serde(default)]
    pub active_modifier_durations: Vec<u32>,
    /// Tiles currently marked as dangerous by the weather system.
    #[serde(default)]
    pub weather_danger_zones: Vec<Position>,
    /// Turn on which the next deeper-floor event is scheduled.
    #[serde(default)]
    pub next_floor_event_turn: u32,
    /// Bounded, oldest-to-newest descriptions of triggered floor events.
    #[serde(default)]
    pub recent_floor_events: Vec<String>,
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
    /// An entity was defeated.
    Defeated { entity: String },
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
    /// An equipped item was upgraded.
    EquipmentUpgraded {
        item_name: String,
        slot: crate::components::EquipmentSlot,
        level: u8,
    },
    /// A character skill was upgraded.
    SkillUpgraded {
        hero: String,
        skill_name: String,
        slot: crate::components::SkillSlot,
        tier: u8,
    },
    /// Defeating an enemy awarded and equipped a set item.
    EquipmentRewarded { item_name: String, hero: String },
    /// A UI/tooling toggle changed a boolean or panel state.
    ToggleChanged { name: String, enabled: bool },
    /// A read-only status panel was viewed.
    StatusViewed { name: String },
    /// A character rested to recover fatigue and morale.
    Rested {
        entity: String,
        fatigue_recovered: i32,
        morale_gained: i32,
    },
    /// Active floor modifiers were rerolled.
    ModifiersRerolled { modifiers: Vec<String> },
    /// A scheduled dynamic floor event fired.
    FloorEventTriggered { description: String },
    /// The current game state was saved.
    GameSaved { path: String },
    /// A saved game state was loaded.
    GameLoaded { path: String },
    /// Action recording was started or stopped.
    RecordingChanged { enabled: bool, records: usize },
    /// Replay mode was started or stopped.
    ReplayChanged {
        enabled: bool,
        actions: usize,
        errors: usize,
    },
    /// A replay action was applied through the normal action path.
    ReplayStepped {
        index: usize,
        action: String,
        verified: bool,
    },
    /// Replay auto-step mode was toggled.
    ReplayAutoChanged { enabled: bool },
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
    InvalidItem,
    InvalidRecipe,
    WrongContext,
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
                } else if reason.starts_with("Invalid item") || reason.starts_with("Upgrade Kit") {
                    Some(FailureCategory::InvalidItem)
                } else if reason.starts_with("Invalid crafting")
                    || reason.starts_with("No valid recipe")
                {
                    Some(FailureCategory::InvalidRecipe)
                } else if reason.starts_with("Inventory must be open")
                    || reason.starts_with("You must stand on")
                    || reason.starts_with("No save file")
                    || reason.starts_with("Replay mode")
                    || reason.starts_with("Enable Replay")
                {
                    Some(FailureCategory::WrongContext)
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
    reg.register_component::<CharacterElement>("CharacterElement");
    reg.register_component::<ElementalShield>("ElementalShield");
    reg.register_component::<Rooted>("Rooted");
    reg.register_component::<Stunned>("Stunned");
    reg.register_component::<Inventory>("Inventory");
    reg.register_component::<Item>("Item");
    reg.register_component::<EchoItem>("EchoItem");
    reg.register_component::<crate::components::PrestigeProgress>("PrestigeProgress");
    reg.register_component::<crate::components::Morale>("Morale");
    reg.register_component::<crate::components::Fatigue>("Fatigue");
    reg.register_component::<crate::components::EquippedItems>("EquippedItems");
    reg.register_component::<crate::components::CharacterTrait>("CharacterTrait");
    reg.register_component::<crate::components::Threat>("Threat");
    reg.register_component::<crate::components::AIArchetype>("AIArchetype");
    reg.register_component::<crate::components::Destructible>("Destructible");
    reg.register_component::<crate::components::SkillTree>("SkillTree");
    reg.register_component::<crate::components::EliteEnemy>("EliteEnemy");

    // Resources
    reg.register_core_resources();
    verryte_terminal::register_terminal_resources(&mut reg);

    reg.register_resource::<GameState>("GameState");
    reg.register_resource::<TelegraphZone>("TelegraphZone");
    reg.register_resource::<crate::map::TacticalMap>("TacticalMap");
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
    reg.register_resource::<crate::components::DynamicFloorEvents>("DynamicFloorEvents");
    reg.register_resource::<crate::components::ActiveHazards>("ActiveHazards");

    reg
}

pub const CURRENT_SAVE_VERSION: u32 = 3;

pub fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= byte as u64;
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

pub fn hash_json_value(value: &serde_json::Value, hash: &mut u64) {
    match value {
        serde_json::Value::Null => update_hash(hash, b"null"),
        serde_json::Value::Bool(b) => update_hash(hash, if *b { b"true" } else { b"false" }),
        serde_json::Value::Number(n) => update_hash(hash, n.to_string().as_bytes()),
        serde_json::Value::String(s) => update_hash(hash, s.as_bytes()),
        serde_json::Value::Array(arr) => {
            update_hash(hash, b"[");
            for item in arr {
                hash_json_value(item, hash);
            }
            update_hash(hash, b"]");
        }
        serde_json::Value::Object(obj) => {
            update_hash(hash, b"{");
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for k in keys {
                update_hash(hash, k.as_bytes());
                hash_json_value(&obj[k], hash);
            }
            update_hash(hash, b"}");
        }
    }
}

pub fn calculate_save_checksum(state: &FullSaveState) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;

    update_hash(&mut hash, state.magic.as_bytes());
    update_hash(&mut hash, &state.version.to_ne_bytes());
    update_hash(&mut hash, state.timestamp.as_bytes());
    for mig in &state.migrations_applied {
        update_hash(&mut hash, mig.as_bytes());
    }

    // Hash the world snapshot resources in sorted order
    let mut res_keys: Vec<&String> = state.world.resources.keys().collect();
    res_keys.sort();
    for k in res_keys {
        update_hash(&mut hash, k.as_bytes());
        hash_json_value(&state.world.resources[k], &mut hash);
    }

    // Hash the entities in sorted order
    let mut entities = state.world.entities.clone();
    entities.sort_by_key(|e| e.entity);
    for ent in &entities {
        update_hash(&mut hash, &ent.entity.index().to_ne_bytes());
        update_hash(&mut hash, &ent.entity.generation().to_ne_bytes());
        let mut comp_keys: Vec<&String> = ent.components.keys().collect();
        comp_keys.sort();
        for k in comp_keys {
            update_hash(&mut hash, k.as_bytes());
            hash_json_value(&ent.components[k], &mut hash);
        }
    }

    format!("{:016x}", hash)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FullSaveState {
    pub magic: String,
    pub version: u32,
    pub timestamp: String,
    pub world: WorldSnapshot,
    #[serde(default)]
    pub migrations_applied: Vec<String>,
    #[serde(default)]
    pub checksum: String,
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
