mod ai;
mod boss;
mod combat;
mod environment;
mod morale;
mod movement;
mod progression;

pub use ai::*;
pub use boss::*;
pub use combat::*;
pub use environment::*;
pub use morale::*;
pub use movement::*;
pub use progression::*;

use crate::components::{
    CharacterClass, EquippedItems, GameEvent, GameState, Rooted, Stats, Team, TurnPhase,
};
use crate::game::Game;

use verryte_core::{Events, World};

pub fn turn_management_system(world: &mut World) {
    let request_end = {
        let trans = world
            .resource_mut::<crate::components::TurnTransition>()
            .expect("TurnTransition must be registered");
        let req = trans.request_end;
        trans.request_end = false;
        req
    };

    if !request_end {
        return;
    }

    let current_phase = world
        .resource::<GameState>()
        .expect("GameState resource must be registered")
        .phase;
    match current_phase {
        TurnPhase::Player => {
            // Player -> Enemy
            {
                let state = world
                    .resource_mut::<GameState>()
                    .expect("GameState must be registered");
                state.phase = TurnPhase::Enemy;
                state.selected_entity = None;
                state.combo_count = 0;
            }
            if let Some(stack) = world.resource_mut::<crate::components::UndoStack>() {
                stack.states.clear();
            }
            if let Some(stack) = world.resource_mut::<crate::components::RedoStack>() {
                stack.states.clear();
            }
            log(world, "[fg:FFA500][b]Enemy Phase starts![/][/fg]");
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::PhaseChanged(TurnPhase::Enemy));
                events.send(GameEvent::TurnEnded);
            }

            apply_weather_hazard_damage(world, Team::Player);
            process_team_status_effects(world, Team::Enemy);
            apply_equipment_hp_regen(world, Team::Enemy);

            // Replenish Enemy AP
            let mut enemies = Vec::new();
            for (e, team) in world.query::<Team>() {
                if *team == Team::Enemy {
                    enemies.push(e);
                }
            }
            for e in enemies {
                let mut cannot_act = false;
                let mut status_msg = "";

                let mut root_remains = false;
                if let Some(rooted) = world.get_mut::<Rooted>(e) {
                    if rooted.duration > 0 {
                        rooted.duration -= 1;
                        cannot_act = true;
                        status_msg = "rooted";
                        if rooted.duration > 0 {
                            root_remains = true;
                        }
                    }
                }
                if cannot_act && !root_remains {
                    world.remove::<Rooted>(e);
                }

                let mut stun_remains = false;
                if let Some(stunned) = world.get_mut::<crate::components::Stunned>(e) {
                    if stunned.duration > 0 {
                        stunned.duration -= 1;
                        cannot_act = true;
                        status_msg = "stunned";
                        if stunned.duration > 0 {
                            stun_remains = true;
                        }
                    }
                }
                if cannot_act && !stun_remains && status_msg == "stunned" {
                    world.remove::<crate::components::Stunned>(e);
                }

                if cannot_act {
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        stats.ap = 0;
                    }
                    let class = *world
                        .get::<CharacterClass>(e)
                        .expect("enemy must have CharacterClass");
                    log(
                        world,
                        format!(
                            "{} is {} and cannot act this turn!",
                            Game::get_class_name(class),
                            status_msg
                        ),
                    );
                } else {
                    let is_berserker = world
                        .get::<crate::components::AIArchetype>(e)
                        .is_some_and(|a| *a == crate::components::AIArchetype::Berserker)
                        || world.get::<CharacterClass>(e).is_some_and(|c| {
                            matches!(
                                *c,
                                CharacterClass::Berserker | CharacterClass::EliteBerserker
                            )
                        });
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        let is_frenzied = is_berserker && stats.hp < stats.max_hp / 2;
                        let bonus = if is_frenzied { 1 } else { 0 };
                        stats.ap = stats.max_ap + bonus;
                    }
                }
            }
        }
        TurnPhase::Enemy => {
            // Enemy -> Player
            {
                let state = world
                    .resource_mut::<GameState>()
                    .expect("GameState must be registered");
                state.phase = TurnPhase::Player;
                state.turn += 1;
                state.combo_count = 0;
            }
            if let Some(bstats) = world.resource_mut::<crate::components::BattleStats>() {
                bstats.total_turns += 1;
            }
            let turn_num = world
                .resource::<GameState>()
                .expect("GameState must be registered")
                .turn;
            log(
                world,
                format!(
                    "[fg:32CD32][b]Player Phase starts! Turn {}[/][/fg]",
                    turn_num
                ),
            );

            process_team_status_effects(world, Team::Player);

            apply_weather_hazard_damage(world, Team::Enemy);
            apply_equipment_hp_regen(world, Team::Player);

            // Replenish Player AP
            let mut players = Vec::new();
            for (e, team) in world.query::<Team>() {
                if *team == Team::Player {
                    players.push(e);
                }
            }
            let has_swift = world
                .resource::<crate::components::EquippedEchoes>()
                .is_some_and(|echoes| {
                    echoes
                        .abilities
                        .contains(&crate::components::EchoAbility::Swift)
                });

            for e in players {
                let mut cannot_act = false;
                let mut status_msg = "";

                let mut root_remains = false;
                if let Some(rooted) = world.get_mut::<Rooted>(e) {
                    if rooted.duration > 0 {
                        rooted.duration -= 1;
                        cannot_act = true;
                        status_msg = "rooted";
                        if rooted.duration > 0 {
                            root_remains = true;
                        }
                    }
                }
                if cannot_act && !root_remains {
                    world.remove::<Rooted>(e);
                }

                let mut stun_remains = false;
                if let Some(stunned) = world.get_mut::<crate::components::Stunned>(e) {
                    if stunned.duration > 0 {
                        stunned.duration -= 1;
                        cannot_act = true;
                        status_msg = "stunned";
                        if stunned.duration > 0 {
                            stun_remains = true;
                        }
                    }
                }
                if cannot_act && !stun_remains && status_msg == "stunned" {
                    world.remove::<crate::components::Stunned>(e);
                }

                if cannot_act {
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        stats.ap = 0;
                    }
                    let class = *world
                        .get::<CharacterClass>(e)
                        .expect("player must have CharacterClass");
                    log(
                        world,
                        format!(
                            "{} is {} and cannot act this turn!",
                            Game::get_class_name(class),
                            status_msg
                        ),
                    );
                } else {
                    let has_swift_foot = world
                        .get::<crate::components::CharacterTrait>(e)
                        .is_some_and(|t| t.trait_type == crate::components::HeroTrait::SwiftFoot);
                    let is_berserker = world
                        .get::<crate::components::AIArchetype>(e)
                        .is_some_and(|a| *a == crate::components::AIArchetype::Berserker)
                        || world.get::<CharacterClass>(e).is_some_and(|c| {
                            matches!(
                                *c,
                                CharacterClass::Berserker | CharacterClass::EliteBerserker
                            )
                        });
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        let mut bonus = 0;
                        if has_swift {
                            bonus += 1;
                        }
                        if has_swift_foot {
                            bonus += 1;
                        }
                        let is_frenzied = is_berserker && stats.hp < stats.max_hp / 2;
                        if is_frenzied {
                            bonus += 1;
                        }
                        stats.ap = stats.max_ap + bonus;
                    }
                }
            }

            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ApReplenished);
                events.send(GameEvent::PhaseChanged(TurnPhase::Player));
            }
        }
    }
}

fn apply_equipment_hp_regen(world: &mut World, team: Team) {
    let mut heals = Vec::new();
    for (entity, entity_team, equipped) in world.query2::<Team, EquippedItems>() {
        if *entity_team == team {
            let amount = equipped.total_hp_regen();
            if amount > 0 {
                heals.push((entity, amount));
            }
        }
    }

    for (entity, amount) in heals {
        let (healed, _defeated) = apply_heal(world, entity, amount);
        if healed != 0 {
            let class = world
                .get::<CharacterClass>(entity)
                .copied()
                .unwrap_or(CharacterClass::Warrior);
            log(
                world,
                format!(
                    "{} regenerated {} HP from equipment.",
                    Game::get_class_name(class),
                    healed
                ),
            );
        }
    }
}
