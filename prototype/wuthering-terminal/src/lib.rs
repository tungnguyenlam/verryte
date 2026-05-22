//! Wuthering Terminal — tactical RPG prototype.

pub mod action;
pub mod components;
pub mod game;
pub mod map;
pub mod snapshot;
pub mod systems;

pub use action::Action;
pub use components::Outcome;
pub use game::Game;
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
    fn test_selection_and_movement() {
        let mut game = Game::new();

        // 1. Select the Warrior (Kael) at (4, 4)
        {
            let mut state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        let selected = game.world.resource::<GameState>().unwrap().selected_entity;
        assert!(selected.is_some(), "Kael should be selected");

        // 2. Move Kael to (4, 5)
        {
            let mut state = game.world.resource_mut::<GameState>().unwrap();
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
            let mut state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);

        // Attack Boss at (4, 5)
        {
            let mut state = game.world.resource_mut::<GameState>().unwrap();
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
            let mut state = game.world.resource_mut::<GameState>().unwrap();
            state.cursor = Position::new(4, 4);
        }
        game.apply_action(Action::Confirm, ActionSource::Terminal);
        {
            let mut state = game.world.resource_mut::<GameState>().unwrap();
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
}
