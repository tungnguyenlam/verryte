use verryte_core::Entity;
use verryte_input::ActionSource;
use wuthering_terminal::components::{
    ActiveFloorModifiers, ActiveHazards, CharacterClass, FloorModifier, GameState, Stats, Team,
    TurnPhase, TurnTransition,
};
use wuthering_terminal::map::{TacticalMap, Tile};
use wuthering_terminal::{Action, Game, Position, Spawner};

fn find_entity_by_class(game: &Game, class: CharacterClass) -> Entity {
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

fn clear_entities_at(game: &mut Game, positions: &[Position]) {
    let mut to_despawn = Vec::new();
    for (e, pos) in game.world.query::<Position>() {
        if positions.contains(pos) {
            to_despawn.push(e);
        }
    }
    for e in to_despawn {
        game.world.despawn(e);
    }
}

// Find a seed for the RNG that yields normal hits (i.e. roll >= 35) for the first 3 rolls.
fn find_deterministic_seed() -> u64 {
    for s in 1..10000 {
        let mut test_rng = verryte_core::rng::Rng::seed(s);
        let r1 = test_rng.next_u32(100);
        let r2 = test_rng.next_u32(100);
        let r3 = test_rng.next_u32(100);
        if r1 >= 35 && r2 >= 35 && r3 >= 35 {
            return s;
        }
    }
    1
}

#[test]
fn test_berserker_frenzy_damage_scaling() {
    let mut game = Game::new();
    let seed = find_deterministic_seed();
    game.world
        .insert_resource(verryte_core::rng::Rng::seed(seed));

    // Clear positions to avoid collision
    clear_entities_at(&mut game, &[Position::new(1, 1), Position::new(1, 2)]);

    // 1. Spawn a Berserker (player team) and a Target (enemy team)
    let berserker =
        game.world
            .spawn_character(Position::new(1, 1), Team::Player, CharacterClass::Berserker);
    let target = game.world.spawn_character(
        Position::new(1, 2),
        Team::Enemy,
        CharacterClass::ShadowStalker,
    );

    // Give Berserker full HP
    if let Some(stats) = game.world.get_mut::<Stats>(berserker) {
        stats.hp = stats.max_hp;
        stats.atk = 20; // Predictable ATK
    }
    if let Some(stats) = game.world.get_mut::<Stats>(target) {
        stats.hp = 100;
        stats.max_hp = 100;
        stats.def = 0; // Predictable DEF
    }

    // Normal attack (HP is 100%): damage should be ATK - DEF = 20
    let initial_hp = game.world.get::<Stats>(target).unwrap().hp;
    wuthering_terminal::systems::resolve_combat_hit(
        &mut game.world,
        Some(berserker),
        target,
        20,
        "Berserker",
        "ShadowStalker",
        Position::new(1, 2),
    );
    let hp_after_normal = game.world.get::<Stats>(target).unwrap().hp;
    let normal_damage = initial_hp - hp_after_normal;
    assert_eq!(normal_damage, 20);

    // Reset target HP
    if let Some(stats) = game.world.get_mut::<Stats>(target) {
        stats.hp = 100;
    }

    // Now set Berserker HP to < 50%
    if let Some(stats) = game.world.get_mut::<Stats>(berserker) {
        stats.hp = stats.max_hp / 2 - 1; // < 50%
    }

    // Attack under frenzy: damage should be 20 * 1.5 = 30
    let hp_before_frenzy = game.world.get::<Stats>(target).unwrap().hp;
    wuthering_terminal::systems::resolve_combat_hit(
        &mut game.world,
        Some(berserker),
        target,
        20,
        "Berserker",
        "ShadowStalker",
        Position::new(1, 2),
    );
    let hp_after_frenzy = game.world.get::<Stats>(target).unwrap().hp;
    let frenzy_damage = hp_before_frenzy - hp_after_frenzy;
    assert_eq!(frenzy_damage, 30);

    // Test Frenzy vulnerability (receiving +50% extra damage)
    if let Some(stats) = game.world.get_mut::<Stats>(berserker) {
        stats.def = 0;
        stats.hp = stats.max_hp / 2 - 1; // still < 50%
    }
    let hp_before_receiving = game.world.get::<Stats>(berserker).unwrap().hp;
    wuthering_terminal::systems::resolve_combat_hit(
        &mut game.world,
        Some(target),
        berserker,
        20,
        "ShadowStalker",
        "Berserker",
        Position::new(1, 1),
    );
    let hp_after_receiving = game.world.get::<Stats>(berserker).unwrap().hp;
    let received_damage = hp_before_receiving - hp_after_receiving;
    assert_eq!(received_damage, 30); // 20 * 1.5 = 30
}

#[test]
fn test_berserker_turn_start_ap_bonus() {
    let mut game = Game::new();

    // Clear positions to avoid collision
    clear_entities_at(&mut game, &[Position::new(1, 1)]);

    // Spawn a Berserker (player team)
    let berserker =
        game.world
            .spawn_character(Position::new(1, 1), Team::Player, CharacterClass::Berserker);

    // Give Berserker full HP, max_ap = 3, initial ap = 0
    if let Some(stats) = game.world.get_mut::<Stats>(berserker) {
        stats.hp = stats.max_hp;
        stats.max_ap = 3;
        stats.ap = 0;
    }

    // Change TurnPhase to Enemy and request end-turn so we transition to Player phase and replenish Player AP
    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        assert_eq!(state.phase, TurnPhase::Player);
        let trans = game.world.resource_mut::<TurnTransition>().unwrap();
        assert!(!trans.request_end);
    }
    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.phase = TurnPhase::Enemy;
        let trans = game.world.resource_mut::<TurnTransition>().unwrap();
        trans.request_end = true;
    }

    wuthering_terminal::systems::turn_management_system(&mut game.world);

    // Since HP is 100%, AP should be replenished to max_ap (3)
    let stats = game.world.get::<Stats>(berserker).unwrap();
    assert_eq!(stats.ap, 3);

    // Now set Berserker HP to < 50%, set AP to 0
    if let Some(stats) = game.world.get_mut::<Stats>(berserker) {
        stats.hp = stats.max_hp / 2 - 1;
        stats.ap = 0;
    }

    // Run transition again
    {
        let state = game.world.resource_mut::<GameState>().unwrap();
        state.phase = TurnPhase::Enemy;
        let trans = game.world.resource_mut::<TurnTransition>().unwrap();
        trans.request_end = true;
    }

    wuthering_terminal::systems::turn_management_system(&mut game.world);

    // Since HP is < 50%, Berserker should get +1 bonus AP (3 + 1 = 4)
    let stats = game.world.get::<Stats>(berserker).unwrap();
    assert_eq!(stats.ap, 4);
}

#[test]
fn test_steam_vent_hazard() {
    let mut game = Game::new();

    // Clear positions to avoid collision
    clear_entities_at(
        &mut game,
        &[
            Position::new(2, 1),
            Position::new(2, 2),
            Position::new(2, 3),
        ],
    );

    // Set map tiles to ensure walkability and clean environment
    if let Some(map) = game.world.resource_mut::<TacticalMap>() {
        let map_mut = map;
        map_mut.tiles.set(Position::new(2, 1), Tile::Grass);
        map_mut.tiles.set(Position::new(2, 2), Tile::SteamVent);
        map_mut.tiles.set(Position::new(2, 3), Tile::Grass);
    }

    // Spawn a triggerer (Warrior) and an adjacent entity
    let triggerer =
        game.world
            .spawn_character(Position::new(2, 1), Team::Player, CharacterClass::Warrior);
    let adjacent = game.world.spawn_character(
        Position::new(2, 3), // Adjacent cardinally (South) of the vent at (2, 2)
        Team::Enemy,
        CharacterClass::ShadowStalker,
    );

    // Initialize stats
    if let Some(stats) = game.world.get_mut::<Stats>(triggerer) {
        stats.hp = 100;
        stats.max_hp = 100;
        stats.ap = 3;
    }
    if let Some(stats) = game.world.get_mut::<Stats>(adjacent) {
        stats.hp = 100;
        stats.max_hp = 100;
    }

    // Reinitialize ActiveHazards from the map
    let map = game.world.resource::<TacticalMap>().unwrap();
    let hazards = wuthering_terminal::hazards::HazardSystem::initialize_hazards(map);
    game.world.insert_resource(hazards);

    // Select triggerer at (2, 1)
    select_character(&mut game, Position::new(2, 1));

    // Set cursor to (2, 2) (the SteamVent tile) and confirm move
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(2, 2);
    game.apply_action(Action::Confirm, ActionSource::Terminal);

    // Assert triggerer took 12 damage (100 - 12 = 88)
    let triggerer_hp = game.world.get::<Stats>(triggerer).unwrap().hp;
    assert_eq!(triggerer_hp, 88);

    // Assert adjacent took 5 damage (100 - 5 = 95)
    let adjacent_hp = game.world.get::<Stats>(adjacent).unwrap().hp;
    assert_eq!(adjacent_hp, 95);

    // Assert hazard trigger count decremented (starts at 3, should be 2)
    let hazards_res = game.world.resource::<ActiveHazards>().unwrap();
    let vent_hazard = hazards_res
        .hazards
        .iter()
        .find(|(pos, _)| *pos == Position::new(2, 2))
        .unwrap();
    assert_eq!(vent_hazard.1.trigger_count, 2);
}

#[test]
fn test_line_of_sight_checks() {
    let mut game = Game::new();

    let mage = find_entity_by_class(&game, CharacterClass::Mage);
    let healer = find_entity_by_class(&game, CharacterClass::Healer);

    // Clear target/wall positions to avoid collision
    clear_entities_at(
        &mut game,
        &[
            Position::new(6, 4),
            Position::new(6, 5),
            Position::new(6, 6),
        ],
    );

    // Spawn an enemy target at distance 2
    let target = game.world.spawn_character(
        Position::new(6, 6),
        Team::Enemy,
        CharacterClass::ShadowStalker,
    );

    // Place a Wall between (6, 4) and (6, 6)
    if let Some(map) = game.world.resource_mut::<TacticalMap>() {
        let map_mut = map;
        map_mut.tiles.set(Position::new(6, 4), Tile::Grass);
        map_mut.tiles.set(Position::new(6, 5), Tile::Wall);
        map_mut.tiles.set(Position::new(6, 6), Tile::Grass);
    }

    // Test Healer basic attack which has range 2.
    *game.world.get_mut::<Position>(healer).unwrap() = Position::new(6, 4);
    if let Some(stats) = game.world.get_mut::<Stats>(healer) {
        stats.ap = 3;
    }
    if let Some(stats) = game.world.get_mut::<Stats>(target) {
        stats.hp = 100;
        stats.max_hp = 100;
    }

    // select Healer, aim at target
    select_character(&mut game, Position::new(6, 4));

    // Move cursor to target (6, 6) and try to attack
    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(6, 6);
    let before_hp = game.world.get::<Stats>(target).unwrap().hp;
    game.apply_action(Action::Confirm, ActionSource::Terminal);
    let after_hp = game.world.get::<Stats>(target).unwrap().hp;

    // HP should NOT change because LOS is blocked by wall at (6, 5)
    assert_eq!(before_hp, after_hp);

    // Clear selection
    game.world
        .resource_mut::<GameState>()
        .unwrap()
        .selected_entity = None;

    // Now let's try Mage with range 3 basic attack
    *game.world.get_mut::<Position>(mage).unwrap() = Position::new(6, 4);
    if let Some(stats) = game.world.get_mut::<Stats>(mage) {
        stats.ap = 3;
    }

    select_character(&mut game, Position::new(6, 4));

    game.world.resource_mut::<GameState>().unwrap().cursor = Position::new(6, 6);
    let before_hp_mage = game.world.get::<Stats>(target).unwrap().hp;
    game.apply_action(Action::Confirm, ActionSource::Terminal);
    let after_hp_mage = game.world.get::<Stats>(target).unwrap().hp;

    // HP should decrease because Mage bypasses LOS blockages!
    assert!(after_hp_mage < before_hp_mage);
}

#[test]
fn test_reversal_modifier() {
    let mut game = Game::new();
    let seed = find_deterministic_seed();
    game.world
        .insert_resource(verryte_core::rng::Rng::seed(seed));

    // Clear positions
    clear_entities_at(&mut game, &[Position::new(1, 1)]);

    // Spawn an entity
    let entity =
        game.world
            .spawn_character(Position::new(1, 1), Team::Player, CharacterClass::Warrior);

    // Set stats
    if let Some(stats) = game.world.get_mut::<Stats>(entity) {
        stats.hp = 80;
        stats.max_hp = 100;
        stats.def = 0;
    }

    // 1. Normal healing: heal 10 HP
    let (healed, defeated) = wuthering_terminal::systems::apply_heal(&mut game.world, entity, 10);
    assert_eq!(healed, 10);
    assert!(!defeated);
    assert_eq!(game.world.get::<Stats>(entity).unwrap().hp, 90);

    // 2. Activate Reversal Modifier
    game.world.insert_resource(ActiveFloorModifiers {
        modifiers: vec![FloorModifier::Reversal],
        turns_remaining: vec![99],
    });

    // 3. Heal 10 HP under Reversal: should DEAL 10 damage! (HP 90 -> 80)
    let (healed, defeated) = wuthering_terminal::systems::apply_heal(&mut game.world, entity, 10);
    assert_eq!(healed, -10);
    assert!(!defeated);
    assert_eq!(game.world.get::<Stats>(entity).unwrap().hp, 80);

    // 4. Deal 15 damage under Reversal: should HEAL 15 HP! (HP 80 -> 95)
    let target_hp_before = game.world.get::<Stats>(entity).unwrap().hp;
    wuthering_terminal::systems::resolve_combat_hit(
        &mut game.world,
        None,
        entity,
        15,
        "None",
        "Warrior",
        Position::new(1, 1),
    );
    let target_hp_after = game.world.get::<Stats>(entity).unwrap().hp;
    assert_eq!(target_hp_after - target_hp_before, 15);
    assert_eq!(target_hp_after, 95);
}

#[test]
fn test_exploding_barrel_trigger() {
    let mut game = Game::new();

    // Clear positions
    clear_entities_at(
        &mut game,
        &[
            Position::new(2, 2),
            Position::new(2, 1),
            Position::new(3, 2),
        ],
    );

    // Place grass at (2,2) and adjacent.
    if let Some(map) = game.world.resource_mut::<TacticalMap>() {
        map.tiles.set(Position::new(2, 2), Tile::ExplodingBarrel);
        map.tiles.set(Position::new(2, 1), Tile::Grass);
        map.tiles.set(Position::new(3, 2), Tile::Grass);
    }

    // Spawn player character at (2, 1) and another adjacent at (3, 2)
    let p1 = game
        .world
        .spawn_character(Position::new(2, 1), Team::Player, CharacterClass::Warrior);
    let p2 = game
        .world
        .spawn_character(Position::new(3, 2), Team::Player, CharacterClass::Mage);

    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 100;
        stats.max_hp = 100;
    }
    if let Some(stats) = game.world.get_mut::<Stats>(p2) {
        stats.hp = 100;
        stats.max_hp = 100;
    }

    // Spawn the barrel entity at (2, 2)
    let barrel = game.world.spawn_barrel(Position::new(2, 2));

    // Trigger handle_defeat on barrel
    game.handle_defeat(
        barrel,
        "Exploding Barrel",
        CharacterClass::DestructibleObject,
        Position::new(2, 2),
    );

    // Verify that the tile at (2,2) and adjacent cross tiles (within radius 1) are replaced with Lava
    if let Some(map) = game.world.resource::<TacticalMap>() {
        assert_eq!(map.tile(2, 2), Tile::Lava);
        assert_eq!(map.tile(2, 1), Tile::Lava);
        assert_eq!(map.tile(3, 2), Tile::Lava);
    }

    // Verify cross-shape AOE fire damage (radius 2)
    // p1 at (2, 1) should take 30 damage -> 70 HP
    // p2 at (3, 2) should take 30 damage -> 70 HP
    let hp1 = game.world.get::<Stats>(p1).unwrap().hp;
    let hp2 = game.world.get::<Stats>(p2).unwrap().hp;
    assert_eq!(hp1, 70);
    assert_eq!(hp2, 70);
}

#[test]
fn test_corrupted_spore_poison() {
    let mut game = Game::new();

    // Clear positions
    clear_entities_at(&mut game, &[Position::new(2, 2), Position::new(2, 1)]);

    // Spawn player character at (2, 1)
    let p1 = game
        .world
        .spawn_character(Position::new(2, 1), Team::Player, CharacterClass::Warrior);
    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 100;
    }

    // Spawn Corrupted Spore at (2, 2)
    let spore = game.world.spawn_character(
        Position::new(2, 2),
        Team::Enemy,
        CharacterClass::CorruptedSpore,
    );

    // Trigger handle_defeat on spore
    game.handle_defeat(
        spore,
        "Corrupted Spore",
        CharacterClass::CorruptedSpore,
        Position::new(2, 2),
    );

    // Verify that p1 is poisoned (duration 3)
    let status = game
        .world
        .get::<wuthering_terminal::components::ElementalStatus>(p1)
        .unwrap();
    assert_eq!(
        *status,
        wuthering_terminal::components::ElementalStatus::Poison { duration: 3 }
    );
}

#[test]
fn test_cheat_console_commands() {
    let mut game = Game::new();

    // Clear and spawn a warrior
    clear_entities_at(&mut game, &[Position::new(1, 1)]);
    let p1 = game
        .world
        .spawn_character(Position::new(1, 1), Team::Player, CharacterClass::Warrior);

    // Select the character
    game.world
        .resource_mut::<GameState>()
        .unwrap()
        .selected_entity = Some(p1);

    // Test /damage cheat command
    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 100;
        stats.max_hp = 100;
    }
    game.execute_console_command("/damage 40");
    let hp_after_dmg = game.world.get::<Stats>(p1).unwrap().hp;
    assert_eq!(hp_after_dmg, 60);

    // Test /heal cheat command
    game.execute_console_command("/heal 25");
    let hp_after_heal = game.world.get::<Stats>(p1).unwrap().hp;
    assert_eq!(hp_after_heal, 85);

    // Test /xp cheat command
    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.xp = 0;
        stats.level = 1;
    }
    game.execute_console_command("/xp 150"); // 150 XP will level up (needs 100 XP for lvl 1)
    let stats = game.world.get::<Stats>(p1).unwrap();
    assert_eq!(stats.level, 2);
    assert_eq!(stats.xp, 50); // 150 - 100

    // Test /modifier cheat command
    game.execute_console_command("/modifier reversal");
    let active_mods = game.world.resource::<ActiveFloorModifiers>().unwrap();
    assert!(active_mods.modifiers.contains(&FloorModifier::Reversal));

    // Test /spawn cheat command
    // Spawn at cursor position (which is 1, 1) or specific position (3, 3)
    game.execute_console_command("/spawn spore 3 3");
    // Verify that a spore exists at (3, 3)
    let spore_exists = game
        .world
        .query2::<Position, CharacterClass>()
        .into_iter()
        .any(|(_, pos, class)| {
            *pos == Position::new(3, 3) && *class == CharacterClass::CorruptedSpore
        });
    assert!(spore_exists);

    // Test /weather cheat command
    game.execute_console_command("/weather snowing");
    let weather = game
        .world
        .resource::<wuthering_terminal::components::Weather>()
        .unwrap();
    assert_eq!(
        weather.current,
        wuthering_terminal::components::WeatherType::Snowing
    );

    // Test /floor cheat command
    let current_floor = game.world.resource::<GameState>().unwrap().floor;
    assert_eq!(current_floor, 1);
    game.execute_console_command("/floor 3");
    let new_floor = game.world.resource::<GameState>().unwrap().floor;
    assert_eq!(new_floor, 3);

    // After transitioning floor, Kael at p1 is despawned. We must select the newly spawned Warrior on Floor 3.
    let p3 = game
        .world
        .query::<CharacterClass>()
        .into_iter()
        .find(|(_, class)| **class == CharacterClass::Warrior)
        .map(|(e, _)| e)
        .unwrap();
    game.world
        .resource_mut::<GameState>()
        .unwrap()
        .selected_entity = Some(p3);

    // Test /elite cheat command
    game.execute_console_command("/elite sturdy");
    let has_sturdy = game
        .world
        .get::<wuthering_terminal::components::EliteEnemy>(p3)
        .is_some_and(|ee| {
            ee.modifiers
                .contains(&wuthering_terminal::components::EliteModifier::Sturdy)
        });
    assert!(has_sturdy);
}

#[test]
fn test_combat_log_scroll() {
    let mut game = Game::new();

    // Toggle Combat Log open
    game.apply_action(Action::ToggleCombatLog, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(
        state.ui_state,
        wuthering_terminal::components::UIState::CombatLog
    );
    assert_eq!(state.log_scroll_offset, 0);

    // Scroll up using MoveNorth (increases offset)
    game.apply_action(Action::MoveNorth, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(state.log_scroll_offset, 1);

    // Scroll down using MoveSouth (decreases offset)
    game.apply_action(Action::MoveSouth, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(state.log_scroll_offset, 0);

    // Check saturating sub on offset (offset should not go below 0)
    game.apply_action(Action::MoveSouth, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(state.log_scroll_offset, 0);

    // Close Combat Log
    game.apply_action(Action::ToggleCombatLog, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_ne!(
        state.ui_state,
        wuthering_terminal::components::UIState::CombatLog
    );
}

#[test]
fn test_save_load_menu() {
    let mut game = Game::new();

    // Toggle SaveLoadMenu open
    game.apply_action(Action::ToggleSaveLoadMenu, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(
        state.ui_state,
        wuthering_terminal::components::UIState::SaveLoadMenu
    );
    assert_eq!(state.selected_save_slot, 0);

    // Scroll selected slot down
    game.apply_action(Action::MoveSouth, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(state.selected_save_slot, 1);

    // Scroll selected slot up
    game.apply_action(Action::MoveNorth, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(state.selected_save_slot, 0);

    // Save to slot 0
    game.apply_action(Action::Save, ActionSource::Terminal);

    // Toggle menu closed
    game.apply_action(Action::ToggleSaveLoadMenu, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_ne!(
        state.ui_state,
        wuthering_terminal::components::UIState::SaveLoadMenu
    );
}

#[test]
fn test_inspect_character() {
    let mut game = Game::new();

    // Toggle InspectCharacter open
    game.apply_action(Action::ToggleInspectCharacter, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_eq!(
        state.ui_state,
        wuthering_terminal::components::UIState::InspectCharacter
    );

    // Close InspectCharacter
    game.apply_action(Action::ToggleInspectCharacter, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert_ne!(
        state.ui_state,
        wuthering_terminal::components::UIState::InspectCharacter
    );
}

#[test]
fn test_threat_map_toggle() {
    let mut game = Game::new();

    let state = game.world.resource::<GameState>().unwrap();
    assert!(!state.show_threat_map);

    // Toggle ThreatMap on
    game.apply_action(Action::ToggleThreatMap, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert!(state.show_threat_map);

    // Toggle ThreatMap off
    game.apply_action(Action::ToggleThreatMap, ActionSource::Terminal);
    let state = game.world.resource::<GameState>().unwrap();
    assert!(!state.show_threat_map);
}

#[test]
fn test_advanced_elemental_reactions() {
    let mut game = Game::new();

    // Clear positions
    clear_entities_at(&mut game, &[Position::new(3, 3), Position::new(3, 4)]);

    // Spawn player character (Warrior) at (3, 3)
    let p1 = game
        .world
        .spawn_character(Position::new(3, 3), Team::Player, CharacterClass::Warrior);
    // Spawn enemy at (3, 4)
    let enemy = game.world.spawn_character(
        Position::new(3, 4),
        Team::Enemy,
        CharacterClass::PlagueWraith,
    );

    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 50;
        stats.max_hp = 100;
    }
    if let Some(stats) = game.world.get_mut::<Stats>(enemy) {
        stats.hp = 100;
    }

    // 1. Test Toxic Shock: Poison + Lightning
    // Apply Poison element first
    game.apply_elemental_status(
        enemy,
        wuthering_terminal::components::ElementalStatus::Poison { duration: 3 },
    );
    // Apply Lightning element to trigger Toxic Shock reaction
    game.apply_elemental_status(
        enemy,
        wuthering_terminal::components::ElementalStatus::Lightning { duration: 3 },
    );

    // Check that target took 30 damage (100 -> 70)
    let enemy_hp = game.world.get::<Stats>(enemy).unwrap().hp;
    assert_eq!(enemy_hp, 70);

    // Check that adjacent player character at (3, 3) got poisoned by the reaction burst
    let status = game
        .world
        .get::<wuthering_terminal::components::ElementalStatus>(p1)
        .unwrap();
    assert_eq!(
        *status,
        wuthering_terminal::components::ElementalStatus::Poison { duration: 3 }
    );

    // 2. Test Purification: Nature + Poison
    // Apply Poison element to p1 (already poisoned above)
    // Apply Nature element to trigger Purification
    game.apply_elemental_status(
        p1,
        wuthering_terminal::components::ElementalStatus::Nature { duration: 3 },
    );

    // Check that p1 is healed (50 -> 80)
    let p1_hp = game.world.get::<Stats>(p1).unwrap().hp;
    assert_eq!(p1_hp, 80);

    // Check that poison status was cleansed (elemental status becomes None)
    let status_after = game
        .world
        .get::<wuthering_terminal::components::ElementalStatus>(p1)
        .unwrap();
    assert_eq!(
        *status_after,
        wuthering_terminal::components::ElementalStatus::None
    );
}

#[test]
fn test_elite_modifier_vampiric() {
    let mut game = Game::new();
    let p1 = game
        .world
        .spawn_character(Position::new(1, 1), Team::Player, CharacterClass::Warrior);
    let enemy = game.world.spawn_character(
        Position::new(1, 2),
        Team::Enemy,
        CharacterClass::PlagueWraith,
    );

    // Make enemy Vampiric
    game.world.insert(
        enemy,
        wuthering_terminal::components::EliteEnemy {
            modifiers: vec![wuthering_terminal::components::EliteModifier::Vampiric],
        },
    );

    // Set healths
    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 100;
        stats.def = 0;
    }
    if let Some(stats) = game.world.get_mut::<Stats>(enemy) {
        stats.hp = 40;
        stats.max_hp = 100;
        stats.atk = 20;
    }

    // Run combat hit from enemy -> player
    wuthering_terminal::systems::resolve_combat_hit(
        &mut game.world,
        Some(enemy),
        p1,
        20,
        "PlagueWraith",
        "Warrior",
        Position::new(1, 1),
    );

    // Damage is 20, lifesteal is 30% of 20 = 6 HP.
    let enemy_hp = game.world.get::<Stats>(enemy).unwrap().hp;
    assert_eq!(enemy_hp, 46); // 40 + 6
}

#[test]
fn test_elite_modifier_sturdy() {
    let mut game = Game::new();
    let enemy = game.world.spawn_character(
        Position::new(1, 2),
        Team::Enemy,
        CharacterClass::PlagueWraith,
    );

    // Give sturdy modifier
    game.world.insert(
        enemy,
        wuthering_terminal::components::EliteEnemy {
            modifiers: vec![wuthering_terminal::components::EliteModifier::Sturdy],
        },
    );

    // Run system once to make sure initial stats are boosted
    // Wait, the sturdy stats boost happens during spawn! Let's spawn another one with floor 3 so that they are rolled, or let's manually spawn with sturdy.
    // If we manually insert the component, we can manually check sturdy_immunity_system.
    game.world.insert(
        enemy,
        wuthering_terminal::components::Rooted { duration: 2 },
    );
    game.world.insert(
        enemy,
        wuthering_terminal::components::Stunned { duration: 1 },
    );

    wuthering_terminal::systems::sturdy_immunity_system(&mut game.world);

    assert!(game
        .world
        .get::<wuthering_terminal::components::Rooted>(enemy)
        .is_none());
    assert!(game
        .world
        .get::<wuthering_terminal::components::Stunned>(enemy)
        .is_none());
}

#[test]
fn test_elite_modifier_fiery() {
    let mut game = Game::new();
    let p1 = game
        .world
        .spawn_character(Position::new(1, 1), Team::Player, CharacterClass::Warrior);
    let enemy = game.world.spawn_character(
        Position::new(1, 2),
        Team::Enemy,
        CharacterClass::PlagueWraith,
    );

    // Set tile at (1, 1) to Grass
    {
        let map = game.world.resource_mut::<TacticalMap>().unwrap();
        map.tiles.set(Position::new(1, 1), Tile::Grass);
    }

    // Make enemy Fiery
    game.world.insert(
        enemy,
        wuthering_terminal::components::EliteEnemy {
            modifiers: vec![wuthering_terminal::components::EliteModifier::Fiery],
        },
    );

    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 100;
        stats.def = 0;
    }

    wuthering_terminal::systems::resolve_combat_hit(
        &mut game.world,
        Some(enemy),
        p1,
        20,
        "PlagueWraith",
        "Warrior",
        Position::new(1, 1),
    );

    // Defender's tile should become Lava
    let map = game.world.resource::<TacticalMap>().unwrap();
    assert_eq!(map.tile(1, 1), Tile::Lava);
}

#[test]
fn test_defender_ai_behavior() {
    let mut game = Game::new();

    // Clear positions
    clear_entities_at(&mut game, &[Position::new(5, 5), Position::new(5, 6), Position::new(7, 5)]);

    // 1. Spawn a Boss (priority to protect) at (5, 5)
    let _boss = game.world.spawn_character(Position::new(5, 5), Team::Enemy, CharacterClass::Boss);
    // 2. Spawn a Defender near the Boss at (5, 6)
    let defender = game.world.spawn_character(Position::new(5, 6), Team::Enemy, CharacterClass::FrozenSentinel);
    // 3. Spawn a Player far away at (7, 5)
    let _player = game.world.spawn_character(Position::new(7, 5), Team::Player, CharacterClass::Warrior);

    if let Some(stats) = game.world.get_mut::<Stats>(defender) {
        stats.ap = 2;
    }

    // Run AI system
    wuthering_terminal::systems::enemy_ai_system(&mut game.world);

    // Defender should stay near Boss (dist <= 1) and possibly move to attack player if in range.
    // In this case, player is at (7, 5), Boss is at (5, 5). Dist is 2.
    // Defender at (5, 6) is dist 1 from Boss.
    // It should stay near Boss and attack Player if Player is within 3 tiles of Boss.
    // Player at (7, 5) is dist 2 from Boss.
    
    // Check if Defender moved or attacked
    let def_pos = game.world.get::<Position>(defender).unwrap();
    // Defender should still be at (5, 6) or (6, 5) or somewhere adjacent to Boss
    let dist_to_boss = (def_pos.x - 5).abs() + (def_pos.y - 5).abs();
    assert!(dist_to_boss <= 1);
}

#[test]
fn test_melt_reaction() {
    let mut game = Game::new();

    // Clear positions
    clear_entities_at(&mut game, &[Position::new(3, 3)]);

    // Spawn player at (3, 3)
    let p1 = game.world.spawn_character(Position::new(3, 3), Team::Player, CharacterClass::Warrior);
    
    // Set tile to Ice
    if let Some(map) = game.world.resource_mut::<TacticalMap>() {
        map.tiles.set(Position::new(3, 3), Tile::Ice);
    }

    if let Some(stats) = game.world.get_mut::<Stats>(p1) {
        stats.hp = 100;
    }

    // Apply Ice status
    game.apply_elemental_status(p1, wuthering_terminal::components::ElementalStatus::Ice { duration: 3 });
    // Apply Fire status to trigger MELT
    game.apply_elemental_status(p1, wuthering_terminal::components::ElementalStatus::Fire { duration: 3 });

    // 1. HP should decrease by 25 (100 -> 75)
    let hp = game.world.get::<Stats>(p1).unwrap().hp;
    assert_eq!(hp, 75);

    // 2. Elemental status should be None
    let status = game.world.get::<wuthering_terminal::components::ElementalStatus>(p1).unwrap();
    assert_eq!(*status, wuthering_terminal::components::ElementalStatus::None);

    // 3. Tile at (3, 3) should be Water now
    let map = game.world.resource::<TacticalMap>().unwrap();
    assert_eq!(map.tile(3, 3), Tile::Water);
}
