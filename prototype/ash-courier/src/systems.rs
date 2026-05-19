use crate::components::{Chaser, GameEvent, GameState, Hazard, Outcome, Player, Position};
use crate::map::{Map, Tile};
use verryte_core::{Entity, Events, MessageLog, Rng, World};

pub fn chaser_system(world: &mut World) {
    let state = world.resource::<GameState>().unwrap();
    if state.outcome != Outcome::Playing {
        return;
    }

    let player_pos = {
        let rows = world.query2::<Position, Player>();
        if let Some((_, p, _)) = rows.first() {
            **p
        } else {
            return;
        }
    };

    let mut chasers: Vec<Entity> = world
        .query2::<Position, Chaser>()
        .into_iter()
        .map(|(e, _, _)| e)
        .collect();

    // Shuffle chaser order each tick so movement priority is not biased by
    // entity allocation order. Uses the seeded RNG for reproducibility.
    if let Some(rng) = world.resource_mut::<Rng>() {
        rng.shuffle(&mut chasers);
    }

    let mut moves = Vec::new();
    {
        let map = world.resource::<Map>().unwrap();
        for &entity in &chasers {
            let pos = *world.get::<Position>(entity).unwrap();
            let start = verryte_map::Point::new(pos.x, pos.y);
            let goal = verryte_map::Point::new(player_pos.x, player_pos.y);

            // Only chase if within 6 steps
            if let Some(path) = map.shortest_walkable_path(start, goal) {
                if path.len() > 1 && path.len() <= 7 {
                    let next = path[1];
                    moves.push((
                        entity,
                        Position {
                            x: next.x,
                            y: next.y,
                        },
                    ));
                }
            }
        }
    }

    let mut moved_events = Vec::new();
    for (entity, next_pos) in moves {
        if let Some(pos) = world.get_mut::<Position>(entity) {
            let from = *pos;
            *pos = next_pos;
            moved_events.push(GameEvent::ChaserMoved { from, to: next_pos });
        }
    }
    if !moved_events.is_empty() {
        let events = world.resource_mut::<Events<GameEvent>>().unwrap();
        for event in moved_events {
            events.send(event);
        }
    }
}

pub fn resolve_tile_system(world: &mut World) {
    let state = world.resource::<GameState>().unwrap();
    if state.outcome != Outcome::Playing {
        return;
    }

    let player_pos = {
        let rows = world.query2::<Position, Player>();
        if let Some((_, p, _)) = rows.first() {
            **p
        } else {
            return;
        }
    };

    let on_hazard = world
        .query2::<Position, Hazard>()
        .into_iter()
        .any(|(_, hazard_pos, _)| *hazard_pos == player_pos);

    if on_hazard {
        world.resource_mut::<GameState>().unwrap().outcome = Outcome::Lost;
        world
            .resource_mut::<Events<GameEvent>>()
            .unwrap()
            .send(GameEvent::OutcomeChanged(Outcome::Lost));
        return;
    }

    let (on_goal, has_pkg) = {
        let map = world.resource::<Map>().unwrap();
        let state = world.resource::<GameState>().unwrap();
        (
            matches!(map.tile(player_pos.x, player_pos.y), Tile::Goal),
            state.has_package,
        )
    };

    if on_goal && has_pkg {
        world.resource_mut::<GameState>().unwrap().outcome = Outcome::Won;
        world
            .resource_mut::<Events<GameEvent>>()
            .unwrap()
            .send(GameEvent::OutcomeChanged(Outcome::Won));
    }
}

pub fn message_system(world: &mut World) {
    let mut messages = Vec::new();
    {
        let events = world.resource::<Events<GameEvent>>().unwrap();
        for event in events.iter() {
            let msg = match event {
                GameEvent::Moved { to, .. } => format!("Moved to {},{}.", to.x, to.y),
                GameEvent::Blocked { to, .. } => format!("Blocked by wall at {},{}.", to.x, to.y),
                GameEvent::Waited { .. } => "Waited...".to_string(),
                GameEvent::PickedUp { .. } => "Picked up a package!".to_string(),
                GameEvent::Dropped { .. } => "Dropped the package.".to_string(),
                GameEvent::Scanned {
                    visible_tiles,
                    visible_hazards,
                    ..
                } => {
                    format!(
                        "Scanned area: {} tiles, {} hazards detected.",
                        visible_tiles, visible_hazards
                    )
                }
                GameEvent::Inspected { at, tile } => {
                    format!("Inspected {},{} ({:?}).", at.x, at.y, tile)
                }
                GameEvent::ChaserMoved { from, to } => {
                    format!(
                        "A chaser moved from {},{} to {},{}.",
                        from.x, from.y, to.x, to.y
                    )
                }
                GameEvent::OutcomeChanged(Outcome::Won) => {
                    "YOU WON! Goal reached with the package.".to_string()
                }
                GameEvent::OutcomeChanged(Outcome::Lost) => {
                    "YOU LOST! Stepped on a hazard.".to_string()
                }
                GameEvent::OutcomeChanged(Outcome::Quit) => "Quitting...".to_string(),
                _ => continue,
            };
            messages.push(msg);
        }
    }

    if !messages.is_empty() {
        let log = world.resource_mut::<MessageLog>().unwrap();
        for msg in messages {
            log.push(msg);
        }
    }
}
