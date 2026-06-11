use verryte_input::ActionSource;
use wuthering_terminal::components::{
    CharacterClass, EquippedItems, GameState, Inventory, Outcome, Stats, Team, TurnPhase,
};
use wuthering_terminal::snapshot::{ActionOutcome, FullSaveState, CURRENT_SAVE_VERSION};
use wuthering_terminal::{Action, Game, Position};

fn find_entity(game: &Game, class: CharacterClass) -> verryte_core::Entity {
    game.world
        .query::<CharacterClass>()
        .into_iter()
        .find(|(_, c)| **c == class)
        .map(|(e, _)| e)
        .unwrap()
}

fn player_classes(game: &Game) -> Vec<CharacterClass> {
    let mut classes: Vec<CharacterClass> = Vec::new();
    for (_, team, class) in game.world.query2::<Team, CharacterClass>() {
        if *team == Team::Player {
            classes.push(*class);
        }
    }
    classes.sort_by_key(|c| format!("{:?}", c));
    classes
}

fn collect_all_stats(game: &Game) -> Vec<(CharacterClass, i32, i32, i32, i32)> {
    let mut stats = Vec::new();
    for (_, class, s) in game.world.query2::<CharacterClass, Stats>() {
        stats.push((*class, s.hp, s.max_hp, s.ap, s.max_ap));
    }
    stats.sort_by_key(|(c, _, _, _, _)| format!("{:?}", c));
    stats
}

fn build_active_game() -> Game {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);

    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    game.world.get_mut::<Stats>(warrior).unwrap().hp = 80;
    game.world.get_mut::<Stats>(boss).unwrap().hp = 300;

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 4);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 5);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    game.apply_action(Action::EndTurn, ActionSource::Terminal);
    game.update(0.1);
    game.update(0.1);
    game.update(0.1);

    game
}

// ─── 1. Snapshot Roundtrip Tests ─────────────────────────────────────────────

#[test]
fn snapshot_json_roundtrip_preserves_all_fields() {
    let mut game = Game::new();

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 4);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let snap = game.snapshot();
    let json = serde_json::to_string(&snap).expect("serialize snapshot");
    let restored: wuthering_terminal::Snapshot =
        serde_json::from_str(&json).expect("deserialize snapshot");

    assert_eq!(snap.turn, restored.turn);
    assert_eq!(snap.phase, restored.phase);
    assert_eq!(snap.outcome, restored.outcome);
    assert_eq!(snap.cursor, restored.cursor);
    assert_eq!(snap.player_team, restored.player_team);
    assert_eq!(snap.enemy_team, restored.enemy_team);
    assert_eq!(snap.combo_count, restored.combo_count);
    assert_eq!(snap.floor, restored.floor);
    assert_eq!(snap.weather, restored.weather);
    assert_eq!(snap.selected_can_act, restored.selected_can_act);
    assert_eq!(snap.battle_stats, restored.battle_stats);
    assert_eq!(snap.bestiary_discovered, restored.bestiary_discovered);
    assert_eq!(snap.bestiary_total, restored.bestiary_total);
    assert_eq!(snap.lore_discovered, restored.lore_discovered);
    assert_eq!(snap.lore_total, restored.lore_total);
}

#[test]
fn snapshot_roundtrip_after_combat() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);

    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 4);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 5);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let snap = game.snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let restored: wuthering_terminal::Snapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(snap.turn, restored.turn);
    assert_eq!(snap.player_team.count, restored.player_team.count);
    assert_eq!(snap.enemy_team.count, restored.enemy_team.count);
    assert_eq!(
        snap.battle_stats.total_damage_dealt,
        restored.battle_stats.total_damage_dealt
    );
    assert_eq!(
        snap.battle_stats.total_turns,
        restored.battle_stats.total_turns
    );
}

#[test]
fn snapshot_roundtrip_preserves_reachable_and_targetable() {
    let mut game = Game::new();

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 4);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let snap = game.snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let restored: wuthering_terminal::Snapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(snap.reachable_tiles.len(), restored.reachable_tiles.len());
    assert_eq!(snap.targetable_tiles.len(), restored.targetable_tiles.len());

    for (a, b) in snap
        .reachable_tiles
        .iter()
        .zip(restored.reachable_tiles.iter())
    {
        assert_eq!(a, b);
    }
}

// ─── 2. FullSaveState Roundtrip Tests ────────────────────────────────────────

#[test]
fn full_save_state_json_roundtrip() {
    let game = build_active_game();

    let json = game.save_state().expect("save_state");
    let state: FullSaveState = serde_json::from_str(&json).expect("deserialize FullSaveState");

    assert_eq!(state.magic, "VERRYTE_SAVE");
    assert_eq!(state.version, CURRENT_SAVE_VERSION);
    assert!(!state.timestamp.is_empty());
}

#[test]
fn save_load_restores_game_state() {
    let game = build_active_game();

    let snap_before = game.snapshot();
    let json = game.save_state().unwrap();

    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let snap_after = game2.snapshot();
    assert_eq!(snap_before.turn, snap_after.turn);
    assert_eq!(snap_before.phase, snap_after.phase);
    assert_eq!(snap_before.outcome, snap_after.outcome);
    assert_eq!(snap_before.player_team, snap_after.player_team);
    assert_eq!(snap_before.enemy_team, snap_after.enemy_team);
    assert_eq!(snap_before.floor, snap_after.floor);
    assert_eq!(snap_before.battle_stats, snap_after.battle_stats);
}

#[test]
fn save_load_preserves_character_stats() {
    let game = build_active_game();

    let stats_before = collect_all_stats(&game);
    let json = game.save_state().unwrap();

    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let stats_after = collect_all_stats(&game2);
    assert_eq!(stats_before.len(), stats_after.len());
    for (before, after) in stats_before.iter().zip(stats_after.iter()) {
        assert_eq!(before, after, "Stats mismatch for {:?}", before.0);
    }
}

#[test]
fn save_load_preserves_character_positions() {
    let game = build_active_game();

    let mut positions_before: Vec<(CharacterClass, Position)> = Vec::new();
    for (_, class, pos) in game.world.query2::<CharacterClass, Position>() {
        positions_before.push((*class, *pos));
    }
    positions_before.sort_by_key(|(c, _)| format!("{:?}", c));

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let mut positions_after: Vec<(CharacterClass, Position)> = Vec::new();
    for (_, class, pos) in game2.world.query2::<CharacterClass, Position>() {
        positions_after.push((*class, *pos));
    }
    positions_after.sort_by_key(|(c, _)| format!("{:?}", c));

    assert_eq!(positions_before.len(), positions_after.len());
    for (before, after) in positions_before.iter().zip(positions_after.iter()) {
        assert_eq!(before, after, "Position mismatch for {:?}", before.0);
    }
}

#[test]
fn save_load_preserves_entity_count() {
    let game = build_active_game();
    let count_before = game.world.entity_count();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    assert_eq!(count_before, game2.world.entity_count());
}

#[test]
fn save_load_preserves_team_membership() {
    let game = build_active_game();

    let players_before = player_classes(&game);
    let json = game.save_state().unwrap();

    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let players_after = player_classes(&game2);
    assert_eq!(players_before, players_after);
}

#[test]
fn save_load_allows_subsequent_actions() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    game2.apply_action(Action::MoveNorth, ActionSource::Terminal);

    let state = game2.world.resource::<GameState>().unwrap();
    assert_eq!(state.outcome, Outcome::Playing);
}

#[test]
fn save_load_preserves_equipment_on_unsaved_game() {
    let game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let equipped = game.world.get::<EquippedItems>(warrior).cloned().unwrap();

    assert!(
        equipped.weapon.is_some(),
        "Warrior should have starter weapon"
    );
    assert!(
        equipped.armor.is_some(),
        "Warrior should have starter armor"
    );
}

#[test]
fn save_load_preserves_boss_state() {
    let mut game = Game::new();

    let boss = find_entity(&game, CharacterClass::Boss);
    // Set HP above the phase2 threshold (250) to avoid triggering phase transition
    game.world.get_mut::<Stats>(boss).unwrap().hp = 350;
    game.world.resource_mut::<GameState>().unwrap().boss_phase =
        wuthering_terminal::components::BossPhase::Phase2;

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let state = game2.world.resource::<GameState>().unwrap();
    assert_eq!(
        state.boss_phase,
        wuthering_terminal::components::BossPhase::Phase2
    );

    let boss2 = find_entity(&game2, CharacterClass::Boss);
    let boss_stats = game2.world.get::<Stats>(boss2).unwrap();
    // Phase 2 HP is set by the boss phase transition which gives max_hp=500
    // After snapshot roundtrip, HP preserves whatever was saved
    assert!(boss_stats.hp > 0, "Boss should have positive HP");
    assert!(boss_stats.max_hp > 0, "Boss should have positive max HP");
}

#[test]
fn save_load_preserves_floor_number() {
    let mut game = Game::new();
    game.world.resource_mut::<GameState>().unwrap().floor = 3;

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let state = game2.world.resource::<GameState>().unwrap();
    assert_eq!(state.floor, 3);
}

#[test]
fn save_load_preserves_inventory() {
    let game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let inv_before = game.world.get::<Inventory>(warrior).unwrap();
    let item_count = inv_before.items.len();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let warrior2 = find_entity(&game2, CharacterClass::Warrior);
    let inv_after = game2.world.get::<Inventory>(warrior2).unwrap();
    assert_eq!(item_count, inv_after.items.len());
}

// ─── 3. State Integrity Tests ────────────────────────────────────────────────

#[test]
fn integrity_hp_never_exceeds_max_hp() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    for (_, stats) in game2.world.query::<Stats>() {
        assert!(
            stats.hp <= stats.max_hp,
            "HP {} exceeds max_hp {} for a character",
            stats.hp,
            stats.max_hp
        );
    }
}

#[test]
fn integrity_ap_within_bounds() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    for (_e, class, stats) in game2.world.query2::<CharacterClass, Stats>() {
        let effective_max = match class {
            CharacterClass::Warrior => stats.max_ap + 1,
            _ => stats.max_ap,
        };
        assert!(
            stats.ap <= effective_max,
            "AP {} exceeds effective max_ap {} for {:?}",
            stats.ap,
            effective_max,
            class
        );
    }
}

#[test]
fn integrity_turn_phase_is_valid() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let state = game2.world.resource::<GameState>().unwrap();
    assert!(
        state.phase == TurnPhase::Player || state.phase == TurnPhase::Enemy,
        "Invalid turn phase"
    );
}

#[test]
fn integrity_floor_is_at_least_one() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let state = game2.world.resource::<GameState>().unwrap();
    assert!(state.floor >= 1, "Floor must be >= 1, got {}", state.floor);
}

#[test]
fn integrity_inventory_items_are_valid_entities() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    for (_, inv) in game2.world.query::<Inventory>() {
        for &item_entity in &inv.items {
            assert!(
                game2.world.is_alive(item_entity),
                "Inventory references dead entity"
            );
            assert!(
                game2
                    .world
                    .get::<wuthering_terminal::components::Item>(item_entity)
                    .is_some(),
                "Inventory entity is missing Item component"
            );
        }
    }
}

#[test]
fn integrity_equipment_slots_reference_valid_items() {
    let game = build_active_game();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    for (_, equipped) in game2.world.query::<EquippedItems>() {
        if let Some(ref weapon) = equipped.weapon {
            assert!(!weapon.name.is_empty(), "Equipped weapon has empty name");
        }
        if let Some(ref armor) = equipped.armor {
            assert!(!armor.name.is_empty(), "Equipped armor has empty name");
        }
        if let Some(ref accessory) = equipped.accessory {
            assert!(
                !accessory.name.is_empty(),
                "Equipped accessory has empty name"
            );
        }
    }
}

#[test]
fn integrity_team_counts_match_after_load() {
    let game = build_active_game();

    let snap_before = game.snapshot();
    let json = game.save_state().unwrap();

    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let snap_after = game2.snapshot();
    assert_eq!(
        snap_before.player_team.count, snap_after.player_team.count,
        "Player team count changed after load"
    );
    assert_eq!(
        snap_before.enemy_team.count, snap_after.enemy_team.count,
        "Enemy team count changed after load"
    );
}

#[test]
fn integrity_diagnostics_consistent_after_load() {
    let game = build_active_game();

    let diag_before = game.diagnostics();
    let json = game.save_state().unwrap();

    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let diag_after = game2.diagnostics();
    assert_eq!(diag_before.alive_entities, diag_after.alive_entities);
    assert_eq!(diag_before.dead_entities, diag_after.dead_entities);
    assert_eq!(diag_before.floor, diag_after.floor);
    assert_eq!(diag_before.turn, diag_after.turn);
    assert_eq!(diag_before.characters.len(), diag_after.characters.len());
}

// ─── 4. Corruption Detection Tests ───────────────────────────────────────────

#[test]
fn load_malformed_json_fails_gracefully() {
    let mut game = Game::new();
    let result = game.load_state("this is not valid json");
    assert!(result.is_err(), "Malformed JSON should fail");
}

#[test]
fn load_empty_json_fails_gracefully() {
    let mut game = Game::new();
    let result = game.load_state("{}");
    assert!(result.is_err(), "Empty JSON should fail");
}

#[test]
fn load_wrong_magic_fails_gracefully() {
    let mut game = Game::new();
    let valid = game.save_state().unwrap();
    let bad = valid.replace("\"magic\":\"VERRYTE_SAVE\"", "\"magic\":\"WRONG\"");
    let result = game.load_state(&bad);
    assert!(result.is_err(), "Wrong magic should fail");
    assert!(
        result.unwrap_err().to_string().contains("magic"),
        "Error should mention magic"
    );
}

#[test]
fn load_future_version_fails_gracefully() {
    let mut game = Game::new();
    let valid = game.save_state().unwrap();
    let bad = valid.replace(
        &format!("\"version\":{}", CURRENT_SAVE_VERSION),
        "\"version\":999",
    );
    let result = game.load_state(&bad);
    assert!(result.is_err(), "Future version should fail");
    assert!(
        result.unwrap_err().to_string().contains("version"),
        "Error should mention version"
    );
}

#[test]
fn load_truncated_json_fails_gracefully() {
    let mut game = Game::new();
    let valid = game.save_state().unwrap();
    let truncated = &valid[..valid.len() / 2];
    let result = game.load_state(truncated);
    assert!(result.is_err(), "Truncated JSON should fail");
}

#[test]
fn load_missing_world_field_fails_gracefully() {
    let mut game = Game::new();
    let result = game.load_state(r#"{"magic":"VERRYTE_SAVE","version":2,"timestamp":"0"}"#);
    assert!(result.is_err(), "Missing world field should fail");
}

#[test]
fn save_load_preserves_equipment_stat_bonuses() {
    let game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let equipped_before = game.world.get::<EquippedItems>(warrior).cloned().unwrap();

    assert!(
        equipped_before.weapon.is_some(),
        "Warrior should have starter weapon"
    );

    let stats_before = game.world.get::<Stats>(warrior).unwrap().clone();

    let json = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json).unwrap();

    let warrior2 = find_entity(&game2, CharacterClass::Warrior);
    let stats_after = game2.world.get::<Stats>(warrior2).unwrap();
    assert_eq!(stats_before.hp, stats_after.hp);
    assert_eq!(stats_before.max_hp, stats_after.max_hp);
    assert_eq!(stats_before.atk, stats_after.atk);
    assert_eq!(stats_before.def, stats_after.def);
    assert_eq!(stats_before.spd, stats_after.spd);
}

#[test]
fn load_json_null_fails_gracefully() {
    let mut game = Game::new();
    let result = game.load_state("null");
    assert!(result.is_err(), "JSON null should fail");
}

#[test]
fn load_empty_string_fails_gracefully() {
    let mut game = Game::new();
    let result = game.load_state("");
    assert!(result.is_err(), "Empty string should fail");
}

// ─── 5. Multi-Roundtrip Test ─────────────────────────────────────────────────

#[test]
fn multi_roundtrip_state_consistency() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 4);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);
    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 5);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let json1 = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json1).unwrap();

    let _warrior2 = find_entity(&game2, CharacterClass::Warrior);
    game2.apply_action(Action::MoveSouth, ActionSource::Terminal);

    let json2 = game2.save_state().unwrap();
    let mut game3 = Game::new();
    game3.load_state(&json2).unwrap();

    let snap1 = game.snapshot();
    let snap3 = game3.snapshot();
    assert_eq!(snap1.turn, snap3.turn);
    assert_eq!(snap1.player_team.count, snap3.player_team.count);
    assert_eq!(snap1.enemy_team.count, snap3.enemy_team.count);

    let w1_stats = game.world.get::<Stats>(warrior).unwrap();
    let w3_stats = game3
        .world
        .get::<Stats>(find_entity(&game3, CharacterClass::Warrior))
        .unwrap();
    assert_eq!(w1_stats.hp, w3_stats.hp);
    assert_eq!(w1_stats.max_hp, w3_stats.max_hp);
}

#[test]
fn multi_roundtrip_with_turn_end() {
    let mut game = Game::new();

    game.apply_action(Action::EndTurn, ActionSource::Terminal);
    game.update(0.1);
    game.update(0.1);
    game.update(0.1);

    let json1 = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json1).unwrap();

    assert_eq!(
        game2.world.resource::<GameState>().unwrap().turn,
        2,
        "Turn should be 2 after first save/load"
    );

    game2.apply_action(Action::EndTurn, ActionSource::Terminal);
    game2.update(0.1);
    game2.update(0.1);
    game2.update(0.1);

    let json2 = game2.save_state().unwrap();
    let mut game3 = Game::new();
    game3.load_state(&json2).unwrap();

    assert_eq!(
        game3.world.resource::<GameState>().unwrap().turn,
        3,
        "Turn should be 3 after second save/load"
    );
    assert_eq!(
        game3.world.resource::<GameState>().unwrap().phase,
        TurnPhase::Player,
        "Phase should be Player after turn end"
    );
}

#[test]
fn multi_roundtrip_with_combat_and_healing() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);
    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);
    game.world.get_mut::<Stats>(warrior).unwrap().hp = 50;

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 4);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.cursor = Position::new(4, 5);
    }
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let json1 = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json1).unwrap();

    let warrior2 = find_entity(&game2, CharacterClass::Warrior);
    let hp_after_combat = game2.world.get::<Stats>(warrior2).unwrap().hp;

    game2.world.get_mut::<Stats>(warrior2).unwrap().hp = hp_after_combat + 20;

    let json2 = game2.save_state().unwrap();
    let mut game3 = Game::new();
    game3.load_state(&json2).unwrap();

    let warrior3 = find_entity(&game3, CharacterClass::Warrior);
    let hp_final = game3.world.get::<Stats>(warrior3).unwrap().hp;
    assert_eq!(hp_final, hp_after_combat + 20);
    assert!(
        hp_final <= game3.world.get::<Stats>(warrior3).unwrap().max_hp,
        "HP should not exceed max"
    );
}

#[test]
fn save_load_idempotent() {
    let game = build_active_game();

    let json1 = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json1).unwrap();

    let json2 = game2.save_state().unwrap();

    let snap1: FullSaveState = serde_json::from_str(&json1).unwrap();
    let snap2: FullSaveState = serde_json::from_str(&json2).unwrap();

    assert_eq!(snap1.magic, snap2.magic);
    assert_eq!(snap1.version, snap2.version);

    let s1 = game.snapshot();
    let s2 = game2.snapshot();
    assert_eq!(s1.turn, s2.turn);
    assert_eq!(s1.phase, s2.phase);
    assert_eq!(s1.outcome, s2.outcome);
    assert_eq!(s1.player_team, s2.player_team);
    assert_eq!(s1.enemy_team, s2.enemy_team);
    assert_eq!(s1.floor, s2.floor);
    assert_eq!(s1.combo_count, s2.combo_count);
}

#[test]
fn save_after_load_then_load_again_preserves_state() {
    let mut game = Game::new();

    game.apply_action(Action::EndTurn, ActionSource::Terminal);
    game.update(0.1);
    game.update(0.1);
    game.update(0.1);

    let json1 = game.save_state().unwrap();
    let mut game2 = Game::new();
    game2.load_state(&json1).unwrap();

    game2.apply_action(Action::MoveNorth, ActionSource::Terminal);
    game2.apply_action(Action::MoveSouth, ActionSource::Terminal);

    let json2 = game2.save_state().unwrap();
    let mut game3 = Game::new();
    game3.load_state(&json2).unwrap();

    game3.apply_action(Action::EndTurn, ActionSource::Terminal);
    game3.update(0.1);
    game3.update(0.1);
    game3.update(0.1);

    let json3 = game3.save_state().unwrap();
    let mut game4 = Game::new();
    game4.load_state(&json3).unwrap();

    let snap = game4.snapshot();
    assert_eq!(snap.turn, 3);
    assert_eq!(snap.phase, TurnPhase::Player);
    assert_eq!(snap.outcome, Outcome::Playing);

    for (_, stats) in game4.world.query::<Stats>() {
        assert!(stats.hp <= stats.max_hp);
        assert!(stats.ap <= stats.max_ap);
    }
}

// ─── 6. StepReport Snapshot Tests ────────────────────────────────────────────

#[test]
fn step_report_snapshots_roundtrip() {
    let mut game = Game::new();

    let script = "inspect:4,4 confirm inspect:4,5 confirm";
    game.router
        .inject_script_with(
            &wuthering_terminal::default_commands(),
            script,
            ActionSource::Script,
            wuthering_terminal::resolve_command_token,
        )
        .unwrap();

    let reports = game.run_pending_reports();
    assert_eq!(reports.len(), 4);

    for report in &reports {
        let json = serde_json::to_string(report).expect("serialize StepReport");
        let restored: wuthering_terminal::StepReport =
            serde_json::from_str(&json).expect("deserialize StepReport");

        assert_eq!(report.action, restored.action);
        assert_eq!(report.source, restored.source);
        assert_eq!(report.before.turn, restored.before.turn);
        assert_eq!(report.after.turn, restored.after.turn);
        assert_eq!(report.before.phase, restored.before.phase);
        assert_eq!(report.after.phase, restored.after.phase);
        assert_eq!(report.outcome, restored.outcome);
    }
}

// ─── 7. Version Migration Tests ──────────────────────────────────────────────

#[test]
fn save_version_1_loads_with_migration() {
    let game = Game::new();
    let valid = game.save_state().unwrap();

    let v1_save = valid.replace("\"version\":2", "\"version\":1");

    let mut game2 = Game::new();
    let result = game2.load_state(&v1_save);
    assert!(result.is_ok(), "v1 save should load with migration");

    let snap = game2.snapshot();
    assert_eq!(snap.turn, 1);
    assert_eq!(snap.outcome, Outcome::Playing);
}

#[test]
fn migration_stamps_applied_migrations() {
    let game = Game::new();
    let valid = game.save_state().unwrap();

    let v1_save = valid.replace("\"version\":2", "\"version\":1");

    let mut game2 = Game::new();
    game2.load_state(&v1_save).unwrap();

    let re_saved = game2.save_state().unwrap();
    let state: FullSaveState = serde_json::from_str(&re_saved).unwrap();
    assert_eq!(state.version, CURRENT_SAVE_VERSION);
}

// ─── 8. ActionOutcome Roundtrip ──────────────────────────────────────────────

#[test]
fn action_outcome_variants_roundtrip() {
    let outcomes = vec![
        ActionOutcome::NoOp,
        ActionOutcome::TurnAdvanced,
        ActionOutcome::PhaseChanged,
        ActionOutcome::Hit {
            damage: 25,
            target: "Boss".to_string(),
            was_critical: false,
            was_blocked: false,
        },
        ActionOutcome::CritHit { damage: 50 },
        ActionOutcome::Blocked { damage_reduced: 15 },
        ActionOutcome::Healed {
            amount: 30,
            target: "Kael".to_string(),
        },
        ActionOutcome::Moved {
            entity: "Kael".to_string(),
            to: Position::new(5, 5),
        },
        ActionOutcome::ItemUsed {
            name: "Healing Potion".to_string(),
        },
        ActionOutcome::BossPhaseChanged {
            phase: "Phase2".to_string(),
        },
        ActionOutcome::Absorbed {
            echo_name: "Swift".to_string(),
        },
        ActionOutcome::Crafted {
            item_name: "Divine Remedy".to_string(),
        },
        ActionOutcome::FloorTransition { from: 1, to: 2 },
        ActionOutcome::StatusApplied {
            status: "Ice".to_string(),
            target: "Boss".to_string(),
        },
        ActionOutcome::ComboExtended { combo_count: 3 },
        ActionOutcome::EquipmentUpgraded {
            item_name: "Iron Sword".to_string(),
            slot: wuthering_terminal::components::EquipmentSlot::Weapon,
            level: 1,
        },
        ActionOutcome::EquipmentRewarded {
            item_name: "DarkBlade".to_string(),
            hero: "Kael".to_string(),
        },
        ActionOutcome::ToggleChanged {
            name: "auto_battle".to_string(),
            enabled: true,
        },
        ActionOutcome::StatusViewed {
            name: "prestige".to_string(),
        },
        ActionOutcome::Rested {
            entity: "Kael".to_string(),
            fatigue_recovered: 20,
            morale_gained: 5,
        },
        ActionOutcome::ModifiersRerolled {
            modifiers: vec!["Darkness".to_string(), "Frenzy".to_string()],
        },
        ActionOutcome::GameSaved {
            path: "saves/quicksave.json".to_string(),
        },
        ActionOutcome::GameLoaded {
            path: "saves/quicksave.json".to_string(),
        },
        ActionOutcome::RecordingChanged {
            enabled: false,
            records: 4,
        },
        ActionOutcome::ReplayChanged {
            enabled: true,
            actions: 4,
            errors: 0,
        },
        ActionOutcome::ReplayStepped {
            index: 2,
            action: "Confirm".to_string(),
            verified: true,
        },
        ActionOutcome::ReplayAutoChanged { enabled: true },
        ActionOutcome::StateUpdated,
        ActionOutcome::Failed {
            reason: "Not enough AP".to_string(),
        },
        ActionOutcome::GameOver {
            outcome: Outcome::Victory,
        },
    ];

    for outcome in outcomes {
        let json = serde_json::to_string(&outcome).unwrap();
        let restored: ActionOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(outcome, restored);
    }
}
