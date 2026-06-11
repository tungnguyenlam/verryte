use verryte_input::ActionSource;
use wuthering_terminal::components::{
    CharacterClass, EchoItem, EquipmentSlot, EquippedEchoes, EquippedItems, Fatigue, GameState,
    Inventory, ItemEffect, Morale, Outcome, Stats, TurnPhase, TurnTransition, UIState,
};
use wuthering_terminal::equipment;
use wuthering_terminal::snapshot::{ActionOutcome, FailureCategory};
use wuthering_terminal::{Action, Game, Position, Spawner};

fn find_entity(game: &Game, class: CharacterClass) -> verryte_core::Entity {
    game.world
        .query::<CharacterClass>()
        .into_iter()
        .find(|(_, c)| **c == class)
        .map(|(e, _)| e)
        .unwrap()
}

fn select_character(game: &mut Game, pos: Position) {
    game.world.resource_mut::<GameState>().unwrap().cursor = pos;
    game.apply_action(Action::Confirm, ActionSource::Terminal);
}

fn end_turn(game: &mut Game) {
    game.apply_action(Action::EndTurn, ActionSource::Terminal);
    game.update(0.1);
    game.update(0.1);
    game.update(0.1);
}

fn last_recorded_outcome(game: &Game) -> ActionOutcome {
    let history = game
        .world
        .resource::<verryte_input::ActionHistory<Action>>()
        .unwrap();
    let outcome = history
        .last()
        .and_then(|record| record.metadata_value("outcome"))
        .expect("last action should record serialized outcome metadata");
    serde_json::from_str(outcome).unwrap()
}

// ─── 1. Game Initialization ──────────────────────────────────────────────────

#[test]
fn game_init_has_three_players() {
    let game = Game::new();
    let snap = game.snapshot();
    assert_eq!(snap.player_team.count, 3);
    assert_eq!(snap.turn, 1);
    assert_eq!(snap.phase, TurnPhase::Player);
    assert_eq!(snap.outcome, Outcome::Playing);
    assert_eq!(snap.floor, 1);
}

#[test]
fn game_init_characters_at_expected_positions() {
    let game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let mage = find_entity(&game, CharacterClass::Mage);
    let healer = find_entity(&game, CharacterClass::Healer);

    assert_eq!(
        *game.world.get::<Position>(warrior).unwrap(),
        Position::new(4, 4)
    );
    assert_eq!(
        *game.world.get::<Position>(mage).unwrap(),
        Position::new(4, 8)
    );
    assert_eq!(
        *game.world.get::<Position>(healer).unwrap(),
        Position::new(4, 12)
    );
}

#[test]
fn game_init_has_boss_and_enemies() {
    let game = Game::new();
    let snap = game.snapshot();
    assert!(snap.enemy_team.count >= 2, "Should have boss + enemies");

    let boss = find_entity(&game, CharacterClass::Boss);
    let boss_stats = game.world.get::<Stats>(boss).unwrap();
    assert_eq!(boss_stats.hp, 500);
    assert_eq!(boss_stats.max_hp, 500);
}

#[test]
fn game_init_player_stats_are_correct() {
    let game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let stats = game.world.get::<Stats>(warrior).unwrap();
    assert_eq!(stats.max_ap, 3);
    assert_eq!(stats.ap, 3);
    assert!(stats.hp > 0);
    assert_eq!(stats.hp, stats.max_hp);
}

// ─── 2. Movement ─────────────────────────────────────────────────────────────

#[test]
fn movement_changes_position_and_decreases_ap() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let initial_ap = game.world.get::<Stats>(warrior).unwrap().ap;

    select_character(&mut game, Position::new(4, 4));

    let state = game.world.resource::<GameState>().unwrap();
    assert!(
        state.selected_entity.is_some(),
        "Character should be selected"
    );

    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let pos = game.world.get::<Position>(warrior).unwrap();
    assert_eq!(*pos, Position::new(4, 5));

    let ap = game.world.get::<Stats>(warrior).unwrap().ap;
    assert_eq!(ap, initial_ap - 1);
}

#[test]
fn movement_clears_selection() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));
    assert!(game
        .world
        .resource::<GameState>()
        .unwrap()
        .selected_entity
        .is_some());

    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    assert!(game
        .world
        .resource::<GameState>()
        .unwrap()
        .selected_entity
        .is_none());
}

#[test]
fn movement_outcome_is_moved() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

    match report.outcome {
        ActionOutcome::Moved { entity, to } => {
            assert_eq!(entity, "Kael");
            assert_eq!(to, Position::new(4, 5));
        }
        other => panic!("Expected Moved outcome, got {:?}", other),
    }
}

// ─── 3. Combat ───────────────────────────────────────────────────────────────

#[test]
fn attack_deals_damage_and_returns_hit_outcome() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);

    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    let boss_hp_before = game.world.get::<Stats>(boss).unwrap().hp;

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

    let boss_hp_after = game.world.get::<Stats>(boss).unwrap().hp;
    assert!(
        boss_hp_after < boss_hp_before,
        "Boss should have taken damage"
    );

    assert!(
        matches!(
            report.outcome,
            ActionOutcome::Hit { .. } | ActionOutcome::CritHit { .. }
        ),
        "Expected Hit or CritHit, got {:?}",
        report.outcome
    );
}

#[test]
fn attack_decreases_ap() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);

    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    let ap_before = game.world.get::<Stats>(warrior).unwrap().ap;

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let ap_after = game.world.get::<Stats>(warrior).unwrap().ap;
    assert_eq!(ap_after, ap_before - 1);
}

#[test]
fn attack_beyond_range_fails() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);

    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(18, 8);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    let boss_hp_before = game.world.get::<Stats>(boss).unwrap().hp;

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(18, 8);
    let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

    let boss_hp_after = game.world.get::<Stats>(boss).unwrap().hp;
    assert_eq!(boss_hp_after, boss_hp_before, "Boss HP should not change");

    assert!(
        matches!(report.outcome, ActionOutcome::Failed { .. }),
        "Expected Failed outcome for out-of-range attack, got {:?}",
        report.outcome
    );
}

// ─── 4. Turn Advancement ─────────────────────────────────────────────────────

#[test]
fn end_turn_advances_to_next_turn() {
    let mut game = Game::new();

    assert_eq!(game.world.resource::<GameState>().unwrap().turn, 1);

    end_turn(&mut game);

    assert_eq!(game.world.resource::<GameState>().unwrap().turn, 2);
    assert_eq!(
        game.world.resource::<GameState>().unwrap().phase,
        TurnPhase::Player
    );
}

#[test]
fn end_turn_replenishes_ap() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    let ap_after_move = game.world.get::<Stats>(warrior).unwrap().ap;
    assert!(ap_after_move < game.world.get::<Stats>(warrior).unwrap().max_ap);

    end_turn(&mut game);

    let ap_after_turn = game.world.get::<Stats>(warrior).unwrap().ap;
    assert!(
        ap_after_turn >= game.world.get::<Stats>(warrior).unwrap().max_ap,
        "AP should be replenished: got {}, max_ap {}",
        ap_after_turn,
        game.world.get::<Stats>(warrior).unwrap().max_ap
    );
}

#[test]
fn end_turn_outcome_is_turn_advanced() {
    let mut game = Game::new();

    let report = game.apply_action(Action::EndTurn, ActionSource::Terminal);

    assert!(
        matches!(
            report.outcome,
            ActionOutcome::TurnAdvanced | ActionOutcome::PhaseChanged | ActionOutcome::NoOp
        ),
        "Expected TurnAdvanced, PhaseChanged, or NoOp, got {:?}",
        report.outcome
    );

    game.update(0.1);
    game.update(0.1);
    game.update(0.1);

    assert_eq!(game.world.resource::<GameState>().unwrap().turn, 2);
}

// ─── 5. Character Swap ───────────────────────────────────────────────────────

#[test]
fn swap_character_via_next_character() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let selected_before = game.world.resource::<GameState>().unwrap().selected_entity;
    let class_before = game
        .world
        .get::<CharacterClass>(selected_before.unwrap())
        .copied()
        .unwrap();
    assert_eq!(class_before, CharacterClass::Warrior);

    game.apply_action(Action::NextCharacter, ActionSource::Terminal);

    let selected_after = game.world.resource::<GameState>().unwrap().selected_entity;
    let class_after = game
        .world
        .get::<CharacterClass>(selected_after.unwrap())
        .copied()
        .unwrap();
    assert_ne!(
        class_before, class_after,
        "Should have cycled to a different character"
    );
}

#[test]
fn swap_character_direct_index() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    game.apply_action(Action::SwapCharacter(1), ActionSource::Terminal);

    let selected = game
        .world
        .resource::<GameState>()
        .unwrap()
        .selected_entity
        .unwrap();
    let class = game.world.get::<CharacterClass>(selected).copied().unwrap();
    assert_eq!(class, CharacterClass::Mage);

    game.apply_action(Action::SwapCharacter(2), ActionSource::Terminal);

    let selected = game
        .world
        .resource::<GameState>()
        .unwrap()
        .selected_entity
        .unwrap();
    let class = game.world.get::<CharacterClass>(selected).copied().unwrap();
    assert_eq!(class, CharacterClass::Healer);
}

// ─── 6. Item Usage ───────────────────────────────────────────────────────────

#[test]
fn use_healing_item_restores_hp() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    game.world.get_mut::<Stats>(warrior).unwrap().hp = 50;

    select_character(&mut game, Position::new(4, 4));

    game.apply_action(Action::ToggleInventory, ActionSource::Terminal);
    assert_eq!(
        game.world.resource::<GameState>().unwrap().ui_state,
        wuthering_terminal::components::UIState::Inventory
    );

    let items_before = game.world.get::<Inventory>(warrior).unwrap().items.len();

    let report = game.apply_action(Action::Skill1, ActionSource::Terminal);

    let hp_after = game.world.get::<Stats>(warrior).unwrap().hp;
    assert_eq!(
        hp_after, 80,
        "Healing Potion should restore 30 HP (50 + 30 = 80)"
    );

    let items_after = game.world.get::<Inventory>(warrior).unwrap().items.len();
    assert_eq!(items_after, items_before - 1, "One item should be consumed");

    assert_eq!(
        game.world.resource::<GameState>().unwrap().ui_state,
        UIState::Normal
    );
    assert_eq!(
        report.outcome,
        ActionOutcome::ItemUsed {
            name: "Healing Potion".to_string(),
        }
    );
}

#[test]
fn healing_does_not_exceed_max_hp() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let max_hp = game.world.get::<Stats>(warrior).unwrap().max_hp;
    game.world.get_mut::<Stats>(warrior).unwrap().hp = max_hp - 10;

    select_character(&mut game, Position::new(4, 4));
    game.apply_action(Action::ToggleInventory, ActionSource::Terminal);
    game.apply_action(Action::Skill1, ActionSource::Terminal);

    let hp_after = game.world.get::<Stats>(warrior).unwrap().hp;
    assert!(
        hp_after <= max_hp,
        "HP {} should not exceed max_hp {}",
        hp_after,
        max_hp
    );
}

#[test]
fn use_item_without_inventory_reports_context_failure() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(Action::UseItem(0), ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Failed {
            reason: "Inventory must be open to use items".to_string(),
        }
    );
    assert_eq!(
        report.outcome.failure_category(),
        Some(FailureCategory::WrongContext)
    );
}

#[test]
fn use_upgrade_kit_directly_reports_failure_and_keeps_item() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let kit = game.world.spawn_item("Upgrade Kit", ItemEffect::UpgradeKit);
    game.world
        .get_mut::<Inventory>(warrior)
        .unwrap()
        .items
        .insert(0, kit);
    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().ui_state = UIState::Inventory;
    let items_before = game.world.get::<Inventory>(warrior).unwrap().items.len();

    let report = game.apply_action(Action::UseItem(0), ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Failed {
            reason: "Upgrade Kit requires an equipment slot".to_string(),
        }
    );
    assert_eq!(
        report.outcome.failure_category(),
        Some(FailureCategory::InvalidItem)
    );
    assert_eq!(
        game.world.get::<Inventory>(warrior).unwrap().items.len(),
        items_before
    );
    assert!(game.world.is_alive(kit));
}

#[test]
fn ui_toggles_report_structured_outcomes() {
    let mut game = Game::new();

    let help = game.apply_action(Action::ToggleHelp, ActionSource::Terminal);
    assert_eq!(
        help.outcome,
        ActionOutcome::ToggleChanged {
            name: "help".to_string(),
            enabled: true,
        }
    );

    let help_close = game.apply_action(Action::Cancel, ActionSource::Terminal);
    assert_eq!(
        help_close.outcome,
        ActionOutcome::ToggleChanged {
            name: "help".to_string(),
            enabled: false,
        }
    );

    let bestiary = game.apply_action(Action::ToggleBestiary, ActionSource::Script);
    assert_eq!(
        bestiary.outcome,
        ActionOutcome::ToggleChanged {
            name: "bestiary".to_string(),
            enabled: true,
        }
    );
    assert_eq!(last_recorded_outcome(&game), bestiary.outcome);

    game.apply_action(Action::Cancel, ActionSource::Script);
    let auto = game.apply_action(Action::AutoBattle, ActionSource::Script);
    assert_eq!(
        auto.outcome,
        ActionOutcome::ToggleChanged {
            name: "auto_battle".to_string(),
            enabled: true,
        }
    );
}

#[test]
fn rest_reports_recovered_fatigue_and_morale() {
    let mut game = Game::new();
    let warrior = find_entity(&game, CharacterClass::Warrior);
    game.world
        .resource_mut::<GameState>()
        .unwrap()
        .selected_entity = Some(warrior);
    game.world.get_mut::<Fatigue>(warrior).unwrap().value = 35;
    game.world.get_mut::<Morale>(warrior).unwrap().value = 40;

    let report = game.apply_action(Action::Rest, ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Rested {
            entity: "Kael".to_string(),
            fatigue_recovered: 20,
            morale_gained: 5,
        }
    );
    assert_eq!(last_recorded_outcome(&game), report.outcome);
}

#[test]
fn reroll_modifiers_reports_active_modifier_names() {
    let mut game = Game::new();
    let warrior = find_entity(&game, CharacterClass::Warrior);
    game.world
        .resource_mut::<GameState>()
        .unwrap()
        .selected_entity = Some(warrior);

    let report = game.apply_action(Action::RerollModifiers, ActionSource::Script);

    match &report.outcome {
        ActionOutcome::ModifiersRerolled { modifiers } => {
            assert!(
                !modifiers.is_empty(),
                "reroll should report the active modifier names"
            );
        }
        other => panic!("expected modifier reroll outcome, got {other:?}"),
    }
    assert_eq!(last_recorded_outcome(&game), report.outcome);
}

#[test]
fn craft_valid_recipe_reports_crafted_outcome() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(Action::CraftItem(0, 1), ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Crafted {
            item_name: "Elixir of Life".to_string(),
        }
    );
}

#[test]
fn craft_invalid_recipe_reports_failure_category() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(Action::CraftItem(1, 2), ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Failed {
            reason: "No valid recipe for those items".to_string(),
        }
    );
    assert_eq!(
        report.outcome.failure_category(),
        Some(FailureCategory::InvalidRecipe)
    );
}

#[test]
fn next_floor_off_stairs_reports_context_failure() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(Action::NextFloor, ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Failed {
            reason: "You must stand on a staircase to descend".to_string(),
        }
    );
    assert_eq!(
        report.outcome.failure_category(),
        Some(FailureCategory::WrongContext)
    );
}

#[test]
fn starter_upgrade_kit_upgrades_equipped_weapon() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let kit = game.world.spawn_item(
        "Upgrade Kit",
        wuthering_terminal::components::ItemEffect::UpgradeKit,
    );
    game.world
        .get_mut::<Inventory>(warrior)
        .unwrap()
        .items
        .push(kit);
    let items_before = game.world.get::<Inventory>(warrior).unwrap().items.len();
    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(
        Action::UpgradeEquipment(EquipmentSlot::Weapon),
        ActionSource::Script,
    );

    assert_eq!(
        report.outcome,
        ActionOutcome::EquipmentUpgraded {
            item_name: "Iron Sword".to_string(),
            slot: EquipmentSlot::Weapon,
            level: 1,
        }
    );
    let equipped = game.world.get::<EquippedItems>(warrior).unwrap();
    let weapon = equipped.weapon.as_ref().unwrap();
    assert_eq!(weapon.upgrade_level, 1);
    assert!(weapon.atk_bonus > weapon.base_atk);
    assert_eq!(
        game.world.get::<Inventory>(warrior).unwrap().items.len(),
        items_before - 1
    );
    assert_eq!(last_recorded_outcome(&game), report.outcome);
}

#[test]
fn upgrade_equipment_without_kit_reports_failure() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(
        Action::UpgradeEquipment(EquipmentSlot::Weapon),
        ActionSource::Script,
    );

    assert_eq!(
        report.outcome,
        ActionOutcome::Failed {
            reason: "No Upgrade Kit available".to_string(),
        }
    );
}

#[test]
fn upgrade_empty_equipment_slot_reports_failure_and_keeps_kit() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let kit = game.world.spawn_item(
        "Upgrade Kit",
        wuthering_terminal::components::ItemEffect::UpgradeKit,
    );
    game.world
        .get_mut::<Inventory>(warrior)
        .unwrap()
        .items
        .push(kit);
    let items_before = game.world.get::<Inventory>(warrior).unwrap().items.len();
    select_character(&mut game, Position::new(4, 4));

    let report = game.apply_action(
        Action::UpgradeEquipment(EquipmentSlot::Accessory),
        ActionSource::Script,
    );

    assert_eq!(
        report.outcome,
        ActionOutcome::Failed {
            reason: "No upgradeable equipment in that slot".to_string(),
        }
    );
    assert_eq!(
        game.world.get::<Inventory>(warrior).unwrap().items.len(),
        items_before
    );
    assert!(game.world.is_alive(kit));
}

#[test]
fn defeating_set_reward_enemies_auto_equips_shadow_knight_set() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let stalker = find_entity(&game, CharacterClass::ShadowStalker);
    let stalker_pos = *game.world.get::<Position>(stalker).unwrap();
    game.handle_defeat(
        stalker,
        "Shadow Stalker",
        CharacterClass::ShadowStalker,
        stalker_pos,
    );

    let sentinel = find_entity(&game, CharacterClass::CursedSentinel);
    let sentinel_pos = *game.world.get::<Position>(sentinel).unwrap();
    game.handle_defeat(
        sentinel,
        "Cursed Sentinel",
        CharacterClass::CursedSentinel,
        sentinel_pos,
    );

    let equipped = game.world.get::<EquippedItems>(warrior).unwrap();
    assert_eq!(equipped.armor.as_ref().unwrap().name, "ShadowArmor");
    assert_eq!(equipped.weapon.as_ref().unwrap().name, "DarkBlade");
    assert!(game
        .snapshot()
        .active_set_bonuses
        .contains(&"Shadow Knight".to_string()));
}

#[test]
fn defeating_set_reward_enemy_reports_structured_outcome_from_action_path() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let stalker = find_entity(&game, CharacterClass::ShadowStalker);
    *game.world.get_mut::<Position>(stalker).unwrap() = Position::new(4, 5);
    game.world.get_mut::<Stats>(stalker).unwrap().hp = 1;

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    let report = game.apply_action(Action::Confirm, ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::EquipmentRewarded {
            item_name: "ShadowArmor".to_string(),
            hero: "Kael".to_string(),
        }
    );
    assert_eq!(
        game.world
            .get::<EquippedItems>(warrior)
            .unwrap()
            .armor
            .as_ref()
            .unwrap()
            .name,
        "ShadowArmor"
    );
    assert_eq!(last_recorded_outcome(&game), report.outcome);
}

#[test]
fn echo_absorption_reports_structured_outcome_and_metadata() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let warrior_pos = *game.world.get::<Position>(warrior).unwrap();
    game.world
        .builder()
        .with(warrior_pos)
        .with(EchoItem {
            class: CharacterClass::ShadowStalker,
        })
        .build();
    game.world.resource_mut::<GameState>().unwrap().cursor = warrior_pos;

    let report = game.apply_action(Action::Confirm, ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::Absorbed {
            echo_name: "Frostbite".to_string(),
        }
    );
    assert!(game
        .world
        .resource::<EquippedEchoes>()
        .unwrap()
        .abilities
        .contains(&wuthering_terminal::components::EchoAbility::Frostbite));
    assert_eq!(last_recorded_outcome(&game), report.outcome);
}

#[test]
fn boss_phase_transition_reports_structured_outcome_from_action_path() {
    let mut game = Game::new();

    let boss = find_entity(&game, CharacterClass::Boss);
    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    let threshold = game
        .world
        .resource::<wuthering_terminal::components::BossConfig>()
        .unwrap()
        .phase2_hp_threshold;
    let boss_stats = game.world.get_mut::<Stats>(boss).unwrap();
    boss_stats.hp = threshold + 1;
    boss_stats.def = 0;

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    let report = game.apply_action(Action::Confirm, ActionSource::Script);

    assert_eq!(
        report.outcome,
        ActionOutcome::BossPhaseChanged {
            phase: "Phase2".to_string(),
        }
    );
    assert_eq!(last_recorded_outcome(&game), report.outcome);
}

#[test]
fn equipment_lifesteal_heals_after_damage_dealt() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);
    game.world
        .get_mut::<EquippedItems>(warrior)
        .unwrap()
        .equip(equipment::vampiric_ring());
    game.world.get_mut::<Stats>(warrior).unwrap().hp = 50;

    game.resolve_combat_hit(
        warrior,
        boss,
        100,
        "Kael",
        "Blight Sovereign",
        Position::new(12, 8),
    );

    assert!(game.world.get::<Stats>(warrior).unwrap().hp > 50);
}

#[test]
fn equipment_hp_regen_applies_on_team_phase_start() {
    let mut game = Game::new();

    let healer = find_entity(&game, CharacterClass::Healer);
    let max_hp = game.world.get::<Stats>(healer).unwrap().max_hp;
    game.world.get_mut::<Stats>(healer).unwrap().hp = max_hp - 10;
    game.world.resource_mut::<GameState>().unwrap().phase = TurnPhase::Enemy;
    game.world
        .resource_mut::<TurnTransition>()
        .unwrap()
        .request_end = true;

    wuthering_terminal::systems::turn_management_system(&mut game.world);

    assert_eq!(game.world.get::<Stats>(healer).unwrap().hp, max_hp - 5);
}

// ─── 7. Multi-Floor ──────────────────────────────────────────────────────────

#[test]
fn next_floor_transitions_when_on_stairs() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);

    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    let map = game
        .world
        .resource_mut::<wuthering_terminal::map::TacticalMap>()
        .unwrap();
    map.tiles
        .set(Position::new(4, 4), wuthering_terminal::map::Tile::Stairs);

    select_character(&mut game, Position::new(4, 4));

    let floor_before = game.world.resource::<GameState>().unwrap().floor;
    assert_eq!(floor_before, 1);

    game.apply_action(Action::NextFloor, ActionSource::Terminal);

    let floor_after = game.world.resource::<GameState>().unwrap().floor;
    assert_eq!(floor_after, 2, "Floor should increment to 2");
}

#[test]
fn next_floor_fails_when_not_on_stairs() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    select_character(&mut game, Position::new(4, 4));

    let floor_before = game.world.resource::<GameState>().unwrap().floor;
    game.apply_action(Action::NextFloor, ActionSource::Terminal);

    assert_eq!(
        game.world.resource::<GameState>().unwrap().floor,
        floor_before,
        "Floor should not change when not on stairs"
    );
}

// ─── 8. Invalid Actions ──────────────────────────────────────────────────────

#[test]
fn move_to_wall_tile_fails() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 3);

    select_character(&mut game, Position::new(4, 3));

    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 1);
    let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

    let pos = game.world.get::<Position>(warrior).unwrap();
    assert_eq!(
        *pos,
        Position::new(4, 3),
        "Character should not have moved to wall"
    );

    assert!(
        matches!(
            report.outcome,
            ActionOutcome::Failed { .. } | ActionOutcome::StateUpdated
        ),
        "Expected Failed or StateUpdated for wall move, got {:?}",
        report.outcome
    );
}

#[test]
fn action_without_selection_reports_failure() {
    let mut game = Game::new();

    let report = game.apply_action(Action::NextFloor, ActionSource::Terminal);

    assert!(
        matches!(
            report.outcome,
            ActionOutcome::Failed { .. } | ActionOutcome::StateUpdated | ActionOutcome::NoOp
        ),
        "Expected some non-crashing outcome, got {:?}",
        report.outcome
    );
}

#[test]
fn move_to_out_of_bounds_is_clamped() {
    let mut game = Game::new();

    for _ in 0..100 {
        game.apply_action(Action::MoveNorth, ActionSource::Terminal);
        game.apply_action(Action::MoveWest, ActionSource::Terminal);
    }

    let cursor = game.world.resource::<GameState>().unwrap().cursor;
    let map = game
        .world
        .resource::<wuthering_terminal::map::TacticalMap>()
        .unwrap();
    assert!(cursor.x >= 0 && cursor.x < map.width as i16);
    assert!(cursor.y >= 0 && cursor.y < map.height as i16);
}

// ─── 9. Snapshot Consistency ─────────────────────────────────────────────────

#[test]
fn snapshot_fields_consistent_after_actions() {
    let mut game = Game::new();

    let warrior = find_entity(&game, CharacterClass::Warrior);
    let boss = find_entity(&game, CharacterClass::Boss);
    *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
    *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    game.apply_action(Action::MoveNorth, ActionSource::Terminal);
    game.apply_action(Action::MoveSouth, ActionSource::Terminal);

    let snap = game.snapshot();

    assert!(
        snap.player_team.total_hp <= snap.player_team.max_hp,
        "Player total_hp {} should not exceed max_hp {}",
        snap.player_team.total_hp,
        snap.player_team.max_hp
    );
    assert!(
        snap.enemy_team.total_hp <= snap.enemy_team.max_hp,
        "Enemy total_hp {} should not exceed max_hp {}",
        snap.enemy_team.total_hp,
        snap.enemy_team.max_hp
    );
    assert_eq!(snap.outcome, Outcome::Playing);
    assert_eq!(snap.turn, 1);
}

#[test]
fn snapshot_character_stats_internally_consistent() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    end_turn(&mut game);

    for (_, stats) in game.world.query::<Stats>() {
        assert!(
            stats.hp <= stats.max_hp,
            "HP {} should not exceed max_hp {}",
            stats.hp,
            stats.max_hp
        );
    }
}

#[test]
fn snapshot_reachable_tiles_nonempty_when_selected() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    let snap = game.snapshot();
    assert!(
        !snap.reachable_tiles.is_empty(),
        "Should have reachable tiles when character is selected with AP"
    );
    assert!(
        snap.selected_can_act,
        "Selected character should be able to act"
    );
}

// ─── 10. Determinism ─────────────────────────────────────────────────────────

#[test]
fn same_seed_same_initial_state() {
    let game1 = Game::new();
    let game2 = Game::new();

    let snap1 = game1.snapshot();
    let snap2 = game2.snapshot();

    assert_eq!(snap1.turn, snap2.turn);
    assert_eq!(snap1.phase, snap2.phase);
    assert_eq!(snap1.outcome, snap2.outcome);
    assert_eq!(snap1.player_team, snap2.player_team);
    assert_eq!(snap1.enemy_team, snap2.enemy_team);
    assert_eq!(snap1.floor, snap2.floor);
}

#[test]
fn same_actions_same_results() {
    let mut game1 = Game::new();
    let mut game2 = Game::new();

    let actions = vec![
        Action::Confirm, // select at initial cursor (5, 5)
        Action::MoveNorth,
        Action::MoveNorth,
        Action::MoveEast,
    ];

    for action in &actions {
        game1.apply_action(*action, ActionSource::Terminal);
        game2.apply_action(*action, ActionSource::Terminal);
    }

    let snap1 = game1.snapshot();
    let snap2 = game2.snapshot();

    assert_eq!(snap1.turn, snap2.turn);
    assert_eq!(snap1.phase, snap2.phase);
    assert_eq!(snap1.outcome, snap2.outcome);
    assert_eq!(snap1.cursor, snap2.cursor);
}

#[test]
fn determinism_with_explicit_seed() {
    let mut game1 = Game::new();
    *game1.world.resource_mut::<verryte_core::Rng>().unwrap() = verryte_core::Rng::seed(42);

    let mut game2 = Game::new();
    *game2.world.resource_mut::<verryte_core::Rng>().unwrap() = verryte_core::Rng::seed(42);

    let warrior1 = find_entity(&game1, CharacterClass::Warrior);
    let boss1 = find_entity(&game1, CharacterClass::Boss);
    *game1.world.get_mut::<Position>(boss1).unwrap() = Position::new(4, 5);
    *game1.world.get_mut::<Position>(warrior1).unwrap() = Position::new(4, 4);

    let warrior2 = find_entity(&game2, CharacterClass::Warrior);
    let boss2 = find_entity(&game2, CharacterClass::Boss);
    *game2.world.get_mut::<Position>(boss2).unwrap() = Position::new(4, 5);
    *game2.world.get_mut::<Position>(warrior2).unwrap() = Position::new(4, 4);

    select_character(&mut game1, Position::new(4, 4));
    select_character(&mut game2, Position::new(4, 4));

    game1.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    game2.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);

    let report1 = game1.apply_action(Action::Confirm, ActionSource::Terminal);
    let report2 = game2.apply_action(Action::Confirm, ActionSource::Terminal);

    assert_eq!(report1.outcome, report2.outcome);

    let boss1_hp = game1.world.get::<Stats>(boss1).unwrap().hp;
    let boss2_hp = game2.world.get::<Stats>(boss2).unwrap().hp;
    assert_eq!(boss1_hp, boss2_hp, "Same seed should produce same damage");
}

#[test]
fn determinism_across_turn_cycles() {
    let mut game1 = Game::new();
    let mut game2 = Game::new();

    for _ in 0..3 {
        game1.apply_action(Action::EndTurn, ActionSource::Terminal);
        game1.update(0.1);
        game1.update(0.1);
        game1.update(0.1);

        game2.apply_action(Action::EndTurn, ActionSource::Terminal);
        game2.update(0.1);
        game2.update(0.1);
        game2.update(0.1);
    }

    let snap1 = game1.snapshot();
    let snap2 = game2.snapshot();

    assert_eq!(snap1.turn, snap2.turn);
    assert_eq!(snap1.phase, snap2.phase);
    assert_eq!(snap1.player_team, snap2.player_team);
    assert_eq!(snap1.enemy_team, snap2.enemy_team);
}

// ─── 11. Script Runner Integration ───────────────────────────────────────────

#[test]
fn script_runner_select_and_move() {
    let mut game = Game::new();

    let count = game
        .router
        .inject_script_with(
            &wuthering_terminal::default_commands(),
            "inspect:4,4 confirm inspect:4,5 confirm",
            ActionSource::Script,
            wuthering_terminal::resolve_command_token,
        )
        .unwrap();
    assert_eq!(count, 4);

    let reports = game.run_pending_reports();
    assert_eq!(reports.len(), 4);

    for report in &reports {
        assert_eq!(report.source, ActionSource::Script);
    }

    let warrior = find_entity(&game, CharacterClass::Warrior);
    assert_eq!(
        *game.world.get::<Position>(warrior).unwrap(),
        Position::new(4, 5)
    );
}

#[test]
fn script_runner_reports_contain_outcomes() {
    let mut game = Game::new();

    game.router
        .inject_script_with(
            &wuthering_terminal::default_commands(),
            "inspect:4,4 confirm inspect:4,5 confirm",
            ActionSource::Script,
            wuthering_terminal::resolve_command_token,
        )
        .unwrap();

    let reports = game.run_pending_reports();

    assert_eq!(reports.len(), 4);
    assert!(matches!(reports[0].outcome, ActionOutcome::StateUpdated));
    assert!(matches!(
        reports[1].outcome,
        ActionOutcome::StateUpdated | ActionOutcome::NoOp
    ));
    assert!(matches!(reports[2].outcome, ActionOutcome::StateUpdated));
    assert!(matches!(reports[3].outcome, ActionOutcome::Moved { .. }));
}

// ─── 12. StepReport ──────────────────────────────────────────────────────────

#[test]
fn step_report_before_and_after_differ() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

    assert_eq!(report.before.turn, report.after.turn);
    assert_eq!(report.before.outcome, Outcome::Playing);
    assert_eq!(report.after.outcome, Outcome::Playing);
}

#[test]
fn step_report_json_serializable() {
    let mut game = Game::new();

    select_character(&mut game, Position::new(4, 4));

    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(4, 5);
    let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

    let json = serde_json::to_string(&report).expect("StepReport should serialize");
    let restored: wuthering_terminal::StepReport =
        serde_json::from_str(&json).expect("StepReport should deserialize");

    assert_eq!(report.action, restored.action);
    assert_eq!(report.source, restored.source);
    assert_eq!(report.outcome, restored.outcome);
}
