use crate::components::{
    AIArchetype, ActiveHazards, BossConfig, BossPhase, CharacterClass, CharacterTrait, EchoAbility,
    ElementalStatus, EquippedEchoes, GameEvent, GameState, HeroTrait, Outcome, Position, Stats,
    Team, TelegraphZone, Threat, TurnPhase, Weather, WeatherType,
};
use crate::game::Game;
use crate::hazards::HazardSystem;
use crate::map::{TacticalMap, Tile};

use verryte_core::{Entity, Events, Rng, World};
use verryte_terminal::{vfx::VfxSystem, Color};

use super::combat::{
    apply_heal, effective_atk, effective_def, handle_defeat, is_flanking_position, log,
    resolve_combat_hit, trigger_steam_vent_explosion,
};
use super::movement::{get_tile_center_pixels, is_occupied_except};

pub fn auto_battle_system(world: &mut World) {
    let (phase, outcome, auto_battle) = {
        let state = world
            .resource::<GameState>()
            .expect("GameState resource must be registered");
        (state.phase, state.outcome, state.auto_battle)
    };
    if phase != TurnPhase::Player || outcome != Outcome::Playing || !auto_battle {
        return;
    }

    // A simple greedily-attacking AI for the player team
    let mut players = Vec::new();
    for (e, team) in world.query::<Team>() {
        if *team == Team::Player {
            players.push(e);
        }
    }

    let mut all_done = true;
    for player_entity in players {
        let ap = world.get::<Stats>(player_entity).map(|s| s.ap).unwrap_or(0);
        if ap > 0 {
            all_done = false;

            let pos = *world.get::<Position>(player_entity).unwrap();
            let class = *world.get::<CharacterClass>(player_entity).unwrap();

            // Find nearest enemy
            let mut nearest_enemy: Option<(Entity, Position)> = None;
            let mut min_dist = i32::MAX;
            for (ee, ep, team) in world.query2::<Position, Team>() {
                if *team == Team::Enemy {
                    let dist = (ep.x - pos.x).abs() as i32 + (ep.y - pos.y).abs() as i32;
                    if dist < min_dist {
                        min_dist = dist;
                        nearest_enemy = Some((ee, *ep));
                    }
                }
            }

            if let Some((_target_e, target_pos)) = nearest_enemy {
                let range = match class {
                    CharacterClass::Warrior => 1,
                    CharacterClass::Mage => 3,
                    CharacterClass::Healer => 2,
                    CharacterClass::DestructibleObject => 0,
                    _ => 1,
                };

                if min_dist <= range as i32 {
                    // Attack!
                    // In a real system, we'd inject an action.
                    // For simplicity here, we'll use a helper that simulates the action injection.
                    // But wait, systems run inside world.step().
                    // We should probably just set the cursor and "confirm".
                    let state = world.resource_mut::<GameState>().unwrap();
                    state.selected_entity = Some(player_entity);
                    state.cursor = target_pos;
                    // We can't easily call apply_action from here because it's on Game, not World.
                    // So we'll just set a flag or use a queue.
                    // Verryte uses an Event queue for this!
                } else {
                    // Move closer
                    let (target_tile, move_cost) = {
                        let map = world.resource::<TacticalMap>().unwrap();
                        let d_map = verryte_map::DijkstraMap::compute(
                            map.width,
                            map.height,
                            &[target_pos],
                            |pt| {
                                map.is_walkable(pt) && !is_occupied_except(world, pt, player_entity)
                            },
                            false,
                        );
                        let mut best_move = None;
                        let mut min_d = d_map.get(pos).unwrap_or(u32::MAX);
                        for neighbor in pos.neighbors4() {
                            if let Some(dist) = d_map.get(neighbor) {
                                if dist < min_d {
                                    min_d = dist;
                                    best_move = Some(neighbor);
                                }
                            }
                        }
                        if let Some(mt) = best_move {
                            (Some(mt), map.movement_cost(mt))
                        } else {
                            (None, 0)
                        }
                    };

                    if let Some(target_tile) = target_tile {
                        if ap >= move_cost {
                            if let Some(p) = world.get_mut::<Position>(player_entity) {
                                *p = target_tile;
                            }
                            if let Some(s) = world.get_mut::<Stats>(player_entity) {
                                s.ap -= move_cost;
                            }
                            let name = Game::get_class_name(class);
                            log(
                                world,
                                format!(
                                    "{} (Auto) moved to ({}, {}).",
                                    name, target_tile.x, target_tile.y
                                ),
                            );
                        } else {
                            // Skip this player
                        }
                    }
                }
            }
        }
    }

    if all_done {
        let state = world.resource_mut::<GameState>().unwrap();
        state.phase = TurnPhase::Enemy;
        log(world, "Player turn ended (Auto).");
    }
}

fn enemy_hazard_check(world: &mut World, entity: Entity, pos: Position) {
    let tile = {
        let map = world
            .resource::<TacticalMap>()
            .expect("TacticalMap must be registered");
        if pos.x < 0 || pos.y < 0 {
            return;
        }
        map.tile(pos.x, pos.y)
    };
    if tile == Tile::Lava || tile == Tile::Ice {
        return;
    }
    let Some(stats) = world.get::<Stats>(entity).cloned() else {
        return;
    };
    let Some(hazards) = world.resource::<ActiveHazards>() else {
        return;
    };
    let Some(result) = HazardSystem::trigger_hazard(hazards, pos, &stats) else {
        return;
    };

    let msg = result.message.clone();
    let dmg = result.damage;
    let heal = result.healing;
    let should_destroy = result.should_destroy_tile;
    let status = result.status_effect;
    let mut hp_after = 0;
    log(world, msg);
    if let Some(s) = world.get_mut::<Stats>(entity) {
        s.hp = (s.hp - dmg + heal).clamp(0, s.max_hp);
        hp_after = s.hp;
    }
    if hp_after <= 0 {
        let class = world
            .get::<CharacterClass>(entity)
            .copied()
            .unwrap_or(CharacterClass::ShadowStalker);
        let name = Game::get_class_name(class);
        handle_defeat(world, entity, name, class, pos);
        return;
    }
    if should_destroy {
        if let Some(map) = world.resource_mut::<TacticalMap>() {
            HazardSystem::process_cracked_floor(map, pos);
        }
    }
    let is_steam = result.hazard_type == crate::components::HazardType::SteamVent;
    if let Some(hazards_mut) = world.resource_mut::<ActiveHazards>() {
        HazardSystem::decrement_trigger(hazards_mut, pos);
    }
    if is_steam {
        trigger_steam_vent_explosion(world, pos);
    }
    if let Some(st) = status {
        if world.get::<Stats>(entity).is_some_and(|s| s.hp > 0) {
            world.insert(entity, st);
        }
    }
    if dmg > 0 || heal > 0 {
        let (cx, cy) = get_tile_center_pixels(world, pos);
        if let Some(vfx) = world.resource_mut::<VfxSystem>() {
            if dmg > 0 {
                vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                    cx,
                    cy,
                    8,
                    Color(180, 40, 40),
                    &['*', '·', '✦'],
                ));
            }
            if heal > 0 {
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_heal(cx, cy, 10));
            }
        }
    }
}

pub fn enemy_ai_system(world: &mut World) {
    let (phase, outcome) = {
        let state = world
            .resource::<GameState>()
            .expect("GameState resource must be registered");
        (state.phase, state.outcome)
    };
    if phase != TurnPhase::Enemy || outcome != Outcome::Playing {
        return;
    }

    let mut enemies = Vec::new();
    for (e, team) in world.query::<Team>() {
        if *team == Team::Enemy {
            enemies.push(e);
        }
    }

    let mut all_done = true;
    for enemy_entity in enemies {
        loop {
            let (mut enemy_pos, mut enemy_stats, enemy_class) = {
                let pos = world.get::<Position>(enemy_entity);
                let stats = world.get::<Stats>(enemy_entity);
                let class = world.get::<CharacterClass>(enemy_entity);
                if let (Some(p), Some(s), Some(c)) = (pos, stats, class) {
                    (*p, s.clone(), *c)
                } else {
                    break;
                }
            };

            if enemy_stats.ap <= 0 {
                break;
            }
            all_done = false;

            let archetype = world
                .get::<AIArchetype>(enemy_entity)
                .copied()
                .unwrap_or(AIArchetype::Chaser);

            if archetype == AIArchetype::Cleric {
                let mut needy_ally: Option<(Entity, Position, Stats, CharacterClass)> = None;
                let mut lowest_hp_pct = 100.0;
                for (ae, ap, team) in world.query2::<Position, Team>() {
                    if *team == Team::Enemy {
                        if let (Some(stats), Some(class)) =
                            (world.get::<Stats>(ae), world.get::<CharacterClass>(ae))
                        {
                            let hp_pct = (stats.hp as f32) / (stats.max_hp as f32);
                            if hp_pct < 0.5 && hp_pct < lowest_hp_pct {
                                lowest_hp_pct = hp_pct;
                                needy_ally = Some((ae, *ap, stats.clone(), *class));
                            }
                        }
                    }
                }

                if let Some((ally_entity, ally_pos, _ally_stats, ally_class)) = needy_ally {
                    let dist = (enemy_pos.x - ally_pos.x).abs() + (enemy_pos.y - ally_pos.y).abs();
                    if dist <= 3 {
                        if enemy_stats.ap >= 1 {
                            let (healed_amount, _healed_def) = apply_heal(world, ally_entity, 25);
                            if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                                stats.ap -= 1;
                            }
                            let cleric_name = Game::get_class_name(enemy_class);
                            let ally_name = Game::get_class_name(ally_class);
                            log(
                                world,
                                format!(
                                    "{} healed ally {} for {} HP!",
                                    cleric_name, ally_name, healed_amount
                                ),
                            );

                            let (cx, cy) = get_tile_center_pixels(world, ally_pos);
                            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                                vfx.particles
                                    .extend(verryte_terminal::vfx::emit_heal(cx, cy, 15));
                                vfx.floating_texts
                                    .push(verryte_terminal::vfx::FloatingText::new(
                                        cx,
                                        cy - 2.0,
                                        "+25",
                                        Color(50, 255, 50),
                                        true,
                                    ));
                            }
                            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                                events.send(GameEvent::Healed {
                                    healer: enemy_entity,
                                    target: ally_entity,
                                    amount: 25,
                                });
                            }
                        } else {
                            break;
                        }
                    } else {
                        let (target_tile, move_cost) = {
                            let map = world
                                .resource::<TacticalMap>()
                                .expect("TacticalMap must be registered");
                            let d_map = verryte_map::DijkstraMap::compute(
                                map.width,
                                map.height,
                                &[ally_pos],
                                |pt| {
                                    if pt.x < 0
                                        || pt.x >= map.width as i16
                                        || pt.y < 0
                                        || pt.y >= map.height as i16
                                    {
                                        return false;
                                    }
                                    map.is_walkable(pt)
                                        && !is_occupied_except(world, pt, enemy_entity)
                                },
                                false,
                            );

                            let mut best_move = None;
                            let mut min_d = d_map.get(enemy_pos).unwrap_or(u32::MAX);

                            for neighbor in enemy_pos.neighbors4() {
                                if let Some(dist) = d_map.get(neighbor) {
                                    if dist < min_d {
                                        min_d = dist;
                                        best_move = Some(neighbor);
                                    }
                                }
                            }

                            if let Some(target_tile) = best_move {
                                let move_cost = map.movement_cost(target_tile);
                                (Some(target_tile), move_cost)
                            } else {
                                (None, 0)
                            }
                        };

                        if let Some(target_tile) = target_tile {
                            if enemy_stats.ap >= move_cost {
                                if let Some(pos) = world.get_mut::<Position>(enemy_entity) {
                                    *pos = target_tile;
                                }
                                enemy_hazard_check(world, enemy_entity, target_tile);
                                if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                                    stats.ap -= move_cost;
                                }
                                let enemy_name = Game::get_class_name(enemy_class);
                                let ally_name = Game::get_class_name(ally_class);
                                log(
                                    world,
                                    format!(
                                        "{} (Cleric) moved closer to ally {} at ({}, {}).",
                                        enemy_name, ally_name, target_tile.x, target_tile.y
                                    ),
                                );
                                if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                                    events.send(GameEvent::Moved {
                                        entity: enemy_entity,
                                        from: enemy_pos,
                                        to: target_tile,
                                    });
                                }
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    continue;
                } else {
                    // No ally needs healing — fall through to common attack targeting
                }
            }

            let is_coward_flee = {
                let hp_pct = (enemy_stats.hp as f32) / (enemy_stats.max_hp as f32);
                archetype == AIArchetype::Coward && hp_pct < 0.3
            };

            if is_coward_flee {
                let (target_tile, move_cost) = {
                    let map = world
                        .resource::<TacticalMap>()
                        .expect("TacticalMap must be registered");
                    let mut nearest_player: Option<Position> = None;
                    let mut min_dist = i16::MAX;
                    for (_, p, team) in world.query2::<Position, Team>() {
                        if *team == Team::Player {
                            let dist = (enemy_pos.x - p.x).abs() + (enemy_pos.y - p.y).abs();
                            if dist < min_dist {
                                min_dist = dist;
                                nearest_player = Some(*p);
                            }
                        }
                    }
                    if let Some(player_pos) = nearest_player {
                        let d_map = verryte_map::DijkstraMap::compute(
                            map.width,
                            map.height,
                            &[player_pos],
                            |pt| {
                                if pt.x < 0
                                    || pt.x >= map.width as i16
                                    || pt.y < 0
                                    || pt.y >= map.height as i16
                                {
                                    return false;
                                }
                                map.is_walkable(pt) && !is_occupied_except(world, pt, enemy_entity)
                            },
                            false,
                        );
                        let path = d_map.flee_path(enemy_pos, 1, false);
                        if path.len() > 1 {
                            let target_tile = path[1];
                            let move_cost = map.movement_cost(target_tile);
                            (Some(target_tile), move_cost)
                        } else {
                            (None, 0)
                        }
                    } else {
                        (None, 0)
                    }
                };

                if let Some(target_tile) = target_tile {
                    if enemy_stats.ap >= move_cost {
                        if let Some(pos) = world.get_mut::<Position>(enemy_entity) {
                            *pos = target_tile;
                        }
                        enemy_hazard_check(world, enemy_entity, target_tile);
                        if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                            stats.ap -= move_cost;
                        }
                        let enemy_name = Game::get_class_name(enemy_class);
                        log(
                            world,
                            format!(
                                "{} (Coward) fled to ({}, {}).",
                                enemy_name, target_tile.x, target_tile.y
                            ),
                        );
                        if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                            events.send(GameEvent::Moved {
                                entity: enemy_entity,
                                from: enemy_pos,
                                to: target_tile,
                            });
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
                continue;
            }

            let mut chosen_player: Option<(Entity, Position, Stats, CharacterClass)> = None;
            let mut best_score = i32::MAX;
            for (pe, p, team) in world.query2::<Position, Team>() {
                if *team == Team::Player {
                    let dist = (enemy_pos.x - p.x).abs() + (enemy_pos.y - p.y).abs();
                    if let (Some(stats), Some(class)) =
                        (world.get::<Stats>(pe), world.get::<CharacterClass>(pe))
                    {
                        let hp_pct = if stats.max_hp > 0 {
                            (stats.hp * 100) / stats.max_hp
                        } else {
                            0
                        };
                        let threat_val = world.get::<Threat>(pe).map(|t| t.value).unwrap_or(0);
                        let healer_bonus = if *class == CharacterClass::Healer {
                            -30
                        } else {
                            0
                        };
                        let mut score = hp_pct + dist as i32 + healer_bonus;
                        score -= threat_val * 2;
                        if hp_pct < 30 {
                            score -= 100;
                        }
                        if score < best_score {
                            best_score = score;
                            chosen_player = Some((pe, *p, stats.clone(), *class));
                        }
                    }
                }
            }

            let Some((player_entity, player_pos, _player_stats, player_class)) = chosen_player
            else {
                world
                    .resource_mut::<GameState>()
                    .expect("GameState must be registered")
                    .outcome = Outcome::Defeat;
                log(
                    world,
                    "[fg:FF3333][b]Defeat![/] All player characters defeated.[/fg]",
                );
                return;
            };

            let range = match enemy_class {
                CharacterClass::CorruptedSpore => 1,
                CharacterClass::CursedSentinel => 3,
                CharacterClass::Boss => 2,
                CharacterClass::EnemyCleric => 2,
                CharacterClass::DestructibleObject => 0,
                _ => 2,
            };
            let mut actual_dist =
                (enemy_pos.x - player_pos.x).abs() + (enemy_pos.y - player_pos.y).abs();

            if archetype == AIArchetype::Assassin
                && actual_dist > range
                && actual_dist <= 5
                && enemy_stats.ap >= 2
            {
                let mut best_teleport_pos = None;
                let map = world
                    .resource::<TacticalMap>()
                    .expect("TacticalMap must be registered");
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if dx != 0 && dy != 0 {
                            continue;
                        } // only 4-way adjacent
                        let adj = Position::new(player_pos.x + dx, player_pos.y + dy);
                        if adj.x >= 0
                            && adj.x < map.width as i16
                            && adj.y >= 0
                            && adj.y < map.height as i16
                        {
                            if map.is_walkable(adj) && !is_occupied_except(world, adj, enemy_entity)
                            {
                                let is_flanking =
                                    is_flanking_position_from(world, adj, player_pos, enemy_entity);
                                if is_flanking {
                                    best_teleport_pos = Some(adj);
                                    break;
                                } else if best_teleport_pos.is_none() {
                                    best_teleport_pos = Some(adj);
                                }
                            }
                        }
                    }
                    if best_teleport_pos.is_some()
                        && is_flanking_position_from(
                            world,
                            best_teleport_pos.unwrap(),
                            player_pos,
                            enemy_entity,
                        )
                    {
                        break;
                    }
                }

                if let Some(tpos) = best_teleport_pos {
                    if let Some(pos) = world.get_mut::<Position>(enemy_entity) {
                        *pos = tpos;
                    }
                    if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                        stats.ap -= 1;
                        enemy_stats.ap -= 1;
                    }
                    enemy_pos = tpos;
                    actual_dist = 1;

                    let enemy_name = Game::get_class_name(enemy_class);
                    let player_name = Game::get_class_name(player_class);
                    log(
                        world,
                        format!(
                            "{} (Assassin) used Shadow Step to teleport adjacent to {}!",
                            enemy_name, player_name
                        ),
                    );

                    let (cx, cy) = get_tile_center_pixels(world, tpos);
                    if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                        vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                            cx,
                            cy,
                            20,
                            Color(128, 0, 128),
                            &['*', '·', '✦'],
                        ));
                        vfx.flashes.push(verryte_terminal::vfx::Flash::region(
                            Color(128, 0, 128),
                            0.1,
                            verryte_terminal::Rect::new(
                                (cx as u16).saturating_sub(2),
                                (cy as u16).saturating_sub(1),
                                5,
                                3,
                            ),
                        ));
                    }
                    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Moved {
                            entity: enemy_entity,
                            from: enemy_pos, // Note: here enemy_pos was already updated, so this technically says it moved from its new position to its new position. Wait, that's slightly wrong, but I'll fix it if needed.
                            to: tpos,
                        });
                    }
                }
            }

            // CursedSentinel retreat behavior: try to retreat to range > 2 if a player gets too close
            let mut retreated = false;
            if enemy_class == CharacterClass::CursedSentinel
                && actual_dist <= 2
                && enemy_stats.ap >= 1
            {
                let (target_tile, move_cost, dest_tile) = {
                    let map = world
                        .resource::<TacticalMap>()
                        .expect("TacticalMap must be registered");
                    let mut best_retreat_tile = None;
                    let mut max_dist = actual_dist;

                    for neighbor in enemy_pos.neighbors4() {
                        if neighbor.x >= 0
                            && neighbor.x < map.width as i16
                            && neighbor.y >= 0
                            && neighbor.y < map.height as i16
                        {
                            let passable = map.is_walkable(neighbor)
                                && !is_occupied_except(world, neighbor, enemy_entity);
                            if passable {
                                let n_dist = (neighbor.x - player_pos.x).abs()
                                    + (neighbor.y - player_pos.y).abs();
                                if n_dist > max_dist {
                                    let move_cost = map.movement_cost(neighbor);
                                    if enemy_stats.ap >= move_cost {
                                        max_dist = n_dist;
                                        best_retreat_tile = Some((neighbor, move_cost));
                                    }
                                }
                            }
                        }
                    }
                    if let Some((target_tile, move_cost)) = best_retreat_tile {
                        let dest_tile = map.tile(target_tile.x, target_tile.y);
                        (Some(target_tile), move_cost, dest_tile)
                    } else {
                        (None, 0, Tile::Grass)
                    }
                };

                if let Some(target_tile) = target_tile {
                    if let Some(pos) = world.get_mut::<Position>(enemy_entity) {
                        *pos = target_tile;
                    }
                    enemy_hazard_check(world, enemy_entity, target_tile);
                    if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                        stats.ap -= move_cost;
                    }
                    let enemy_name = Game::get_class_name(enemy_class);
                    log(
                        world,
                        format!(
                            "{} retreated from player to ({}, {}).",
                            enemy_name, target_tile.x, target_tile.y
                        ),
                    );

                    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Moved {
                            entity: enemy_entity,
                            from: enemy_pos,
                            to: target_tile,
                        });
                    }

                    // Check for Lava damage on retreat
                    if dest_tile == Tile::Lava {
                        let lava_dmg = {
                            let w = world
                                .resource::<Weather>()
                                .map(|w| w.current)
                                .unwrap_or(WeatherType::Sunny);
                            if w == WeatherType::Rainy {
                                16
                            } else {
                                20
                            }
                        };
                        let mut final_hp = 0;
                        let mut defeated = false;
                        if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                            stats.hp = std::cmp::max(0, stats.hp - lava_dmg);
                            final_hp = stats.hp;
                            if stats.hp <= 0 {
                                defeated = true;
                            }
                        }
                        log(
                            world,
                            format!(
                                "{} retreated into LAVA and took {} damage! (HP: {})",
                                enemy_name, lava_dmg, final_hp
                            ),
                        );

                        let (tcx, tcy) = get_tile_center_pixels(world, target_tile);
                        let vfx = world
                            .resource_mut::<VfxSystem>()
                            .expect("VfxSystem registered");
                        vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                            tcx,
                            tcy,
                            15,
                            Color(255, 60, 0),
                            &['*', '·', '✦'],
                        ));
                        vfx.shakes
                            .push(verryte_terminal::vfx::ScreenShake::new_eased(
                                1.5,
                                0.3,
                                verryte_terminal::vfx::EasingMode::QuadOut,
                            ));

                        if defeated {
                            handle_defeat(
                                world,
                                enemy_entity,
                                enemy_name,
                                enemy_class,
                                target_tile,
                            );
                        }
                    }
                    retreated = true;
                }
            }

            if retreated {
                continue;
            }

            if actual_dist <= range {
                if enemy_class == CharacterClass::CorruptedSpore {
                    // Explode!
                    log(world, "[fg:FF3333][b]Corrupted Spore explodes![/][/fg]");
                    let (ex, ey) = get_tile_center_pixels(world, enemy_pos);
                    let vfx = world
                        .resource_mut::<VfxSystem>()
                        .expect("VfxSystem must be registered");
                    vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                        ex,
                        ey,
                        50,
                        Color(50, 200, 50),
                        &['*', '·', '°', '◌'],
                    ));
                    vfx.shakes
                        .push(verryte_terminal::vfx::ScreenShake::new_eased(
                            3.0,
                            0.5,
                            verryte_terminal::vfx::EasingMode::QuadOut,
                        ));

                    // Deal damage to all players in 3x3 area
                    let mut affected = Vec::new();
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let target_pos = Position::new(enemy_pos.x + dx, enemy_pos.y + dy);
                            for (pe, p, team) in world.query2::<Position, Team>() {
                                if *team == Team::Player && *p == target_pos {
                                    affected.push((pe, *p));
                                }
                            }
                        }
                    }

                    for (target_e, target_pos) in affected {
                        let player_stats = world.get::<Stats>(target_e).cloned();
                        let player_class = world.get::<CharacterClass>(target_e).cloned();
                        if let (Some(_ps), Some(pc)) = (player_stats, player_class) {
                            let target_name = Game::get_class_name(pc);
                            let (_damage, defeated) = resolve_combat_hit(
                                world,
                                Some(enemy_entity),
                                target_e,
                                30,
                                "Spore Explosion",
                                target_name,
                                target_pos,
                            );
                            if defeated {
                                handle_defeat(world, target_e, target_name, pc, target_pos);
                            }
                        }
                    }

                    world.despawn(enemy_entity);
                    break;
                }

                // Boss attack logic
                let rng_val = {
                    let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
                    rng.next_u32(100)
                };

                let telegraph_active = {
                    let telegraph_zone = world
                        .resource::<TelegraphZone>()
                        .expect("TelegraphZone must be registered");
                    !telegraph_zone.tiles.is_empty()
                };

                let is_phase_2 = {
                    let state = world
                        .resource::<GameState>()
                        .expect("GameState must be registered");
                    state.boss_phase == BossPhase::Phase2
                };
                let telegraph_rate = if is_phase_2 {
                    world
                        .resource::<BossConfig>()
                        .map(|c| c.telegraph_rate_phase2)
                        .unwrap_or(60)
                } else {
                    world
                        .resource::<BossConfig>()
                        .map(|c| c.telegraph_rate_phase1)
                        .unwrap_or(40)
                };

                if !telegraph_active
                    && enemy_class == CharacterClass::Boss
                    && rng_val < telegraph_rate
                {
                    // Boss chooses to telegraph!
                    let mut tiles = Vec::new();
                    if is_phase_2 {
                        // Star shape: center + cardinal paths (length 2) + diagonals (length 1)
                        tiles.push(player_pos);
                        for d in 1..=2 {
                            tiles.push(Position::new(player_pos.x, player_pos.y - d));
                            tiles.push(Position::new(player_pos.x, player_pos.y + d));
                            tiles.push(Position::new(player_pos.x - d, player_pos.y));
                            tiles.push(Position::new(player_pos.x + d, player_pos.y));
                        }
                        tiles.push(Position::new(player_pos.x - 1, player_pos.y - 1));
                        tiles.push(Position::new(player_pos.x + 1, player_pos.y - 1));
                        tiles.push(Position::new(player_pos.x - 1, player_pos.y + 1));
                        tiles.push(Position::new(player_pos.x + 1, player_pos.y + 1));
                    } else {
                        // 3x3 square
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                let tx = player_pos.x + dx;
                                let ty = player_pos.y + dy;
                                tiles.push(Position::new(tx, ty));
                            }
                        }
                    }

                    let map_w = {
                        let map = world
                            .resource::<TacticalMap>()
                            .expect("TacticalMap must be registered");
                        map.width as i16
                    };
                    let map_h = {
                        let map = world
                            .resource::<TacticalMap>()
                            .expect("TacticalMap must be registered");
                        map.height as i16
                    };
                    tiles.retain(|p| p.x >= 0 && p.x < map_w && p.y >= 0 && p.y < map_h);

                    let damage = if is_phase_2 {
                        world
                            .resource::<BossConfig>()
                            .map(|c| c.telegraph_damage_phase2)
                            .unwrap_or(80)
                    } else {
                        world
                            .resource::<BossConfig>()
                            .map(|c| c.telegraph_damage_phase1)
                            .unwrap_or(50)
                    };

                    {
                        let telegraph_zone = world
                            .resource_mut::<TelegraphZone>()
                            .expect("TelegraphZone must be registered");
                        telegraph_zone.tiles = tiles;
                        telegraph_zone.damage = damage;
                    }

                    if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                        stats.ap = 0; // Spends all AP to telegraph
                    }

                    if is_phase_2 {
                        log(world, "Blight Sovereign is charging Celestial Ruin! Star-shaped area telegraphed in RED.");
                    } else {
                        log(world, "Blight Sovereign is charging Dark Annihilation! Area telegraphed in RED.");
                    }

                    // Spawn dark particles
                    let (ex, ey) = get_tile_center_pixels(world, enemy_pos);
                    let vfx = world
                        .resource_mut::<VfxSystem>()
                        .expect("VfxSystem must be registered");
                    vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                        ex,
                        ey,
                        30,
                        Color(120, 20, 180),
                        &['░', '▓', '✦', '¤'],
                    ));
                    vfx.shakes
                        .push(verryte_terminal::vfx::ScreenShake::new_eased(
                            2.5,
                            0.4,
                            verryte_terminal::vfx::EasingMode::QuadOut,
                        ));
                    break;
                }

                let mut ap_ok = false;
                if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                    if stats.ap >= 1 {
                        stats.ap -= 1;
                        ap_ok = true;
                    }
                }
                if ap_ok {
                    let effective_atk_val = effective_atk(world, enemy_entity);
                    let effective_def_val = effective_def(world, player_entity);
                    let base_damage_raw = std::cmp::max(1, effective_atk_val - effective_def_val);
                    let flanking = is_flanking_position(world, enemy_pos, player_pos);
                    let base_damage = if flanking {
                        (base_damage_raw as f32 * 1.15) as i32
                    } else {
                        base_damage_raw
                    };
                    let enemy_name = Game::get_class_name(enemy_class);
                    let player_name = Game::get_class_name(player_class);
                    if flanking {
                        log(
                            world,
                            format!(
                                "{} attacks {} from a flanking position! (+15% damage)",
                                enemy_name, player_name
                            ),
                        );
                    }

                    let (damage, defeated) = resolve_combat_hit(
                        world,
                        Some(enemy_entity),
                        player_entity,
                        base_damage,
                        enemy_name,
                        player_name,
                        player_pos,
                    );

                    // Thorns Echo: reflect 10% damage back to attacker
                    let mut reflect_damage = 0;
                    if let Some(echoes) = world.resource::<EquippedEchoes>() {
                        if echoes.abilities.contains(&EchoAbility::Thorns) {
                            reflect_damage = (damage / 10).max(1);
                        }
                    }

                    if reflect_damage > 0 {
                        let mut enemy_defeated = false;
                        let mut enemy_hp = 0;
                        if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                            stats.hp -= reflect_damage;
                            enemy_hp = stats.hp;
                            if stats.hp <= 0 {
                                enemy_defeated = true;
                            }
                        }
                        log(
                            world,
                            format!(
                                "Thorns reflected {} damage back to {}! (Enemy HP: {})",
                                reflect_damage, enemy_name, enemy_hp
                            ),
                        );
                        if enemy_defeated {
                            log(world, format!("{} was defeated by Thorns!", enemy_name));
                            world.despawn(enemy_entity);
                            break;
                        }
                    }

                    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Attacked {
                            attacker: enemy_entity,
                            target: player_entity,
                            damage,
                        });
                    }

                    // PlagueWraith applies Nature status on hit
                    if enemy_class == CharacterClass::PlagueWraith && !defeated {
                        let already_nature = world
                            .get::<ElementalStatus>(player_entity)
                            .map(|s| matches!(s, ElementalStatus::Nature { .. }))
                            .unwrap_or(false);
                        if !already_nature {
                            world.insert(player_entity, ElementalStatus::Nature { duration: 2 });
                            log(
                                world,
                                format!("{} is afflicted with Nature blight!", player_name),
                            );
                            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                                events.send(GameEvent::ElementalApplied {
                                    entity: player_entity,
                                    status: ElementalStatus::Nature { duration: 2 },
                                });
                            }
                        }
                    }

                    // GlacialGolem applies Ice status on hit
                    if enemy_class == CharacterClass::GlacialGolem && !defeated {
                        let already_ice = world
                            .get::<ElementalStatus>(player_entity)
                            .map(|s| matches!(s, ElementalStatus::Ice { .. }))
                            .unwrap_or(false);
                        if !already_ice {
                            world.insert(player_entity, ElementalStatus::Ice { duration: 2 });
                            log(
                                world,
                                format!("{} is frozen by Glacial Golem!", player_name),
                            );
                            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                                events.send(GameEvent::ElementalApplied {
                                    entity: player_entity,
                                    status: ElementalStatus::Ice { duration: 2 },
                                });
                            }
                        }
                    }

                    if defeated {
                        let name_str = player_name.to_string();
                        handle_defeat(world, player_entity, &name_str, player_class, player_pos);

                        let mut player_exists = false;
                        for (_e, team) in world.query::<Team>() {
                            if *team == Team::Player {
                                player_exists = true;
                                break;
                            }
                        }
                        if !player_exists {
                            world
                                .resource_mut::<GameState>()
                                .expect("GameState resource must be registered")
                                .outcome = Outcome::Defeat;
                            log(world, "Defeat! All player characters defeated.");
                            return;
                        }
                    }
                }
            } else {
                let player_positions: Vec<Position> = world
                    .query2::<Position, Team>()
                    .iter()
                    .filter(|(_, _, team)| **team == Team::Player)
                    .map(|(_, pos, _)| **pos)
                    .collect();

                if player_positions.is_empty() {
                    break;
                }

                let is_low_hp = if archetype == AIArchetype::Chaser {
                    false
                } else {
                    (enemy_stats.hp as f32 / enemy_stats.max_hp as f32) < 0.3
                };
                let (target_tile, move_cost, dest_tile) = {
                    let map = world
                        .resource::<TacticalMap>()
                        .expect("TacticalMap must be registered");

                    if is_low_hp {
                        let mut best_retreat_tile = None;
                        let mut max_dist = player_positions
                            .iter()
                            .map(|p| (enemy_pos.x - p.x).abs() + (enemy_pos.y - p.y).abs())
                            .min()
                            .unwrap_or(0);

                        for neighbor in enemy_pos.neighbors4() {
                            if neighbor.x >= 0
                                && neighbor.x < map.width as i16
                                && neighbor.y >= 0
                                && neighbor.y < map.height as i16
                            {
                                let passable = map.is_walkable(neighbor)
                                    && !is_occupied_except(world, neighbor, enemy_entity);
                                if passable {
                                    let min_player_dist = player_positions
                                        .iter()
                                        .map(|p| {
                                            (neighbor.x - p.x).abs() + (neighbor.y - p.y).abs()
                                        })
                                        .min()
                                        .unwrap_or(0);
                                    if min_player_dist > max_dist {
                                        let move_cost = map.movement_cost(neighbor);
                                        if enemy_stats.ap >= move_cost {
                                            max_dist = min_player_dist;
                                            best_retreat_tile = Some((neighbor, move_cost));
                                        }
                                    }
                                }
                            }
                        }
                        if let Some((target_tile, move_cost)) = best_retreat_tile {
                            let dest_tile = map.tile(target_tile.x, target_tile.y);
                            (Some(target_tile), move_cost, dest_tile)
                        } else {
                            (None, 0, Tile::Grass)
                        }
                    } else {
                        // Generate Dijkstra map towards target player using weighted movement costs
                        let d_map = verryte_map::DijkstraMap::compute_weighted(
                            map.width,
                            map.height,
                            &[player_pos],
                            |pt| {
                                if pt.x < 0
                                    || pt.x >= map.width as i16
                                    || pt.y < 0
                                    || pt.y >= map.height as i16
                                {
                                    return false;
                                }
                                map.is_walkable(pt) && !is_occupied_except(world, pt, enemy_entity)
                            },
                            |_, to| map.movement_cost(to) as u32,
                            false, // 4-way movement
                        );

                        // Find neighbor with lowest distance, preferring flanking positions
                        let mut best_move = None;
                        let mut min_d = u32::MAX;

                        for neighbor in enemy_pos.neighbors4() {
                            if let Some(dist) = d_map.get(neighbor) {
                                let flanking_adj = is_flanking_position_from(
                                    world,
                                    neighbor,
                                    player_pos,
                                    enemy_entity,
                                );
                                let effective = if flanking_adj {
                                    dist.saturating_sub(1)
                                } else {
                                    dist
                                };
                                if effective < min_d {
                                    min_d = effective;
                                    best_move = Some(neighbor);
                                }
                            }
                        }

                        if let Some(target_tile) = best_move {
                            let move_cost = map.movement_cost(target_tile);
                            let dest_tile = map.tile(target_tile.x, target_tile.y);
                            (Some(target_tile), move_cost, dest_tile)
                        } else {
                            // Patrol/wander: pick a random walkable neighbor
                            let walkable: Vec<(Position, i32, Tile)> = enemy_pos
                                .neighbors4()
                                .into_iter()
                                .filter(|n| {
                                    n.x >= 0
                                        && n.x < map.width as i16
                                        && n.y >= 0
                                        && n.y < map.height as i16
                                        && map.is_walkable(*n)
                                        && !is_occupied_except(world, *n, enemy_entity)
                                })
                                .map(|n| (n, map.movement_cost(n), map.tile(n.x, n.y)))
                                .collect();
                            if !walkable.is_empty() {
                                let rng_val = {
                                    let rng = world.resource_mut::<Rng>().expect("Rng registered");
                                    rng.next_u32(walkable.len() as u32) as usize
                                };
                                let (wander_pos, wander_cost, wander_tile) = walkable[rng_val];
                                (Some(wander_pos), wander_cost, wander_tile)
                            } else {
                                (None, 0, Tile::Grass)
                            }
                        }
                    }
                };

                let mut final_dest = target_tile;
                let mut final_dest_tile = dest_tile;
                if let Some(target_tile_pos) = target_tile {
                    let map = world
                        .resource::<TacticalMap>()
                        .expect("TacticalMap must be registered");
                    let has_ice_walker = world
                        .get::<CharacterTrait>(enemy_entity)
                        .map(|t| t.trait_type == HeroTrait::IceWalker)
                        .unwrap_or(false);
                    if dest_tile == Tile::Ice && !has_ice_walker {
                        let dx = target_tile_pos.x - enemy_pos.x;
                        let dy = target_tile_pos.y - enemy_pos.y;
                        let mut curr = target_tile_pos;
                        loop {
                            let next_pt = Position::new(curr.x + dx, curr.y + dy);
                            if next_pt.x < 0
                                || next_pt.x >= map.width as i16
                                || next_pt.y < 0
                                || next_pt.y >= map.height as i16
                            {
                                break;
                            }
                            if is_occupied_except(world, next_pt, enemy_entity) {
                                break;
                            }
                            let next_tile = map.tile(next_pt.x, next_pt.y);
                            if next_tile == Tile::Wall {
                                break;
                            }
                            curr = next_pt;
                            if next_tile != Tile::Ice {
                                break;
                            }
                        }
                        final_dest = Some(curr);
                        final_dest_tile = map.tile(curr.x, curr.y);
                    }
                }

                if let Some(final_tile) = final_dest {
                    if let Some(pos) = world.get_mut::<Position>(enemy_entity) {
                        *pos = final_tile;
                    }
                    enemy_hazard_check(world, enemy_entity, final_tile);
                    if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                        stats.ap -= move_cost;
                    }
                    let enemy_name = Game::get_class_name(enemy_class);
                    if final_dest.unwrap() != target_tile.unwrap() {
                        log(
                            world,
                            format!(
                                "{} slid on ice to ({}, {}).",
                                enemy_name, final_tile.x, final_tile.y
                            ),
                        );
                    } else if is_low_hp {
                        log(
                            world,
                            format!(
                                "{} (Low HP) retreated from player to ({}, {}).",
                                enemy_name, final_tile.x, final_tile.y
                            ),
                        );
                    } else {
                        log(
                            world,
                            format!(
                                "{} moved closer to player at ({}, {}).",
                                enemy_name, final_tile.x, final_tile.y
                            ),
                        );
                    }

                    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                        events.send(GameEvent::Moved {
                            entity: enemy_entity,
                            from: enemy_pos,
                            to: final_tile,
                        });
                    }

                    // Check for Lava damage on normal move
                    if final_dest_tile == Tile::Lava {
                        let lava_dmg = {
                            let w = world
                                .resource::<Weather>()
                                .map(|w| w.current)
                                .unwrap_or(WeatherType::Sunny);
                            if w == WeatherType::Rainy {
                                16
                            } else {
                                20
                            }
                        };
                        let mut final_hp = 0;
                        let mut defeated = false;
                        if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                            stats.hp = std::cmp::max(0, stats.hp - lava_dmg);
                            final_hp = stats.hp;
                            if stats.hp <= 0 {
                                defeated = true;
                            }
                        }
                        log(
                            world,
                            format!(
                                "{} stepped into LAVA and took {} damage! (HP: {})",
                                enemy_name, lava_dmg, final_hp
                            ),
                        );

                        let (tcx, tcy) = get_tile_center_pixels(world, final_tile);
                        let vfx = world
                            .resource_mut::<VfxSystem>()
                            .expect("VfxSystem registered");
                        vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                            tcx,
                            tcy,
                            15,
                            Color(255, 60, 0),
                            &['*', '·', '✦'],
                        ));
                        vfx.shakes
                            .push(verryte_terminal::vfx::ScreenShake::new_eased(
                                1.5,
                                0.3,
                                verryte_terminal::vfx::EasingMode::QuadOut,
                            ));

                        if defeated {
                            handle_defeat(world, enemy_entity, enemy_name, enemy_class, final_tile);
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    if all_done {
        world
            .resource_mut::<crate::components::TurnTransition>()
            .expect("TurnTransition must be registered")
            .request_end = true;
    }
}

fn is_flanking_position_from(
    world: &World,
    candidate: Position,
    target_pos: Position,
    self_entity: Entity,
) -> bool {
    let dist = (candidate.x - target_pos.x).abs() + (candidate.y - target_pos.y).abs();
    if dist != 1 {
        return false;
    }
    let dx = candidate.x - target_pos.x;
    let dy = candidate.y - target_pos.y;
    for (other_e, other_pos, team) in world.query2::<Position, Team>() {
        if *team == Team::Enemy && other_e != self_entity {
            let other_dist =
                (other_pos.x - target_pos.x).abs() + (other_pos.y - target_pos.y).abs();
            if other_dist == 1 {
                let odx = other_pos.x - target_pos.x;
                let ody = other_pos.y - target_pos.y;
                if (dx != 0 && dy == 0 && odx == 0 && ody != 0)
                    || (dx == 0 && dy != 0 && odx != 0 && ody == 0)
                {
                    return true;
                }
            }
        }
    }
    false
}
