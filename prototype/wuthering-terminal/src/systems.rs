use crate::components::{
    BossPhase, CharacterClass, EchoItem, ElementalShield, ElementalStatus, GameEvent, GameState,
    Outcome, Position, Rooted, ShieldType, Stats, Team, TelegraphZone, TurnPhase,
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

            let mut nearest_player: Option<(Entity, Position, Stats, CharacterClass)> = None;
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
                        let healer_bonus = if *class == CharacterClass::Healer {
                            -30
                        } else {
                            0
                        };
                        let score = hp_pct + dist as i32 + healer_bonus;
                        if score < best_score {
                            best_score = score;
                            nearest_player = Some((pe, *p, stats.clone(), *class));
                        }
                    }
                }
            }

            let Some((player_entity, player_pos, player_stats, player_class)) = nearest_player
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
                            let tile = map.tile(neighbor.x, neighbor.y);
                            let passable = matches!(tile, Tile::Grass | Tile::Water | Tile::Lava)
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
                    let base_damage = std::cmp::max(1, enemy_stats.atk - player_stats.def);
                    let enemy_name = Game::get_class_name(enemy_class);
                    let player_name = Game::get_class_name(player_class);

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

                let is_low_hp = (enemy_stats.hp as f32 / enemy_stats.max_hp as f32) < 0.3;
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
                                let tile = map.tile(neighbor.x, neighbor.y);
                                let passable =
                                    matches!(
                                        tile,
                                        Tile::Grass | Tile::Water | Tile::Lava | Tile::Ice
                                    ) && !is_occupied_except(world, neighbor, enemy_entity);
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
                        // Generate Dijkstra map towards all players using weighted movement costs
                        let d_map = verryte_map::DijkstraMap::compute_weighted(
                            map.width,
                            map.height,
                            &player_positions,
                            |pt| {
                                if pt.x < 0
                                    || pt.x >= map.width as i16
                                    || pt.y < 0
                                    || pt.y >= map.height as i16
                                {
                                    return false;
                                }
                                let tile = map.tiles.get(pt).expect("point passed bounds check");
                                matches!(tile, Tile::Grass | Tile::Water | Tile::Lava | Tile::Ice)
                                    && !is_occupied_except(world, pt, enemy_entity)
                            },
                            |_, to| map.movement_cost(to) as u32,
                            false, // 4-way movement
                        );

                        // Find neighbor with lowest distance
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
                            let dest_tile = map.tile(target_tile.x, target_tile.y);
                            (Some(target_tile), move_cost, dest_tile)
                        } else {
                            (None, 0, Tile::Grass)
                        }
                    }
                };

                let mut final_dest = target_tile;
                let mut final_dest_tile = dest_tile;
                if let Some(target_tile_pos) = target_tile {
                    let map = world
                        .resource::<TacticalMap>()
                        .expect("TacticalMap must be registered");
                    if dest_tile == Tile::Ice {
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
            log(world, "[fg:FFA500][b]Enemy Phase starts![/][/fg]");
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::PhaseChanged(TurnPhase::Enemy));
                events.send(GameEvent::TurnEnded);
            }

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

            // Decrement elemental statuses
            let mut status_entities = Vec::new();
            for (e, _status) in world.query::<ElementalStatus>() {
                status_entities.push(e);
            }
            for e in status_entities {
                if let Some(status) = world.get_mut::<ElementalStatus>(e) {
                    *status = match *status {
                        ElementalStatus::Ice { duration } if duration > 1 => ElementalStatus::Ice {
                            duration: duration - 1,
                        },
                        ElementalStatus::Lightning { duration } if duration > 1 => {
                            ElementalStatus::Lightning {
                                duration: duration - 1,
                            }
                        }
                        ElementalStatus::Nature { duration } if duration > 1 => {
                            ElementalStatus::Nature {
                                duration: duration - 1,
                            }
                        }
                        _ => ElementalStatus::None,
                    };
                }
            }

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
                    if let Some(stats) = world.get_mut::<Stats>(e) {
                        let mut bonus = 0;
                        if has_swift {
                            bonus = 1;
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

                if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
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

pub fn resolve_combat_hit(
    world: &mut World,
    target: Entity,
    base_damage: i32,
    attacker_name: &str,
    target_name: &str,
    pos: Position,
) -> (i32, bool) {
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
