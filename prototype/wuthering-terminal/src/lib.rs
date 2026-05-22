//! Wuthering Terminal — tactical RPG prototype.

pub mod action;
pub mod components;
pub mod game;
pub mod map;
pub mod snapshot;
pub mod systems;

pub use action::{default_commands, resolve_command_token, Action};
pub use components::Outcome;
pub use game::Game;
pub use snapshot::{FullSaveState, SavedEntity, Snapshot, StepReport};
pub use verryte_map::Point as Position;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{CharacterClass, GameState, Stats, Team, TurnPhase};
    use verryte_input::ActionSource;

    #[test]
    fn test_game_init() {
        let game = Game::new();
        assert_eq!(game.world.entity_count(), 4); // 3 player chars + 1 boss

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
        assert!(selected_before.is_some(), "Kael should be selected before save");

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

        // Check that all 4 entities are restored
        assert_eq!(game2.world.entity_count(), 4);
        
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
            !game.vfx.particles.is_empty(),
            "Particles should spawn on skill cast"
        );
        assert!(
            !game.vfx.shakes.is_empty(),
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

        // Now test Boss defeat and Echo drop. Set boss HP to 1.
        if let Some(stats) = game.world.get_mut::<Stats>(boss) {
            stats.hp = 1;
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
}
