use crate::components::*;
use crate::game::Game;
use crate::map::TacticalMap;
use verryte_core::{Entity, World};

pub struct BattlePreview;

impl BattlePreview {
    pub fn calculate_damage_preview(
        attacker_atk: i32,
        attacker_level: u32,
        target_def: i32,
        target_level: u32,
        elemental_modifier: f32,
        crit_chance: u32,
    ) -> DamagePreview {
        let base_damage = (attacker_atk - target_def / 2).max(1);
        let level_bonus = ((attacker_level as i32 - target_level as i32) * 2).clamp(-10, 10);
        let min_dmg =
            ((base_damage + level_bonus) as f32 * elemental_modifier * 0.8).max(1.0) as i32;
        let max_dmg =
            ((base_damage + level_bonus) as f32 * elemental_modifier * 1.2).max(1.0) as i32;
        let expected = (min_dmg + max_dmg) / 2;

        DamagePreview {
            min_damage: min_dmg,
            max_damage: max_dmg,
            expected_damage: expected,
            hit_chance: 85,
            crit_chance,
            element: None,
            can_kill: false,
        }
    }

    pub fn calculate_aoe_preview(
        center: Position,
        radius: i16,
        shape: AoEShape,
        world: &World,
        map: &TacticalMap,
        attacker_team: Team,
    ) -> AoEPreview {
        let tiles = match shape {
            AoEShape::Circle => Self::circle_tiles(center, radius),
            AoEShape::Square => Self::square_tiles(center, radius),
            AoEShape::Cross => Self::cross_tiles(center, radius),
            AoEShape::Line => Self::line_tiles(center, radius),
            AoEShape::Cone => Self::cone_tiles(center, radius),
        };

        let valid_tiles: Vec<Position> = tiles
            .into_iter()
            .filter(|t| t.x >= 0 && t.x < map.width as i16 && t.y >= 0 && t.y < map.height as i16)
            .collect();

        let mut affected_enemies = Vec::new();
        let mut affected_allies = Vec::new();
        let mut total_damage = 0;

        for (e, pos, team) in world.query2::<Position, Team>() {
            if valid_tiles.contains(pos) {
                if *team == attacker_team {
                    affected_allies.push(e);
                } else {
                    affected_enemies.push(e);
                    if let Some(stats) = world.get::<Stats>(e) {
                        total_damage += stats.hp.min(50);
                    }
                }
            }
        }

        AoEPreview {
            center: Some(center),
            affected_tiles: valid_tiles,
            affected_enemies,
            affected_allies,
            total_potential_damage: total_damage,
        }
    }

    pub fn calculate_turn_order(world: &World, current_entity: Option<Entity>) -> TurnOrderDisplay {
        let mut entries: Vec<TurnOrderEntry> = Vec::new();

        for (e, stats, team, class) in world.query3::<Stats, Team, CharacterClass>() {
            if stats.hp > 0 {
                let name = Game::get_class_name(*class).to_string();
                let hp_ratio = stats.hp as f32 / stats.max_hp as f32;
                entries.push(TurnOrderEntry {
                    entity: e,
                    name,
                    team: *team,
                    spd: stats.spd,
                    is_current: current_entity == Some(e),
                    hp_ratio,
                });
            }
        }

        entries.sort_by(|a, b| b.spd.cmp(&a.spd).then(a.entity.cmp(&b.entity)));

        let current_index = entries.iter().position(|e| e.is_current).unwrap_or(0);

        TurnOrderDisplay {
            entries,
            current_index,
        }
    }

    pub fn predict_enemy_intents(
        world: &World,
        _map: &TacticalMap,
        telegraph: &TelegraphZone,
    ) -> EnemyIntentions {
        let mut intents = Vec::new();

        for (e, pos, team, class) in world.query3::<Position, Team, CharacterClass>() {
            if *team != Team::Enemy {
                continue;
            }
            let stats = match world.get::<Stats>(e) {
                Some(s) => s,
                None => continue,
            };
            if stats.ap <= 0 {
                continue;
            }

            let archetype = world
                .get::<AIArchetype>(e)
                .copied()
                .unwrap_or(AIArchetype::Chaser);

            let name = Game::get_class_name(*class);

            match archetype {
                AIArchetype::Chaser => {
                    let mut nearest_player: Option<(Entity, Position, i16)> = None;
                    for (pe, ppos, pteam) in world.query2::<Position, Team>() {
                        if *pteam == Team::Player {
                            if let Some(pstats) = world.get::<Stats>(pe) {
                                if pstats.hp > 0 {
                                    let dist = (ppos.x - pos.x).abs() + (ppos.y - pos.y).abs();
                                    if nearest_player.as_ref().is_none_or(|n| dist < n.2) {
                                        nearest_player = Some((pe, *ppos, dist));
                                    }
                                }
                            }
                        }
                    }

                    if let Some((_target_ent, target_pos, dist)) = nearest_player {
                        if dist <= 1 {
                            intents.push(EnemyIntent {
                                entity: e,
                                intent_type: IntentType::Attack,
                                target: Some(target_pos),
                                predicted_damage: (stats.atk as f32 * 0.9) as i32,
                                description: format!(
                                    "{}: Attack nearest player at ({},{})",
                                    name, target_pos.x, target_pos.y
                                ),
                            });
                        } else {
                            intents.push(EnemyIntent {
                                entity: e,
                                intent_type: IntentType::Move,
                                target: Some(target_pos),
                                predicted_damage: 0,
                                description: format!(
                                    "{}: Move toward player at ({},{})",
                                    name, target_pos.x, target_pos.y
                                ),
                            });
                        }
                    }
                }
                AIArchetype::Cleric => {
                    let mut needy_ally: Option<(Entity, f32)> = None;
                    for (ae, _apos, ateam) in world.query2::<Position, Team>() {
                        if *ateam == Team::Enemy && ae != e {
                            if let Some(astats) = world.get::<Stats>(ae) {
                                if astats.hp > 0 {
                                    let hp_pct = astats.hp as f32 / astats.max_hp as f32;
                                    if hp_pct < 0.5
                                        && needy_ally.as_ref().is_none_or(|n| hp_pct < n.1)
                                    {
                                        needy_ally = Some((ae, hp_pct));
                                    }
                                }
                            }
                        }
                    }

                    if let Some((ally, _)) = needy_ally {
                        let heal_amount = (stats.atk as f32 * 0.8) as i32;
                        intents.push(EnemyIntent {
                            entity: e,
                            intent_type: IntentType::Heal,
                            target: world.get::<Position>(ally).copied(),
                            predicted_damage: -heal_amount,
                            description: format!(
                                "{}: Heal injured ally for ~{} HP",
                                name, heal_amount
                            ),
                        });
                    } else {
                        intents.push(EnemyIntent {
                            entity: e,
                            intent_type: IntentType::Move,
                            target: None,
                            predicted_damage: 0,
                            description: format!("{}: Advance with allies", name),
                        });
                    }
                }
                AIArchetype::Coward => {
                    let mut nearest_player_dist = i16::MAX;
                    for (_pe, ppos, pteam) in world.query2::<Position, Team>() {
                        if *pteam == Team::Player {
                            let dist = (ppos.x - pos.x).abs() + (ppos.y - pos.y).abs();
                            if dist < nearest_player_dist {
                                nearest_player_dist = dist;
                            }
                        }
                    }

                    if nearest_player_dist <= 2 {
                        intents.push(EnemyIntent {
                            entity: e,
                            intent_type: IntentType::Move,
                            target: None,
                            predicted_damage: 0,
                            description: format!("{}: Retreat from player", name),
                        });
                    } else {
                        intents.push(EnemyIntent {
                            entity: e,
                            intent_type: IntentType::Move,
                            target: None,
                            predicted_damage: 0,
                            description: format!("{}: Hold position (cautious)", name),
                        });
                    }
                }
                AIArchetype::Berserker => {
                    intents.push(EnemyIntent {
                        entity: e,
                        intent_type: IntentType::Attack,
                        target: None,
                        predicted_damage: stats.atk,
                        description: format!("{}: Berserk attack", name),
                    });
                }
                AIArchetype::Tactician => {
                    intents.push(EnemyIntent {
                        entity: e,
                        intent_type: IntentType::Move,
                        target: None,
                        predicted_damage: 0,
                        description: format!("{}: Tactical reposition", name),
                    });
                }
                AIArchetype::Summoner => {
                    intents.push(EnemyIntent {
                        entity: e,
                        intent_type: IntentType::Summon,
                        target: None,
                        predicted_damage: 0,
                        description: format!("{}: Summon minion", name),
                    });
                }
                AIArchetype::Assassin => {
                    intents.push(EnemyIntent {
                        entity: e,
                        intent_type: IntentType::Attack,
                        target: None,
                        predicted_damage: stats.atk * 2,
                        description: format!("{}: Assassinate", name),
                    });
                }
                AIArchetype::Defender => {
                    intents.push(EnemyIntent {
                        entity: e,
                        intent_type: IntentType::Defend,
                        target: None,
                        predicted_damage: 0,
                        description: format!("{}: Protecting allies", name),
                    });
                }
            }
        }

        if !telegraph.tiles.is_empty() {
            intents.push(EnemyIntent {
                entity: Entity::INVALID,
                intent_type: IntentType::AoEAttack,
                target: telegraph.tiles.first().copied(),
                predicted_damage: telegraph.damage,
                description: format!(
                    "BOSS: Telegraphed AoE attack ({} dmg) on {} tiles",
                    telegraph.damage,
                    telegraph.tiles.len()
                ),
            });
        }

        EnemyIntentions { intents }
    }

    pub fn format_damage_preview(preview: &DamagePreview) -> String {
        let kill_tag = if preview.can_kill { " [KILL]" } else { "" };
        let elem_tag = preview
            .element
            .as_ref()
            .map(|e| format!(" [{}]", e))
            .unwrap_or_default();
        format!(
            "DMG: {}-{} (avg {}) | HIT: {}% | CRIT: {}%{}{}",
            preview.min_damage,
            preview.max_damage,
            preview.expected_damage,
            preview.hit_chance,
            preview.crit_chance,
            elem_tag,
            kill_tag,
        )
    }

    pub fn format_turn_order(order: &TurnOrderDisplay) -> String {
        order
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let marker = if e.is_current { ">>>" } else { "   " };
                let team_tag = match e.team {
                    Team::Player => "P",
                    Team::Enemy => "E",
                };
                let hp_bar = Self::mini_hp_bar(e.hp_ratio);
                format!(
                    "{} {}. {} [{}] {} SPD:{}",
                    marker,
                    i + 1,
                    e.name,
                    team_tag,
                    hp_bar,
                    e.spd
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn format_intents(intents: &EnemyIntentions) -> Vec<String> {
        intents
            .intents
            .iter()
            .map(|i| i.description.clone())
            .collect()
    }

    fn circle_tiles(center: Position, radius: i16) -> Vec<Position> {
        let mut tiles = Vec::new();
        let r2 = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= r2 {
                    tiles.push(Position::new(center.x + dx, center.y + dy));
                }
            }
        }
        tiles
    }

    fn square_tiles(center: Position, radius: i16) -> Vec<Position> {
        let mut tiles = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                tiles.push(Position::new(center.x + dx, center.y + dy));
            }
        }
        tiles
    }

    fn cross_tiles(center: Position, radius: i16) -> Vec<Position> {
        let mut tiles = Vec::new();
        tiles.push(center);
        for d in 1..=radius {
            tiles.push(Position::new(center.x + d, center.y));
            tiles.push(Position::new(center.x - d, center.y));
            tiles.push(Position::new(center.x, center.y + d));
            tiles.push(Position::new(center.x, center.y - d));
        }
        tiles
    }

    fn line_tiles(center: Position, length: i16) -> Vec<Position> {
        let mut tiles = Vec::new();
        for d in 0..length {
            tiles.push(Position::new(center.x + d, center.y));
        }
        tiles
    }

    fn cone_tiles(center: Position, radius: i16) -> Vec<Position> {
        let mut tiles = Vec::new();
        for d in 0i16..=radius {
            for dx in -d..=d {
                tiles.push(Position::new(center.x + dx, center.y - d));
            }
        }
        tiles
    }

    fn mini_hp_bar(ratio: f32) -> String {
        let filled = (ratio * 5.0).round() as usize;
        let empty = 5_usize.saturating_sub(filled);
        format!(
            "[{}{}]",
            "\u{2588}".repeat(filled),
            "\u{2591}".repeat(empty)
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AoEShape {
    Circle,
    Square,
    Line,
    Cone,
    Cross,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TacticalMap;

    fn make_game_state() -> GameState {
        GameState {
            turn: 1,
            phase: TurnPhase::Player,
            outcome: Outcome::Playing,
            cursor: Position::new(0, 0),
            selected_entity: None,
            concert_energy: 0,
            targeting: TargetingMode::None,
            boss_phase: BossPhase::Phase1,
            ui_state: UIState::Normal,
            show_perf: false,
            auto_battle: false,
            is_recording: false,
            show_minimap: true,
            combo_count: 0,
            floor: 1,
            log_scroll_offset: 0,
            selected_save_slot: 0,
            show_threat_map: false,
        }
    }

    #[test]
    fn test_damage_preview_basic() {
        let preview = BattlePreview::calculate_damage_preview(20, 1, 10, 1, 1.0, 20);
        assert_eq!(preview.min_damage, 12);
        assert_eq!(preview.max_damage, 18);
        assert_eq!(preview.expected_damage, 15);
        assert_eq!(preview.hit_chance, 85);
        assert_eq!(preview.crit_chance, 20);
    }

    #[test]
    fn test_damage_preview_with_elemental_modifier() {
        let preview = BattlePreview::calculate_damage_preview(30, 1, 10, 1, 1.5, 25);
        assert_eq!(preview.min_damage, 30);
        assert_eq!(preview.max_damage, 45);
        assert_eq!(preview.expected_damage, 37);
    }

    #[test]
    fn test_damage_preview_min_le_expected_le_max() {
        let cases = [
            (20, 1, 10, 1, 1.0, 20),
            (35, 1, 5, 1, 1.5, 25),
            (40, 10, 20, 1, 1.0, 20),
            (10, 1, 15, 5, 0.8, 10),
        ];
        for (atk, al, def, dl, elem, crit) in cases {
            let p = BattlePreview::calculate_damage_preview(atk, al, def, dl, elem, crit);
            assert!(
                p.min_damage <= p.expected_damage && p.expected_damage <= p.max_damage,
                "Invariant violated: min={}, expected={}, max={}",
                p.min_damage,
                p.expected_damage,
                p.max_damage
            );
        }
    }

    #[test]
    fn test_aoe_circle_shape() {
        let map = TacticalMap::new(24, 16);
        let world = World::new();
        let center = Position::new(5, 5);
        let preview = BattlePreview::calculate_aoe_preview(
            center,
            1,
            AoEShape::Circle,
            &world,
            &map,
            Team::Player,
        );
        assert_eq!(preview.affected_tiles.len(), 5);
        assert!(preview.affected_tiles.contains(&center));
        assert!(preview.affected_tiles.contains(&Position::new(5, 4)));
        assert!(preview.affected_tiles.contains(&Position::new(5, 6)));
        assert!(preview.affected_tiles.contains(&Position::new(4, 5)));
        assert!(preview.affected_tiles.contains(&Position::new(6, 5)));
    }

    #[test]
    fn test_aoe_square_shape() {
        let map = TacticalMap::new(24, 16);
        let world = World::new();
        let center = Position::new(5, 5);
        let preview = BattlePreview::calculate_aoe_preview(
            center,
            1,
            AoEShape::Square,
            &world,
            &map,
            Team::Player,
        );
        assert_eq!(preview.affected_tiles.len(), 9);
    }

    #[test]
    fn test_aoe_cross_shape() {
        let map = TacticalMap::new(24, 16);
        let world = World::new();
        let center = Position::new(5, 5);
        let preview = BattlePreview::calculate_aoe_preview(
            center,
            2,
            AoEShape::Cross,
            &world,
            &map,
            Team::Player,
        );
        assert_eq!(preview.affected_tiles.len(), 9);
        assert!(preview.affected_tiles.contains(&center));
        assert!(preview.affected_tiles.contains(&Position::new(7, 5)));
        assert!(preview.affected_tiles.contains(&Position::new(3, 5)));
        assert!(preview.affected_tiles.contains(&Position::new(5, 7)));
        assert!(preview.affected_tiles.contains(&Position::new(5, 3)));
    }

    #[test]
    fn test_turn_order_sorts_by_spd_desc() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let e1 = world
            .builder()
            .with(Position::new(0, 0))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();
        let e2 = world
            .builder()
            .with(Position::new(1, 0))
            .with(Team::Enemy)
            .with(CharacterClass::ShadowStalker)
            .with(Stats {
                hp: 80,
                max_hp: 80,
                atk: 25,
                def: 5,
                spd: 8,
                ap: 4,
                max_ap: 4,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();
        let e3 = world
            .builder()
            .with(Position::new(2, 0))
            .with(Team::Player)
            .with(CharacterClass::Healer)
            .with(Stats {
                hp: 70,
                max_hp: 70,
                atk: 10,
                def: 8,
                spd: 6,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let order = BattlePreview::calculate_turn_order(&world, Some(e1));

        assert_eq!(order.entries.len(), 3);
        assert_eq!(order.entries[0].entity, e2);
        assert_eq!(order.entries[1].entity, e3);
        assert_eq!(order.entries[2].entity, e1);
    }

    #[test]
    fn test_turn_order_includes_all_alive() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _e1 = world
            .builder()
            .with(Position::new(0, 0))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();
        let _e2 = world
            .builder()
            .with(Position::new(1, 0))
            .with(Team::Enemy)
            .with(CharacterClass::ShadowStalker)
            .with(Stats {
                hp: 0,
                max_hp: 80,
                atk: 25,
                def: 5,
                spd: 8,
                ap: 0,
                max_ap: 4,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();
        let _e3 = world
            .builder()
            .with(Position::new(2, 0))
            .with(Team::Player)
            .with(CharacterClass::Healer)
            .with(Stats {
                hp: 70,
                max_hp: 70,
                atk: 10,
                def: 8,
                spd: 6,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let order = BattlePreview::calculate_turn_order(&world, None);
        assert_eq!(order.entries.len(), 2);
    }

    #[test]
    fn test_enemy_intent_chaser() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _player = world
            .builder()
            .with(Position::new(5, 5))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let _enemy = world
            .builder()
            .with(Position::new(5, 6))
            .with(Team::Enemy)
            .with(CharacterClass::ShadowStalker)
            .with(Stats {
                hp: 80,
                max_hp: 80,
                atk: 25,
                def: 5,
                spd: 8,
                ap: 4,
                max_ap: 4,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Chaser)
            .build();

        let map = TacticalMap::new(24, 16);
        let telegraph = TelegraphZone::default();
        let intents = BattlePreview::predict_enemy_intents(&world, &map, &telegraph);

        assert_eq!(intents.intents.len(), 1);
        assert_eq!(intents.intents[0].intent_type, IntentType::Attack);
        assert!(intents.intents[0].predicted_damage > 0);
    }

    #[test]
    fn test_enemy_intent_cleric_heal() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _player = world
            .builder()
            .with(Position::new(0, 0))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let _wounded_ally = world
            .builder()
            .with(Position::new(15, 8))
            .with(Team::Enemy)
            .with(CharacterClass::ShadowStalker)
            .with(Stats {
                hp: 20,
                max_hp: 80,
                atk: 25,
                def: 5,
                spd: 8,
                ap: 0,
                max_ap: 4,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Chaser)
            .build();

        let _cleric = world
            .builder()
            .with(Position::new(15, 9))
            .with(Team::Enemy)
            .with(CharacterClass::EnemyCleric)
            .with(Stats {
                hp: 55,
                max_hp: 55,
                atk: 12,
                def: 6,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 2,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Cleric)
            .build();

        let map = TacticalMap::new(24, 16);
        let telegraph = TelegraphZone::default();
        let intents = BattlePreview::predict_enemy_intents(&world, &map, &telegraph);

        let heal_intent = intents
            .intents
            .iter()
            .find(|i| i.intent_type == IntentType::Heal);
        assert!(heal_intent.is_some(), "Cleric should have a heal intent");
        assert!(heal_intent.unwrap().predicted_damage < 0);
    }

    #[test]
    fn test_format_damage_preview() {
        let preview = DamagePreview {
            min_damage: 12,
            max_damage: 18,
            expected_damage: 15,
            hit_chance: 85,
            crit_chance: 20,
            element: Some("Ice".to_string()),
            can_kill: true,
        };
        let formatted = BattlePreview::format_damage_preview(&preview);
        assert!(formatted.contains("12-18"));
        assert!(formatted.contains("avg 15"));
        assert!(formatted.contains("HIT: 85%"));
        assert!(formatted.contains("CRIT: 20%"));
        assert!(formatted.contains("[Ice]"));
        assert!(formatted.contains("[KILL]"));
    }

    #[test]
    fn test_format_turn_order() {
        let order = TurnOrderDisplay {
            entries: vec![
                TurnOrderEntry {
                    entity: Entity::INVALID,
                    name: "Shadow Stalker".to_string(),
                    team: Team::Enemy,
                    spd: 8,
                    is_current: false,
                    hp_ratio: 1.0,
                },
                TurnOrderEntry {
                    entity: Entity::INVALID,
                    name: "Kael".to_string(),
                    team: Team::Player,
                    spd: 5,
                    is_current: true,
                    hp_ratio: 0.75,
                },
            ],
            current_index: 1,
        };
        let formatted = BattlePreview::format_turn_order(&order);
        assert!(formatted.contains("Shadow Stalker"));
        assert!(formatted.contains("Kael"));
        assert!(formatted.contains(">>>"));
        assert!(formatted.contains("[P]"));
        assert!(formatted.contains("[E]"));
        assert!(formatted.contains("SPD:8"));
        assert!(formatted.contains("SPD:5"));
    }

    #[test]
    fn test_can_kill_flag() {
        let preview = BattlePreview::calculate_damage_preview(40, 5, 5, 1, 1.0, 20);
        let mut p = preview.clone();
        p.can_kill = p.max_damage >= 30;
        assert!(p.can_kill);

        let mut p2 = preview.clone();
        p2.can_kill = p2.max_damage >= 200;
        assert!(!p2.can_kill);
    }

    #[test]
    fn test_enemy_intent_berserker() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _player = world
            .builder()
            .with(Position::new(5, 5))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let _enemy = world
            .builder()
            .with(Position::new(5, 6))
            .with(Team::Enemy)
            .with(CharacterClass::Berserker)
            .with(Stats {
                hp: 80,
                max_hp: 80,
                atk: 30,
                def: 3,
                spd: 8,
                ap: 4,
                max_ap: 4,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Berserker)
            .build();

        let map = TacticalMap::new(24, 16);
        let telegraph = TelegraphZone::default();
        let intents = BattlePreview::predict_enemy_intents(&world, &map, &telegraph);

        let attack_intent = intents
            .intents
            .iter()
            .find(|i| i.intent_type == IntentType::Attack);
        assert!(
            attack_intent.is_some(),
            "Berserker should have an attack intent"
        );
    }

    #[test]
    fn test_enemy_intent_tactician() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _player = world
            .builder()
            .with(Position::new(5, 5))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let _enemy = world
            .builder()
            .with(Position::new(5, 6))
            .with(Team::Enemy)
            .with(CharacterClass::Tactician)
            .with(Stats {
                hp: 70,
                max_hp: 70,
                atk: 18,
                def: 8,
                spd: 6,
                ap: 3,
                max_ap: 3,
                level: 2,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Tactician)
            .build();

        let map = TacticalMap::new(24, 16);
        let telegraph = TelegraphZone::default();
        let intents = BattlePreview::predict_enemy_intents(&world, &map, &telegraph);

        let move_intent = intents
            .intents
            .iter()
            .find(|i| i.intent_type == IntentType::Move);
        assert!(move_intent.is_some(), "Tactician should have a move intent");
    }

    #[test]
    fn test_enemy_intent_summoner() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _player = world
            .builder()
            .with(Position::new(5, 5))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let _enemy = world
            .builder()
            .with(Position::new(5, 6))
            .with(Team::Enemy)
            .with(CharacterClass::Summoner)
            .with(Stats {
                hp: 60,
                max_hp: 60,
                atk: 15,
                def: 5,
                spd: 4,
                ap: 3,
                max_ap: 3,
                level: 2,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Summoner)
            .build();

        let map = TacticalMap::new(24, 16);
        let telegraph = TelegraphZone::default();
        let intents = BattlePreview::predict_enemy_intents(&world, &map, &telegraph);

        let summon_intent = intents
            .intents
            .iter()
            .find(|i| i.intent_type == IntentType::Summon);
        assert!(
            summon_intent.is_some(),
            "Summoner should have a summon intent"
        );
    }

    #[test]
    fn test_enemy_intent_assassin() {
        let mut world = World::new();
        world.insert_resource(make_game_state());
        world.insert_resource(TelegraphZone::default());

        let _player = world
            .builder()
            .with(Position::new(5, 5))
            .with(Team::Player)
            .with(CharacterClass::Warrior)
            .with(Stats {
                hp: 100,
                max_hp: 100,
                atk: 20,
                def: 10,
                spd: 5,
                ap: 3,
                max_ap: 3,
                level: 1,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .build();

        let _enemy = world
            .builder()
            .with(Position::new(5, 6))
            .with(Team::Enemy)
            .with(CharacterClass::Assassin)
            .with(Stats {
                hp: 50,
                max_hp: 50,
                atk: 35,
                def: 2,
                spd: 12,
                ap: 4,
                max_ap: 4,
                level: 3,
                xp: 0,
            })
            .with(ElementalStatus::None)
            .with(AIArchetype::Assassin)
            .build();

        let map = TacticalMap::new(24, 16);
        let telegraph = TelegraphZone::default();
        let intents = BattlePreview::predict_enemy_intents(&world, &map, &telegraph);

        let attack_intent = intents
            .intents
            .iter()
            .find(|i| i.intent_type == IntentType::Attack);
        assert!(
            attack_intent.is_some(),
            "Assassin should have an attack intent"
        );
    }
}
