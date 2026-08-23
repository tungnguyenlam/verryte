#[cfg(test)]
use crate::components::AIBehavior;
use crate::components::{AIAction, AIArchetype, Position, Stats, TacticalAssessment, Team};
use crate::map::{TacticalMap, Tile};
use verryte_core::{Entity, World};

pub struct TacticalAI;

impl TacticalAI {
    pub fn assess_battlefield(
        world: &World,
        map: &TacticalMap,
        enemy: Entity,
    ) -> TacticalAssessment {
        let enemy_pos = *world.get::<Position>(enemy).unwrap_or(&Position::new(0, 0));

        let mut players = Vec::new();
        let mut allies = Vec::new();
        for (e, pos, team) in world.query2::<Position, Team>() {
            if *team == Team::Player {
                if let Some(stats) = world.get::<Stats>(e) {
                    players.push((e, *pos, stats.clone()));
                }
            } else if *team == Team::Enemy && e != enemy {
                if let Some(stats) = world.get::<Stats>(e) {
                    allies.push((e, *pos, stats.clone()));
                }
            }
        }

        let ally_positions: Vec<Position> = allies.iter().map(|(_, p, _)| *p).collect();
        let player_positions: Vec<Position> = players.iter().map(|(_, p, _)| *p).collect();

        let threat_map = Self::calculate_threat_map(map, &players);
        let cover_positions = Self::find_cover_positions(map, &ally_positions, &player_positions);
        let flank_positions = if let Some((_, target_pos, _)) = players.first() {
            Self::find_flank_positions(enemy_pos, &ally_positions, *target_pos)
        } else {
            Vec::new()
        };
        let safe_positions = Self::find_safe_positions(map, enemy_pos, &threat_map);

        TacticalAssessment {
            threat_map,
            cover_positions,
            flank_positions,
            safe_positions,
        }
    }

    pub fn find_cover_positions(
        map: &TacticalMap,
        _enemies: &[Position],
        players: &[Position],
    ) -> Vec<Position> {
        let mut covers = Vec::new();
        let w = map.width as i16;
        let h = map.height as i16;

        for y in 0..h {
            for x in 0..w {
                let pos = Position::new(x, y);
                if !map.is_walkable(pos) {
                    continue;
                }

                let mut adjacent_wall = false;
                for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h && map.tile(nx, ny) == Tile::Wall {
                        adjacent_wall = true;
                        break;
                    }
                }

                if !adjacent_wall {
                    continue;
                }

                let blocks_some_los = players.iter().any(|pp| {
                    let dx = (pp.x - x).abs();
                    let dy = (pp.y - y).abs();
                    let in_range = dx + dy <= 4;
                    in_range && Self::is_covered(pos, map, *pp)
                });

                if blocks_some_los {
                    covers.push(pos);
                }
            }
        }

        covers
    }

    pub fn find_flank_positions(
        enemy_pos: Position,
        allies: &[Position],
        target: Position,
    ) -> Vec<Position> {
        let mut flanks = Vec::new();
        let ally_opposite: Vec<Position> = allies
            .iter()
            .filter_map(|a| {
                let dx = a.x - target.x;
                let dy = a.y - target.y;
                if dx.abs() + dy.abs() == 1 {
                    let opp = Position::new(target.x - dx, target.y - dy);
                    if opp != enemy_pos {
                        Some(opp)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        for pos in ally_opposite {
            let dist = (pos.x - enemy_pos.x).abs() + (pos.y - enemy_pos.y).abs();
            if dist <= 3 && !flanks.contains(&pos) {
                flanks.push(pos);
            }
        }

        for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
            let adj = Position::new(target.x + dx, target.y + dy);
            if adj == enemy_pos {
                continue;
            }
            let is_perpendicular = allies.iter().any(|a| {
                let ax = a.x - target.x;
                let ay = a.y - target.y;
                let adist = ax.abs() + ay.abs();
                if adist != 1 {
                    return false;
                }
                (dx != &0 && ay != 0) || (dy != &0 && ax != 0)
            });
            if is_perpendicular && !flanks.contains(&adj) {
                flanks.push(adj);
            }
        }

        flanks
    }

    pub fn calculate_threat_map(
        map: &TacticalMap,
        players: &[(Entity, Position, Stats)],
    ) -> Vec<(Position, i32)> {
        let w = map.width as i16;
        let h = map.height as i16;
        let mut threat = Vec::with_capacity((w as usize) * (h as usize));

        for y in 0..h {
            for x in 0..w {
                let pos = Position::new(x, y);
                let mut total_threat = 0i32;

                for (_, ppos, stats) in players {
                    let dist = ((ppos.x - x).abs() + (ppos.y - y).abs()) as i32;
                    if dist == 0 {
                        total_threat += stats.atk * 3;
                    } else if dist <= 2 {
                        total_threat += stats.atk * 2 / dist.max(1);
                    } else if dist <= 5 {
                        total_threat += stats.atk / dist.max(1);
                    }
                }

                if map.tile(x, y) == Tile::Lava {
                    total_threat += 20;
                }

                threat.push((pos, total_threat));
            }
        }

        threat
    }

    pub fn decide_action(
        world: &World,
        _map: &TacticalMap,
        enemy: Entity,
        assessment: &TacticalAssessment,
    ) -> AIAction {
        let archetype = world
            .get::<AIArchetype>(enemy)
            .copied()
            .unwrap_or(AIArchetype::Chaser);
        let enemy_pos = *world.get::<Position>(enemy).unwrap_or(&Position::new(0, 0));
        let stats = world.get::<Stats>(enemy).cloned().unwrap_or(Stats {
            hp: 1,
            max_hp: 1,
            atk: 1,
            def: 0,
            spd: 1,
            ap: 0,
            max_ap: 1,
            level: 1,
            xp: 0,
        });

        let mut players = Vec::new();
        for (e, pos, team) in world.query2::<Position, Team>() {
            if *team == Team::Player {
                if let Some(s) = world.get::<Stats>(e) {
                    players.push((e, *pos, s.clone()));
                }
            }
        }

        let mut allies = Vec::new();
        for (e, pos, team) in world.query2::<Position, Team>() {
            if *team == Team::Enemy && e != enemy {
                if let Some(s) = world.get::<Stats>(e) {
                    allies.push((e, *pos, s.clone()));
                }
            }
        }

        match archetype {
            AIArchetype::Chaser => Self::chaser_strategy(enemy_pos, &stats, &players, assessment),
            AIArchetype::Cleric => Self::cleric_strategy(enemy_pos, &stats, &allies, &players),
            AIArchetype::Coward => Self::coward_strategy(enemy_pos, &stats, &players, assessment),
            AIArchetype::Berserker => {
                Self::chaser_strategy(enemy_pos, &stats, &players, assessment)
            }
            AIArchetype::Tactician => {
                Self::chaser_strategy(enemy_pos, &stats, &players, assessment)
            }
            AIArchetype::Summoner => Self::cleric_strategy(enemy_pos, &stats, &allies, &players),
            AIArchetype::Assassin => Self::chaser_strategy(enemy_pos, &stats, &players, assessment),
            AIArchetype::Defender => Self::defender_strategy(enemy_pos, &stats, &allies, &players),
        }
    }

    pub fn chaser_strategy(
        enemy_pos: Position,
        stats: &Stats,
        players: &[(Entity, Position, Stats)],
        assessment: &TacticalAssessment,
    ) -> AIAction {
        if players.is_empty() {
            return AIAction::Defend;
        }

        let hp_pct = if stats.max_hp > 0 {
            stats.hp as f32 / stats.max_hp as f32
        } else {
            0.0
        };

        if hp_pct < 0.2 {
            let best_safe = assessment
                .safe_positions
                .iter()
                .map(|p| {
                    let threat = assessment
                        .threat_map
                        .iter()
                        .find(|(tp, _)| *tp == *p)
                        .map(|(_, t)| *t)
                        .unwrap_or(0);
                    (*p, threat)
                })
                .min_by_key(|(_, t)| *t);
            if let Some((safe_pos, _)) = best_safe {
                return AIAction::MoveTo(safe_pos);
            }
            return AIAction::Retreat;
        }

        let best_flank = assessment.flank_positions.iter().find_map(|fp| {
            players
                .iter()
                .find(|(_, pp, _)| {
                    let d = (pp.x - fp.x).abs() + (pp.y - fp.y).abs();
                    d <= 2
                })
                .map(|(e, _, _)| (*e, *fp))
        });
        if let Some((target_ent, flank_pos)) = best_flank {
            let dist = (enemy_pos.x - flank_pos.x).abs() + (enemy_pos.y - flank_pos.y).abs();
            if dist <= 2 {
                return AIAction::FlankAttack(target_ent, flank_pos);
            }
        }

        let focus = Self::focus_fire_target(players);
        if let Some((target_ent, target_pos)) = focus {
            let dist = (enemy_pos.x - target_pos.x).abs() + (enemy_pos.y - target_pos.y).abs();
            if dist <= 2 {
                return AIAction::Attack(target_ent);
            }
            return AIAction::MoveTo(target_pos);
        }

        AIAction::Defend
    }

    pub fn defender_strategy(
        enemy_pos: Position,
        stats: &Stats,
        allies: &[(Entity, Position, Stats)],
        players: &[(Entity, Position, Stats)],
    ) -> AIAction {
        if allies.is_empty() {
            return Self::chaser_strategy(
                enemy_pos,
                stats,
                players,
                &TacticalAssessment::default(),
            );
        }

        // 1. Find the highest priority ally to protect
        let priority_ally = allies.iter().min_by_key(|(_, _, s)| {
            // Heuristically: Boss is priority 0, then based on HP pct
            // We don't have CharacterClass here easily without a query,
            // but we can assume lower max_hp might be more fragile or
            // just use a placeholder for now since we can't easily query
            // character class from just Stats.
            // Actually, let's just pick the one with lowest HP percentage.
            if s.max_hp > 0 {
                (s.hp * 100) / s.max_hp
            } else {
                100
            }
        });

        if let Some((_ally_ent, ally_pos, _)) = priority_ally {
            let dist_to_ally = (enemy_pos.x - ally_pos.x).abs() + (enemy_pos.y - ally_pos.y).abs();

            if dist_to_ally > 1 {
                // Move towards ally
                return AIAction::MoveTo(*ally_pos);
            } else {
                // We are near the ally, look for nearby enemies to attack
                let nearest_player = players
                    .iter()
                    .min_by_key(|(_, pp, _)| (pp.x - ally_pos.x).abs() + (pp.y - ally_pos.y).abs());

                if let Some((player_ent, player_pos, _)) = nearest_player {
                    let dist_player_to_ally =
                        (player_pos.x - ally_pos.x).abs() + (player_pos.y - ally_pos.y).abs();
                    if dist_player_to_ally <= 3 {
                        // Player is threatening the ally
                        let dist_to_player =
                            (enemy_pos.x - player_pos.x).abs() + (enemy_pos.y - player_pos.y).abs();
                        if dist_to_player <= 2 {
                            return AIAction::Attack(*player_ent);
                        } else {
                            return AIAction::MoveTo(*player_pos);
                        }
                    }
                }
            }
        }

        AIAction::Defend
    }

    pub fn cleric_strategy(
        enemy_pos: Position,
        _stats: &Stats,
        allies: &[(Entity, Position, Stats)],
        players: &[(Entity, Position, Stats)],
    ) -> AIAction {
        let mut needy: Option<(Entity, Position, f32)> = None;
        let mut lowest_pct = 1.0f32;
        for &(ae, apos, ref astats) in allies {
            let pct = if astats.max_hp > 0 {
                astats.hp as f32 / astats.max_hp as f32
            } else {
                0.0
            };
            if pct < 0.5 && pct < lowest_pct {
                lowest_pct = pct;
                needy = Some((ae, apos, pct));
            }
        }

        if let Some((ally_ent, ally_pos, _)) = needy {
            let dist = (enemy_pos.x - ally_pos.x).abs() + (enemy_pos.y - ally_pos.y).abs();
            if dist <= 3 {
                return AIAction::HealAlly(ally_ent);
            }
            return AIAction::MoveTo(ally_pos);
        }

        if !players.is_empty() {
            let focus = Self::focus_fire_target(players);
            if let Some((target_ent, target_pos)) = focus {
                let dist = (enemy_pos.x - target_pos.x).abs() + (enemy_pos.y - target_pos.y).abs();
                if dist <= 2 {
                    return AIAction::Attack(target_ent);
                }
                return AIAction::MoveTo(target_pos);
            }
        }

        AIAction::Defend
    }

    pub fn coward_strategy(
        enemy_pos: Position,
        stats: &Stats,
        players: &[(Entity, Position, Stats)],
        assessment: &TacticalAssessment,
    ) -> AIAction {
        let hp_pct = if stats.max_hp > 0 {
            stats.hp as f32 / stats.max_hp as f32
        } else {
            0.0
        };

        if hp_pct < 0.3 {
            if let Some(cover) = assessment.cover_positions.first() {
                return AIAction::MoveTo(*cover);
            }
            return AIAction::Retreat;
        }

        if hp_pct >= 0.3 {
            let focus = Self::focus_fire_target(players);
            if let Some((target_ent, target_pos)) = focus {
                let dist = (enemy_pos.x - target_pos.x).abs() + (enemy_pos.y - target_pos.y).abs();
                let is_safe = assessment
                    .threat_map
                    .iter()
                    .find(|(p, _)| *p == enemy_pos)
                    .map(|(_, t)| *t < 10)
                    .unwrap_or(true);
                if dist <= 2 && is_safe {
                    return AIAction::Attack(target_ent);
                }
                let best_safe = assessment
                    .safe_positions
                    .iter()
                    .map(|p| {
                        let threat = assessment
                            .threat_map
                            .iter()
                            .find(|(tp, _)| *tp == *p)
                            .map(|(_, t)| *t)
                            .unwrap_or(0);
                        (*p, threat)
                    })
                    .min_by_key(|(_, t)| *t);
                if let Some((safe_pos, _)) = best_safe {
                    return AIAction::MoveTo(safe_pos);
                }
            }
        }

        AIAction::Defend
    }

    pub fn is_covered(pos: Position, map: &TacticalMap, threat_pos: Position) -> bool {
        let dx = threat_pos.x - pos.x;
        let dy = threat_pos.y - pos.y;
        let steps = dx.abs().max(dy.abs());
        if steps == 0 {
            return false;
        }

        let mut x = pos.x as f32;
        let mut y = pos.y as f32;
        let sx = dx as f32 / steps as f32;
        let sy = dy as f32 / steps as f32;

        for _ in 0..steps {
            x += sx;
            y += sy;
            let tile_x = x.round() as i16;
            let tile_y = y.round() as i16;
            if tile_x == pos.x && tile_y == pos.y {
                continue;
            }
            if tile_x == threat_pos.x && tile_y == threat_pos.y {
                return false;
            }
            if map.tile(tile_x, tile_y) == Tile::Wall {
                return true;
            }
        }
        false
    }

    pub fn safest_move_target(
        pos: Position,
        ap: i32,
        map: &TacticalMap,
        assessment: &TacticalAssessment,
    ) -> Position {
        let mut best = pos;
        let mut best_threat = assessment
            .threat_map
            .iter()
            .find(|(p, _)| *p == pos)
            .map(|(_, t)| *t)
            .unwrap_or(0);

        for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
            let nx = pos.x + dx;
            let ny = pos.y + dy;
            let np = Position::new(nx, ny);
            if !map.is_walkable(np) {
                continue;
            }
            let cost = map.movement_cost(np);
            if cost > ap {
                continue;
            }
            let threat = assessment
                .threat_map
                .iter()
                .find(|(p, _)| *p == np)
                .map(|(_, t)| *t)
                .unwrap_or(0);
            if threat < best_threat {
                best_threat = threat;
                best = np;
            }
        }

        best
    }

    pub fn focus_fire_target(players: &[(Entity, Position, Stats)]) -> Option<(Entity, Position)> {
        players
            .iter()
            .min_by_key(|(_, _, s)| s.hp)
            .map(|(e, p, _)| (*e, *p))
    }

    fn find_safe_positions(
        map: &TacticalMap,
        pos: Position,
        threat_map: &[(Position, i32)],
    ) -> Vec<Position> {
        let mut safe = Vec::new();
        for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
            let np = Position::new(pos.x + dx, pos.y + dy);
            if !map.is_walkable(np) {
                continue;
            }
            let threat = threat_map
                .iter()
                .find(|(p, _)| *p == np)
                .map(|(_, t)| *t)
                .unwrap_or(0);
            let current_threat = threat_map
                .iter()
                .find(|(p, _)| *p == pos)
                .map(|(_, t)| *t)
                .unwrap_or(0);
            if threat < current_threat {
                safe.push(np);
            }
        }
        safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CharacterClass;

    fn make_test_map() -> TacticalMap {
        TacticalMap::from_ascii(
            "\
....#....\n\
....#....\n\
.........\n\
.........\n\
.........",
        )
    }

    fn spawn_test_entity(
        world: &mut World,
        pos: Position,
        team: Team,
        hp: i32,
        max_hp: i32,
        atk: i32,
    ) -> Entity {
        world
            .builder()
            .with(pos)
            .with(team)
            .with(CharacterClass::ShadowStalker)
            .with(Stats {
                hp,
                max_hp,
                atk,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .build()
    }

    #[test]
    fn test_cover_detection() {
        let map = TacticalMap::from_ascii(
            "\
.....\n\
..#..\n\
.....\n\
.....",
        );
        let threat = Position::new(0, 1);
        let behind_wall = Position::new(3, 1);
        let covered = TacticalAI::is_covered(behind_wall, &map, threat);
        assert!(
            covered,
            "Position behind wall should be covered from threat"
        );
    }

    #[test]
    fn test_cover_detection_no_wall() {
        let map = TacticalMap::from_ascii(
            "\
.....\n\
.....\n\
.....\n\
.....",
        );
        let threat = Position::new(0, 1);
        let pos = Position::new(3, 1);
        let covered = TacticalAI::is_covered(pos, &map, threat);
        assert!(!covered, "Position without wall should not be covered");
    }

    #[test]
    fn test_flanking_calculation() {
        let enemy_pos = Position::new(2, 2);
        let target = Position::new(3, 2);
        let allies = vec![Position::new(3, 1)];

        let flanks = TacticalAI::find_flank_positions(enemy_pos, &allies, target);
        assert!(
            flanks.iter().any(|p| *p == Position::new(3, 3)),
            "Should find position opposite ally as flanking position"
        );
    }

    #[test]
    fn test_flanking_no_allies() {
        let enemy_pos = Position::new(2, 2);
        let target = Position::new(3, 2);
        let allies = vec![];

        let flanks = TacticalAI::find_flank_positions(enemy_pos, &allies, target);
        assert!(
            flanks.is_empty(),
            "Should find no flanking positions without allies"
        );
    }

    #[test]
    fn test_threat_map() {
        let map = make_test_map();
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(4, 2), Team::Player, 100, 100, 20);
        let stats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(4, 2), stats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let threat_at_player = threat_map
            .iter()
            .find(|(p, _)| *p == Position::new(4, 2))
            .map(|(_, t)| *t)
            .unwrap_or(0);
        let threat_far = threat_map
            .iter()
            .find(|(p, _)| *p == Position::new(0, 0))
            .map(|(_, t)| *t)
            .unwrap_or(0);

        assert!(
            threat_at_player > threat_far,
            "Threat at player ({}) should exceed far ({})",
            threat_at_player,
            threat_far
        );
    }

    #[test]
    fn test_chaser_strategy_pursues_nearest() {
        let map = make_test_map();
        let enemy_pos = Position::new(0, 0);
        let stats = Stats {
            hp: 80,
            max_hp: 80,
            atk: 20,
            def: 5,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        };
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(3, 0), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(3, 0), pstats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let assessment = TacticalAssessment {
            threat_map,
            cover_positions: vec![],
            flank_positions: vec![],
            safe_positions: vec![],
        };

        let action = TacticalAI::chaser_strategy(enemy_pos, &stats, &players, &assessment);
        match action {
            AIAction::MoveTo(pos) => {
                let dist_before = (enemy_pos.x - 3).abs() + enemy_pos.y.abs();
                let dist_after = (pos.x - 3).abs() + pos.y.abs();
                assert!(
                    dist_after <= dist_before,
                    "Chaser should move closer to player"
                );
            }
            AIAction::Attack(_) => {}
            other => panic!("Expected MoveTo or Attack, got {:?}", other),
        }
    }

    #[test]
    fn test_cleric_strategy_heals_low_hp_ally() {
        let enemy_pos = Position::new(2, 2);
        let stats = Stats {
            hp: 55,
            max_hp: 55,
            atk: 12,
            def: 6,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 2,
            xp: 0,
        };
        let mut world = World::new();
        let ally = spawn_test_entity(&mut world, Position::new(2, 3), Team::Enemy, 100, 500, 40);
        let ally_stats = world.get::<Stats>(ally).unwrap().clone();
        let allies = vec![(ally, Position::new(2, 3), ally_stats)];
        let pe = spawn_test_entity(&mut world, Position::new(5, 5), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(5, 5), pstats)];

        let action = TacticalAI::cleric_strategy(enemy_pos, &stats, &allies, &players);
        match action {
            AIAction::HealAlly(e) => assert_eq!(e, ally),
            AIAction::MoveTo(pos) => assert_eq!(pos, Position::new(2, 3)),
            other => panic!("Expected HealAlly or MoveTo, got {:?}", other),
        }
    }

    #[test]
    fn test_coward_strategy_retreats_when_low_hp() {
        let map = make_test_map();
        let enemy_pos = Position::new(2, 2);
        let stats = Stats {
            hp: 20,
            max_hp: 80,
            atk: 15,
            def: 0,
            spd: 10,
            ap: 2,
            max_ap: 2,
            level: 1,
            xp: 0,
        };
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(2, 1), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(2, 1), pstats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let cover_positions = TacticalAI::find_cover_positions(&map, &[], &[Position::new(2, 1)]);
        let assessment = TacticalAssessment {
            threat_map,
            cover_positions,
            flank_positions: vec![],
            safe_positions: vec![],
        };

        let action = TacticalAI::coward_strategy(enemy_pos, &stats, &players, &assessment);
        match action {
            AIAction::Retreat | AIAction::MoveTo(_) => {}
            other => panic!("Expected Retreat or MoveTo, got {:?}", other),
        }
    }

    #[test]
    fn test_focus_fire_targets_lowest_hp() {
        let mut world = World::new();
        let e1 = spawn_test_entity(&mut world, Position::new(4, 4), Team::Player, 100, 100, 20);
        let s1 = world.get::<Stats>(e1).unwrap().clone();
        let e2 = spawn_test_entity(&mut world, Position::new(4, 8), Team::Player, 30, 60, 35);
        let s2 = world.get::<Stats>(e2).unwrap().clone();
        let players = vec![(e1, Position::new(4, 4), s1), (e2, Position::new(4, 8), s2)];

        let result = TacticalAI::focus_fire_target(&players);
        assert!(result.is_some());
        let (ent, pos) = result.unwrap();
        assert_eq!(ent, e2, "Should target lowest HP player");
        assert_eq!(pos, Position::new(4, 8));
    }

    #[test]
    fn test_safest_move_target() {
        let map = TacticalMap::from_ascii("...\n...\n...");
        let pos = Position::new(1, 1);
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(2, 1), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(2, 1), pstats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let assessment = TacticalAssessment {
            threat_map,
            cover_positions: vec![],
            flank_positions: vec![],
            safe_positions: vec![],
        };

        let safest = TacticalAI::safest_move_target(pos, 1, &map, &assessment);
        let threat_at_current = assessment
            .threat_map
            .iter()
            .find(|(p, _)| *p == pos)
            .map(|(_, t)| *t)
            .unwrap_or(0);
        let threat_at_safest = assessment
            .threat_map
            .iter()
            .find(|(p, _)| *p == safest)
            .map(|(_, t)| *t)
            .unwrap_or(0);

        assert!(
            threat_at_safest <= threat_at_current,
            "Safest move should have threat <= current. current={}, safest={}",
            threat_at_current,
            threat_at_safest
        );
    }

    #[test]
    fn test_cover_positions_adjacent_to_wall() {
        let map = TacticalMap::from_ascii(
            "\
...\n\
.#.\n\
...",
        );
        let cover = TacticalAI::find_cover_positions(&map, &[], &[Position::new(0, 1)]);
        assert!(
            cover.iter().any(|p| *p == Position::new(2, 1)),
            "Position (2,1) adjacent to wall at (1,1) and blocking LOS from (0,1) should be cover"
        );
    }

    #[test]
    fn test_ai_behavior_defaults() {
        let behavior = AIBehavior::new(AIArchetype::Chaser);
        assert_eq!(behavior.archetype, AIArchetype::Chaser);
        assert_eq!(behavior.aggression, 80);
        assert_eq!(behavior.caution, 20);
        assert_eq!(behavior.coordination, 40);
        assert!(behavior.last_action.is_none());

        let cleric = AIBehavior::new(AIArchetype::Cleric);
        assert_eq!(cleric.coordination, 80);

        let coward = AIBehavior::new(AIArchetype::Coward);
        assert_eq!(coward.caution, 90);
    }

    #[test]
    fn test_berserker_behavior_defaults() {
        let berserker = AIBehavior::new(AIArchetype::Berserker);
        assert_eq!(berserker.aggression, 95);
        assert_eq!(berserker.caution, 5);
        assert_eq!(berserker.coordination, 10);
    }

    #[test]
    fn test_tactician_behavior_defaults() {
        let tactician = AIBehavior::new(AIArchetype::Tactician);
        assert_eq!(tactician.aggression, 70);
        assert_eq!(tactician.caution, 40);
        assert_eq!(tactician.coordination, 85);
    }

    #[test]
    fn test_summoner_behavior_defaults() {
        let summoner = AIBehavior::new(AIArchetype::Summoner);
        assert_eq!(summoner.aggression, 50);
        assert_eq!(summoner.caution, 30);
        assert_eq!(summoner.coordination, 70);
    }

    #[test]
    fn test_assassin_behavior_defaults() {
        let assassin = AIBehavior::new(AIArchetype::Assassin);
        assert_eq!(assassin.aggression, 85);
        assert_eq!(assassin.caution, 15);
        assert_eq!(assassin.coordination, 50);
    }

    #[test]
    fn test_berserker_strategy_attacks() {
        let map = make_test_map();
        let enemy_pos = Position::new(0, 0);
        let stats = Stats {
            hp: 80,
            max_hp: 80,
            atk: 20,
            def: 5,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        };
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(1, 0), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(1, 0), pstats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let assessment = TacticalAssessment {
            threat_map,
            cover_positions: vec![],
            flank_positions: vec![],
            safe_positions: vec![],
        };

        let action = TacticalAI::chaser_strategy(enemy_pos, &stats, &players, &assessment);
        match action {
            AIAction::Attack(_) | AIAction::MoveTo(_) => {}
            other => panic!("Expected Attack or MoveTo, got {:?}", other),
        }
    }

    #[test]
    fn test_tactician_strategy_moves() {
        let map = make_test_map();
        let enemy_pos = Position::new(0, 0);
        let stats = Stats {
            hp: 80,
            max_hp: 80,
            atk: 20,
            def: 5,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        };
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(3, 0), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(3, 0), pstats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let assessment = TacticalAssessment {
            threat_map,
            cover_positions: vec![],
            flank_positions: vec![],
            safe_positions: vec![],
        };

        let action = TacticalAI::chaser_strategy(enemy_pos, &stats, &players, &assessment);
        match action {
            AIAction::MoveTo(pos) => {
                let dist_before = (enemy_pos.x - 3).abs() + enemy_pos.y.abs();
                let dist_after = (pos.x - 3).abs() + pos.y.abs();
                assert!(dist_after <= dist_before);
            }
            AIAction::Attack(_) => {}
            other => panic!("Expected MoveTo or Attack, got {:?}", other),
        }
    }

    #[test]
    fn test_summoner_strategy_heals_allies() {
        let enemy_pos = Position::new(2, 2);
        let stats = Stats {
            hp: 55,
            max_hp: 55,
            atk: 12,
            def: 6,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 2,
            xp: 0,
        };
        let mut world = World::new();
        let ally = spawn_test_entity(&mut world, Position::new(2, 3), Team::Enemy, 100, 500, 40);
        let ally_stats = world.get::<Stats>(ally).unwrap().clone();
        let allies = vec![(ally, Position::new(2, 3), ally_stats)];
        let pe = spawn_test_entity(&mut world, Position::new(5, 5), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(5, 5), pstats)];

        let action = TacticalAI::cleric_strategy(enemy_pos, &stats, &allies, &players);
        match action {
            AIAction::HealAlly(e) => assert_eq!(e, ally),
            AIAction::MoveTo(pos) => assert_eq!(pos, Position::new(2, 3)),
            other => panic!("Expected HealAlly or MoveTo, got {:?}", other),
        }
    }

    #[test]
    fn test_assassin_strategy_pursues() {
        let map = make_test_map();
        let enemy_pos = Position::new(0, 0);
        let stats = Stats {
            hp: 80,
            max_hp: 80,
            atk: 20,
            def: 5,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        };
        let mut world = World::new();
        let pe = spawn_test_entity(&mut world, Position::new(3, 0), Team::Player, 100, 100, 20);
        let pstats = world.get::<Stats>(pe).unwrap().clone();
        let players = vec![(pe, Position::new(3, 0), pstats)];

        let threat_map = TacticalAI::calculate_threat_map(&map, &players);
        let assessment = TacticalAssessment {
            threat_map,
            cover_positions: vec![],
            flank_positions: vec![],
            safe_positions: vec![],
        };

        let action = TacticalAI::chaser_strategy(enemy_pos, &stats, &players, &assessment);
        match action {
            AIAction::MoveTo(pos) => {
                let dist_before = (enemy_pos.x - 3).abs() + enemy_pos.y.abs();
                let dist_after = (pos.x - 3).abs() + pos.y.abs();
                assert!(dist_after <= dist_before);
            }
            AIAction::Attack(_) => {}
            other => panic!("Expected MoveTo or Attack, got {:?}", other),
        }
    }
}
