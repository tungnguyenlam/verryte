use crate::components::{
    Chaser, ChaserBehavior, GameEvent, GameState, Hazard, Outcome, Player, Position,
    PreviousPosition, ScentTrail,
};
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

    // Lazily fetch or insert the ScentTrail resource.
    let trail_positions = {
        if world.resource::<ScentTrail>().is_none() {
            world.insert_resource(ScentTrail {
                positions: vec![player_pos],
            });
            vec![player_pos]
        } else {
            let trail = world.resource_mut::<ScentTrail>().unwrap();
            if trail.positions.last() != Some(&player_pos) {
                trail.positions.push(player_pos);
                if trail.positions.len() > 100 {
                    trail.positions.remove(0);
                }
            }
            trail.positions.clone()
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
    let mut new_patrol_currents = Vec::new();

    {
        let map = world.resource::<Map>().unwrap();
        for &entity in &chasers {
            let pos = *world.get::<Position>(entity).unwrap();
            let prev = world.get::<PreviousPosition>(entity).map(|pp| pp.0);
            let start = verryte_map::Point::new(pos.x, pos.y);
            let goal = verryte_map::Point::new(player_pos.x, player_pos.y);

            let behavior = world.get::<ChaserBehavior>(entity).cloned();
            let mut target_pos = None;
            let mut custom_path = None;

            if let Some(ref beh) = behavior {
                match beh {
                    ChaserBehavior::Patrol { waypoints, current } => {
                        // Check LOS to player
                        let has_los = {
                            let (_path, blocked) =
                                map.tiles
                                    .raycast_opaque(start, goal, |_, tile| *tile == Tile::Wall);
                            !blocked
                        };

                        if has_los {
                            // Direct pursuit
                            target_pos = Some(goal);
                        } else if !waypoints.is_empty() {
                            let mut curr_idx = *current;
                            let mut waypoint = waypoints[curr_idx];
                            if start == waypoint {
                                curr_idx = (curr_idx + 1) % waypoints.len();
                                waypoint = waypoints[curr_idx];
                                new_patrol_currents.push((entity, curr_idx));
                            }
                            target_pos = Some(waypoint);
                        }
                    }
                    ChaserBehavior::ScentTracker { max_scent_age } => {
                        let mut chased_directly = false;
                        if let Some(path) = map.shortest_walkable_path(start, goal) {
                            if path.len() > 1 && path.len() <= 7 {
                                target_pos = Some(goal);
                                chased_directly = true;
                            }
                        }

                        if !chased_directly {
                            // Follow scent trail
                            for (age, &scent_pos) in trail_positions.iter().rev().enumerate() {
                                if age > *max_scent_age {
                                    break;
                                }
                                if let Some(path) = map.shortest_walkable_path(start, scent_pos) {
                                    if path.len() > 1 {
                                        custom_path = Some(path);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // Default direct pursuit if within 6 steps (path length <= 7)
                if let Some(path) = map.shortest_walkable_path(start, goal) {
                    if path.len() > 1 && path.len() <= 7 {
                        target_pos = Some(goal);
                    }
                }
            }

            let path = custom_path
                .or_else(|| target_pos.and_then(|t| map.shortest_walkable_path(start, t)));

            if let Some(path) = path {
                if path.len() > 1 {
                    let next = path[1];
                    let actual_next = if Some(next) == prev && path.len() > 2 {
                        let alt = path[2];
                        let is_adjacent = verryte_map::Direction::ALL
                            .iter()
                            .any(|&d| start.step(d) == alt);
                        if is_adjacent && map.is_walkable(alt) && Some(alt) != prev {
                            alt
                        } else {
                            next
                        }
                    } else {
                        next
                    };
                    moves.push((
                        entity,
                        Position {
                            x: actual_next.x,
                            y: actual_next.y,
                        },
                        pos,
                    ));
                }
            }
        }
    }

    // Apply the patrol waypoint updates
    for (entity, next_idx) in new_patrol_currents {
        if let Some(ChaserBehavior::Patrol { current, .. }) =
            world.get_mut::<ChaserBehavior>(entity)
        {
            *current = next_idx;
        }
    }

    let mut moved_events = Vec::new();
    for (entity, next_pos, from) in moves {
        if let Some(pos) = world.get_mut::<Position>(entity) {
            *pos = next_pos;
            moved_events.push(GameEvent::ChaserMoved { from, to: next_pos });
        }
        // Update previous position for next tick.
        if world.get::<PreviousPosition>(entity).is_some() {
            *world.get_mut::<PreviousPosition>(entity).unwrap() = PreviousPosition(from);
        } else {
            world.insert(entity, PreviousPosition(from));
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
                GameEvent::CursorCleared { at } => {
                    format!("Cleared cursor at {},{}.", at.x, at.y)
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
