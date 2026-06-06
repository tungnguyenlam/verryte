use crate::action::Action;
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
    CorruptedSpore,
    CursedSentinel,
    PlagueWraith,
    GlacialGolem,
    EnemyCleric,
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BossConfig {
    pub phase2_hp_threshold: i32,
    pub phase2_max_hp: i32,
    pub phase2_atk_bonus: i32,
    pub phase2_def_bonus: i32,
    pub phase2_spd_bonus: i32,
    pub phase2_max_ap: i32,
    pub telegraph_damage_phase1: i32,
    pub telegraph_damage_phase2: i32,
    pub telegraph_rate_phase1: u32,
    pub telegraph_rate_phase2: u32,
    /// Shield amount applied to boss on entering phase 2.
    pub phase2_shield_amount: i32,
    /// Shield type applied to boss on entering phase 2.
    pub phase2_shield_type: ShieldType,
}

impl Default for BossConfig {
    fn default() -> Self {
        Self {
            phase2_hp_threshold: 250,
            phase2_max_hp: 500,
            phase2_atk_bonus: 10,
            phase2_def_bonus: 5,
            phase2_spd_bonus: 2,
            phase2_max_ap: 7,
            telegraph_damage_phase1: 50,
            telegraph_damage_phase2: 80,
            telegraph_rate_phase1: 40,
            telegraph_rate_phase2: 60,
            phase2_shield_amount: 100,
            phase2_shield_type: ShieldType::Physical,
        }
    }
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
    #[serde(default = "default_true")]
    pub show_minimap: bool,
    #[serde(default)]
    pub combo_count: u32,
    #[serde(default = "default_floor")]
    pub floor: u32,
}

fn default_floor() -> u32 {
    1
}

fn default_true() -> bool {
    true
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
    Poison { duration: u32 },
    Regen { duration: u32 },
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
    Stun,      // 15% chance to stun target
    Lifesteal, // Heal for 15% of damage dealt
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stunned {
    pub duration: u32,
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
    RestoreShield(ShieldType, i32),
    Combined(i32, i32),  // heal, ap
    CleanseAndHeal(i32), // cleanse, heal amount
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub name: String,
    pub effect: ItemEffect,
    pub consumed: bool,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TurnTransition {
    pub request_end: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ReplayState {
    pub active: bool,
    pub auto: bool,
    pub trace: verryte_input::ActionTrace<Action>,
    pub next_index: usize,
    #[serde(default)]
    pub verification_errors: Vec<String>,
    #[serde(default)]
    pub expected_outcomes: Vec<crate::snapshot::ActionOutcome>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub items: Vec<verryte_core::Entity>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShieldType {
    Ice,
    Lightning,
    Nature,
    Physical,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ElementalShield {
    pub shield_type: ShieldType,
    pub amount: i32,
    pub max_amount: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BattleStats {
    pub total_damage_dealt: i32,
    pub total_damage_taken: i32,
    pub total_healing_done: i32,
    pub total_turns: u32,
    pub total_kills: u32,
    pub max_combo_reached: u32,
    pub total_swaps: u32,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct UndoStack {
    pub states: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RedoStack {
    pub states: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HeroTrait {
    SwiftFoot,      // Kael: starts turn with 3 AP instead of 2
    StormChaser,    // Lyra: Lightning reactions deal +10 damage
    PurifyingTouch, // Mira: 50% chance to cleanse negative status effects when healing a character
    IceWalker,      // Glacial Golem / custom: prevents sliding on Ice terrain
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CharacterTrait {
    pub trait_type: HeroTrait,
}

#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum WeatherType {
    #[default]
    Sunny,
    Rainy,
    Snowing,
    LightningStorm,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Weather {
    pub current: WeatherType,
    pub danger_zones: Vec<Position>,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            current: WeatherType::Sunny,
            danger_zones: Vec::new(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Threat {
    pub value: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AIArchetype {
    Chaser,
    Cleric,
    Coward,
}
