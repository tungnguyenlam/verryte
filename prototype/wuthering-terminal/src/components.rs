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
    Help,
    Bestiary,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BestiaryEntry {
    pub class: CharacterClass,
    pub name: String,
    pub encountered: bool,
    pub defeated_count: u32,
    pub times_killed_by: u32,
    pub hits_taken: u32,
    pub known_weakness: Option<String>,
    pub known_resistance: Option<String>,
    pub lore_text: String,
    pub drop_table: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Bestiary {
    pub entries: Vec<BestiaryEntry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LoreEntry {
    pub id: String,
    pub title: String,
    pub text: String,
    pub category: LoreCategory,
    pub discovered: bool,
    pub turn_discovered: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LoreCategory {
    World,
    Character,
    Enemy,
    Item,
    Mechanic,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct LoreJournal {
    pub entries: Vec<LoreEntry>,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HazardType {
    SpikeTrap,
    PoisonCloud,
    HealingSpring,
    CrackedFloor,
    PressurePlate,
    ThornBush,
    FireTile,
    IceTile,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HazardEffect {
    pub hazard_type: HazardType,
    pub damage: i32,
    pub healing: i32,
    pub status: Option<ElementalStatus>,
    pub duration: u32,
    pub trigger_count: i32,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ActiveHazards {
    pub hazards: Vec<(Position, HazardEffect)>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Destructible {
    pub hp: i32,
    pub max_hp: i32,
    pub destroyed: bool,
    pub replacement_tile: crate::map::Tile,
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AIBehavior {
    pub archetype: AIArchetype,
    pub aggression: i32,
    pub caution: i32,
    pub coordination: i32,
    pub last_action: Option<AIAction>,
    pub target_priority: Option<verryte_core::Entity>,
}

impl AIBehavior {
    pub fn new(archetype: AIArchetype) -> Self {
        let (aggression, caution, coordination) = match archetype {
            AIArchetype::Chaser => (80, 20, 40),
            AIArchetype::Cleric => (40, 60, 80),
            AIArchetype::Coward => (20, 90, 30),
        };
        Self {
            archetype,
            aggression,
            caution,
            coordination,
            last_action: None,
            target_priority: None,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AIAction {
    Attack(verryte_core::Entity),
    MoveTo(Position),
    Retreat,
    Defend,
    HealAlly(verryte_core::Entity),
    UseCover,
    FlankAttack(verryte_core::Entity, Position),
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TacticalAssessment {
    pub threat_map: Vec<(Position, i32)>,
    pub cover_positions: Vec<Position>,
    pub flank_positions: Vec<Position>,
    pub safe_positions: Vec<Position>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Accessory,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Equipment {
    pub name: String,
    pub slot: EquipmentSlot,
    pub atk_bonus: i32,
    pub def_bonus: i32,
    pub hp_bonus: i32,
    pub spd_bonus: i32,
    pub special: Option<EquipmentSpecial>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EquipmentSpecial {
    LifestealPercent(u32),
    CritBoost(u32),
    ElementalDamage(Element, i32),
    HpRegen(i32),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Element {
    Ice,
    Lightning,
    Nature,
    Physical,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EquippedItems {
    pub weapon: Option<Equipment>,
    pub armor: Option<Equipment>,
    pub accessory: Option<Equipment>,
}

impl EquippedItems {
    pub fn total_atk_bonus(&self) -> i32 {
        self.weapon.as_ref().map_or(0, |e| e.atk_bonus)
            + self.armor.as_ref().map_or(0, |e| e.atk_bonus)
            + self.accessory.as_ref().map_or(0, |e| e.atk_bonus)
    }

    pub fn total_def_bonus(&self) -> i32 {
        self.weapon.as_ref().map_or(0, |e| e.def_bonus)
            + self.armor.as_ref().map_or(0, |e| e.def_bonus)
            + self.accessory.as_ref().map_or(0, |e| e.def_bonus)
    }

    pub fn total_hp_bonus(&self) -> i32 {
        self.weapon.as_ref().map_or(0, |e| e.hp_bonus)
            + self.armor.as_ref().map_or(0, |e| e.hp_bonus)
            + self.accessory.as_ref().map_or(0, |e| e.hp_bonus)
    }

    pub fn total_spd_bonus(&self) -> i32 {
        self.weapon.as_ref().map_or(0, |e| e.spd_bonus)
            + self.armor.as_ref().map_or(0, |e| e.spd_bonus)
            + self.accessory.as_ref().map_or(0, |e| e.spd_bonus)
    }

    pub fn equip(&mut self, item: Equipment) -> Option<Equipment> {
        match item.slot {
            EquipmentSlot::Weapon => self.weapon.replace(item),
            EquipmentSlot::Armor => self.armor.replace(item),
            EquipmentSlot::Accessory => self.accessory.replace(item),
        }
    }

    pub fn unequip(&mut self, slot: EquipmentSlot) -> Option<Equipment> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.take(),
            EquipmentSlot::Armor => self.armor.take(),
            EquipmentSlot::Accessory => self.accessory.take(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DamagePreview {
    pub min_damage: i32,
    pub max_damage: i32,
    pub expected_damage: i32,
    pub hit_chance: u32,
    pub crit_chance: u32,
    pub element: Option<String>,
    pub can_kill: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AoEPreview {
    pub center: Option<Position>,
    pub affected_tiles: Vec<Position>,
    pub affected_enemies: Vec<verryte_core::Entity>,
    pub affected_allies: Vec<verryte_core::Entity>,
    pub total_potential_damage: i32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TurnOrderEntry {
    pub entity: verryte_core::Entity,
    pub name: String,
    pub team: Team,
    pub spd: i32,
    pub is_current: bool,
    pub hp_ratio: f32,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TurnOrderDisplay {
    pub entries: Vec<TurnOrderEntry>,
    pub current_index: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EnemyIntent {
    pub entity: verryte_core::Entity,
    pub intent_type: IntentType,
    pub target: Option<Position>,
    pub predicted_damage: i32,
    pub description: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IntentType {
    Attack,
    Defend,
    Heal,
    Move,
    AoEAttack,
    Buff,
    Unknown,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EnemyIntentions {
    pub intents: Vec<EnemyIntent>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SkillSlot {
    Skill1,
    Skill2,
    Skill3,
    Passive,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum UpgradeEffect {
    DamageBoost(i32),
    CooldownReduction(u32),
    RangeBoost(i32),
    AoEBonus(i32),
    StatusChance(u32),
    HealBoost(i32),
    Lifesteal(u32),
    ExtraHit(u32),
    PierceResist(i32),
    Passive {
        atk: i32,
        def: i32,
        hp: i32,
        spd: i32,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SkillUpgrade {
    pub skill_slot: SkillSlot,
    pub upgrade_id: String,
    pub name: String,
    pub description: String,
    pub tier: u32,
    pub cost: u32,
    pub unlocked: bool,
    pub effect: UpgradeEffect,
    pub prerequisites: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SkillTree {
    pub skill_points: u32,
    pub upgrades: Vec<SkillUpgrade>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FloorModifier {
    Darkness,
    GravityWell,
    ElementalStorm,
    HealingSurge,
    Frenzy,
    FogOfWar,
    Reversal,
}

impl FloorModifier {
    pub fn all() -> &'static [FloorModifier] {
        &[
            FloorModifier::Darkness,
            FloorModifier::GravityWell,
            FloorModifier::ElementalStorm,
            FloorModifier::HealingSurge,
            FloorModifier::Frenzy,
            FloorModifier::FogOfWar,
            FloorModifier::Reversal,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FloorModifier::Darkness => "Darkness",
            FloorModifier::GravityWell => "Gravity Well",
            FloorModifier::ElementalStorm => "Elemental Storm",
            FloorModifier::HealingSurge => "Healing Surge",
            FloorModifier::Frenzy => "Frenzy",
            FloorModifier::FogOfWar => "Fog of War",
            FloorModifier::Reversal => "Reversal",
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ActiveFloorModifiers {
    pub modifiers: Vec<FloorModifier>,
    pub turns_remaining: Vec<u32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ComboSkill {
    BladeStorm,
    HolySmite,
    ArcaneSanctuary,
    TrinityStrike,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ComboSkillDef {
    pub skill: ComboSkill,
    pub name: String,
    pub description: String,
    pub ap_cost: i32,
    pub base_damage: i32,
    pub base_healing: i32,
    pub range: i32,
}

impl ComboSkillDef {
    pub fn for_skill(skill: &ComboSkill) -> Self {
        match skill {
            ComboSkill::BladeStorm => Self {
                skill: ComboSkill::BladeStorm,
                name: "Blade Storm".to_string(),
                description: "Warrior+Mage AoE: lightning+slash around both".to_string(),
                ap_cost: 3,
                base_damage: 0,
                base_healing: 0,
                range: 2,
            },
            ComboSkill::HolySmite => Self {
                skill: ComboSkill::HolySmite,
                name: "Holy Smite".to_string(),
                description: "Warrior+Healer: massive single target + self-heal".to_string(),
                ap_cost: 2,
                base_damage: 0,
                base_healing: 0,
                range: 2,
            },
            ComboSkill::ArcaneSanctuary => Self {
                skill: ComboSkill::ArcaneSanctuary,
                name: "Arcane Sanctuary".to_string(),
                description: "Mage+Healer: shield all allies + heal over time".to_string(),
                ap_cost: 3,
                base_damage: 0,
                base_healing: 15,
                range: 0,
            },
            ComboSkill::TrinityStrike => Self {
                skill: ComboSkill::TrinityStrike,
                name: "Trinity Strike".to_string(),
                description: "All 3 adjacent: devastating nuke + stun".to_string(),
                ap_cost: 4,
                base_damage: 0,
                base_healing: 0,
                range: 3,
            },
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AvailableCombos {
    pub combos: Vec<(ComboSkill, Vec<verryte_core::Entity>)>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Morale {
    pub value: i32,
    pub max: i32,
}

impl Default for Morale {
    fn default() -> Self {
        Self {
            value: 70,
            max: 100,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Fatigue {
    pub value: i32,
    pub max: i32,
    pub actions_taken: u32,
}

impl Default for Fatigue {
    fn default() -> Self {
        Self {
            value: 0,
            max: 100,
            actions_taken: 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MoraleState {
    Confident,
    Steady,
    Stressed,
    Breaking,
    Broken,
}

impl MoraleState {
    pub fn from_morale(morale: i32) -> Self {
        if morale >= 80 {
            MoraleState::Confident
        } else if morale >= 50 {
            MoraleState::Steady
        } else if morale >= 25 {
            MoraleState::Stressed
        } else if morale >= 1 {
            MoraleState::Breaking
        } else {
            MoraleState::Broken
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            MoraleState::Confident => "Confident",
            MoraleState::Steady => "Steady",
            MoraleState::Stressed => "Stressed",
            MoraleState::Breaking => "Breaking",
            MoraleState::Broken => "Broken",
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrestigeClass {
    #[default]
    None,
    BladeMaster,
    Archmage,
    DivineHealer,
}

impl PrestigeClass {
    pub fn display_name(&self) -> &'static str {
        match self {
            PrestigeClass::None => "None",
            PrestigeClass::BladeMaster => "BladeMaster",
            PrestigeClass::Archmage => "Archmage",
            PrestigeClass::DivineHealer => "DivineHealer",
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PrestigeProgress {
    pub class: PrestigeClass,
    pub kill_count: u32,
    pub total_damage_dealt: i32,
    pub total_healing_done: i32,
    pub promoted: bool,
}
