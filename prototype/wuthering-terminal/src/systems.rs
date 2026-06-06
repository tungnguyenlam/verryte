use crate::components::{
    AIArchetype, BossPhase, CharacterClass, EchoItem, ElementalShield, ElementalStatus, GameEvent,
    GameState, Outcome, Position, Rooted, ShieldType, Stats, Team, TelegraphZone, Threat,
    TurnPhase, Weather, WeatherType,
};
use crate::game::Game;
use crate::map::{TacticalMap, Tile};

use verryte_core::{Entity, Events, MessageLog, Rng, World};
use verryte_terminal::{vfx::VfxSystem, Color};

pub fn visibility_system(world: &mut World) {
    let player_positions: Vec<Position> = world
        .query2::<Position, Team>()
        .iter()
        .filter(|(_, _, team)| **team == Team::Player)
        .map(|(_, pos, _)| **pos)
        .collect();

    let map_tiles = world
        .resource::<TacticalMap>()
        .expect("TacticalMap resource must be registered")
        .tiles
        .clone();
    let visibility = world
        .resource_mut::<verryte_map::VisibilityMap>()
        .expect("VisibilityMap resource must be registered");

    visibility.clear_visible();

    for pos in player_positions {
        visibility.compute_fov_incremental(pos, 8, |p| {
            map_tiles
                .get(p)
                .map(|t| matches!(t, Tile::Wall))
                .unwrap_or(true)
        });
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
            let (enemy_pos, enemy_stats, enemy_class) = {
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
                            if hp_pct < 0.5 {
                                if hp_pct < lowest_hp_pct {
                                    lowest_hp_pct = hp_pct;
                                    needy_ally = Some((ae, *ap, stats.clone(), *class));
                                }
                            }
                        }
                    }
                }

                if let Some((ally_entity, ally_pos, _ally_stats, ally_class)) = needy_ally {
                    let dist = (enemy_pos.x - ally_pos.x).abs() + (enemy_pos.y - ally_pos.y).abs();
                    if dist <= 3 {
                        if enemy_stats.ap >= 1 {
                            if let Some(stats) = world.get_mut::<Stats>(ally_entity) {
                                stats.hp = (stats.hp + 25).min(stats.max_hp);
                            }
                            if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                                stats.ap -= 1;
                            }
                            let cleric_name = Game::get_class_name(enemy_class);
                            let ally_name = Game::get_class_name(ally_class);
                            log(
                                world,
                                format!("{} healed ally {} for 25 HP!", cleric_name, ally_name),
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

            let Some((player_entity, player_pos, player_stats, player_class)) = chosen_player
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
                _ => 2,
            };
            let actual_dist =
                (enemy_pos.x - player_pos.x).abs() + (enemy_pos.y - player_pos.y).abs();

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
                        let mut final_hp = 0;
                        let mut defeated = false;
                        if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                            stats.hp = std::cmp::max(0, stats.hp - 20);
                            final_hp = stats.hp;
                            if stats.hp <= 0 {
                                defeated = true;
                            }
                        }
                        log(
                            world,
                            format!(
                                "{} retreated into LAVA and took 20 damage! (HP: {})",
                                enemy_name, final_hp
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
                        .resource::<crate::components::BossConfig>()
                        .map(|c| c.telegraph_rate_phase2)
                        .unwrap_or(60)
                } else {
                    world
                        .resource::<crate::components::BossConfig>()
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
                            .resource::<crate::components::BossConfig>()
                            .map(|c| c.telegraph_damage_phase2)
                            .unwrap_or(80)
                    } else {
                        world
                            .resource::<crate::components::BossConfig>()
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
                    let base_damage_raw = std::cmp::max(1, enemy_stats.atk - player_stats.def);
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
                        player_entity,
                        base_damage,
                        enemy_name,
                        player_name,
                        player_pos,
                    );

                    // Thorns Echo: reflect 10% damage back to attacker
                    let mut reflect_damage = 0;
                    if let Some(echoes) = world.resource::<crate::components::EquippedEchoes>() {
                        if echoes
                            .abilities
                            .contains(&crate::components::EchoAbility::Thorns)
                        {
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
                        .get::<crate::components::CharacterTrait>(enemy_entity)
                        .map(|t| t.trait_type == crate::components::HeroTrait::IceWalker)
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
                        let mut final_hp = 0;
                        let mut defeated = false;
                        if let Some(stats) = world.get_mut::<Stats>(enemy_entity) {
                            stats.hp = std::cmp::max(0, stats.hp - 20);
                            final_hp = stats.hp;
                            if stats.hp <= 0 {
                                defeated = true;
                            }
                        }
                        log(
                            world,
                            format!(
                                "{} stepped into LAVA and took 20 damage! (HP: {})",
                                enemy_name, final_hp
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

            process_team_status_effects(world, Team::Enemy);

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
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        stats.ap = stats.max_ap;
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
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        let mut bonus = 0;
                        if has_swift {
                            bonus = 1;
                        }
                        if has_swift_foot {
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

pub fn end_player_turn_system(world: &mut World) {
    // Execute any telegraphed attacks first!
    let (telegraph_tiles, telegraph_damage) = {
        let telegraph_zone = world
            .resource_mut::<TelegraphZone>()
            .expect("TelegraphZone must be registered");
        let tiles = telegraph_zone.tiles.clone();
        let damage = telegraph_zone.damage;
        telegraph_zone.tiles.clear();
        (tiles, damage)
    };

    if !telegraph_tiles.is_empty() {
        log(
            world,
            "[fg:E6E600][b]Blight Sovereign[/] releases [fg:9933FF][b]Dark Annihilation![/][/fg]",
        );

        // VFX feedback!
        {
            let vfx = world
                .resource_mut::<VfxSystem>()
                .expect("VfxSystem must be registered");
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(120, 0, 180),
                    0.3,
                    verryte_terminal::EasingMode::ExpoOut,
                ));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new_eased(
                    4.5,
                    0.6,
                    verryte_terminal::vfx::EasingMode::ExpoOut,
                ));
        }

        let mut hit_count = 0;
        // Check all player characters standing in telegraph tiles
        let mut players_in_zone = Vec::new();
        for (e, p, team) in world.query2::<Position, Team>() {
            if *team == Team::Player && telegraph_tiles.contains(p) {
                players_in_zone.push(e);
            }
        }

        for pe in players_in_zone {
            let target_class = *world
                .get::<CharacterClass>(pe)
                .expect("player must have CharacterClass");
            let target_pos = *world
                .get::<Position>(pe)
                .expect("player must have Position");
            let target_name = Game::get_class_name(target_class);
            let mut final_hp = 0;
            if let Some(stats) = world.get_mut::<Stats>(pe) {
                stats.hp -= telegraph_damage;
                final_hp = stats.hp;
            }
            log(
                world,
                format!(
                    "Dark Annihilation hit {} for {} damage! (HP: {})",
                    target_name, telegraph_damage, final_hp
                ),
            );

            // Thorns Echo: reflect 10% damage back to boss
            if let Some(echoes) = world.resource::<crate::components::EquippedEchoes>() {
                if echoes
                    .abilities
                    .contains(&crate::components::EchoAbility::Thorns)
                {
                    let reflect_damage = (telegraph_damage as f32 * 0.1) as i32;
                    let mut boss_ent = None;
                    for (e, class) in world.query::<CharacterClass>() {
                        if *class == CharacterClass::Boss {
                            boss_ent = Some(e);
                            break;
                        }
                    }
                    if let Some(be) = boss_ent {
                        let mut boss_hp = 0;
                        if let Some(stats) = world.get_mut::<Stats>(be) {
                            stats.hp -= reflect_damage;
                            boss_hp = stats.hp;
                        }
                        log(
                            world,
                            format!(
                                "Thorns reflected {} damage back to Blight Sovereign! (Boss HP: {})",
                                reflect_damage, boss_hp
                            ),
                        );
                    }
                }
            }

            hit_count += 1;

            let (cx, cy) = get_tile_center_pixels(world, target_pos);
            {
                let vfx = world
                    .resource_mut::<VfxSystem>()
                    .expect("VfxSystem must be registered");
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new_eased(
                        cx,
                        cy - 2.0,
                        "-50",
                        Color(255, 20, 20),
                        true,
                        verryte_terminal::vfx::EasingMode::QuadOut,
                    ));
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_fire(cx, cy, 15));
            }

            if final_hp <= 0 {
                let name_str = target_name.to_string();
                handle_defeat(world, pe, &name_str, target_class, target_pos);
            }
        }

        if hit_count == 0 {
            log(world, "Dark Annihilation missed everyone!");
        }
    }

    {
        let state = world
            .resource_mut::<GameState>()
            .expect("GameState must be registered");
        state.phase = TurnPhase::Enemy;
        state.selected_entity = None;
    }
    log(world, "Enemy Phase starts!");
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::PhaseChanged(TurnPhase::Enemy));
        events.send(GameEvent::TurnEnded);
    }

    // Run enemy AI
    enemy_ai_system(world);
}

pub fn log(world: &mut World, msg: impl Into<String>) {
    if let Some(log) = world.resource_mut::<MessageLog>() {
        log.push(msg);
    }
}

pub fn award_xp(world: &mut World, amount: u32) {
    let players: Vec<Entity> = world
        .query::<Team>()
        .iter()
        .filter(|(_, team)| **team == Team::Player)
        .map(|(e, _)| *e)
        .collect();

    let mut level_ups = Vec::new();
    for e in players {
        let class = *world
            .get::<CharacterClass>(e)
            .expect("player must have CharacterClass");
        if let Some(stats) = world.get_mut::<Stats>(e) {
            stats.xp += amount;
            let needed = stats.level * 100;
            if stats.xp >= needed {
                stats.xp -= needed;
                stats.level += 1;
                stats.max_hp += 10;
                stats.hp = stats.max_hp;
                stats.atk += 2;
                stats.def += 1;
                level_ups.push((class, stats.level));

                let char_pos = world.get::<Position>(e).copied();
                if let Some(pos) = char_pos {
                    play_spatial_sfx(world, "level_up", pos);
                } else if let Some(events) =
                    world.resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("level_up"));
                }
            }
        }
    }

    for (class, level) in level_ups {
        log(
            world,
            format!(
                "[fg:FFD700][b]LEVEL UP![/] {} reached level {}![/fg]",
                Game::get_class_name(class),
                level
            ),
        );
    }
}

pub fn get_tile_dimensions(_world: &World) -> (u16, u16) {
    let (term_w, term_h) = verryte_tty::terminal_size();
    let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
    tier.tile_dimensions()
}

pub fn get_tile_center_pixels(world: &World, pos: Position) -> (f32, f32) {
    let (tile_w, tile_h) = get_tile_dimensions(world);
    let cx = pos.x as f32 * tile_w as f32 + (tile_w as f32 / 2.0);
    let cy = pos.y as f32 * tile_h as f32 + (tile_h as f32 / 2.0);
    (cx, cy)
}

pub fn play_spatial_sfx(world: &mut World, name: &str, emitter_pos: Position) {
    let listener = world
        .resource::<GameState>()
        .map(|s| s.cursor)
        .unwrap_or(Position::new(12, 8));
    let dx = (emitter_pos.x - listener.x) as f32;
    let dy = (emitter_pos.y - listener.y) as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    let max_range = 15.0_f32;
    let volume = (1.0 - (dist / max_range)).clamp(0.0, 1.0);
    let pan = if max_range > 0.0 {
        (dx / max_range).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
        events.send(
            verryte_core::AudioEvent::play(name)
                .with_volume(volume)
                .with_pan(pan),
        );
    }
}

pub fn weather_damage_modifier(world: &World, base_damage: i32, attacker_name: &str) -> i32 {
    let weather = match world.resource::<Weather>() {
        Some(w) => w.current,
        None => return base_damage,
    };
    let is_fire = attacker_name.contains("Warrior")
        || attacker_name.contains("Kael")
        || attacker_name.contains("Dragon");
    let modifier: f32 = match (weather, is_fire) {
        (WeatherType::Rainy, true) => 0.80,
        _ => 1.0,
    };
    (base_damage as f32 * modifier) as i32
}

pub fn resolve_combat_hit(
    world: &mut World,
    target: Entity,
    base_damage: i32,
    attacker_name: &str,
    target_name: &str,
    pos: Position,
) -> (i32, bool) {
    let sfx_name = if attacker_name.contains("Warrior") || attacker_name.contains("Kael") {
        "warrior_attack"
    } else if attacker_name.contains("Mage") || attacker_name.contains("Lyra") {
        "mage_attack"
    } else if attacker_name.contains("Healer") || attacker_name.contains("Mira") {
        "healer_attack"
    } else if attacker_name.contains("Boss") || attacker_name.contains("Blight") {
        "boss_attack"
    } else {
        "enemy_attack"
    };
    play_spatial_sfx(world, sfx_name, pos);

    let base_damage = weather_damage_modifier(world, base_damage, attacker_name);

    let (is_crit, is_block, damage) = {
        let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
        let roll = rng.next_u32(100);
        if roll < 20 {
            (true, false, (base_damage as f32 * 1.5) as i32)
        } else if roll < 35 {
            (false, true, (base_damage / 2).max(1))
        } else {
            (false, false, base_damage)
        }
    };

    let weather = world
        .resource::<Weather>()
        .map(|w| w.current)
        .unwrap_or(WeatherType::Sunny);
    let mut lightning_bonus = 0;
    if weather == WeatherType::LightningStorm {
        let bonus_roll = {
            let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
            rng.next_u32(100)
        };
        if bonus_roll < 25 {
            lightning_bonus = 15;
            log(
                world,
                "[fg:FFFF64]Lightning strikes from the storm! Bonus 15 damage![/fg]",
            );
        }
    }
    let damage = damage + lightning_bonus;

    let mut shield_absorbed = 0;
    let mut shield_broke = false;
    let mut shield_type_opt = None;
    let mut shield_remaining = 0;
    if let Some(shield) = world.get_mut::<ElementalShield>(target) {
        shield_type_opt = Some(shield.shield_type);
        if shield.amount >= damage {
            shield.amount -= damage;
            shield_absorbed = damage;
            shield_remaining = shield.amount;
        } else {
            shield_absorbed = shield.amount;
            shield.amount = 0;
            shield_broke = true;
        }
    }
    if shield_broke {
        world.remove::<ElementalShield>(target);
    }

    let actual_damage = damage - shield_absorbed;
    let mut defeated = false;
    let mut final_hp = 0;
    if let Some(stats) = world.get_mut::<Stats>(target) {
        stats.hp -= actual_damage;
        final_hp = stats.hp;
        if stats.hp <= 0 {
            defeated = true;
        }
    }

    let log_msg = if is_crit {
        format!(
            "[fg:FF8080]{} attacked {} for [b]{} damage![/] [fg:E6E600][b](CRITICAL HIT!)[/] (Target HP: {})[/fg]",
            attacker_name, target_name, damage, final_hp
        )
    } else if is_block {
        format!(
            "[fg:CCCCCC]{} attacked {} for {} damage! (BLOCKED!) (Target HP: {})[/fg]",
            attacker_name, target_name, damage, final_hp
        )
    } else {
        format!(
            "{} attacked {} for [fg:FF3333]{} damage![/] (Target HP: {})",
            attacker_name, target_name, damage, final_hp
        )
    };
    log(world, log_msg);

    if shield_absorbed > 0 {
        let shield_name =
            match shield_type_opt.expect("shield_type must be set when shield_absorbed > 0") {
                ShieldType::Ice => "[fg:80D0FF]Ice Shield[/fg]",
                ShieldType::Lightning => "[fg:FFD700]Lightning Shield[/fg]",
                ShieldType::Nature => "[fg:50DC64]Nature Shield[/fg]",
                ShieldType::Physical => "[fg:CCCCCC]Physical Shield[/fg]",
            };
        if shield_broke {
            log(
                world,
                format!(
                    "{}'s {} broke! It absorbed {} damage.",
                    target_name, shield_name, shield_absorbed
                ),
            );
        } else {
            log(
                world,
                format!(
                    "{}'s {} absorbed {} damage! (Shield HP: {})",
                    target_name, shield_name, shield_absorbed, shield_remaining
                ),
            );
        }
    }

    let (tcx, tcy) = get_tile_center_pixels(world, pos);
    let float_text = if is_crit {
        format!("CRIT! -{}", actual_damage)
    } else if is_block {
        format!("BLOCK! -{}", actual_damage)
    } else {
        format!("-{}", actual_damage)
    };

    let float_color = if is_crit {
        Color(255, 215, 0) // Gold
    } else if is_block {
        Color(160, 160, 160) // Grey
    } else {
        Color(255, 50, 50) // Red
    };

    {
        if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
            let pan = ((pos.x as f32 - 12.0) / 12.0).clamp(-1.0, 1.0);
            if is_crit {
                events.send(verryte_core::AudioEvent::play("crit").with_pan(pan));
            } else {
                events.send(verryte_core::AudioEvent::play("hit").with_pan(pan));
            }
        }

        let vfx = world
            .resource_mut::<VfxSystem>()
            .expect("VfxSystem must be registered");
        vfx.floating_texts
            .push(verryte_terminal::vfx::FloatingText::new_eased(
                tcx,
                tcy - 2.0,
                &float_text,
                float_color,
                is_crit,
                verryte_terminal::vfx::EasingMode::QuadOut,
            ));

        if shield_absorbed > 0 {
            let shield_color =
                match shield_type_opt.expect("shield_type must be set when shield_absorbed > 0") {
                    ShieldType::Ice => Color(100, 200, 255),
                    ShieldType::Lightning => Color(255, 230, 50),
                    ShieldType::Nature => Color(50, 220, 100),
                    ShieldType::Physical => Color(160, 160, 160),
                };
            vfx.floating_texts
                .push(verryte_terminal::vfx::FloatingText::new_eased(
                    tcx + 2.0,
                    tcy - 1.0,
                    &format!("SHIELD -{}", shield_absorbed),
                    shield_color,
                    false,
                    verryte_terminal::vfx::EasingMode::QuadOut,
                ));

            // Custom spiral particle trajectory for shield hits!
            let mut shield_particles = Vec::new();
            for i in 0..10 {
                let angle = (i as f32 / 10.0) * std::f32::consts::TAU;
                shield_particles.push(verryte_terminal::vfx::Particle {
                    x: tcx,
                    y: tcy,
                    vx: angle.cos() * 1.5,
                    vy: angle.sin() * 0.7,
                    glyph: '✦',
                    fg: shield_color,
                    bg: Color::BLACK,
                    lifetime: 0.5,
                    max_lifetime: 0.5,
                    attrs: verryte_terminal::CellAttrs::NONE.bold(),
                    trajectory: verryte_terminal::vfx::Trajectory::Spiral {
                        speed: 2.0,
                        radius: 1.5,
                    },
                });
            }
            vfx.particles.extend(shield_particles);
        }

        let flash_color = if is_crit {
            Color(255, 215, 0)
        } else {
            Color(255, 100, 100)
        };
        let flash_duration = if is_crit { 0.15 } else { 0.1 };
        vfx.flashes
            .push(verryte_terminal::vfx::Flash::full_screen_eased(
                flash_color,
                flash_duration,
                verryte_terminal::EasingMode::QuadOut,
            ));

        vfx.particles.extend(verryte_terminal::vfx::emit_slash(
            tcx,
            tcy,
            if is_crit { 2.0 } else { 1.0 },
        ));

        let shake_intensity = if is_crit { 3.5 } else { 1.5 };
        let shake_duration = if is_crit { 0.4 } else { 0.25 };
        let shake_easing = if is_crit {
            verryte_terminal::vfx::EasingMode::ExpoOut
        } else {
            verryte_terminal::vfx::EasingMode::QuadOut
        };
        vfx.shakes
            .push(verryte_terminal::vfx::ScreenShake::new_eased(
                shake_intensity,
                shake_duration,
                shake_easing,
            ));
    }

    (actual_damage, defeated)
}

pub fn handle_defeat(
    world: &mut World,
    entity: Entity,
    name: &str,
    class: CharacterClass,
    pos: Position,
) {
    if class == CharacterClass::Boss {
        let phase = world
            .resource::<GameState>()
            .expect("GameState resource must be registered")
            .boss_phase;
        if phase == BossPhase::Phase1 {
            let config = world
                .resource::<crate::components::BossConfig>()
                .cloned()
                .unwrap_or_default();
            if let Some(stats) = world.get_mut::<Stats>(entity) {
                stats.max_hp = config.phase2_max_hp;
                stats.hp = config.phase2_max_hp;
                stats.atk += config.phase2_atk_bonus;
                stats.def += config.phase2_def_bonus;
                stats.spd += config.phase2_spd_bonus;
                stats.max_ap = config.phase2_max_ap;
                stats.ap = config.phase2_max_ap;
            }

            if let Some(state) = world.resource_mut::<GameState>() {
                state.boss_phase = BossPhase::Phase2;
            }

            log(world, "Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");

            let (bx, by) = get_tile_center_pixels(world, pos);

            let vfx = world
                .resource_mut::<VfxSystem>()
                .expect("VfxSystem must be registered");
            vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                bx,
                by,
                50,
                Color(255, 0, 0),
                &['✦', '*', '░', '▓', '¤'],
            ));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new_eased(
                    5.0,
                    1.0,
                    verryte_terminal::vfx::EasingMode::ExpoOut,
                ));
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(255, 0, 0),
                    0.5,
                    verryte_terminal::EasingMode::ExpoOut,
                ));
            return;
        }
    }

    log(world, format!("{} was defeated!", name));
    if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
        events.send(verryte_core::AudioEvent::play("defeat"));
    }
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::Defeated { entity });
    }
    world.despawn(entity);

    // Award XP if it was an enemy
    if class != CharacterClass::Boss
        || (class == CharacterClass::Boss
            && world
                .resource::<GameState>()
                .expect("GameState must be registered")
                .boss_phase
                == BossPhase::Phase2)
    {
        let xp_amount = match class {
            CharacterClass::Boss => 1000,
            CharacterClass::ShadowStalker => 50,
            CharacterClass::CursedSentinel => 40,
            CharacterClass::PlagueWraith => 35,
            CharacterClass::GlacialGolem => 60,
            _ => 20,
        };
        award_xp(world, xp_amount);
    }

    if class == CharacterClass::Boss {
        log(
            world,
            "Blight Sovereign dropped an Echo! Move a character to its tile to absorb it.",
        );
        world.builder().with(pos).with(EchoItem { class }).build();

        // Visual boss death burst
        let (cx, cy) = get_tile_center_pixels(world, pos);
        let vfx = world
            .resource_mut::<VfxSystem>()
            .expect("VfxSystem must be registered");
        vfx.particles.extend(verryte_terminal::vfx::emit_burst(
            cx,
            cy,
            40,
            Color(180, 50, 255),
            &['✦', '✧', '░', '▓', '¤'],
        ));
        vfx.shakes
            .push(verryte_terminal::vfx::ScreenShake::new_eased(
                4.0,
                0.8,
                verryte_terminal::vfx::EasingMode::ExpoOut,
            ));
        vfx.flashes
            .push(verryte_terminal::vfx::Flash::full_screen_eased(
                Color(255, 255, 255),
                0.4,
                verryte_terminal::EasingMode::ExpoOut,
            ));
    }

    let enemy_exists = world
        .query::<Team>()
        .into_iter()
        .any(|(_, team)| *team == Team::Enemy);
    let echo_exists = world.query::<EchoItem>().into_iter().next().is_some();
    if !enemy_exists && !echo_exists {
        world
            .resource_mut::<GameState>()
            .expect("GameState resource must be registered")
            .outcome = Outcome::Victory;
        log(
            world,
            "[fg:32CD32][b]Victory![/] All enemies defeated.[/fg]",
        );
    }
}

pub fn is_occupied_except(world: &World, pos: Position, except: Entity) -> bool {
    for (e, p) in world.query::<Position>() {
        if e != except && *p == pos && world.get::<Team>(e).is_some() {
            return true;
        }
    }
    false
}

pub fn apply_spread_status(world: &mut World, target: Entity, new_status: ElementalStatus) {
    if !world.is_alive(target) {
        return;
    }
    let old_status = world
        .get::<ElementalStatus>(target)
        .copied()
        .unwrap_or(ElementalStatus::None);
    if old_status == new_status {
        let old_dur = match old_status {
            ElementalStatus::Ice { duration } => duration,
            ElementalStatus::Lightning { duration } => duration,
            ElementalStatus::Nature { duration } => duration,
            ElementalStatus::Poison { duration } => duration,
            ElementalStatus::Regen { duration } => duration,
            _ => 0,
        };
        let new_dur = match new_status {
            ElementalStatus::Ice { duration } => duration,
            ElementalStatus::Lightning { duration } => duration,
            ElementalStatus::Nature { duration } => duration,
            ElementalStatus::Poison { duration } => duration,
            ElementalStatus::Regen { duration } => duration,
            _ => 0,
        };
        if new_dur > old_dur {
            world.insert(target, new_status);
        }
        return;
    }

    let target_class = world
        .get::<CharacterClass>(target)
        .copied()
        .unwrap_or(CharacterClass::Warrior);
    let target_name = crate::game::Game::get_class_name(target_class);
    let target_pos = world
        .get::<Position>(target)
        .copied()
        .unwrap_or(Position::new(0, 0));

    match (old_status, new_status) {
        // Shatter: Ice + Lightning
        (ElementalStatus::Ice { .. }, ElementalStatus::Lightning { .. })
        | (ElementalStatus::Lightning { .. }, ElementalStatus::Ice { .. }) => {
            log(
                world,
                format!(
                    "[fg:64C8FF][b]Elemental Reaction: SHATTER[/] on {} via spread![/fg]",
                    target_name
                ),
            );
            let bonus_damage = 30;
            let mut defeated = false;
            if let Some(stats) = world.get_mut::<Stats>(target) {
                stats.hp -= bonus_damage;
                if stats.hp <= 0 {
                    defeated = true;
                }
            }
            let (tx, ty) = get_tile_center_pixels(world, target_pos);
            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        tx,
                        ty - 1.0,
                        &format!("SHATTER! -{}", bonus_damage),
                        Color(100, 200, 255),
                        true,
                    ));
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_shatter(tx, ty, 20));
            }
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ReactionTriggered {
                    entity: target,
                    reaction: "Shatter".to_owned(),
                    damage: bonus_damage,
                    healing: 0,
                });
            }
            world.insert(target, ElementalStatus::None);
            if defeated {
                handle_defeat(world, target, target_name, target_class, target_pos);
            }
        }
        // Overgrowth: Lightning + Nature
        (ElementalStatus::Lightning { .. }, ElementalStatus::Nature { .. })
        | (ElementalStatus::Nature { .. }, ElementalStatus::Lightning { .. }) => {
            log(
                world,
                format!(
                    "[fg:32DC64][b]Elemental Reaction: OVERGROWTH[/] on {} via spread![/fg]",
                    target_name
                ),
            );
            let bonus_damage = 10;
            let mut defeated = false;
            if let Some(stats) = world.get_mut::<Stats>(target) {
                stats.hp -= bonus_damage;
                if stats.hp <= 0 {
                    defeated = true;
                }
            }
            world.insert(target, Rooted { duration: 1 });
            let (tx, ty) = get_tile_center_pixels(world, target_pos);
            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        tx,
                        ty - 1.0,
                        &format!("OVERGROWTH! -{} [ROOTED]", bonus_damage),
                        Color(50, 220, 100),
                        true,
                    ));
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_bloom(tx, ty, 10));
            }
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ReactionTriggered {
                    entity: target,
                    reaction: "Overgrowth".to_owned(),
                    damage: bonus_damage,
                    healing: 0,
                });
            }
            world.insert(target, ElementalStatus::None);
            if defeated {
                handle_defeat(world, target, target_name, target_class, target_pos);
            }
        }
        // Bloom: Nature + Ice
        (ElementalStatus::Nature { .. }, ElementalStatus::Ice { .. })
        | (ElementalStatus::Ice { .. }, ElementalStatus::Nature { .. }) => {
            log(
                world,
                format!(
                    "[fg:FFD700][b]Elemental Reaction: BLOOM[/] on {} via spread![/fg]",
                    target_name
                ),
            );
            let healing_amount = 20;
            let mut allies = Vec::new();
            let target_team = world.get::<Team>(target).copied().unwrap_or(Team::Player);
            for (e, p, team) in world.query2::<Position, Team>() {
                if *team == target_team {
                    let dist = (p.x - target_pos.x).abs() + (p.y - target_pos.y).abs();
                    if dist <= 1 {
                        allies.push(e);
                    }
                }
            }
            for ally in allies {
                let a_class = world
                    .get::<CharacterClass>(ally)
                    .copied()
                    .unwrap_or(CharacterClass::Warrior);
                let a_pos = world
                    .get::<Position>(ally)
                    .copied()
                    .unwrap_or(Position::new(0, 0));
                let mut final_hp = 0;
                if let Some(stats) = world.get_mut::<Stats>(ally) {
                    stats.hp = std::cmp::min(stats.max_hp, stats.hp + healing_amount);
                    final_hp = stats.hp;
                }
                log(
                    world,
                    format!(
                        "[fg:32FF32]Bloom healed {} for [b]{} HP![/] (HP: {})[/fg]",
                        crate::game::Game::get_class_name(a_class),
                        healing_amount,
                        final_hp
                    ),
                );
                let (ax, ay) = get_tile_center_pixels(world, a_pos);
                if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                    vfx.floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            ax,
                            ay - 2.0,
                            &format!("+{} (Bloom)", healing_amount),
                            Color(50, 255, 50),
                            true,
                        ));
                    vfx.particles
                        .extend(verryte_terminal::vfx::emit_bloom(ax, ay, 8));
                }
            }
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ReactionTriggered {
                    entity: target,
                    reaction: "Bloom".to_owned(),
                    damage: 0,
                    healing: healing_amount,
                });
            }
            world.insert(target, ElementalStatus::None);
        }
        _ => {
            world.insert(target, new_status);
            let badge = match new_status {
                ElementalStatus::Ice { .. } => "Ice",
                ElementalStatus::Lightning { .. } => "Lightning",
                ElementalStatus::Nature { .. } => "Nature",
                ElementalStatus::Poison { .. } => "Poison",
                ElementalStatus::Regen { .. } => "Regen",
                _ => "None",
            };
            let color_hex = match badge {
                "Ice" => "64C8FF",
                "Lightning" => "FFFF64",
                "Nature" => "32DC64",
                "Poison" => "A020F0",
                "Regen" => "32CD32",
                _ => "FFFFFF",
            };
            log(
                world,
                format!(
                    "Applied [fg:{}][b]{}[/] element to {} via spread.",
                    color_hex, badge, target_name
                ),
            );
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ElementalApplied {
                    entity: target,
                    status: new_status,
                });
            }
        }
    }
}

pub fn process_team_status_effects(world: &mut World, team: Team) {
    let mut targets = Vec::new();
    for (e, pos, t, class) in world.query3::<Position, Team, CharacterClass>() {
        if *t == team {
            if let Some(status) = world.get::<ElementalStatus>(e).copied() {
                if status != ElementalStatus::None {
                    targets.push((e, status, *pos, *class));
                }
            }
        }
    }

    for (entity, original_status, pos, class) in targets {
        if !world.is_alive(entity) {
            continue;
        }

        let mut current_hp = world.get::<Stats>(entity).map(|s| s.hp).unwrap_or(0);
        if current_hp <= 0 {
            continue;
        }

        let target_name = crate::game::Game::get_class_name(class);
        let mut defeated = false;
        let mut next_status = ElementalStatus::None;

        match original_status {
            ElementalStatus::Poison { duration } => {
                if let Some(stats) = world.get_mut::<Stats>(entity) {
                    stats.hp = (stats.hp - 10).max(0);
                    current_hp = stats.hp;
                    if stats.hp <= 0 {
                        defeated = true;
                    }
                }
                log(
                    world,
                    format!(
                        "{} suffered [fg:A020F0][b]10 Poison damage![/] (HP: {})",
                        target_name, current_hp
                    ),
                );
                let (tx, ty) = get_tile_center_pixels(world, pos);
                if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                    vfx.floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            tx,
                            ty - 1.0,
                            "-10 (Poison)",
                            Color(160, 32, 240),
                            true,
                        ));
                    vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                        tx,
                        ty,
                        8,
                        Color(160, 32, 240),
                        &['*'],
                    ));
                }
                next_status = if duration > 1 && !defeated {
                    ElementalStatus::Poison {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Regen { duration } => {
                if let Some(stats) = world.get_mut::<Stats>(entity) {
                    stats.hp = (stats.hp + 10).min(stats.max_hp);
                    current_hp = stats.hp;
                }
                log(
                    world,
                    format!(
                        "{} healed for [fg:32CD32][b]10 HP[/] via Regen. (HP: {})",
                        target_name, current_hp
                    ),
                );
                let (tx, ty) = get_tile_center_pixels(world, pos);
                if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                    vfx.floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            tx,
                            ty - 1.0,
                            "+10 (Regen)",
                            Color(50, 205, 50),
                            true,
                        ));
                    vfx.particles
                        .extend(verryte_terminal::vfx::emit_bloom(tx, ty, 8));
                }
                next_status = if duration > 1 {
                    ElementalStatus::Regen {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Ice { duration } => {
                next_status = if duration > 1 {
                    ElementalStatus::Ice {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Lightning { duration } => {
                next_status = if duration > 1 {
                    ElementalStatus::Lightning {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Nature { duration } => {
                next_status = if duration > 1 {
                    ElementalStatus::Nature {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::None => {}
        }

        if !defeated {
            if let Some(status) = world.get_mut::<ElementalStatus>(entity) {
                *status = next_status;
            }
        } else {
            handle_defeat(world, entity, target_name, class, pos);
            continue;
        }

        let spread_status_opt = match original_status {
            ElementalStatus::Poison { duration } => Some(ElementalStatus::Poison {
                duration: duration.saturating_sub(1).max(1),
            }),
            ElementalStatus::Ice { duration } => Some(ElementalStatus::Ice {
                duration: duration.saturating_sub(1).max(1),
            }),
            ElementalStatus::Lightning { duration } => Some(ElementalStatus::Lightning {
                duration: duration.saturating_sub(1).max(1),
            }),
            ElementalStatus::Nature { duration } => Some(ElementalStatus::Nature {
                duration: duration.saturating_sub(1).max(1),
            }),
            _ => None,
        };

        if let Some(spread_status) = spread_status_opt {
            let mut adj_entities = Vec::new();
            for (other_e, other_pos) in world.query::<Position>() {
                if other_e != entity {
                    let dist = (pos.x - other_pos.x).abs() + (pos.y - other_pos.y).abs();
                    if dist == 1 {
                        if world.get::<Team>(other_e).is_some() {
                            adj_entities.push(other_e);
                        }
                    }
                }
            }

            for adj_e in adj_entities {
                apply_spread_status(world, adj_e, spread_status);
            }
        }
    }
}

pub fn weather_ambient_system(world: &mut World) {
    use crate::components::{Weather, WeatherType};

    let current = world
        .resource::<Weather>()
        .map(|w| w.current)
        .unwrap_or(WeatherType::Sunny);

    let ambient_name = match current {
        WeatherType::Rainy => "ambient_rain",
        WeatherType::LightningStorm => "ambient_thunder",
        WeatherType::Sunny => "ambient_birds",
        WeatherType::Snowing => "ambient_wind",
    };
    let volume: f32 = match current {
        WeatherType::Rainy => 0.4,
        WeatherType::LightningStorm => 0.5,
        WeatherType::Sunny => 0.3,
        WeatherType::Snowing => 0.35,
    };

    if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
        events.send(verryte_core::AudioEvent::loop_music(ambient_name).with_volume(volume));
    }
}

pub const WEATHER_CYCLE_TURNS: u32 = 3;

pub fn weather_cycle_system(world: &mut World) {
    let current = world
        .resource::<Weather>()
        .map(|w| w.current)
        .unwrap_or(WeatherType::Sunny);

    apply_per_turn_weather_effects(world, current);

    let turn = world.resource::<GameState>().map(|s| s.turn).unwrap_or(1);
    if turn == 1 || turn % WEATHER_CYCLE_TURNS != 1 {
        return;
    }
    let next = match current {
        WeatherType::Sunny => WeatherType::Rainy,
        WeatherType::Rainy => WeatherType::LightningStorm,
        WeatherType::LightningStorm => WeatherType::Snowing,
        WeatherType::Snowing => WeatherType::Sunny,
    };
    if let Some(weather) = world.resource_mut::<Weather>() {
        weather.current = next;
        weather.danger_zones.clear();
    }
    log(
        world,
        format!("[fg:87CEEB][b]Weather changed to {:?}![/][/fg]", next),
    );
}

fn apply_per_turn_weather_effects(world: &mut World, weather: WeatherType) {
    match weather {
        WeatherType::LightningStorm => {
            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones.clear();
            }

            let map_w;
            let map_h;
            {
                let map = world
                    .resource::<TacticalMap>()
                    .expect("TacticalMap must be registered");
                map_w = map.width as i16;
                map_h = map.height as i16;
            }

            let count = {
                let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
                1 + rng.next_u32(2) as usize
            };

            let mut zones = Vec::new();
            for _ in 0..count {
                let (x, y) = {
                    let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
                    let x = rng.next_u32(map_w.max(1) as u32) as i16;
                    let y = rng.next_u32(map_h.max(1) as u32) as i16;
                    (x, y)
                };
                let pos = Position::new(x, y);
                let walkable = world
                    .resource::<TacticalMap>()
                    .map(|m| m.is_walkable(pos))
                    .unwrap_or(false);
                if walkable {
                    zones.push(pos);
                }
            }

            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones = zones.clone();
            }

            for zone in &zones {
                let mut victims = Vec::new();
                for (e, p, team) in world.query2::<Position, Team>() {
                    if *p == *zone {
                        victims.push((e, *team));
                    }
                }
                for (victim, _team) in victims {
                    let victim_class = world
                        .get::<CharacterClass>(victim)
                        .copied()
                        .unwrap_or(CharacterClass::Warrior);
                    let victim_name = Game::get_class_name(victim_class);
                    let mut final_hp = 0;
                    let mut defeated = false;
                    if let Some(stats) = world.get_mut::<Stats>(victim) {
                        stats.hp -= 15;
                        final_hp = stats.hp;
                        if stats.hp <= 0 {
                            defeated = true;
                        }
                    }
                    log(
                        world,
                        format!(
                            "[fg:FFFF64]Lightning struck {} at ({}, {}) for 15 damage! (HP: {})[/fg]",
                            victim_name, zone.x, zone.y, final_hp
                        ),
                    );
                    if defeated {
                        let name_str = victim_name.to_string();
                        handle_defeat(world, victim, &name_str, victim_class, *zone);
                    }
                }
            }
        }
        WeatherType::Snowing => {
            let mut targets = Vec::new();
            for (e, class) in world.query::<CharacterClass>() {
                if let Some(status) = world.get::<ElementalStatus>(e) {
                    if matches!(status, ElementalStatus::Ice { .. }) {
                        targets.push((e, *class));
                    }
                }
            }
            for (entity, class) in targets {
                let mut log_msg = None;
                if let Some(status) = world.get_mut::<ElementalStatus>(entity) {
                    if let ElementalStatus::Ice { ref mut duration } = *status {
                        *duration += 1;
                        let new_dur = *duration;
                        let name = Game::get_class_name(class);
                        log_msg = Some(format!(
                            "[fg:64C8FF]Snowing extends Ice on {} by 1 turn (now {}).[/fg]",
                            name, new_dur
                        ));
                    }
                }
                if let Some(msg) = log_msg {
                    log(world, msg);
                }
            }
        }
        _ => {}
    }
}

pub fn is_flanking_position(world: &World, attacker_pos: Position, target_pos: Position) -> bool {
    let dist = (attacker_pos.x - target_pos.x).abs() + (attacker_pos.y - target_pos.y).abs();
    if dist != 1 {
        return false;
    }
    let dx = attacker_pos.x - target_pos.x;
    let dy = attacker_pos.y - target_pos.y;
    for (_other_e, other_pos, team) in world.query2::<Position, Team>() {
        if *team == Team::Enemy {
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
