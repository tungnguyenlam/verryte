//! State snapshots for Wuthering Terminal.

use crate::action::Action;
use crate::components::{
    CharacterClass, EchoItem, ElementalStatus, GameEvent, GameState, Outcome, Position, Rooted,
    Stats, Team, TelegraphZone, TurnPhase,
};
use verryte_input::ActionSource;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub turn: u32,
    pub phase: TurnPhase,
    pub outcome: Outcome,
    pub cursor: Position,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StepReport {
    pub action: Action,
    pub source: ActionSource,
    pub before: Snapshot,
    pub after: Snapshot,
    pub events: Vec<GameEvent>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SavedEntity {
    pub entity: verryte_core::Entity,
    pub position: Option<Position>,
    pub team: Option<Team>,
    pub class: Option<CharacterClass>,
    pub stats: Option<Stats>,
    pub echo_item: Option<EchoItem>,
    pub elemental_status: Option<ElementalStatus>,
    pub rooted: Option<Rooted>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FullSaveState {
    pub game_state: GameState,
    pub telegraph_zone: TelegraphZone,
    pub message_log: verryte_core::MessageLog,
    pub clock: verryte_core::GameClock,
    pub rng: verryte_core::Rng,
    pub map: crate::map::TacticalMap,
    pub camera: verryte_terminal::Camera,
    pub action_history: verryte_input::ActionHistory<Action>,
    pub entities: Vec<SavedEntity>,
}
