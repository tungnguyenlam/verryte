use crate::components::{Fatigue, Morale, Stats, Team};

use verryte_core::{Entity, World};

pub fn morale_fatigue_system(world: &mut World) {
    let mut morale_drops_hp25: Vec<Entity> = Vec::new();
    for (e, stats, _team) in world.query2::<Stats, Team>() {
        if stats.max_hp > 0 && stats.hp > 0 {
            let hp_pct = stats.hp as f32 / stats.max_hp as f32;
            if hp_pct < 0.25 {
                morale_drops_hp25.push(e);
            }
        }
    }
    for e in morale_drops_hp25 {
        if let Some(morale) = world.get_mut::<Morale>(e) {
            morale.value = (morale.value - 10).max(0);
        }
    }

    let mut high_fatigue_entities: Vec<Entity> = Vec::new();
    for (e, fatigue, _team) in world.query2::<Fatigue, Team>() {
        if fatigue.value > 50 {
            high_fatigue_entities.push(e);
        }
    }
    for e in high_fatigue_entities {
        if let Some(morale) = world.get_mut::<Morale>(e) {
            morale.value = (morale.value - 2).max(0);
        }
    }

    let mut very_high_fatigue: Vec<Entity> = Vec::new();
    for (e, fatigue, _team) in world.query2::<Fatigue, Team>() {
        if fatigue.value > 75 {
            very_high_fatigue.push(e);
        }
    }
    for e in very_high_fatigue {
        if let Some(morale) = world.get_mut::<Morale>(e) {
            morale.value = (morale.value - 3).max(0);
        }
    }

    let morale_entities: Vec<Entity> = world.query::<Morale>().iter().map(|(e, _)| *e).collect();
    for e in morale_entities {
        if let Some(morale) = world.get_mut::<Morale>(e) {
            morale.value = morale.value.clamp(0, morale.max);
        }
    }
}

pub fn apply_ally_defeated_morale(world: &mut World, _defeated: Entity) {
    let living_allies: Vec<Entity> = world
        .query2::<Team, Stats>()
        .iter()
        .filter(|(_, team, stats)| **team == Team::Player && stats.hp > 0)
        .map(|(e, _, _)| *e)
        .collect();
    for ally in living_allies {
        if let Some(morale) = world.get_mut::<Morale>(ally) {
            morale.value = (morale.value - 15).max(0);
        }
    }
}

pub fn apply_enemy_defeated_morale(world: &mut World) {
    let living_players: Vec<Entity> = world
        .query2::<Team, Stats>()
        .iter()
        .filter(|(_, team, stats)| **team == Team::Player && stats.hp > 0)
        .map(|(e, _, _)| *e)
        .collect();
    for player in living_players {
        if let Some(morale) = world.get_mut::<Morale>(player) {
            morale.value = (morale.value + 10).min(morale.max);
        }
    }
}

pub fn apply_boss_phase_morale(world: &mut World) {
    let living_players: Vec<Entity> = world
        .query2::<Team, Stats>()
        .iter()
        .filter(|(_, team, stats)| **team == Team::Player && stats.hp > 0)
        .map(|(e, _, _)| *e)
        .collect();
    for player in living_players {
        if let Some(morale) = world.get_mut::<Morale>(player) {
            morale.value = (morale.value - 20).max(0);
        }
    }
}

pub fn apply_heal_morale(world: &mut World, target: Entity) {
    if let Some(morale) = world.get_mut::<Morale>(target) {
        morale.value = (morale.value + 5).min(morale.max);
    }
}

pub fn increment_fatigue_on_action(
    world: &mut World,
    entity: Entity,
    is_skill: bool,
    is_wait: bool,
) {
    if let Some(fatigue) = world.get_mut::<Fatigue>(entity) {
        let increase = if is_wait {
            1
        } else if is_skill {
            5
        } else {
            3
        };
        fatigue.value = (fatigue.value + increase).min(fatigue.max);
        fatigue.actions_taken += 1;
    }
}
