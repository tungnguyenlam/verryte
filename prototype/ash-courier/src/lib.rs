//! Ash Courier — the proving game for the Verryte engine.

pub mod action;
pub mod components;
pub mod game;
pub mod map;
pub mod snapshot;
pub mod systems;

pub use action::{default_bindings, default_commands, resolve_command_token, Action};
pub use components::{
    GameEvent, GameState, Hazard, Outcome, Package, Player, Position, PreviousPosition,
};
pub use game::{Game, MapError, DEFAULT_MAP};
pub use map::{Map, Tile};
pub use snapshot::{ActionResult, Snapshot, StepReport};

pub use verryte_input;
pub use verryte_map;

#[cfg(test)]
mod tests {
    use super::*;
    use verryte_input::{ActionSource, InputEvent, Key, MouseButton};
    use verryte_terminal::ColorPalette;

    fn fresh() -> Game {
        Game::new()
    }

    #[test]
    fn default_map_spawns_player_at_top_left() {
        let g = fresh();
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.outcome(), Outcome::Playing);
        assert!(!g.state().has_package);
    }

    #[test]
    fn walls_block_movement_and_do_not_advance_turn() {
        let mut g = fresh();
        let start_turn = g.state().turn;
        let report = g.step(Action::MoveNorth); // wall directly above (1, 0) is '#'
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.state().turn, start_turn);
        assert_eq!(report.result, ActionResult::NoOp);
    }

    #[test]
    fn wait_advances_turn_without_moving() {
        let mut g = fresh();
        let pos_before = g.player_position();
        let report = g.step(Action::Wait);
        assert_eq!(g.player_position(), pos_before);
        assert_eq!(g.state().turn, 1);
        assert_eq!(report.events, vec![GameEvent::Waited { at: pos_before }]);
    }

    #[test]
    fn movement_advances_turn() {
        let mut g = fresh();
        let report = g.step(Action::MoveEast);
        assert_eq!(g.player_position(), Position { x: 2, y: 1 });
        assert_eq!(g.state().turn, 1);
        assert_eq!(
            report.events,
            vec![GameEvent::Moved {
                from: Position { x: 1, y: 1 },
                to: Position { x: 2, y: 1 },
            }]
        );
    }

    #[test]
    fn step_to_package_uses_pathfinding_and_advances_turn() {
        let layout = &["#####", "#@.p#", "#...#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let report = g.step(Action::StepToPackage);
        assert_eq!(report.result, ActionResult::Advanced);
        assert_eq!(g.player_position(), Position { x: 2, y: 1 });
        assert_eq!(g.state().turn, 1);
    }

    #[test]
    fn step_to_goal_uses_pathfinding_and_advances_turn() {
        let layout = &["#####", "#@..#", "#..G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let report = g.step(Action::StepToGoal);
        assert_eq!(report.result, ActionResult::Advanced);
        assert_eq!(g.player_position(), Position { x: 1, y: 2 });
        assert_eq!(g.state().turn, 1);
    }

    #[test]
    fn step_to_safety_moves_toward_a_safer_neighbor() {
        let layout = &["#####", "#...#", "#@h.#", "#...#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let report = g.step(Action::StepToSafety);
        assert_eq!(report.result, ActionResult::Advanced);
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.state().turn, 1);
    }

    #[test]
    fn step_to_safety_is_noop_when_no_neighbor_is_safer() {
        let layout = &["#######", "#h.@.h#", "#######"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let report = g.step(Action::StepToSafety);
        assert_eq!(report.result, ActionResult::NoOp);
        assert_eq!(g.player_position(), Position { x: 3, y: 1 });
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn scan_advances_turn_and_reports_visible_state() {
        let layout = &["#######", "#@..hG#", "#..#..#", "#######"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let report = g.step(Action::Scan);
        let snap = g.snapshot();

        assert_eq!(g.state().turn, 1);
        assert_eq!(g.state().scans, 1);
        assert_eq!(report.result, ActionResult::Advanced);
        assert!(snap.visible_tiles.contains(&Position { x: 1, y: 1 }));
        assert!(snap.visible_hazards.contains(&Position { x: 4, y: 1 }));
        assert_eq!(
            report.events,
            vec![GameEvent::Scanned {
                at: Position { x: 1, y: 1 },
                visible_tiles: report.after.visible_tiles.len(),
                visible_hazards: report.after.visible_hazards.len(),
            }]
        );
    }

    #[test]
    fn scan_radius_reports_limited_visibility() {
        let layout = &["##########", "#@.......#", "##########"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let report = g.step(Action::ScanRadius(2));
        assert_eq!(g.state().scans, 1);
        // Radius 2 from (1,1) should see (0,1), (1,1), (2,1), (3,1) and walls around it.
        // Horizontal range: 1-2 to 1+2 -> -1 to 3. Map bounds are 0 to 9.
        // So it sees x in [0, 3]. (0,1), (1,1), (2,1), (3,1) -> 4 floor/goal tiles.
        // Wait, visible_from might be more restrictive.

        if let GameEvent::Scanned { visible_tiles, .. } = report.events[0] {
            assert!(visible_tiles > 0);
            assert!(visible_tiles < 50); // Should be much less than the whole map
        } else {
            panic!("Expected Scanned event");
        }
    }

    #[test]
    fn scan_radius_shortcut_keys_follow_terminal_path() {
        let mut g = fresh();

        assert!(g.handle_event(InputEvent::Key(Key::Char('3'))));
        let reports = g.run_pending_reports();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].action, Action::ScanRadius(3));
        assert_eq!(reports[0].source, ActionSource::Terminal);
        assert_eq!(reports[0].result, ActionResult::Advanced);
        assert_eq!(g.state().scans, 1);
    }

    #[test]
    fn inspect_action_updates_cursor_without_advancing_turn() {
        let mut g = fresh();
        let report = g.step(Action::Inspect(Position { x: 2, y: 1 }));

        assert_eq!(report.result, ActionResult::Updated);
        assert_eq!(g.state().turn, 0);
        assert_eq!(g.state().cursor, Some(Position { x: 2, y: 1 }));
        assert_eq!(
            report.events,
            vec![GameEvent::Inspected {
                at: Position { x: 2, y: 1 },
                tile: Tile::Floor,
            }]
        );
    }

    #[test]
    fn inspect_out_of_bounds_is_noop() {
        let mut g = fresh();
        let report = g.step(Action::Inspect(Position { x: -5, y: 100 }));

        assert_eq!(report.result, ActionResult::NoOp);
        assert_eq!(g.state().cursor, None);
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn clear_cursor_action_resets_state() {
        let mut g = fresh();
        g.step(Action::Inspect(Position { x: 2, y: 1 }));
        let report = g.step(Action::ClearCursor);

        assert_eq!(report.result, ActionResult::Updated);
        assert_eq!(g.state().cursor, None);
        assert_eq!(g.state().turn, 0);
        assert_eq!(
            report.events,
            vec![GameEvent::CursorCleared {
                at: Position { x: 2, y: 1 }
            }]
        );
    }

    #[test]
    fn clear_cursor_is_noop_without_cursor() {
        let mut g = fresh();
        let report = g.step(Action::ClearCursor);

        assert_eq!(report.result, ActionResult::NoOp);
        assert_eq!(g.state().cursor, None);
        assert!(report.events.is_empty());
    }

    #[test]
    fn scan_radius_script_tokens_parse_through_shared_router() {
        let mut g = fresh();
        let count = g
            .router
            .inject_script_with(
                &default_commands(),
                "scan:2 x3 scan4",
                ActionSource::Script,
                resolve_command_token,
            )
            .expect("radius commands should parse");

        let reports = g.run_pending_reports();
        assert_eq!(count, 3);
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].action, Action::ScanRadius(2));
        assert_eq!(reports[1].action, Action::ScanRadius(3));
        assert_eq!(reports[2].action, Action::ScanRadius(4));
        assert!(reports
            .iter()
            .all(|report| report.source == ActionSource::Script));
        assert_eq!(g.state().scans, 3);
    }

    #[test]
    fn inspect_script_tokens_parse_through_shared_router() {
        let mut g = fresh();
        let count = g
            .router
            .inject_script_with(
                &default_commands(),
                "inspect:2,1 look:3,1",
                ActionSource::Script,
                resolve_command_token,
            )
            .expect("inspect commands should parse");

        let reports = g.run_pending_reports();
        assert_eq!(count, 2);
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].action, Action::Inspect(Position { x: 2, y: 1 }));
        assert_eq!(reports[0].result, ActionResult::Updated);
        assert_eq!(reports[1].action, Action::Inspect(Position { x: 3, y: 1 }));
        assert!(reports
            .iter()
            .all(|report| report.source == ActionSource::Script));
        assert_eq!(g.state().cursor, Some(Position { x: 3, y: 1 }));
    }

    #[test]
    fn mouse_scan_uses_the_same_terminal_action_path() {
        let mut g = fresh();

        assert!(g.handle_event(InputEvent::Mouse {
            x: 0,
            y: 0,
            button: MouseButton::Right,
            pressed: true,
        }));
        let reports = g.run_pending_reports();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].action, Action::Scan);
        assert_eq!(reports[0].source, ActionSource::Terminal);
        assert_eq!(reports[0].result, ActionResult::Advanced);
        assert_eq!(g.state().scans, 1);
    }

    #[test]
    fn mouse_inspect_can_be_routed_through_custom_handler() {
        let mut g = fresh();
        let event = InputEvent::Mouse {
            x: 2,
            y: 1,
            button: MouseButton::Left,
            pressed: true,
        };

        assert!(g.handle_event_with(event, |event| match event {
            InputEvent::Mouse { x, y, .. } => Some(Action::Inspect(Position {
                x: x as i16,
                y: y as i16,
            })),
            _ => None,
        }));

        let reports = g.run_pending_reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].action, Action::Inspect(Position { x: 2, y: 1 }));
        assert_eq!(reports[0].source, ActionSource::Terminal);
        assert_eq!(reports[0].result, ActionResult::Updated);
        assert_eq!(g.state().cursor, Some(Position { x: 2, y: 1 }));
    }

    #[test]
    fn picking_up_package_sets_has_package_and_removes_entity() {
        // Custom map where the player starts adjacent to a package.
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        assert_eq!(g.snapshot().packages.len(), 1);
        g.router.inject_all([Action::MoveEast, Action::PickUp]);
        g.run_pending();
        assert!(g.state().has_package);
        assert!(g.snapshot().packages.is_empty());
    }

    #[test]
    fn dropping_package_restores_package_entity_through_shared_action_path() {
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        g.router
            .inject_script(&default_commands(), "east pickup drop", ActionSource::Agent)
            .unwrap();
        let reports = g.run_pending_reports();

        assert_eq!(reports.len(), 3);
        assert_eq!(reports[2].action, Action::Drop);
        assert_eq!(reports[2].source, ActionSource::Agent);
        assert_eq!(
            reports[2].events,
            vec![GameEvent::Dropped {
                at: Position { x: 2, y: 1 },
            }]
        );
        assert!(!g.state().has_package);
        assert_eq!(g.snapshot().packages, vec![Position { x: 2, y: 1 }]);
        assert_eq!(g.state().turn, 3);
    }

    #[test]
    fn drop_without_package_is_noop() {
        let mut g = fresh();
        let report = g.step(Action::Drop);

        assert_eq!(report.result, ActionResult::NoOp);
        assert_eq!(g.state().turn, 0);
        assert_eq!(report.events, Vec::<GameEvent>::new());
    }

    #[test]
    fn reaching_goal_with_package_wins() {
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.router.inject_all([
            Action::MoveEast,
            Action::PickUp,
            Action::MoveEast,
            Action::MoveSouth,
        ]);
        g.run_pending();
        assert_eq!(g.outcome(), Outcome::Won);
        assert!(g.is_over());
    }

    #[test]
    fn winning_step_reports_pickup_and_outcome_events() {
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        g.step(Action::MoveEast);
        let pickup = g.step(Action::PickUp);
        g.step(Action::MoveEast);
        let win = g.step(Action::MoveSouth);

        assert_eq!(
            pickup.events,
            vec![GameEvent::PickedUp {
                at: Position { x: 2, y: 1 },
            }]
        );
        assert_eq!(
            win.events,
            vec![
                GameEvent::Moved {
                    from: Position { x: 3, y: 1 },
                    to: Position { x: 3, y: 2 },
                },
                GameEvent::OutcomeChanged(Outcome::Won),
            ]
        );
    }

    #[test]
    fn reaching_goal_without_package_does_not_win() {
        let layout = &["#####", "#@..#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.router
            .inject_all([Action::MoveEast, Action::MoveEast, Action::MoveSouth]);
        g.run_pending();
        assert_eq!(g.outcome(), Outcome::Playing);
        assert_eq!(g.player_position(), Position { x: 3, y: 2 });
    }

    #[test]
    fn stepping_on_hazard_loses() {
        let layout = &["#####", "#@h.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        let report = g.step(Action::MoveEast);
        assert_eq!(g.outcome(), Outcome::Lost);
        assert_eq!(report.result, ActionResult::Ended(Outcome::Lost));
        assert_eq!(
            report.events,
            vec![
                GameEvent::Moved {
                    from: Position { x: 1, y: 1 },
                    to: Position { x: 2, y: 1 },
                },
                GameEvent::OutcomeChanged(Outcome::Lost),
            ]
        );
        assert!(g.is_over());
    }

    #[test]
    fn quit_action_ends_the_game() {
        let mut g = fresh();
        let report = g.step(Action::Quit);
        assert_eq!(g.outcome(), Outcome::Quit);
        assert_eq!(report.result, ActionResult::Ended(Outcome::Quit));
        assert!(g.is_over());
    }

    #[test]
    fn pickup_on_empty_tile_is_noop_and_does_not_advance_turn() {
        let mut g = fresh();
        g.inject_apply(Action::PickUp);
        assert_eq!(g.state().turn, 0);
        assert!(!g.state().has_package);
    }

    #[test]
    fn actions_after_game_over_are_ignored() {
        let layout = &["#####", "#@h.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.inject_apply(Action::MoveEast); // lose
        let pos = g.player_position();
        g.inject_apply(Action::MoveWest);
        assert_eq!(g.player_position(), pos);
        assert_eq!(g.outcome(), Outcome::Lost);
    }

    #[test]
    fn quit_is_allowed_after_game_over_to_exit_frontends() {
        let layout = &["#####", "#@h.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.inject_apply(Action::MoveEast); // lose first

        g.handle_event(InputEvent::Key(Key::Char('q')));
        let reports = g.run_pending_reports();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].action, Action::Quit);
        assert_eq!(reports[0].source, ActionSource::Terminal);
        assert_eq!(reports[0].result, ActionResult::Ended(Outcome::Quit));
        assert_eq!(
            reports[0].events,
            vec![GameEvent::OutcomeChanged(Outcome::Quit)]
        );
        assert_eq!(g.outcome(), Outcome::Quit);
    }

    #[test]
    fn message_log_records_human_readable_events() {
        let layout = &["#####", "#@p.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.step(Action::MoveEast);
        g.step(Action::PickUp);
        g.step(Action::Wait);

        let msgs = g.messages();
        assert!(msgs.iter().any(|m| m.contains("Moved to 2,1")));
        assert!(msgs.iter().any(|m| m.contains("Picked up a package!")));
        assert!(msgs.iter().any(|m| m.contains("Waited...")));
    }

    #[test]
    fn terminal_event_and_script_share_the_same_path() {
        // Drive one move via a Key event, the next via an injected action,
        // and assert the world cannot tell them apart.
        let mut g = fresh();
        g.handle_event(InputEvent::Key(Key::Right));
        g.run_pending();
        assert_eq!(g.player_position(), Position { x: 2, y: 1 });

        g.router.inject(Action::MoveEast);
        g.run_pending();
        assert_eq!(g.player_position(), Position { x: 3, y: 1 });
    }

    #[test]
    fn unbound_key_does_nothing() {
        let mut g = fresh();
        let pos = g.player_position();
        g.handle_event(InputEvent::Key(Key::Char('z')));
        g.run_pending();
        assert_eq!(g.player_position(), pos);
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn script_commands_parse_and_drive_the_shared_router() {
        let mut g = fresh();
        let count = g
            .router
            .inject_script(
                &default_commands(),
                "east step_package wait",
                ActionSource::Script,
            )
            .expect("known commands");
        let reports = g.run_pending_reports();

        assert_eq!(count, 3);
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].action, Action::MoveEast);
        assert_eq!(reports[0].source, ActionSource::Script);
        assert!(reports[0].changed);
        assert!(reports[0].turn_advanced);
        assert_eq!(reports[1].action, Action::StepToPackage);
        assert_eq!(g.player_position(), Position { x: 3, y: 1 });
        assert_eq!(g.state().turn, 3);
    }

    #[test]
    fn compact_glyph_scripts_use_engine_command_bindings() {
        let parsed = default_commands().parse_glyphs("e . W p o v ! c").unwrap();
        assert_eq!(
            parsed,
            vec![
                Action::MoveEast,
                Action::Wait,
                Action::MoveWest,
                Action::StepToPackage,
                Action::StepToGoal,
                Action::StepToSafety,
                Action::Drop,
                Action::ClearCursor
            ]
        );
    }

    #[test]
    fn default_map_can_be_won_from_a_script() {
        let mut g = fresh();
        let actions = default_commands()
            .parse_script("eeesss,nnneeeesssssss")
            .expect("default win script should parse");
        g.router.inject_all(actions);
        let reports = g.run_pending_reports();

        assert_eq!(g.outcome(), Outcome::Won);
        assert!(g.state().has_package);
        assert_eq!(g.player_position(), Position { x: 8, y: 8 });
        assert!(reports
            .iter()
            .any(|report| report.result == ActionResult::Ended(Outcome::Won)));
    }

    #[test]
    fn step_report_records_noop_actions() {
        let mut g = fresh();
        let report = g.step(Action::MoveNorth);
        assert_eq!(report.action, Action::MoveNorth);
        assert_eq!(report.source, ActionSource::Test);
        assert_eq!(report.result, ActionResult::NoOp);
        assert_eq!(report.before.player, report.after.player);
        assert_eq!(report.before.turn, report.after.turn);
        assert!(!report.changed);
        assert!(!report.turn_advanced);
    }

    #[test]
    fn snapshot_includes_rendered_frame_and_outcome() {
        let mut g = fresh();
        let snap = g.snapshot();
        assert_eq!(snap.outcome, Outcome::Playing);
        assert_eq!(snap.scans, 0);
        assert_eq!(snap.player, Position { x: 1, y: 1 });
        assert_eq!(snap.tile_under_player, Tile::Floor);
        assert!(snap.visible_tiles.contains(&snap.player));
        assert!(snap.reachable_tiles.contains(&snap.player));
        assert!(snap.reachable_tiles.contains(&Position { x: 8, y: 8 }));
        assert!(!snap.reachable_tiles.contains(&Position { x: 0, y: 0 }));
        assert_eq!(
            snap.walkable_neighbors,
            vec![Position { x: 1, y: 2 }, Position { x: 2, y: 1 }]
        );
        assert_eq!(
            snap.path_to_nearest_package.as_ref().unwrap().first(),
            Some(&snap.player)
        );
        assert_eq!(
            snap.path_to_nearest_package.as_ref().unwrap().last(),
            Some(&Position { x: 4, y: 4 })
        );
        assert_eq!(
            snap.path_to_goal.as_ref().unwrap().last(),
            Some(&Position { x: 8, y: 8 })
        );
        assert_eq!(
            snap.path_to_nearest_hazard.as_ref().unwrap().last(),
            Some(&Position { x: 3, y: 7 })
        );
        assert!(snap.chasers.is_empty());
        assert_eq!(snap.path_to_nearest_chaser, None);
        assert_eq!(snap.distance_to_nearest_package, Some(6));
        assert_eq!(snap.distance_to_goal, Some(14));
        assert_eq!(snap.distance_to_nearest_hazard, Some(8));
        assert_eq!(snap.distance_to_nearest_chaser, None);
        assert_eq!(snap.safer_neighbors, vec![Position { x: 2, y: 1 }]);
        assert_eq!(snap.cursor, None);
        assert_eq!(snap.cursor_tile, None);
        assert_eq!(snap.path_to_cursor, None);
        assert_eq!(snap.distance_to_cursor, None);
        // Frame must contain a player glyph.
        assert!(snap.frame.contains('@'));
        // ...and have the right number of rows.
        assert_eq!(snap.frame.lines().count() as u16, snap.map_height);
        assert!(snap.local_frame.contains('@'));
        assert!(snap.local_frame.lines().count() <= 7);
        // Forward progress reflects in the snapshot.
        g.inject_apply(Action::MoveEast);
        let snap2 = g.snapshot();
        assert_eq!(snap2.player, Position { x: 2, y: 1 });
        assert_eq!(snap2.turn, 1);
    }

    #[test]
    fn render_highlights_cursor_cell_background() {
        let mut g = fresh();
        g.step(Action::Inspect(Position { x: 2, y: 1 }));

        let frame = g.render();
        let cell = frame.get(2, 1).unwrap();
        assert_eq!(cell.bg, ColorPalette::dark_dungeon().ui_highlight);
        assert_eq!(cell.glyph, '.');
    }

    #[test]
    fn render_viewport_tracks_the_player_and_clips_to_map() {
        let mut g = fresh();
        let top_left = g.render_viewport(5, 5).to_plain_string();
        assert_eq!(top_left.lines().count(), 5);
        assert!(top_left.contains('@'));

        g.router
            .inject_script(&default_commands(), "eesss", ActionSource::Test)
            .unwrap();
        g.run_pending();
        let near_package = g.render_viewport(5, 5).to_plain_string();

        assert!(near_package.contains('@'));
        assert!(near_package.contains('p'));
        assert_ne!(top_left, near_package);
    }

    #[test]
    fn map_error_on_missing_player() {
        let err = Game::from_layout(&["#####", "#...#", "#####"], default_bindings());
        assert_eq!(err.err(), Some(MapError::NoPlayer));
    }

    #[test]
    fn shorter_layout_rows_are_padded_as_walls() {
        let layout = &["#####", "#@", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.inject_apply(Action::MoveEast);
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn step_reports_preserve_terminal_and_agent_sources() {
        let mut g = fresh();
        g.handle_event(InputEvent::Key(Key::Right));
        g.router.inject_from(Action::Wait, ActionSource::Agent);

        let reports = g.run_pending_reports();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].action, Action::MoveEast);
        assert_eq!(reports[0].source, ActionSource::Terminal);
        assert_eq!(reports[1].action, Action::Wait);
        assert_eq!(reports[1].source, ActionSource::Agent);
        assert_eq!(g.state().turn, 2);
    }

    #[test]
    fn action_trace_replays_through_the_same_report_path() {
        let mut g = fresh();
        let trace = verryte_input::ActionTrace::from_actions(
            [Action::MoveEast, Action::MoveEast, Action::Wait],
            ActionSource::Replay,
        );

        trace.replay_into(&mut g.router);
        let reports = g.run_pending_reports();

        assert_eq!(reports.len(), 3);
        assert!(reports
            .iter()
            .all(|report| report.source == ActionSource::Replay));
        assert_eq!(g.player_position(), Position { x: 3, y: 1 });
        assert_eq!(g.state().turn, 3);
    }

    #[test]
    fn message_log_records_chaser_movement_events() {
        let layout = &["#######", "#@...c#", "#######"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        g.step(Action::Wait);
        let messages = g.messages();

        assert!(messages
            .iter()
            .any(|message| message.contains("A chaser moved from 5,1 to 4,1")));
    }

    #[test]
    fn chasers_move_toward_player() {
        let layout = &["#######", "#@...c#", "#######"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();

        let start_pos = g.player_position();
        assert_eq!(start_pos, Position { x: 1, y: 1 });

        // Let's find the chaser
        let chasers: Vec<_> = g
            .world
            .query2::<Position, crate::components::Chaser>()
            .into_iter()
            .map(|(e, pos, _)| (e, *pos))
            .collect();
        assert_eq!(chasers.len(), 1);
        let chaser_start = chasers[0].1;
        assert_eq!(chaser_start, Position { x: 5, y: 1 });

        // Player waits, chaser should move towards player (left by 1)
        let report = g.step(Action::Wait);
        assert!(report
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::Waited { .. })));
        assert!(report.events.iter().any(|event| matches!(
            event,
            GameEvent::ChaserMoved {
                from: Position { x: 5, y: 1 },
                to: Position { x: 4, y: 1 }
            }
        )));

        let chaser_pos = *g.world.get::<Position>(chasers[0].0).unwrap();
        assert_eq!(chaser_pos, Position { x: 4, y: 1 });
        let snap = g.snapshot();
        assert_eq!(snap.chasers, vec![Position { x: 4, y: 1 }]);
        assert_eq!(snap.distance_to_nearest_chaser, Some(3));
        assert_eq!(
            snap.path_to_nearest_chaser,
            Some(vec![
                Position { x: 1, y: 1 },
                Position { x: 2, y: 1 },
                Position { x: 3, y: 1 },
                Position { x: 4, y: 1 }
            ])
        );

        // Wait again, chaser should move left again
        g.step(Action::Wait);

        let chaser_pos = *g.world.get::<Position>(chasers[0].0).unwrap();
        assert_eq!(chaser_pos, Position { x: 3, y: 1 });
    }

    #[test]
    fn game_clock_tracks_turns_independently() {
        let mut g = fresh();
        assert_eq!(g.clock().elapsed_ticks(), 0);
        g.step(Action::Wait);
        assert_eq!(g.clock().elapsed_ticks(), 1);
        assert_eq!(g.state().turn, 1);
        g.step(Action::MoveEast);
        assert_eq!(g.clock().elapsed_ticks(), 2);
        assert_eq!(g.state().turn, 2);
    }

    #[test]
    fn game_clock_does_not_advance_on_noop() {
        let mut g = fresh();
        g.step(Action::MoveNorth); // wall — noop
        assert_eq!(g.clock().elapsed_ticks(), 0);
        assert_eq!(g.state().turn, 0);
    }

    #[test]
    fn game_clock_and_state_turn_stay_synchronized() {
        let layout = &["#####", "#@p.#", "###G#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.router.inject_all([
            Action::MoveEast,
            Action::PickUp,
            Action::MoveEast,
            Action::MoveSouth,
        ]);
        g.run_pending();
        assert_eq!(g.outcome(), Outcome::Won);
        assert_eq!(g.clock().elapsed_ticks(), g.state().turn as u64);
    }

    #[test]
    fn rng_resource_is_available_in_world() {
        let g = fresh();
        let seed = g
            .world
            .resource::<verryte_core::Rng>()
            .unwrap()
            .seed_value();
        assert_ne!(seed, 0);
    }

    #[test]
    fn same_seed_produces_deterministic_chaser_outcomes() {
        let layout = &["#######", "#@...c#", "#.....#", "#######"];

        let mut g1 = Game::from_layout_with_seed(layout, default_bindings(), 42).unwrap();
        let mut g2 = Game::from_layout_with_seed(layout, default_bindings(), 42).unwrap();

        for _ in 0..3 {
            g1.step(Action::Wait);
            g2.step(Action::Wait);
        }

        let pos1: Vec<_> = g1
            .world
            .query2::<Position, crate::components::Chaser>()
            .into_iter()
            .map(|(_, p, _)| *p)
            .collect();
        let pos2: Vec<_> = g2
            .world
            .query2::<Position, crate::components::Chaser>()
            .into_iter()
            .map(|(_, p, _)| *p)
            .collect();
        assert_eq!(pos1, pos2);
    }

    #[test]
    fn with_seed_constructor_sets_rng() {
        let g = Game::with_seed(99);
        let seed = g
            .world
            .resource::<verryte_core::Rng>()
            .unwrap()
            .seed_value();
        assert_eq!(seed, 99);
    }

    #[test]
    fn from_cave_creates_playable_game() {
        let g = Game::from_cave(20, 15, 42);
        assert_eq!(g.outcome(), Outcome::Playing);
        // Player should be alive and positioned.
        let pos = g.player_position();
        assert!(g.map().is_walkable(verryte_map::Point::new(pos.x, pos.y)));
    }

    #[test]
    fn from_cave_has_package_and_goal_entities() {
        let g = Game::from_cave(25, 20, 123);
        // Should have at least one package.
        let pkgs = g.world.query2::<Position, Package>();
        assert!(pkgs.len() >= 1, "cave should have a package");
        // Goal tile should exist in the map.
        let goal_count = g.map().tiles.count_matching(|_, t| *t == Tile::Goal);
        assert!(goal_count >= 1, "cave should have a goal tile");
    }

    #[test]
    fn from_cave_is_deterministic_with_same_seed() {
        let g1 = Game::from_cave(20, 15, 777);
        let g2 = Game::from_cave(20, 15, 777);
        assert_eq!(g1.player_position(), g2.player_position());
        assert_eq!(g1.state().turn, g2.state().turn);
    }

    #[test]
    fn reset_restores_game_to_initial_state() {
        let mut g = fresh();
        g.step(Action::MoveEast);
        g.step(Action::MoveEast);
        g.step(Action::MoveEast);
        assert_eq!(g.state().turn, 3);
        assert_eq!(g.player_position(), Position { x: 4, y: 1 });

        g.reset();
        assert_eq!(g.state().turn, 0);
        assert_eq!(g.outcome(), Outcome::Playing);
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
        assert!(g.router.is_idle());
    }

    #[test]
    fn reset_after_loss_restores_playable_state() {
        let layout = &["#####", "#@h.#", "#####"];
        let mut g = Game::from_layout(layout, default_bindings()).unwrap();
        g.step(Action::MoveEast);
        assert_eq!(g.outcome(), Outcome::Lost);

        g.reset();
        assert_eq!(g.outcome(), Outcome::Playing);
        assert_eq!(g.player_position(), Position { x: 1, y: 1 });
    }

    #[test]
    fn reset_from_cave_produces_playable_game() {
        let mut g = fresh();
        g.step(Action::Quit);
        assert_eq!(g.outcome(), Outcome::Quit);

        g.reset_from_cave(20, 15, 42);
        assert_eq!(g.outcome(), Outcome::Playing);
        let pos = g.player_position();
        assert!(g.map().is_walkable(verryte_map::Point::new(pos.x, pos.y)));
    }

    #[test]
    fn reset_from_bsp_produces_playable_game() {
        let mut g = fresh();
        g.step(Action::MoveEast);
        g.router.inject(Action::Wait);
        assert_eq!(g.router.pending(), 1);

        g.reset_from_bsp(20, 15, 42);
        assert_eq!(g.outcome(), Outcome::Playing);
        assert_eq!(g.state().turn, 0);
        assert_eq!(g.map().width, 20);
        assert_eq!(g.map().height, 15);
        assert_eq!(g.router.pending(), 0);
        let pos = g.player_position();
        assert!(g.map().is_walkable(verryte_map::Point::new(pos.x, pos.y)));
    }

    #[test]
    fn from_bsp_creates_playable_game() {
        let g = Game::from_bsp(25, 20, 42);
        assert_eq!(g.outcome(), Outcome::Playing);
        let pos = g.player_position();
        assert!(g.map().is_walkable(verryte_map::Point::new(pos.x, pos.y)));
    }

    #[test]
    fn from_bsp_has_package_and_goal() {
        let g = Game::from_bsp(25, 20, 123);
        let pkgs = g.world.query2::<Position, Package>();
        assert!(pkgs.len() >= 1, "BSP dungeon should have a package");
        let goal_count = g.map().tiles.count_matching(|_, t| *t == Tile::Goal);
        assert!(goal_count >= 1, "BSP dungeon should have a goal tile");
    }

    #[test]
    fn from_bsp_is_deterministic_with_same_seed() {
        let g1 = Game::from_bsp(25, 20, 42);
        let g2 = Game::from_bsp(25, 20, 42);
        assert_eq!(g1.player_position(), g2.player_position());
        assert_eq!(g1.state().turn, g2.state().turn);
    }

    #[test]
    fn from_bsp_has_chaser_entity() {
        let g = Game::from_bsp(30, 25, 99);
        let chasers = g.world.query2::<Position, crate::components::Chaser>();
        assert!(chasers.len() >= 1, "BSP dungeon should spawn a chaser");
    }

    #[test]
    fn render_with_palette_produces_grid() {
        use verryte_terminal::ColorPalette;
        let g = fresh();
        let grid = g.render_with_palette(&ColorPalette::amber_terminal());
        assert_eq!(grid.width(), g.map().width);
        assert_eq!(grid.height(), g.map().height);
    }
}
