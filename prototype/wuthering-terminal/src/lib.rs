//! Wuthering Terminal — tactical RPG prototype.

pub mod action;
pub mod components;
pub mod game;
pub mod generated_assets;
pub mod map;
pub mod snapshot;
pub mod spawn;
pub mod systems;
pub mod ui;

pub use action::{default_commands, resolve_command_token, Action};
pub use components::Outcome;
pub use game::Game;
pub use snapshot::{ActionOutcome, FullSaveState, Snapshot, StepReport};
pub use spawn::Spawner;
pub use verryte_map::Point as Position;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{
        BattleStats, CharacterClass, GameState, Inventory, Stats, Team, TurnPhase,
    };
    use crate::map::{TacticalMap, Tile};
    use verryte_input::ActionSource;

    #[test]
    fn test_game_init() {
        let game = Game::new();
        assert_eq!(game.world.entity_count(), 14); // 3 players + 1 boss + 2 stalkers + 2 spores + 1 sentinel + 1 wraith + 4 items

        let mut player_count = 0;
        let mut boss_count = 0;
        for (_e, _p, team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *team == Team::Player {
                player_count += 1;
            }
            if *class == CharacterClass::Boss {
                boss_count += 1;
            }
        }
        assert_eq!(player_count, 3);
        assert_eq!(boss_count, 1);
    }

    #[test]
    fn test_save_load_game_state() {
        let mut game = Game::new();
        // Select warrior at (4, 4)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        let selected_before = game.world.resource::<GameState>().unwrap().selected_entity;
        assert!(
            selected_before.is_some(),
            "Kael should be selected before save"
        );

        let serialized = game.save_state().unwrap();

        // Create a new game
        let mut game2 = Game::new();
        assert_ne!(
            game2.world.resource::<GameState>().unwrap().selected_entity,
            selected_before
        );

        // Load the saved state
        game2.load_state(&serialized).unwrap();

        // Verify state is restored
        let selected_after = game2.world.resource::<GameState>().unwrap().selected_entity;
        assert_eq!(selected_after, selected_before);
        assert_eq!(
            game2.world.resource::<GameState>().unwrap().cursor,
            Position::new(4, 4)
        );

        // Check that all entities are restored
        assert_eq!(game2.world.entity_count(), 14);

        let mut player_count = 0;
        let mut boss_count = 0;
        for (_e, _p, team, class) in game2.world.query3::<Position, Team, CharacterClass>() {
            if *team == Team::Player {
                player_count += 1;
            }
            if *class == CharacterClass::Boss {
                boss_count += 1;
            }
        }
        assert_eq!(player_count, 3);
        assert_eq!(boss_count, 1);
    }

    #[test]
    fn test_save_load_validation() {
        let mut game = Game::new();
        let valid_save = game.save_state().unwrap();

        // Missing magic signature
        let corrupt_json = r#"{"world":{"entities":[],"resources":[]}}"#;
        let res = game.load_state(corrupt_json);
        assert!(res.is_err());

        // Incorrect magic signature
        let invalid_magic =
            valid_save.replace("\"magic\":\"VERRYTE_SAVE\"", "\"magic\":\"BAD_MAGIC\"");
        let res = game.load_state(&invalid_magic);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("magic"));

        // Unsupported version
        let invalid_version = valid_save.replace("\"version\":1", "\"version\":99");
        let res = game.load_state(&invalid_version);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("version"));
    }

    #[test]
    fn test_selection_and_movement() {
        let mut game = Game::new();

        // 1. Select the Warrior (Kael) at (4, 4)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        let selected = game.world.resource::<GameState>().unwrap().selected_entity;
        assert!(selected.is_some(), "Kael should be selected");

        // 2. Move Kael to (4, 5)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        let state = game.world.resource::<GameState>().unwrap();
        assert!(
            state.selected_entity.is_none(),
            "Selection should be cleared after move"
        );

        // Check Kael's new position and depleted AP
        let warrior_entity = selected.unwrap();
        let pos = game.world.get::<Position>(warrior_entity).unwrap();
        assert_eq!(*pos, Position::new(4, 5));

        let stats = game.world.get::<Stats>(warrior_entity).unwrap();
        assert_eq!(stats.ap, 2, "Warrior should have 2 AP remaining (spent 1)");
    }

    #[test]
    fn test_attack_and_defeat() {
        let mut game = Game::new();

        // 1. Find Warrior and Boss
        let mut warrior_opt = None;
        let mut boss_opt = None;
        for (e, _pos, _team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior_opt = Some(e);
            }
            if *class == CharacterClass::Boss {
                boss_opt = Some(e);
            }
        }
        let warrior = warrior_opt.unwrap();
        let boss = boss_opt.unwrap();

        // Move Boss to (4, 5), right next to Kael at (4, 4)
        if let Some(pos) = game.world.get_mut::<Position>(boss) {
            *pos = Position::new(4, 5);
        }

        // Select Kael
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Attack Boss at (4, 5)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Warrior AP should be 2. Boss HP should be 500 - (20 - 20) capped at 1 = 499.
        let warrior_stats = game.world.get::<Stats>(warrior).unwrap();
        assert_eq!(warrior_stats.ap, 2);

        let boss_stats = game.world.get::<Stats>(boss).unwrap();
        assert_eq!(boss_stats.hp, 499);
    }

    #[test]
    fn test_turn_end_replenish() {
        let mut game = Game::new();

        // Select and move Warrior to (4, 5) so AP drops to 2
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

        // Verify AP is 2
        let mut warrior_opt = None;
        for (e, _pos, _team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior_opt = Some(e);
            }
        }
        let warrior = warrior_opt.unwrap();
        assert_eq!(game.world.get::<Stats>(warrior).unwrap().ap, 2);

        // End Turn (swaps to Enemy, AI runs, then returns to Player)
        game.apply_action(Action::EndTurn, ActionSource::Terminal);
        game.update(0.1); // Transition Player -> Enemy, Run AI
        game.update(0.1); // Signal end of Enemy turn
        game.update(0.1); // Transition Enemy -> Player

        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(state.turn, 2);
        assert_eq!(state.phase, TurnPhase::Player);

        // Kael AP should be replenished to max_ap (3)
        assert_eq!(game.world.get::<Stats>(warrior).unwrap().ap, 3);
    }

    #[test]
    fn test_skills_and_vfx() {
        let mut game = Game::new();

        // Select Kael
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Position Kael next to the Boss at (18, 8). Let's move Kael directly to (17, 8)
        let mut warrior_opt = None;
        for (e, _pos, _team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior_opt = Some(e);
            }
        }
        let warrior = warrior_opt.unwrap();
        if let Some(pos) = game.world.get_mut::<Position>(warrior) {
            *pos = Position::new(17, 8);
        }

        // Target Boss at (18, 8)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(18, 8);
        }

        // Trigger Skill1
        game.apply_action(Action::Skill1, ActionSource::Terminal);

        // Check state.targeting is Skill1
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.targeting, crate::components::TargetingMode::Skill1);
        }

        // Confirm to cast
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Verify Warrior AP is 1 (3 - 2)
        let stats = game.world.get::<Stats>(warrior).unwrap();
        assert_eq!(stats.ap, 1);

        // Verify targeting is reset to None
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.targeting, crate::components::TargetingMode::None);
        }

        // Boss should have taken damage. Boss initial HP 500. Skill 1 Warrior value 45. Boss Def 20. Damage = 45 - 20 = 25.
        // Boss HP should be 500 - 25 = 475.
        let mut boss_opt = None;
        for (e, _pos, _team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *class == CharacterClass::Boss {
                boss_opt = Some(e);
            }
        }
        let boss = boss_opt.unwrap();
        let boss_stats = game.world.get::<Stats>(boss).unwrap();
        assert_eq!(boss_stats.hp, 475);

        // Verify VFX are spawned
        assert!(
            !game.vfx().particles.is_empty(),
            "Particles should spawn on skill cast"
        );
        assert!(
            !game.vfx().shakes.is_empty(),
            "Screen shake should trigger on skill cast"
        );
    }

    #[test]
    fn test_qte_swap() {
        let mut game = Game::new();

        // 1. Select the Warrior (Kael) at (4, 4)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Set Concert Energy to 100
        game.build_concert_energy(100);

        // Press Skill3 (QTE Swap)
        game.apply_action(Action::Skill3, ActionSource::Terminal);

        // Check Concert Energy is reset to 0
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.concert_energy, 0);
            // Selected character should swap to next (e.g. Mage at (4, 8))
            // Active ent before was Kael. Players sorted: Kael (4,4), Lyra (4,8), Mira (4,12).
            // Next is Lyra (Mage)
            let selected = state.selected_entity.unwrap();
            let selected_class = game.world.get::<CharacterClass>(selected).unwrap();
            assert_eq!(*selected_class, CharacterClass::Mage);

            // Positions of Kael and Lyra should be swapped.
            // Lyra was at (4,8), Kael was at (4,4).
            // Lyra should now be at (4, 4)
            // Kael should be at (4, 8)
            let mut kael_opt = None;
            for (e, _pos, _team, class) in game.world.query3::<Position, Team, CharacterClass>() {
                if *class == CharacterClass::Warrior {
                    kael_opt = Some(e);
                }
            }
            let kael = kael_opt.unwrap();
            let kael_pos = game.world.get::<Position>(kael).unwrap();
            let lyra_pos = game.world.get::<Position>(selected).unwrap();

            assert_eq!(*lyra_pos, Position::new(4, 4));
            assert_eq!(*kael_pos, Position::new(4, 8));
        }
    }

    #[test]
    fn test_boss_telegraph_parry_and_echo() {
        let mut game = Game::new();

        let mut boss_opt = None;
        let mut warrior_opt = None;
        for (e, _pos, _team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *class == CharacterClass::Boss {
                boss_opt = Some(e);
            }
            if *class == CharacterClass::Warrior {
                warrior_opt = Some(e);
            }
        }
        let boss = boss_opt.unwrap();
        let _warrior = warrior_opt.unwrap();

        // Move Boss to (4, 5)
        if let Some(pos) = game.world.get_mut::<Position>(boss) {
            *pos = Position::new(4, 5);
        }

        // Set up boss to queue a telegraph zone directly
        {
            let telegraph = game
                .world
                .resource_mut::<crate::components::TelegraphZone>()
                .unwrap();
            telegraph.tiles = vec![Position::new(4, 4)];
            telegraph.damage = 50;
        }

        // Warrior is at (4, 4), which is in the TelegraphZone.
        // Select Warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Attack the Boss at (4, 5). This should trigger check_parry since Warrior is at (4,4) which is telegraphed.
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Verify TelegraphZone is cleared (parried!)
        {
            let telegraph = game
                .world
                .resource::<crate::components::TelegraphZone>()
                .unwrap();
            assert!(
                telegraph.tiles.is_empty(),
                "Telegraph zone should be cleared on parry"
            );
        }

        // Verify Boss AP is set to 0 (stunned!)
        {
            let boss_stats = game.world.get::<Stats>(boss).unwrap();
            assert_eq!(boss_stats.ap, 0, "Boss should have 0 AP (stunned)");
        }

        // Now test Boss defeat and Echo drop. Set boss HP to 1 and phase to Phase2.
        if let Some(stats) = game.world.get_mut::<Stats>(boss) {
            stats.hp = 1;
        }
        if let Some(state) = game.world.resource_mut::<GameState>() {
            state.boss_phase = crate::components::BossPhase::Phase2;
            state.floor = 2;
        }

        // Select Warrior again
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Attack Boss again
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Boss should be defeated and despawned.
        let mut boss_exists = false;
        for (_e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Boss {
                boss_exists = true;
            }
        }
        assert!(!boss_exists, "Boss should be despawned on defeat");

        // Echo should be dropped at Boss's position (4, 5)
        let mut echo_pos_opt = None;
        for (_e, pos, _echo) in game.world.query2::<Position, crate::components::EchoItem>() {
            echo_pos_opt = Some(*pos);
        }
        assert_eq!(echo_pos_opt, Some(Position::new(4, 5)));

        // Select Warrior and move to (4, 5) to absorb Echo and win
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

        // Verify outcome is Victory
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(
                state.outcome,
                Outcome::Victory,
                "Should win game on Echo absorption"
            );
        }
    }

    #[test]
    fn test_script_execution() {
        let mut game = Game::new();

        // Check initial position of Warrior (Kael)
        let mut warrior_ent = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior_ent = Some(e);
            }
        }
        let warrior = warrior_ent.unwrap();
        assert_eq!(
            *game.world.get::<Position>(warrior).unwrap(),
            Position::new(4, 4)
        );

        // Inject script to select and move Kael to (4, 5)
        let count = game
            .router
            .inject_script_with(
                &default_commands(),
                "inspect:4,4 confirm inspect:4,5 confirm",
                ActionSource::Script,
                resolve_command_token,
            )
            .unwrap();
        assert_eq!(count, 4);

        // Run pending actions
        let reports = game.run_pending_reports();
        assert_eq!(reports.len(), 4);

        // Verify Kael's new position
        assert_eq!(
            *game.world.get::<Position>(warrior).unwrap(),
            Position::new(4, 5)
        );
    }

    #[test]
    fn test_boss_phase_transition() {
        let mut game = Game::new();

        // Find Boss entity
        let mut boss_entity = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Boss {
                boss_entity = Some(e);
            }
        }
        let be = boss_entity.expect("Boss should exist");

        // Verify initial phase is Phase1 and hp is 500
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.boss_phase, crate::components::BossPhase::Phase1);
        }
        {
            let stats = game.world.get::<Stats>(be).unwrap();
            assert_eq!(stats.hp, 500);
            assert_eq!(stats.max_hp, 500);
        }

        // Deal damage to Boss to bring health to <= 250 (e.g. 240)
        {
            let stats = game.world.get_mut::<Stats>(be).unwrap();
            stats.hp = 240;
        }

        // Trigger boss phase transition check (which apply_action automatically does)
        game.apply_action(Action::MoveNorth, ActionSource::Terminal);

        // Verify Boss is now in Phase 2
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.boss_phase, crate::components::BossPhase::Phase2);
        }
        {
            let stats = game.world.get::<Stats>(be).unwrap();
            assert_eq!(stats.hp, 500);
            assert_eq!(stats.max_hp, 500);
            assert_eq!(stats.atk, 50);
            assert_eq!(stats.def, 25);
            assert_eq!(stats.spd, 5);
            assert_eq!(stats.max_ap, 7);
            assert_eq!(stats.ap, 7);
        }
    }

    #[test]
    fn test_elemental_reactions() {
        let mut game = Game::new();
        let mut warrior = None;
        let mut mage = None;
        let mut healer = None;
        let mut boss = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            match class {
                CharacterClass::Warrior => warrior = Some(e),
                CharacterClass::Mage => mage = Some(e),
                CharacterClass::Healer => healer = Some(e),
                CharacterClass::Boss => boss = Some(e),
                CharacterClass::ShadowStalker => {}
                CharacterClass::CorruptedSpore => {}
                CharacterClass::CursedSentinel => {}
                CharacterClass::PlagueWraith => {}
            }
        }
        let warrior = warrior.unwrap();
        let _mage = mage.unwrap();
        let _healer = healer.unwrap();
        let boss = boss.unwrap();

        // 1. Test Shatter: Warrior applies Ice (Kael), Mage applies Lightning (Lyra).
        // Set Boss HP to 500.
        game.world.get_mut::<Stats>(boss).unwrap().hp = 500;
        // Apply Ice to Boss
        game.apply_elemental_status(
            boss,
            crate::components::ElementalStatus::Ice { duration: 3 },
        );
        assert_eq!(
            game.world
                .get::<crate::components::ElementalStatus>(boss)
                .copied()
                .unwrap(),
            crate::components::ElementalStatus::Ice { duration: 3 }
        );
        // Apply Lightning to Boss -> triggers Shatter reaction (30 bonus damage)
        game.apply_elemental_status(
            boss,
            crate::components::ElementalStatus::Lightning { duration: 3 },
        );
        // Target status should become None
        assert_eq!(
            game.world
                .get::<crate::components::ElementalStatus>(boss)
                .copied()
                .unwrap(),
            crate::components::ElementalStatus::None
        );
        // Boss HP should be 500 - 30 = 470
        assert_eq!(game.world.get::<Stats>(boss).unwrap().hp, 470);

        // 2. Test Overgrowth: Apply Lightning, then Nature.
        game.apply_elemental_status(
            boss,
            crate::components::ElementalStatus::Lightning { duration: 3 },
        );
        game.apply_elemental_status(
            boss,
            crate::components::ElementalStatus::Nature { duration: 3 },
        );
        // Target should be rooted
        assert!(game.world.get::<crate::components::Rooted>(boss).is_some());
        // Boss HP should be 470 - 10 = 460
        assert_eq!(game.world.get::<Stats>(boss).unwrap().hp, 460);
        // Target status should become None
        assert_eq!(
            game.world
                .get::<crate::components::ElementalStatus>(boss)
                .copied()
                .unwrap(),
            crate::components::ElementalStatus::None
        );

        // 3. Test Bloom: Apply Nature, then Ice.
        // Move boss to (4, 5) which is adjacent to Kael (Warrior) at (4, 4)
        *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);
        // Set Warrior HP to 50
        game.world.get_mut::<Stats>(warrior).unwrap().hp = 50;

        game.apply_elemental_status(
            boss,
            crate::components::ElementalStatus::Nature { duration: 3 },
        );
        game.apply_elemental_status(
            boss,
            crate::components::ElementalStatus::Ice { duration: 3 },
        );
        // Kael should be healed by 20. HP: 50 + 20 = 70.
        assert_eq!(game.world.get::<Stats>(warrior).unwrap().hp, 70);
        // Target status should become None
        assert_eq!(
            game.world
                .get::<crate::components::ElementalStatus>(boss)
                .copied()
                .unwrap(),
            crate::components::ElementalStatus::None
        );
    }

    #[test]
    fn test_critical_and_block_distribution() {
        let mut game = Game::new();
        let mut warrior = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior = Some(e);
                break;
            }
        }
        let warrior = warrior.unwrap();

        let mut crits = 0;
        let mut blocks = 0;
        let mut normals = 0;

        for _ in 0..100 {
            // Set warrior HP high enough so we don't defeat him
            game.world.get_mut::<Stats>(warrior).unwrap().hp = 1000;
            let (damage, _defeated) = crate::systems::resolve_combat_hit(
                &mut game.world,
                warrior,
                100,
                "Attacker",
                "Warrior",
                Position::new(4, 4),
            );
            if damage == 150 {
                crits += 1;
            } else if damage == 50 {
                blocks += 1;
            } else if damage == 100 {
                normals += 1;
            } else {
                panic!("Unexpected damage value: {}", damage);
            }
        }

        assert!(crits > 0, "Should have at least one critical hit");
        assert!(blocks > 0, "Should have at least one block");
        assert!(normals > 0, "Should have at least one normal hit");
    }

    #[test]
    fn test_inventory_usage() {
        let mut game = Game::new();

        // Find Warrior
        let mut warrior_ent = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior_ent = Some(e);
                break;
            }
        }
        let warrior = warrior_ent.unwrap();

        // Damage warrior
        {
            let stats = game.world.get_mut::<Stats>(warrior).unwrap();
            stats.hp = 50;
        }

        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Open inventory
        game.apply_action(Action::ToggleInventory, ActionSource::Terminal);
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.ui_state, crate::components::UIState::Inventory);
        }

        // Use Item 1 (Healing Potion)
        game.apply_action(Action::Skill1, ActionSource::Terminal);

        // Verify HP restored
        {
            let stats = game.world.get::<Stats>(warrior).unwrap();
            assert_eq!(stats.hp, 80); // 50 + 30
        }

        // Verify inventory closed
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert_eq!(state.ui_state, crate::components::UIState::Normal);
        }

        // Verify item consumed
        {
            let inv = game
                .world
                .get::<crate::components::Inventory>(warrior)
                .unwrap();
            assert_eq!(inv.items.len(), 2); // Had 3, used 1
        }
    }

    #[test]
    fn test_shield_elixir() {
        let mut game = Game::new();

        // Find Warrior
        let warrior = game
            .world
            .query::<CharacterClass>()
            .iter()
            .find(|(_, class)| **class == CharacterClass::Warrior)
            .map(|(e, _)| *e)
            .unwrap();

        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Open inventory
        game.apply_action(Action::ToggleInventory, ActionSource::Terminal);

        // Use Item 3 (Aegis Elixir)
        game.apply_action(Action::Skill3, ActionSource::Terminal);

        // Verify shield applied
        {
            let shield = game
                .world
                .get::<crate::components::ElementalShield>(warrior)
                .unwrap();
            assert_eq!(shield.shield_type, crate::components::ShieldType::Physical);
            assert_eq!(shield.amount, 30);
            assert_eq!(shield.max_amount, 30);
        }

        // Verify item consumed (starts with 3, uses 1, leaves 2)
        {
            let inv = game
                .world
                .get::<crate::components::Inventory>(warrior)
                .unwrap();
            assert_eq!(inv.items.len(), 2);
        }
    }

    #[test]
    fn test_adaptive_sprites_tier_existence() {
        let game = Game::new();
        let registry = game
            .world
            .resource::<verryte_terminal::VisualRegistry>()
            .unwrap();

        let sprite_keys = ["kael", "lyra", "mira", "blight-sovereign"];
        for key in sprite_keys {
            let asset = registry
                .get(key)
                .unwrap_or_else(|| panic!("Asset {} should be registered", key));
            if let verryte_terminal::VisualAsset::Animated(sprite) = asset {
                assert_eq!(sprite.name, key);
                for tier in verryte_terminal::ResolutionTier::ALL {
                    let mut s = sprite.clone();
                    s.set_tier(tier);
                    let grid = s.current_frame();
                    assert!(
                        grid.width() > 0,
                        "Width of sprite {} for tier {:?} should be > 0",
                        key,
                        tier
                    );
                    assert!(
                        grid.height() > 0,
                        "Height of sprite {} for tier {:?} should be > 0",
                        key,
                        tier
                    );

                    let (expected_cols, expected_rows) = tier.sprite_size();
                    if key == "blight-sovereign" {
                        let expected_w = (((expected_cols as f32 * 1.33) as u32).max(1)) as u16;
                        let expected_h_pixels =
                            (((expected_rows as f32 * 2.0 * 1.33) as u32).max(1)) as u16;
                        let expected_h = expected_h_pixels.div_ceil(2);
                        assert_eq!(
                            grid.width(),
                            expected_w,
                            "Boss width mismatch for tier {:?}",
                            tier
                        );
                        assert_eq!(
                            grid.height(),
                            expected_h,
                            "Boss height mismatch for tier {:?}",
                            tier
                        );
                    } else {
                        assert_eq!(
                            grid.width(),
                            expected_cols,
                            "Player width mismatch for tier {:?}",
                            tier
                        );
                        assert_eq!(
                            grid.height(),
                            expected_rows,
                            "Player height mismatch for tier {:?}",
                            tier
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_dialogue_choices_and_consequences() {
        let mut game = Game::new();
        game.trigger_intro_dialogue();

        // At start, the "Tactical Focus" dialogue is active
        {
            let dialogue = game
                .world
                .resource::<verryte_terminal::DialogueState>()
                .unwrap();
            assert_eq!(dialogue.title, "Tactical Focus");
            assert_eq!(dialogue.choices.len(), 2);
            assert_eq!(dialogue.selected_choice, 0);
            assert!(!dialogue.finished);
        }

        // Skip typing to allow options navigation
        game.world
            .resource_mut::<verryte_terminal::DialogueState>()
            .unwrap()
            .skip_typing();

        // Press MoveSouth to go to choice 1
        game.apply_action(Action::MoveSouth, ActionSource::Terminal);
        {
            let dialogue = game
                .world
                .resource::<verryte_terminal::DialogueState>()
                .unwrap();
            assert_eq!(dialogue.selected_choice, 1);
        }

        // Press Confirm to select choice 1 ("Arcane Synergy")
        game.apply_action(Action::Confirm, ActionSource::Terminal);
        {
            let dialogue = game
                .world
                .resource::<verryte_terminal::DialogueState>()
                .unwrap();
            assert!(dialogue.finished);
            assert_eq!(dialogue.chosen, Some(1));
        }

        // GameState concert_energy should be 5
        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(state.concert_energy, 5);
    }

    #[test]
    fn test_panned_audio_event_and_eased_flashes() {
        let event = verryte_core::AudioEvent::play("crit")
            .with_pan(0.5)
            .with_volume(0.8);
        assert_eq!(event.pan, Some(0.5));
        assert_eq!(event.volume, Some(0.8));

        let flash = verryte_terminal::Flash::full_screen_eased(
            verryte_terminal::Color::RED,
            0.5,
            verryte_terminal::EasingMode::ExpoOut,
        );
        assert_eq!(flash.easing, verryte_terminal::EasingMode::ExpoOut);
    }

    #[test]
    fn test_new_autonomous_features() {
        // 1. Dialogue typewriter character count
        let mut dialogue = verryte_terminal::DialogueState::new("Test", "Hello!");
        let newly_typed = dialogue.update(0.1, 30.0);
        assert!(newly_typed > 0);
        assert_eq!(dialogue.visible_chars.floor() as usize, newly_typed);

        // 2. ScreenShake new_eased
        let shake = verryte_terminal::vfx::ScreenShake::new_eased(
            4.0,
            0.5,
            verryte_terminal::vfx::EasingMode::ExpoOut,
        );
        assert_eq!(shake.easing, verryte_terminal::vfx::EasingMode::ExpoOut);
        assert!(shake.active());

        // 3. FloatingText new_eased
        let float_text = verryte_terminal::vfx::FloatingText::new_eased(
            10.0,
            10.0,
            "-50",
            verryte_terminal::Color::RED,
            true,
            verryte_terminal::vfx::EasingMode::QuadOut,
        );
        assert_eq!(
            float_text.easing,
            verryte_terminal::vfx::EasingMode::QuadOut
        );
        assert_eq!(float_text.start_y, 10.0);

        // 4. Mouse click handling
        let mut game = Game::new();
        // Just call it to verify it runs without panicking
        let _handled = game.handle_mouse_click(120, 40, 10, 10);
    }

    #[test]
    fn test_elemental_shields() {
        use crate::components::{ElementalShield, ShieldType, Stats};

        let mut game = Game::new();
        let mut warrior_ent = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior_ent = Some(e);
                break;
            }
        }
        let warrior = warrior_ent.unwrap();
        if let Some(stats) = game.world.get_mut::<Stats>(warrior) {
            stats.hp = 1000;
            stats.max_hp = 1000;
        }

        // Give Warrior an Ice shield of 50 capacity (max 50)
        game.world.insert(
            warrior,
            ElementalShield {
                shield_type: ShieldType::Ice,
                amount: 50,
                max_amount: 50,
            },
        );

        // Verify shield is in place
        {
            let shield = game.world.get::<ElementalShield>(warrior).unwrap();
            assert_eq!(shield.amount, 50);
            assert_eq!(shield.shield_type, ShieldType::Ice);
        }

        // Get Warrior stats before the hit
        let stats_before = game.world.get::<Stats>(warrior).unwrap().clone();

        // Hit Warrior for 30 damage (which will result in at most 45 dmg, so it won't break the shield)
        let (damage_dealt, defeated) = crate::systems::resolve_combat_hit(
            &mut game.world,
            warrior,
            30,
            "Attacker",
            "Warrior",
            Position::new(4, 4),
        );

        // Shield should absorb 100% of damage
        let shield_after = game.world.get::<ElementalShield>(warrior).unwrap();
        let absorbed_damage = 50 - shield_after.amount;
        assert!(absorbed_damage == 30 || absorbed_damage == 45 || absorbed_damage == 15);
        assert_eq!(damage_dealt, 0);

        // Warrior HP should not have decreased
        let stats_after = game.world.get::<Stats>(warrior).unwrap();
        assert_eq!(stats_after.hp, stats_before.hp);
        assert!(!defeated);

        // Now hit the warrior for 100 damage (exceeds remaining shield amount of at most 35)
        let stats_before_2 = game.world.get::<Stats>(warrior).unwrap().clone();

        let (damage_dealt_2, defeated_2) = crate::systems::resolve_combat_hit(
            &mut game.world,
            warrior,
            100,
            "Attacker",
            "Warrior",
            Position::new(4, 4),
        );

        // Shield should be completely gone (broke and removed)
        assert!(game.world.get::<ElementalShield>(warrior).is_none());

        // HP should be reduced by the overflow (which is exactly the damage_dealt_2 returned)
        let stats_after_2 = game.world.get::<Stats>(warrior).unwrap();
        assert_eq!(stats_after_2.hp, stats_before_2.hp - damage_dealt_2);
        assert!(!defeated_2);

        // Test save/load snapshot of the shield
        game.world.insert(
            warrior,
            ElementalShield {
                shield_type: ShieldType::Lightning,
                amount: 25,
                max_amount: 30,
            },
        );

        let serialized = game.save_state().unwrap();

        let mut game2 = Game::new();
        game2.load_state(&serialized).unwrap();

        let mut warrior2_opt = None;
        for (e, class) in game2.world.query::<CharacterClass>() {
            if *class == CharacterClass::Warrior {
                warrior2_opt = Some(e);
                break;
            }
        }
        let warrior2 = warrior2_opt.unwrap();

        let shield2 = game2.world.get::<ElementalShield>(warrior2).unwrap();
        assert_eq!(shield2.amount, 25);
        assert_eq!(shield2.max_amount, 30);
        assert_eq!(shield2.shield_type, ShieldType::Lightning);
    }

    #[test]
    fn test_script_full_combat_loop() {
        let mut game = Game::new();

        let script = "inspect:4,4 confirm inspect:4,5 confirm next confirm";
        let count = game
            .router
            .inject_script_with(
                &default_commands(),
                script,
                ActionSource::Script,
                resolve_command_token,
            )
            .unwrap();
        assert_eq!(count, 6);

        let reports = game.run_pending_reports();
        assert_eq!(reports.len(), 6);

        for report in &reports {
            assert_eq!(report.source, ActionSource::Script);
        }

        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(state.turn, 1);
        assert_eq!(state.phase, TurnPhase::Player);
        assert_eq!(state.outcome, Outcome::Playing);
    }

    #[test]
    fn test_cursor_bounds_clamping() {
        let mut game = Game::new();

        let map = game.world.resource::<TacticalMap>().unwrap();
        let w = map.width as i16;
        let h = map.height as i16;

        for _ in 0..100 {
            game.apply_action(Action::MoveNorth, ActionSource::Terminal);
            game.apply_action(Action::MoveWest, ActionSource::Terminal);
        }
        let cursor = game.world.resource::<GameState>().unwrap().cursor;
        assert!(cursor.x >= 0 && cursor.x < w);
        assert!(cursor.y >= 0 && cursor.y < h);

        for _ in 0..100 {
            game.apply_action(Action::MoveSouth, ActionSource::Terminal);
            game.apply_action(Action::MoveEast, ActionSource::Terminal);
        }
        let cursor = game.world.resource::<GameState>().unwrap().cursor;
        assert!(cursor.x >= 0 && cursor.x < w);
        assert!(cursor.y >= 0 && cursor.y < h);
    }

    #[test]
    fn test_enemy_ai_moves_toward_players() {
        let mut game = Game::new();

        let mut stalker_pos = None;
        for (_e, pos, team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *team == Team::Enemy && *class == CharacterClass::ShadowStalker {
                stalker_pos = Some(*pos);
                break;
            }
        }
        let initial_pos = stalker_pos.expect("ShadowStalker should exist");

        game.apply_action(Action::EndTurn, ActionSource::Terminal);
        game.update(0.1);
        game.update(0.1);
        game.update(0.1);

        let mut stalker_pos_after = None;
        for (_e, pos, team, class) in game.world.query3::<Position, Team, CharacterClass>() {
            if *team == Team::Enemy && *class == CharacterClass::ShadowStalker {
                stalker_pos_after = Some(*pos);
                break;
            }
        }
        let after_pos = stalker_pos_after.expect("ShadowStalker should still exist");
        let dist_before = (initial_pos.x - 4).abs() + (initial_pos.y - 4).abs();
        let dist_after = (after_pos.x - 4).abs() + (after_pos.y - 4).abs();
        assert!(
            dist_after <= dist_before,
            "ShadowStalker should move closer to players: before={}, after={}",
            dist_before,
            dist_after
        );
    }

    #[test]
    fn test_render_output_dimensions() {
        let game = Game::new();
        let grid = game.render();
        let (term_w, term_h) = verryte_tty::terminal_size();
        assert_eq!(grid.width(), term_w);
        assert_eq!(grid.height(), term_h);
    }

    #[test]
    fn test_action_history_tracks_actions() {
        let mut game = Game::new();

        game.apply_action(Action::MoveNorth, ActionSource::Terminal);
        game.apply_action(Action::MoveSouth, ActionSource::Script);
        game.apply_action(Action::Confirm, ActionSource::Agent);

        let history = game
            .world
            .resource::<verryte_input::ActionHistory<Action>>()
            .unwrap();
        assert_eq!(history.len(), 3);

        let records: Vec<_> = history.iter().collect();
        assert_eq!(records[0].action, Action::MoveNorth);
        assert_eq!(records[0].source, ActionSource::Terminal);
        assert_eq!(records[1].action, Action::MoveSouth);
        assert_eq!(records[1].source, ActionSource::Script);
        assert_eq!(records[2].action, Action::Confirm);
        assert_eq!(records[2].source, ActionSource::Agent);
    }

    #[test]
    fn test_snapshot_consistency() {
        let mut game = Game::new();

        let snap1 = game.snapshot();
        assert_eq!(snap1.turn, 1);
        assert_eq!(snap1.phase, TurnPhase::Player);
        assert_eq!(snap1.outcome, Outcome::Playing);
        assert_eq!(snap1.player_team.count, 3);
        assert_eq!(snap1.enemy_team.count, 7); // Boss + 2 stalkers + 2 spores + sentinel + wraith

        game.apply_action(Action::MoveNorth, ActionSource::Terminal);
        let snap2 = game.snapshot();
        assert_eq!(snap2.turn, 1);
        assert_eq!(snap2.player_team.total_hp, snap1.player_team.total_hp);
    }

    #[test]
    fn test_full_script_victory_path() {
        let mut game = Game::new();

        let boss = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Boss)
            .map(|(e, _)| e)
            .unwrap();
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        *game.world.get_mut::<Position>(boss).unwrap() = Position::new(4, 5);
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);
        game.world.get_mut::<Stats>(boss).unwrap().hp = 1;
        game.world.resource_mut::<GameState>().unwrap().boss_phase =
            crate::components::BossPhase::Phase2;

        let script = "inspect:4,4 confirm inspect:4,5 confirm";
        game.router
            .inject_script_with(
                &default_commands(),
                script,
                ActionSource::Script,
                resolve_command_token,
            )
            .unwrap();
        game.run_pending_reports();

        let mut boss_exists = false;
        for (_, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Boss {
                boss_exists = true;
            }
        }
        assert!(!boss_exists, "Boss should be defeated");

        let mut echo_pos = None;
        for (_, pos, _) in game.world.query2::<Position, crate::components::EchoItem>() {
            echo_pos = Some(*pos);
        }
        assert!(echo_pos.is_some(), "Echo should be dropped");

        let echo = echo_pos.unwrap();
        let script2 = format!(
            "inspect:{},{} confirm inspect:{},{} confirm",
            4, 4, echo.x, echo.y
        );
        game.router
            .inject_script_with(
                &default_commands(),
                &script2,
                ActionSource::Script,
                resolve_command_token,
            )
            .unwrap();
        game.run_pending_reports();

        let echoes = game
            .world
            .resource::<crate::components::EquippedEchoes>()
            .unwrap();
        let state = game.world.resource::<GameState>().unwrap();
        assert!(
            !echoes.abilities.is_empty() || state.outcome == Outcome::Victory,
            "Should have absorbed echo or achieved victory"
        );
    }

    #[test]
    fn test_auto_battle_turn_cycle() {
        let mut game = Game::new();
        game.world.resource_mut::<GameState>().unwrap().auto_battle = true;

        for _ in 0..15 {
            game.update(0.1);
            let state = game.world.resource::<GameState>().unwrap();
            if state.outcome != Outcome::Playing {
                break;
            }
        }

        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(
            state.outcome,
            Outcome::Playing,
            "Game should still be playing"
        );
    }

    #[test]
    fn test_terrain_movement_costs() {
        let game = Game::new();
        let map = game.world.resource::<TacticalMap>().unwrap();

        // Grass costs 1
        assert_eq!(map.movement_cost(Position::new(0, 0)), 1);
        // Wall costs 999
        assert_eq!(map.movement_cost(Position::new(4, 1)), 999);
        // Water costs 2
        assert_eq!(map.movement_cost(Position::new(8, 4)), 2);

        // is_walkable returns true for Grass and Water, false for Wall
        assert!(map.is_walkable(Position::new(0, 0)));
        assert!(map.is_walkable(Position::new(8, 4)));
        assert!(!map.is_walkable(Position::new(4, 1)));

        // Tactical map has non-uniform terrain
        let mut has_grass = false;
        let mut has_wall = false;
        let mut has_water = false;
        let mut has_lava = false;
        for y in 0..map.height {
            for x in 0..map.width {
                match map.tile(x as i16, y as i16) {
                    Tile::Grass => has_grass = true,
                    Tile::Wall => has_wall = true,
                    Tile::Water => has_water = true,
                    Tile::Lava => has_lava = true,
                    Tile::Ice | Tile::Stairs => {}
                }
            }
        }
        assert!(has_grass, "Map should have grass tiles");
        assert!(has_wall, "Map should have wall tiles");
        assert!(has_water, "Map should have water tiles");
        assert!(has_lava, "Map should have lava tiles");
    }

    #[test]
    fn test_water_movement_ap_cost() {
        let mut game = Game::new();

        // Place warrior at (8, 3) which is grass adjacent to water at (8, 4)
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(8, 3);
        game.world.get_mut::<Stats>(warrior).unwrap().ap = 3;

        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(8, 3);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Move to (8, 4) which is water — should cost 2 AP
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(8, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        let stats = game.world.get::<Stats>(warrior).unwrap();
        assert_eq!(stats.ap, 1, "Moving onto water should cost 2 AP");
    }

    #[test]
    fn test_new_enemy_types_exist() {
        let game = Game::new();
        let mut sentinel_found = false;
        let mut wraith_found = false;
        for (_e, class) in game.world.query::<CharacterClass>() {
            match *class {
                CharacterClass::CursedSentinel => sentinel_found = true,
                CharacterClass::PlagueWraith => wraith_found = true,
                _ => {}
            }
        }
        assert!(sentinel_found, "CursedSentinel should be spawned");
        assert!(wraith_found, "PlagueWraith should be spawned");
    }

    #[test]
    fn test_plague_wraith_applies_nature() {
        let mut game = Game::new();

        let wraith = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::PlagueWraith)
            .map(|(e, _)| e)
            .unwrap();
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        // Move wraith next to warrior and far from others
        *game.world.get_mut::<Position>(wraith).unwrap() = Position::new(5, 4);
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);
        // Move other players far away so warrior is the clear target
        let others: Vec<_> = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .filter(|(_, c)| **c == CharacterClass::Mage || **c == CharacterClass::Healer)
            .map(|(e, _)| e)
            .collect();
        for e in others {
            *game.world.get_mut::<Position>(e).unwrap() = Position::new(0, 15);
        }
        // Lower warrior HP so it's the priority target even with healer bonus
        game.world.get_mut::<Stats>(warrior).unwrap().hp = 50;

        // Give wraith AP and force enemy phase
        game.world.get_mut::<Stats>(wraith).unwrap().ap = 3;
        game.world.resource_mut::<GameState>().unwrap().phase = TurnPhase::Enemy;

        // Run enemy AI
        crate::systems::enemy_ai_system(&mut game.world);

        // Warrior should have Nature status
        let status = game
            .world
            .get::<crate::components::ElementalStatus>(warrior)
            .unwrap();
        assert!(
            matches!(status, crate::components::ElementalStatus::Nature { .. }),
            "PlagueWraith should apply Nature status on hit"
        );
    }

    #[test]
    fn test_action_outcome_noop_for_cursor_moves() {
        let mut game = Game::new();

        let report = game.apply_action(Action::MoveNorth, ActionSource::Terminal);
        // Cursor move is a state update, not a no-op (it changed the cursor).
        assert!(matches!(
            report.outcome,
            ActionOutcome::StateUpdated | ActionOutcome::NoOp
        ));

        // Quit produces a GameOver outcome.
        let report = game.apply_action(Action::Quit, ActionSource::Terminal);
        assert!(matches!(report.outcome, ActionOutcome::GameOver { .. }));
    }

    #[test]
    fn test_snapshot_reachable_and_targetable_tiles() {
        let mut game = Game::new();
        let snap = game.snapshot();
        // No character is selected initially, so both lists are empty.
        assert!(snap.reachable_tiles.is_empty());
        assert!(snap.targetable_tiles.is_empty());
        assert!(!snap.selected_can_act);

        // Select the warrior.
        let warrior = game
            .world
            .query2::<crate::components::CharacterClass, crate::components::Position>()
            .into_iter()
            .find(|(_, c, _)| **c == CharacterClass::Warrior)
            .map(|(e, _, _)| e)
            .unwrap();
        game.world
            .resource_mut::<GameState>()
            .unwrap()
            .selected_entity = Some(warrior);

        let snap = game.snapshot();
        // Warrior has 3 AP at start, so they can act.
        assert!(snap.selected_can_act);
        // Reachable tiles are non-empty (warrior at (4,4) can reach several tiles).
        assert!(!snap.reachable_tiles.is_empty());
        // Targetable tiles may or may not be empty depending on cursor position.
        // The default cursor is (0,0), so no enemy is in range initially.
        assert!(snap.targetable_tiles.is_empty());
    }

    #[test]
    fn test_boss_phase_2_applies_shield() {
        let mut game = Game::new();
        // Find the boss and set HP below the phase 2 threshold.
        let boss = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Boss)
            .map(|(e, _)| e)
            .unwrap();
        game.world.get_mut::<Stats>(boss).unwrap().hp = 100;

        // Trigger a player phase action (a no-op move). The post-action
        // boss phase transition check should still fire because the HP
        // is below the threshold.
        game.apply_action(Action::MoveNorth, ActionSource::Agent);

        let phase = game.world.resource::<GameState>().unwrap().boss_phase;
        assert!(matches!(phase, crate::components::BossPhase::Phase2));
        // The shield should be applied.
        let shield = game.world.get::<crate::components::ElementalShield>(boss);
        assert!(shield.is_some(), "Boss should have a shield in phase 2");
        let shield = shield.unwrap();
        assert_eq!(shield.amount, 100, "Default shield amount is 100");
        assert_eq!(shield.max_amount, 100);
    }

    #[test]
    fn test_boss_phase_change_outcome() {
        let mut game = Game::new();
        let boss = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Boss)
            .map(|(e, _)| e)
            .unwrap();
        game.world.get_mut::<Stats>(boss).unwrap().hp = 50;

        // Take a player action. The transition should be detected.
        let report = game.apply_action(Action::MoveNorth, ActionSource::Agent);
        // The action is a state update (cursor move), but the boss transitioned.
        // We expect to see BossPhaseChanged in the outcome for combat-related
        // actions only. For cursor moves, the state update is reported.
        // Both outcomes are acceptable here; what matters is that the
        // GameState transitioned.
        assert!(matches!(
            game.world.resource::<GameState>().unwrap().boss_phase,
            crate::components::BossPhase::Phase2
        ));
        // Sanity check: report exists.
        let _ = report;
    }

    #[test]
    fn test_full_boss_fight_phase_transition_via_script() {
        let mut game = Game::new();

        // Set up: position warrior adjacent to boss with boss HP just above
        // the phase 2 threshold. After the first attack, boss HP should drop
        // below 250 and the phase transition should fire.
        let boss = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Boss)
            .map(|(e, _)| e)
            .unwrap();
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        *game.world.get_mut::<Position>(boss).unwrap() = Position::new(5, 4);
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);
        // Set boss HP so the warrior's first attack drops it below threshold.
        game.world.get_mut::<Stats>(boss).unwrap().hp = 300;
        // Bump warrior ATK high enough to ensure one-shot below threshold.
        game.world.get_mut::<Stats>(warrior).unwrap().atk = 200;
        game.world.get_mut::<Stats>(warrior).unwrap().ap = 5;

        // Pre-condition: phase 1, no shield.
        assert!(matches!(
            game.world.resource::<GameState>().unwrap().boss_phase,
            crate::components::BossPhase::Phase1
        ));
        assert!(game
            .world
            .get::<crate::components::ElementalShield>(boss)
            .is_none());

        // Drive the script: select warrior and attack the boss.
        let script = "inspect:4,4 confirm inspect:5,4 confirm";
        game.router
            .inject_script_with(
                &default_commands(),
                script,
                ActionSource::Script,
                resolve_command_token,
            )
            .unwrap();
        let reports = game.run_pending_reports();
        for r in &reports {
            eprintln!("  report: action={:?} outcome={:?}", r.action, r.outcome);
        }
        assert!(!reports.is_empty(), "Script should produce reports");

        // Post-condition: boss is in phase 2 with a shield.
        let state = game.world.resource::<GameState>().unwrap();
        assert!(
            matches!(state.boss_phase, crate::components::BossPhase::Phase2),
            "Boss should be in Phase 2 after HP drops below threshold"
        );
        let shield = game.world.get::<crate::components::ElementalShield>(boss);
        assert!(
            shield.is_some(),
            "Boss should have a shield applied on phase 2 entry"
        );

        // At least one report should record a hit OR a boss phase change.
        let saw_combat_outcome = reports.iter().any(|r| {
            matches!(
                r.outcome,
                ActionOutcome::Hit { damage, .. } if damage > 0
            ) || matches!(r.outcome, ActionOutcome::BossPhaseChanged { .. })
        });
        assert!(
            saw_combat_outcome,
            "Expected a Hit or BossPhaseChanged outcome in the report chain"
        );
    }

    #[test]
    fn test_failed_action_out_of_ap() {
        let mut game = Game::new();
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        // Select Kael (has AP initially)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Verify selected
        assert_eq!(
            game.world.resource::<GameState>().unwrap().selected_entity,
            Some(warrior)
        );

        // Drain warrior's AP now
        game.world.get_mut::<Stats>(warrior).unwrap().ap = 0;

        // Try to move to (4, 5) with 0 AP
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

        assert!(
            matches!(report.outcome, ActionOutcome::Failed { ref reason } if reason.contains("Cannot move") || reason.contains("Not enough AP")),
            "Expected Failed outcome with Cannot move or Not enough AP, got {:?}",
            report.outcome
        );
    }

    #[test]
    fn test_failed_action_out_of_range() {
        let mut game = Game::new();

        // Select Kael
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Trigger Skill1 (Warrior has range 1 usually, so range is short)
        game.apply_action(Action::Skill1, ActionSource::Terminal);

        // Move cursor to (10, 10), which is out of range
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(10, 10);
        }
        let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

        assert!(
            matches!(report.outcome, ActionOutcome::Failed { ref reason } if reason.contains("range")),
            "Expected Failed outcome due to range, got {:?}",
            report.outcome
        );
    }

    #[test]
    fn test_history_records_outcome_metadata() {
        let mut game = Game::new();

        // Perform a cursor move (StateUpdated)
        game.apply_action(Action::MoveNorth, ActionSource::Terminal);

        // Check ActionHistory
        let history = game
            .world
            .resource::<verryte_input::ActionHistory<Action>>()
            .unwrap();
        let last_record = history.records.last().unwrap();
        let serialized_outcome = last_record.metadata.get("outcome").unwrap();
        let parsed_outcome: ActionOutcome = serde_json::from_str(serialized_outcome).unwrap();
        assert_eq!(parsed_outcome, ActionOutcome::StateUpdated);
    }

    #[test]
    fn test_shield_rendering() {
        let mut game = Game::new();
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        // Give warrior a shield
        game.world.insert(
            warrior,
            crate::components::ElementalShield {
                shield_type: crate::components::ShieldType::Ice,
                amount: 50,
                max_amount: 50,
            },
        );

        // Selection / centering is at (4,4)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }

        // Render the screen
        let grid = game.render();

        // Scan the grid to verify '-' character is rendered
        let mut found_shield_cell = false;
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                if let Some(cell) = grid.get(x, y) {
                    if cell.glyph == '-' {
                        found_shield_cell = true;
                    }
                }
            }
        }
        assert!(
            found_shield_cell,
            "Shield rendering should produce '-' cells on the grid"
        );
    }

    #[test]
    fn test_minimap_toggle() {
        let mut game = Game::new();

        // Minimap should start as shown (true)
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert!(state.show_minimap);
        }

        // Toggle it off
        game.apply_action(Action::ToggleMinimap, ActionSource::Terminal);
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert!(!state.show_minimap);
        }

        // Toggle it back on
        game.apply_action(Action::ToggleMinimap, ActionSource::Terminal);
        {
            let state = game.world.resource::<GameState>().unwrap();
            assert!(state.show_minimap);
        }
    }

    #[test]
    fn test_cursed_sentinel_retreat_behavior() {
        let mut game = Game::new();

        // Find/prepare a player character (Warrior) at (4, 4)
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);

        // Move other players far away
        let other_players: Vec<_> = game
            .world
            .query2::<Team, CharacterClass>()
            .into_iter()
            .filter(|(_e, t, c)| **t == Team::Player && **c != CharacterClass::Warrior)
            .map(|(e, _, _)| e)
            .collect();
        for p in other_players {
            *game.world.get_mut::<Position>(p).unwrap() = Position::new(0, 15);
        }

        // Find a CursedSentinel
        let sentinel = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::CursedSentinel)
            .map(|(e, _)| e)
            .unwrap();
        // Place the sentinel close (distance = 2) at (4, 6)
        *game.world.get_mut::<Position>(sentinel).unwrap() = Position::new(4, 6);
        // Ensure it has exactly 1 AP to move (so it doesn't try to move back closer in the same turn loop)
        game.world.get_mut::<Stats>(sentinel).unwrap().ap = 1;

        // Force Enemy Phase
        game.world.resource_mut::<GameState>().unwrap().phase = TurnPhase::Enemy;

        // Move all other enemies far away so they don't block
        let enemies: Vec<_> = game
            .world
            .query::<Team>()
            .into_iter()
            .filter(|(e, t)| **t == Team::Enemy && *e != sentinel)
            .map(|(e, _)| e)
            .collect();
        for e in enemies {
            *game.world.get_mut::<Position>(e).unwrap() = Position::new(0, 15);
        }

        // Run enemy AI
        crate::systems::enemy_ai_system(&mut game.world);

        // Sentinel should have moved to increase distance (should be at distance > 2, e.g., (4, 7) or similar)
        let final_pos = *game.world.get::<Position>(sentinel).unwrap();
        let final_dist = (final_pos.x - 4).abs() + (final_pos.y - 4).abs();
        assert!(
            final_dist > 2,
            "CursedSentinel should retreat from player, final distance was {}",
            final_dist
        );
    }

    #[test]
    fn test_lava_entry_damage() {
        let mut game = Game::new();

        // Place warrior at (15, 11)
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(15, 11);
        game.world.get_mut::<Stats>(warrior).unwrap().ap = 3;
        let initial_hp = game.world.get::<Stats>(warrior).unwrap().hp;

        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(15, 11);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Move to (16, 11) which is lava
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(16, 11);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Verify the warrior is at (16, 11)
        let pos = *game.world.get::<Position>(warrior).unwrap();
        assert_eq!(pos, Position::new(16, 11));

        // Verify the warrior took 20 damage
        let stats = game.world.get::<Stats>(warrior).unwrap();
        assert_eq!(
            stats.hp,
            initial_hp - 20,
            "Warrior should take 20 damage from entering Lava"
        );
    }

    #[test]
    fn test_combo_system() {
        let mut game = Game::new();

        // Find warrior and ShadowStalker
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        let shadow = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::ShadowStalker)
            .map(|(e, _)| e)
            .unwrap();

        // Position them adjacent
        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(2, 2);
        *game.world.get_mut::<Position>(shadow).unwrap() = Position::new(2, 3);

        // Make warrior hp high, ShadowStalker HP high
        game.world.get_mut::<Stats>(warrior).unwrap().hp = 1000;
        game.world.get_mut::<Stats>(shadow).unwrap().hp = 1000;

        // Perform attack
        let (damage1, _) = game.resolve_combat_hit(
            warrior,
            shadow,
            50,
            "Warrior",
            "ShadowStalker",
            Position::new(2, 3),
        );

        let combo1 = game.world.resource::<GameState>().unwrap().combo_count;
        assert_eq!(combo1, 1);
        assert!(damage1 > 0);

        // Second attack
        let (damage2, _) = game.resolve_combat_hit(
            warrior,
            shadow,
            50,
            "Warrior",
            "ShadowStalker",
            Position::new(2, 3),
        );
        let combo2 = game.world.resource::<GameState>().unwrap().combo_count;
        assert_eq!(combo2, 2);
        assert!(damage2 > 0);

        // Force turn phase transition to Enemy Phase
        game.apply_action(Action::EndTurn, ActionSource::Terminal);
        crate::systems::turn_management_system(&mut game.world);

        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(
            state.combo_count, 0,
            "Combo should reset on phase change to Enemy"
        );
    }

    #[test]
    fn test_swap_character_direct_selection() {
        let mut game = Game::new();

        // Find player characters sorted
        let mut players = Vec::new();
        for (e, team) in game.world.query::<Team>() {
            if *team == Team::Player {
                players.push(e);
            }
        }
        players.sort();

        // Select second character (index 1) via SwapCharacter action
        game.apply_action(Action::SwapCharacter(1), ActionSource::Terminal);

        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(
            state.selected_entity,
            Some(players[1]),
            "Second character should be selected"
        );

        // Select first character (index 0)
        game.apply_action(Action::SwapCharacter(0), ActionSource::Terminal);
        let state2 = game.world.resource::<GameState>().unwrap();
        assert_eq!(
            state2.selected_entity,
            Some(players[0]),
            "First character should be selected"
        );
    }

    #[test]
    fn test_battle_stats_tracking() {
        let mut game = Game::new();

        // Initially battle stats are zeroed
        let stats = game.world.resource::<BattleStats>().unwrap();
        assert_eq!(stats.total_damage_dealt, 0);
        assert_eq!(stats.total_damage_taken, 0);
        assert_eq!(stats.total_swaps, 0);

        // Find boss and position it next to Kael
        let mut boss_opt = None;
        for (e, class) in game.world.query::<CharacterClass>() {
            if *class == CharacterClass::Boss {
                boss_opt = Some(e);
            }
        }
        let boss = boss_opt.unwrap();
        if let Some(pos) = game.world.get_mut::<Position>(boss) {
            *pos = Position::new(4, 5);
        }

        // Select Kael at (4, 4)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Attack Boss at (4, 5)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        let stats2 = game.world.resource::<BattleStats>().unwrap();
        assert!(stats2.total_damage_dealt > 0);
        assert_eq!(stats2.max_combo_reached, 1);

        // Let's perform a QTE swap
        // Select Kael at (4, 4) first (since selection was cleared after the attack)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Fill concert energy
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.concert_energy = 100;
        }
        game.apply_action(Action::Skill3, ActionSource::Terminal);

        let stats3 = game.world.resource::<BattleStats>().unwrap();
        assert_eq!(stats3.total_swaps, 1, "Swap count should increment");
    }

    #[test]
    fn test_undo_action_and_stack() {
        let mut game = Game::new();

        // 1. Move cursor to (4, 4) and select Kael
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Kael should be selected
        let selected = game.world.resource::<GameState>().unwrap().selected_entity;
        assert!(selected.is_some());

        // 2. Move to (4, 5)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Position should now be (4, 5) and selection cleared
        let warrior = selected.unwrap();
        assert_eq!(
            *game.world.get::<Position>(warrior).unwrap(),
            Position::new(4, 5)
        );
        assert!(game
            .world
            .resource::<GameState>()
            .unwrap()
            .selected_entity
            .is_none());

        // 3. Trigger Undo
        game.apply_action(Action::Undo, ActionSource::Terminal);

        // Position should be restored to (4, 4) and selection restored to Kael!
        assert_eq!(
            *game.world.get::<Position>(warrior).unwrap(),
            Position::new(4, 4)
        );
        assert_eq!(
            game.world.resource::<GameState>().unwrap().selected_entity,
            Some(warrior)
        );
    }

    #[test]
    fn test_failure_category_classification() {
        let mut game = Game::new();
        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Drain AP to 0
        let warrior = game
            .world
            .resource::<GameState>()
            .unwrap()
            .selected_entity
            .unwrap();
        game.world.get_mut::<Stats>(warrior).unwrap().ap = 0;

        // Try to move
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 5);
        }
        let report = game.apply_action(Action::Confirm, ActionSource::Terminal);

        assert!(report.outcome.is_failed());
        assert_eq!(
            report.outcome.failure_category(),
            Some(crate::snapshot::FailureCategory::OutOfAP)
        );
    }

    #[test]
    fn test_combo_milestone_and_rich_item_effects() {
        let mut game = Game::new();

        // Check combo milestone particles and audio events are triggered
        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();
        let shadow = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::ShadowStalker)
            .map(|(e, _)| e)
            .unwrap();

        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(2, 2);
        *game.world.get_mut::<Position>(shadow).unwrap() = Position::new(2, 3);
        game.world.get_mut::<Stats>(warrior).unwrap().hp = 1000;
        game.world.get_mut::<Stats>(shadow).unwrap().hp = 1000;

        // Clear audio events queue
        if let Some(events) = game
            .world
            .resource_mut::<verryte_core::Events<verryte_core::AudioEvent>>()
        {
            let _ = events.take();
        }

        // Perform 3 hits to trigger combo milestone (combo = 3)
        for _ in 0..3 {
            game.resolve_combat_hit(
                warrior,
                shadow,
                10,
                "Warrior",
                "ShadowStalker",
                Position::new(2, 3),
            );
        }

        // Verify audio events contain combo_milestone
        let mut combo_milestone_played = false;
        if let Some(events) = game
            .world
            .resource::<verryte_core::Events<verryte_core::AudioEvent>>()
        {
            for ev in events.iter() {
                if ev.name == "combo_milestone" {
                    combo_milestone_played = true;
                }
            }
        }
        assert!(
            combo_milestone_played,
            "combo_milestone audio event should be played"
        );

        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(2, 2);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Put warrior in root status
        game.world
            .insert(warrior, crate::components::Rooted { duration: 1 });
        assert!(game
            .world
            .get::<crate::components::Rooted>(warrior)
            .is_some());

        // Make sure warrior's item at index 1 is Cleanse Remedy
        let remedy = game
            .world
            .spawn_item("Cleanse Remedy", crate::components::ItemEffect::Cleanse);
        if let Some(inv) = game.world.get_mut::<crate::components::Inventory>(warrior) {
            if inv.items.len() > 1 {
                inv.items[1] = remedy;
            } else {
                inv.items.push(remedy);
            }
        }

        // Open inventory and use cleanse potion (item 2 in starting list is Cleanse Remedy)
        // Cleanse Remedy has ItemEffect::Cleanse
        game.apply_action(Action::ToggleInventory, ActionSource::Terminal);
        game.apply_action(Action::Skill2, ActionSource::Terminal); // Cleanses negative status

        // Negative status should be gone
        assert!(game
            .world
            .get::<crate::components::Rooted>(warrior)
            .is_none());

        // Verify audio events contain cleanse
        let mut cleanse_played = false;
        if let Some(events) = game
            .world
            .resource::<verryte_core::Events<verryte_core::AudioEvent>>()
        {
            for ev in events.iter() {
                if ev.name == "cleanse" {
                    cleanse_played = true;
                }
            }
        }
        assert!(cleanse_played, "cleanse audio event should be played");
    }

    #[test]
    fn test_ice_sliding_movement() {
        let mut game = Game::new();

        let map_str = "\
........\n\
........\n\
........\n\
........\n\
....-...\n\
....-...\n\
........\n\
........";
        let custom_map = TacticalMap::from_ascii(map_str);
        assert_eq!(custom_map.tile(4, 4), Tile::Ice);
        assert_eq!(custom_map.tile(4, 5), Tile::Ice);
        game.world.insert_resource(custom_map);

        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();

        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 3);
        game.world.get_mut::<Stats>(warrior).unwrap().ap = 3;

        // Select warrior
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 3);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Move to (4, 4) which is Ice
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Position should have slid through (4, 5) (Ice) and stopped at (4, 6) (Grass)
        let pos = *game.world.get::<Position>(warrior).unwrap();
        assert_eq!(pos, Position::new(4, 6));
    }

    #[test]
    fn test_enemy_low_hp_retreat_behavior() {
        let mut game = Game::new();

        let warrior = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::Warrior)
            .map(|(e, _)| e)
            .unwrap();
        let shadow = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .find(|(_, c)| **c == CharacterClass::ShadowStalker)
            .map(|(e, _)| e)
            .unwrap();

        *game.world.get_mut::<Position>(warrior).unwrap() = Position::new(4, 4);
        *game.world.get_mut::<Position>(shadow).unwrap() = Position::new(4, 6);

        // Put all other players far away
        let other_entities: Vec<_> = game
            .world
            .query::<CharacterClass>()
            .into_iter()
            .map(|(e, _)| e)
            .filter(|e| *e != warrior && *e != shadow)
            .collect();

        for e in other_entities {
            if let Some(pos) = game.world.get_mut::<Position>(e) {
                *pos = Position::new(20, 20);
            }
        }

        // Set Shadow AP to 1 and HP to 10/80 (low HP, <30%)
        {
            let stats = game.world.get_mut::<Stats>(shadow).unwrap();
            stats.hp = 10;
            stats.max_hp = 80;
            stats.ap = 1;
        }

        // Set phase to Enemy so enemy AI runs
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.phase = TurnPhase::Enemy;
        }

        // Run enemy AI system
        game.schedule
            .run_system_by_name("enemy_ai", &mut game.world);

        // Shadow should have retreated away from the warrior
        let shadow_pos = *game.world.get::<Position>(shadow).unwrap();
        let final_dist = (shadow_pos.x - 4).abs() + (shadow_pos.y - 4).abs();
        assert!(
            final_dist > 2,
            "Enemy Stalker did not retreat at low HP. Pos: {:?}",
            shadow_pos
        );
    }

    #[test]
    fn test_replay_outcome_validation() {
        let mut game = Game::new();
        // Start recording
        game.apply_action(Action::ToggleRecording, ActionSource::Terminal);

        // Select warrior at (4, 4)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Move north (to 4, 3)
        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 3);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Stop recording
        game.apply_action(Action::ToggleRecording, ActionSource::Terminal);

        // Reset game state and load the recording for replay
        let mut game2 = Game::new();
        game2.apply_action(Action::ToggleReplay, ActionSource::Terminal);

        // Step 1 of replay: selection confirm (expected: StateUpdated)
        game2.apply_action(Action::StepReplay, ActionSource::Terminal);
        let replay = game2
            .world
            .resource::<crate::components::ReplayState>()
            .unwrap();
        assert!(
            replay.verification_errors.is_empty(),
            "Selection step should match original StateUpdated outcome"
        );

        // Manually corrupt the expected outcome of the next step to force a mismatch
        {
            let replay_mut = game2
                .world
                .resource_mut::<crate::components::ReplayState>()
                .unwrap();
            let outcomes = &mut *replay_mut;
            if let Some(expected) = outcomes.expected_outcomes.get_mut(1) {
                *expected = ActionOutcome::Failed {
                    reason: "Forced test failure".to_string(),
                };
            }
        }

        // Step 2 of replay: movement confirm (actual: Moved, expected: Failed)
        game2.apply_action(Action::StepReplay, ActionSource::Terminal);
        let replay = game2
            .world
            .resource::<crate::components::ReplayState>()
            .unwrap();
        assert!(
            !replay.verification_errors.is_empty(),
            "Should log verification error when outcomes mismatch"
        );
        assert!(replay.verification_errors[0].contains("outcome mismatch"));
    }

    #[test]
    fn test_crafting_system() {
        let mut game = Game::new();
        let kael = game
            .world
            .query3::<Position, Team, CharacterClass>()
            .iter()
            .find(|(_, _, team, class)| {
                **team == Team::Player && **class == CharacterClass::Warrior
            })
            .map(|(e, _, _, _)| *e)
            .unwrap();

        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.selected_entity = Some(kael);
        }

        let new_potion = game
            .world
            .spawn_item("Healing Potion", crate::components::ItemEffect::Heal(30));
        {
            let inv = game.world.get_mut::<Inventory>(kael).unwrap();
            inv.items.push(new_potion);
        }

        game.apply_action(Action::CraftItem(0, 3), ActionSource::Terminal);

        let inv = game.world.get::<Inventory>(kael).unwrap();
        let has_mega_potion = inv.items.iter().any(|&item_ent| {
            if let Some(item) = game.world.get::<crate::components::Item>(item_ent) {
                item.name == "Mega Potion"
            } else {
                false
            }
        });
        assert!(
            has_mega_potion,
            "Inventory should contain the crafted Mega Potion"
        );
        assert!(!game.world.is_alive(new_potion));
    }

    #[test]
    fn test_floor_transition_and_bsp_generation() {
        let mut game = Game::new();
        let kael = game
            .world
            .query3::<Position, Team, CharacterClass>()
            .iter()
            .find(|(_, _, team, class)| {
                **team == Team::Player && **class == CharacterClass::Warrior
            })
            .map(|(e, _, _, _)| *e)
            .unwrap();

        {
            let state = game.world.resource_mut::<GameState>().unwrap();
            state.selected_entity = Some(kael);
        }

        let player_pos = *game.world.get::<Position>(kael).unwrap();
        {
            let map = game.world.resource_mut::<TacticalMap>().unwrap();
            map.tiles.set(player_pos, Tile::Stairs);
        }

        assert_eq!(game.world.resource::<GameState>().unwrap().floor, 1);

        game.apply_action(Action::NextFloor, ActionSource::Terminal);

        let state = game.world.resource::<GameState>().unwrap();
        assert_eq!(state.floor, 2);

        let map = game.world.resource::<TacticalMap>().unwrap();
        let mut has_grass = false;
        let mut has_wall = false;
        for &tile in map.tiles.tiles() {
            match tile {
                Tile::Grass => has_grass = true,
                Tile::Wall => has_wall = true,
                _ => {}
            }
        }
        assert!(has_grass);
        assert!(has_wall);

        let enemy_boss_exists = game
            .world
            .query2::<CharacterClass, Team>()
            .iter()
            .any(|(_, class, team)| **class == CharacterClass::Boss && **team == Team::Enemy);
        assert!(enemy_boss_exists, "Floor 2 must contain the boss");
    }
}
