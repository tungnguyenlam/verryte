//! Terminal-agnostic input model.
//!
//! `verryte-input` exists to enforce the most important shape in the engine:
//!
//! ```text
//! terminal event -> game action -> game system -> observable state
//! script command -> game action -> game system -> observable state
//! ```
//!
//! Both halves go through the same [`InputRouter`]:
//!
//! * Interactive frontends translate their native key/mouse events into the
//!   neutral [`InputEvent`] type and feed them through [`InputRouter::handle`].
//!   Simple key and mouse-button bindings can queue actions directly.
//! * Scripts, tests, and agents push fully-formed actions through
//!   [`InputRouter::inject`].
//!
//! Downstream, the game just drains the action queue. It cannot tell — and
//! does not need to tell — whether an action came from a keypress or a script.
//! If a harness wants that information for logs, replays, or debugging, it can
//! drain [`QueuedAction`] values and read their [`ActionSource`].
//!
//! The router is generic over the game's action enum, so games define their
//! own action vocabulary without giving up the shared dispatch path.
//!
//! # Example
//!
//! ```rust
//! use verryte_input::{Bindings, InputRouter, Key, InputEvent, KeyEventKind};
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Action {
//!     MoveUp,
//!     MoveDown,
//!     Quit,
//! }
//!
//! let mut bindings = Bindings::new();
//! bindings.bind(Key::Char('w'), Action::MoveUp);
//! bindings.bind(Key::Char('s'), Action::MoveDown);
//! bindings.bind(Key::Esc, Action::Quit);
//!
//! let mut router = InputRouter::new(bindings);
//!
//! // Simulate a key press
//! router.handle(InputEvent::Key { key: Key::Char('w'), kind: KeyEventKind::Press });
//!
//! // Game loop consumes the actions
//! let mut actions: Vec<_> = router.drain().collect();
//! assert_eq!(actions, vec![Action::MoveUp]);
//! ```

mod action;
mod bindings;
#[cfg(test)]
mod bindings_ext_tests;
mod key;
mod replay;
mod router;
#[cfg(test)]
mod router_ext_tests;
mod text_input;
mod trace;

pub use action::*;
pub use bindings::*;
pub use key::*;
pub use replay::*;
pub use router::*;
pub use text_input::*;
pub use trace::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Move {
        North,
        South,
        East,
        West,
        Wait,
        Scan(u16),
    }

    fn bound_router() -> InputRouter<Move> {
        let mut bindings = Bindings::new();
        bindings.bind(Key::Up, Move::North);
        bindings.bind(Key::Down, Move::South);
        bindings.bind(Key::Left, Move::West);
        bindings.bind(Key::Right, Move::East);
        bindings.bind(Key::Char('.'), Move::Wait);
        InputRouter::new(bindings)
    }

    #[test]
    fn test_key_repeat_config_and_ticking() {
        let mut router = bound_router();
        router.set_repeat_config(RepeatConfig {
            delay: 0.1,
            interval: 0.05,
        });

        // Key Press
        router.handle_from(
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            ActionSource::Terminal,
        );

        // First action is immediately queued
        assert_eq!(router.total_actions_queued(), 1);
        assert_eq!(router.next_action(), Some(Move::North));

        // Advance by 0.05s (less than delay) -> no extra repeat
        router.tick(0.05);
        assert_eq!(router.total_actions_queued(), 1); // no action added yet

        // Advance by another 0.06s (total 0.11s, >= delay 0.1s, interval >= 0.05s) -> 1 action repeated
        router.tick(0.06);
        assert_eq!(router.total_actions_queued(), 2);
        assert_eq!(router.next_action(), Some(Move::North));

        // Advance by another 0.06s -> another action repeated
        router.tick(0.06);
        assert_eq!(router.total_actions_queued(), 3);
        assert_eq!(router.next_action(), Some(Move::North));

        // Release Key
        router.handle_from(
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Release,
            },
            ActionSource::Terminal,
        );

        // Advance by 0.1s -> no actions queued since key was released
        router.tick(0.1);
        assert_eq!(router.total_actions_queued(), 3);
    }

    #[test]
    fn test_router_history_tracking() {
        let mut router = bound_router();
        assert!(router.history().is_empty());

        router.inject_from(Move::North, ActionSource::Terminal);
        router.inject_from(Move::Wait, ActionSource::Script);
        assert!(router.history().is_empty()); // not popped yet

        let a1 = router.next_queued().unwrap();
        assert_eq!(a1.action, Move::North);
        assert_eq!(router.history().len(), 1);
        assert_eq!(router.history()[0].action, Move::North);

        let a2 = router.next_queued().unwrap();
        assert_eq!(a2.action, Move::Wait);
        assert_eq!(router.history().len(), 2);
        assert_eq!(router.history()[1].action, Move::Wait);

        router.clear_history();
        assert!(router.history().is_empty());
    }

    #[test]
    fn action_trace_serialization_round_trip() {
        let mut trace = ActionTrace::new();
        trace.push(Move::North, ActionSource::Terminal);
        trace.push(Move::Wait, ActionSource::Script);
        trace.push(Move::Scan(3), ActionSource::Agent);

        let format_move = |m: &Move| match m {
            Move::North => "north".to_owned(),
            Move::South => "south".to_owned(),
            Move::East => "east".to_owned(),
            Move::West => "west".to_owned(),
            Move::Wait => "wait".to_owned(),
            Move::Scan(r) => format!("scan:{}", r),
        };

        let parse_move = |s: &str| match s {
            "north" => Some(Move::North),
            "south" => Some(Move::South),
            "east" => Some(Move::East),
            "west" => Some(Move::West),
            "wait" => Some(Move::Wait),
            other => {
                if let Some(r_str) = other.strip_prefix("scan:") {
                    r_str.parse::<u16>().ok().map(Move::Scan)
                } else {
                    None
                }
            }
        };

        let serialized = trace.to_detailed_string(format_move);
        assert_eq!(serialized, "Terminal:north\nScript:wait\nAgent:scan:3\n");

        let deserialized = ActionTrace::from_detailed_string(&serialized, parse_move).unwrap();
        assert_eq!(deserialized, trace);

        // Check comments and blank lines are ignored
        let comment_str = "# this is a comment\n\nTerminal:north\n  # inner comment\nScript:wait\n";
        let parsed_comments = ActionTrace::from_detailed_string(comment_str, parse_move).unwrap();
        assert_eq!(parsed_comments.len(), 2);
        assert_eq!(parsed_comments.into_steps()[0].action, Move::North);
    }

    #[test]
    fn action_trace_file_round_trip() {
        let mut trace = ActionTrace::new();
        trace.push(Move::South, ActionSource::Terminal);
        trace.push(Move::Wait, ActionSource::Script);

        let format_move = |m: &Move| match m {
            Move::North => "north".to_owned(),
            Move::South => "south".to_owned(),
            Move::East => "east".to_owned(),
            Move::West => "west".to_owned(),
            Move::Wait => "wait".to_owned(),
            Move::Scan(r) => format!("scan:{}", r),
        };

        let parse_move = |s: &str| match s {
            "north" => Some(Move::North),
            "south" => Some(Move::South),
            "east" => Some(Move::East),
            "west" => Some(Move::West),
            "wait" => Some(Move::Wait),
            other => {
                if let Some(r_str) = other.strip_prefix("scan:") {
                    r_str.parse::<u16>().ok().map(Move::Scan)
                } else {
                    None
                }
            }
        };

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_action_trace.txt");

        trace.save_to_file(&path, format_move).unwrap();
        let loaded = ActionTrace::<Move>::load_from_file(&path, parse_move).unwrap();
        assert_eq!(loaded, trace);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn key_event_translates_to_action() {
        let mut router = bound_router();
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
        assert!(router.is_idle());
    }

    #[test]
    fn unbound_key_is_dropped() {
        let mut router = bound_router();
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('z'),
            kind: KeyEventKind::Press
        }));
        assert!(router.is_idle());
    }

    #[test]
    fn non_key_events_are_ignored_by_default() {
        let mut router = bound_router();
        assert!(!router.handle(InputEvent::Tick));
        assert!(!router.handle(InputEvent::Resize {
            width: 80,
            height: 24
        }));
        assert!(!router.handle(InputEvent::MouseScroll {
            x: 1,
            y: 1,
            direction: ScrollDirection::Up,
        }));
        assert!(router.is_idle());
    }

    #[test]
    fn is_empty_checks_both_bindings_and_pending() {
        let router: InputRouter<Move> = InputRouter::new(Bindings::new());
        assert!(router.is_empty());

        let mut router = bound_router();
        assert!(!router.is_empty()); // has bindings

        router.bindings_mut().unbind(Key::Up);
        router.bindings_mut().unbind(Key::Down);
        router.bindings_mut().unbind(Key::Left);
        router.bindings_mut().unbind(Key::Right);
        router.bindings_mut().unbind(Key::Char('.'));
        assert!(router.is_empty()); // no bindings, no pending

        router.inject(Move::Wait);
        assert!(!router.is_empty()); // has pending
    }

    #[test]
    fn mouse_bindings_enter_the_same_action_queue() {
        let mut router = bound_router();
        router
            .bindings_mut()
            .bind_mouse(MouseButton::Right, true, Move::Wait);

        assert!(router.handle(InputEvent::Mouse {
            x: 12,
            y: 4,
            button: MouseButton::Right,
            pressed: true,
        }));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Wait, ActionSource::Terminal))
        );
    }

    #[test]
    fn scroll_bindings_enter_the_same_action_queue() {
        let mut router = bound_router();
        router
            .bindings_mut()
            .bind_scroll(ScrollDirection::Down, Move::Scan(1));

        assert!(router.handle(InputEvent::MouseScroll {
            x: 2,
            y: 3,
            direction: ScrollDirection::Down,
        }));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Scan(1), ActionSource::Terminal))
        );
    }

    #[test]
    fn handle_with_prefers_custom_translation() {
        let mut router = bound_router();
        let handled = router.handle_with(
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            |_| Some(Move::Scan(2)),
        );
        assert!(handled);
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::Scan(2), ActionSource::Terminal))
        );
    }

    #[test]
    fn handle_with_falls_back_to_bindings() {
        let mut router = bound_router();
        let handled = router.handle_with(
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press,
            },
            |_| None,
        );
        assert!(handled);
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::East, ActionSource::Terminal))
        );
    }

    #[test]
    fn input_events_can_be_queued_with_explicit_source() {
        let mut router = bound_router();
        assert!(router.handle_from(
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press
            },
            ActionSource::Replay
        ));
        assert_eq!(
            router.next_queued(),
            Some(QueuedAction::new(Move::East, ActionSource::Replay))
        );
    }

    #[test]
    fn injected_actions_share_queue_with_translated_events() {
        let mut router = bound_router();
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        router.inject(Move::Wait);
        router.handle(InputEvent::Key {
            key: Key::Right,
            kind: KeyEventKind::Press,
        });
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::Wait, Move::East]);
    }

    #[test]
    fn queued_actions_track_source_without_changing_order() {
        let mut router = bound_router();
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        router.inject_from(Move::Wait, ActionSource::Agent);
        router.inject_all_from([Move::East, Move::South], ActionSource::Replay);

        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(
            queued,
            vec![
                QueuedAction::new(Move::North, ActionSource::Terminal),
                QueuedAction::new(Move::Wait, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Replay),
                QueuedAction::new(Move::South, ActionSource::Replay),
            ]
        );
    }

    #[test]
    fn drain_trace_preserves_sources_and_clears_queue() {
        let mut router = bound_router();
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        router.inject_from(Move::Wait, ActionSource::Agent);
        router.handle_from(
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press,
            },
            ActionSource::Replay,
        );

        let trace = router.drain_trace();
        assert!(router.is_idle());

        let steps = trace.into_steps();
        assert_eq!(
            steps,
            vec![
                QueuedAction::new(Move::North, ActionSource::Terminal),
                QueuedAction::new(Move::Wait, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Replay),
            ]
        );
    }

    #[test]
    fn inject_all_preserves_order() {
        let mut router = bound_router();
        router.inject_all([Move::North, Move::North, Move::East]);
        assert_eq!(router.pending(), 3);
        assert_eq!(router.next_action(), Some(Move::North));
        assert_eq!(router.next_action(), Some(Move::North));
        assert_eq!(router.next_action(), Some(Move::East));
        assert_eq!(router.next_action(), None);
    }

    #[test]
    fn rebinding_replaces_action() {
        let mut router = bound_router();
        let prev = router.bindings_mut().bind(Key::Up, Move::Wait);
        assert_eq!(prev, Some(Move::North));
        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        assert_eq!(router.next_action(), Some(Move::Wait));
    }

    #[test]
    fn command_words_parse_to_actions() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_name("wait", Move::Wait);

        let parsed = commands.parse_words("north wait north").unwrap();
        assert_eq!(parsed, vec![Move::North, Move::Wait, Move::North]);
    }

    #[test]
    fn command_glyphs_parse_to_actions_and_ignore_whitespace() {
        let mut commands = CommandBindings::new();
        commands.bind_glyph('n', Move::North);
        commands.bind_glyph('.', Move::Wait);

        let parsed = commands.parse_glyphs("n . n").unwrap();
        assert_eq!(parsed, vec![Move::North, Move::Wait, Move::North]);
    }

    #[test]
    fn mixed_scripts_parse_words_and_glyph_runs() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_name("wait", Move::Wait);
        commands.bind_glyph('e', Move::East);
        commands.bind_glyph('w', Move::West);
        commands.bind_glyph('.', Move::Wait);

        let parsed = commands.parse_script("north ew wait .").unwrap();
        assert_eq!(
            parsed,
            vec![Move::North, Move::East, Move::West, Move::Wait, Move::Wait]
        );
    }

    #[test]
    fn mixed_scripts_accept_commas_semicolons_and_comments() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_glyph('e', Move::East);
        commands.bind_glyph('.', Move::Wait);

        let parsed = commands
            .parse_script("north,e.; # stop parsing this line\ne")
            .unwrap();
        assert_eq!(
            parsed,
            vec![Move::North, Move::East, Move::Wait, Move::East]
        );
    }

    #[test]
    fn mixed_scripts_can_use_custom_token_resolver() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_glyph('e', Move::East);

        let parsed = commands
            .parse_script_with("north scan:3 ee x2", |token| {
                token
                    .strip_prefix("scan:")
                    .or_else(|| token.strip_prefix('x'))
                    .and_then(|digits| digits.parse::<u16>().ok())
                    .map(Move::Scan)
            })
            .unwrap();
        assert_eq!(
            parsed,
            vec![
                Move::North,
                Move::Scan(3),
                Move::East,
                Move::East,
                Move::Scan(2)
            ]
        );
    }

    #[test]
    fn command_parse_errors_identify_unknown_input() {
        let commands = CommandBindings::<Move>::new();
        assert_eq!(
            commands.parse_words("north").unwrap_err(),
            CommandParseError::UnknownCommand("north".to_owned())
        );
        assert_eq!(
            commands.parse_glyphs("x").unwrap_err(),
            CommandParseError::UnknownGlyph {
                glyph: 'x',
                index: 0
            }
        );
    }

    #[test]
    fn router_can_parse_and_enqueue_script_actions_with_source() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);
        commands.bind_glyph('e', Move::East);

        let mut router = bound_router();
        let count = router
            .inject_script(&commands, "north ee", ActionSource::Agent)
            .unwrap();

        assert_eq!(count, 3);
        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn router_can_enqueue_script_actions_with_custom_token_resolver() {
        let mut commands = CommandBindings::new();
        commands.bind_name("north", Move::North);

        let mut router = bound_router();
        let count = router
            .inject_script_with(&commands, "north scan:4", ActionSource::Agent, |token| {
                token
                    .strip_prefix("scan:")
                    .and_then(|digits| digits.parse::<u16>().ok())
                    .map(Move::Scan)
            })
            .unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Agent),
                QueuedAction::new(Move::Scan(4), ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn action_trace_replays_sourced_actions_through_router() {
        let mut trace = ActionTrace::new();
        trace.push(Move::North, ActionSource::Terminal);
        trace.push(Move::Wait, ActionSource::Agent);
        assert_eq!(trace.len(), 2);

        let mut router = bound_router();
        replay_trace(&trace, &mut router);

        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Terminal),
                QueuedAction::new(Move::Wait, ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn pending_trace_snapshots_queue_without_draining() {
        let mut router = bound_router();
        router.inject_from(Move::North, ActionSource::Agent);
        router.handle(InputEvent::Key {
            key: Key::Right,
            kind: KeyEventKind::Press,
        });

        let trace = router.pending_trace();
        assert_eq!(router.pending(), 2);
        assert_eq!(
            trace.into_steps(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Agent),
                QueuedAction::new(Move::East, ActionSource::Terminal),
            ]
        );
    }

    #[test]
    fn action_trace_can_be_built_from_unsourced_action_runs() {
        let trace = ActionTrace::from_actions([Move::East, Move::East], ActionSource::Replay);

        assert_eq!(
            trace.into_steps(),
            vec![
                QueuedAction::new(Move::East, ActionSource::Replay),
                QueuedAction::new(Move::East, ActionSource::Replay),
            ]
        );
    }

    #[test]
    fn action_trace_can_extend() {
        let mut trace1 = ActionTrace::from_actions([Move::North], ActionSource::Test);
        let trace2 = ActionTrace::from_actions([Move::South], ActionSource::Agent);
        trace1.extend(trace2);
        assert_eq!(
            trace1.into_steps(),
            vec![
                QueuedAction::new(Move::North, ActionSource::Test),
                QueuedAction::new(Move::South, ActionSource::Agent),
            ]
        );
    }

    #[test]
    fn handle_batch_queues_multiple_events() {
        let mut router = bound_router();
        let count = router.handle_batch([
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            InputEvent::Key {
                key: Key::Right,
                kind: KeyEventKind::Press,
            },
            InputEvent::Key {
                key: Key::Down,
                kind: KeyEventKind::Press,
            },
        ]);
        assert_eq!(count, 3);
        assert_eq!(router.pending(), 3);
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::East, Move::South]);
    }

    #[test]
    fn handle_batch_skips_unbound_events() {
        let mut router = bound_router();
        let count = router.handle_batch([
            InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press,
            },
            InputEvent::Key {
                key: Key::Char('z'),
                kind: KeyEventKind::Press,
            },
            InputEvent::Tick,
            InputEvent::Key {
                key: Key::Left,
                kind: KeyEventKind::Press,
            },
        ]);
        assert_eq!(count, 2);
        assert_eq!(router.pending(), 2);
    }

    #[test]
    fn handle_batch_from_preserves_source() {
        let mut router = bound_router();
        router.handle_batch_from(
            [
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                },
                InputEvent::Key {
                    key: Key::Right,
                    kind: KeyEventKind::Press,
                },
            ],
            ActionSource::Agent,
        );
        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(queued.len(), 2);
        assert!(queued.iter().all(|q| q.source == ActionSource::Agent));
    }

    #[test]
    fn handle_batch_with_prefers_custom_translation() {
        let mut router = bound_router();
        let count = router.handle_batch_with(
            [
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                },
                InputEvent::Key {
                    key: Key::Right,
                    kind: KeyEventKind::Press,
                },
            ],
            |event| match event {
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                } => Some(Move::Scan(2)),
                _ => None,
            },
        );
        assert_eq!(count, 2);
        assert_eq!(
            router.drain_queued().collect::<Vec<_>>(),
            vec![
                QueuedAction::new(Move::Scan(2), ActionSource::Terminal),
                QueuedAction::new(Move::East, ActionSource::Terminal),
            ]
        );
    }

    #[test]
    fn handle_batch_with_from_preserves_source() {
        let mut router = bound_router();
        let count = router.handle_batch_with_from(
            [
                InputEvent::Key {
                    key: Key::Up,
                    kind: KeyEventKind::Press,
                },
                InputEvent::Key {
                    key: Key::Down,
                    kind: KeyEventKind::Press,
                },
            ],
            ActionSource::Replay,
            |_| None,
        );
        assert_eq!(count, 2);
        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert!(queued.iter().all(|q| q.source == ActionSource::Replay));
    }

    #[test]
    fn set_bindings_swaps_keymap_and_returns_old() {
        let mut router = bound_router();
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));

        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);
        menu_bindings.bind(Key::Esc, Move::Wait);

        let old = router.set_bindings(menu_bindings);
        assert!(old.translate(Key::Up).is_some());
        assert!(!router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert!(router.handle(InputEvent::Key {
            key: Key::Enter,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::Wait));

        router.set_bindings(old);
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_guard_restores_on_drop() {
        let mut router = bound_router();
        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);

        // Verify original bindings work.
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));

        {
            let mut guard = router.bindings_guard(menu_bindings);
            assert!(guard.handle(InputEvent::Key {
                key: Key::Enter,
                kind: KeyEventKind::Press
            }));
            assert_eq!(guard.next_action(), Some(Move::Wait));
            assert!(!guard.handle(InputEvent::Key {
                key: Key::Up,
                kind: KeyEventKind::Press
            }));
        }

        // Verify original bindings are restored.
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_guard_restores_even_on_panic() {
        let mut router = bound_router();
        let mut menu_bindings = Bindings::new();
        menu_bindings.bind(Key::Enter, Move::Wait);

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = router.bindings_guard(menu_bindings);
            panic!("intentional");
        }));

        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
    }

    #[test]
    fn bindings_merge_combines_key_and_mouse_maps() {
        let mut base = Bindings::new();
        base.bind(Key::Up, Move::North);
        base.bind(Key::Down, Move::South);

        let mut overlay = Bindings::new();
        overlay.bind(Key::Down, Move::Wait);
        overlay.bind(Key::Left, Move::West);
        overlay.bind_mouse(MouseButton::Right, true, Move::Scan(1));

        base.merge(overlay);

        assert_eq!(base.translate(Key::Up), Some(Move::North));
        assert_eq!(base.translate(Key::Down), Some(Move::Wait));
        assert_eq!(base.translate(Key::Left), Some(Move::West));
        assert_eq!(
            base.translate_mouse(MouseButton::Right, true),
            Some(Move::Scan(1))
        );
        assert_eq!(base.len(), 4);
    }

    #[test]
    fn command_bindings_merge_combines_names_and_glyphs() {
        let mut base = CommandBindings::new();
        base.bind_name("north", Move::North);
        base.bind_glyph('e', Move::East);

        let mut overlay = CommandBindings::new();
        overlay.bind_name("north", Move::Wait); // overwrite
        overlay.bind_name("south", Move::South);
        overlay.bind_glyph('w', Move::West);

        base.merge(overlay);

        assert_eq!(base.translate_name("north"), Some(Move::Wait));
        assert_eq!(base.translate_name("south"), Some(Move::South));
        assert_eq!(base.translate_glyph('e'), Some(Move::East));
        assert_eq!(base.translate_glyph('w'), Some(Move::West));
        assert_eq!(base.name_count(), 2);
        assert_eq!(base.glyph_count(), 2);
    }

    #[test]
    fn total_actions_queued_tracks_lifetime_count() {
        let mut router = bound_router();
        assert_eq!(router.total_actions_queued(), 0);

        router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press,
        });
        assert_eq!(router.total_actions_queued(), 1);

        router.inject(Move::Wait);
        assert_eq!(router.total_actions_queued(), 2);

        router.inject_all([Move::East, Move::South]);
        assert_eq!(router.total_actions_queued(), 4);

        // Draining does not decrease the counter.
        let _: Vec<Move> = router.drain().collect();
        assert_eq!(router.total_actions_queued(), 4);

        router.handle(InputEvent::Key {
            key: Key::Down,
            kind: KeyEventKind::Press,
        });
        assert_eq!(router.total_actions_queued(), 5);
    }

    #[test]
    fn text_input_accepts_characters() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        assert_eq!(input.text(), "hi");
    }

    #[test]
    fn text_input_respects_max_length() {
        let mut input = TextInput::with_max(3);
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Char('d'));
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn text_input_backspace_deletes_before_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Backspace);
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn text_input_backspace_at_start_does_nothing() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Backspace); // deletes 'a', cursor at 0
        assert_eq!(input.text(), "");
        input.handle_key(Key::Backspace); // does nothing, already at start
        assert_eq!(input.text(), "");
    }

    #[test]
    fn text_input_delete_deletes_at_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Left);
        input.handle_key(Key::Left);
        input.handle_key(Key::Delete);
        assert_eq!(input.text(), "ac");
    }

    #[test]
    fn text_input_cursor_movement() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        assert_eq!(input.cursor(), 3);

        input.handle_key(Key::Left);
        assert_eq!(input.cursor(), 2);

        input.handle_key(Key::Right);
        assert_eq!(input.cursor(), 3);

        input.handle_key(Key::Home);
        assert_eq!(input.cursor(), 0);

        input.handle_key(Key::End);
        assert_eq!(input.cursor(), 3);
    }

    #[test]
    fn text_input_ctrl_navigation_moves_cursor() {
        let mut input = TextInput::new();
        input.set_text("hello".to_owned());
        assert_eq!(input.cursor(), 5);

        input.handle_key(Key::modified('b', true, false, false));
        assert_eq!(input.cursor(), 4);

        input.handle_key(Key::modified('f', true, false, false));
        assert_eq!(input.cursor(), 5);

        input.handle_key(Key::modified('a', true, false, false));
        assert_eq!(input.cursor(), 0);

        input.handle_key(Key::modified('e', true, false, false));
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn text_input_ctrl_w_deletes_word_left() {
        let mut input = TextInput::new();
        input.set_text("hello world".to_owned());

        input.handle_key(Key::modified('w', true, false, false));
        assert_eq!(input.text(), "hello ");
        assert_eq!(input.cursor(), 6);
    }

    #[test]
    fn text_input_ctrl_u_deletes_to_start() {
        let mut input = TextInput::new();
        input.set_text("hello world".to_owned());
        input.set_cursor(5);

        input.handle_key(Key::modified('u', true, false, false));
        assert_eq!(input.text(), " world");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_ctrl_k_deletes_to_end() {
        let mut input = TextInput::new();
        input.set_text("hello world".to_owned());
        input.set_cursor(6);

        input.handle_key(Key::modified('k', true, false, false));
        assert_eq!(input.text(), "hello ");
        assert_eq!(input.cursor(), 6);
    }

    #[test]
    fn text_input_inserts_at_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Left);
        input.handle_key(Key::Char('b'));
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn text_input_enter_returns_true() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        assert!(input.handle_key(Key::Enter));
    }

    #[test]
    fn text_input_take_text_clears_buffer() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        let text = input.take_text();
        assert_eq!(text, "hi");
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_esc_clears_all() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        input.handle_key(Key::Esc);
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_dirty_tracking() {
        let mut input = TextInput::new();
        assert!(!input.is_dirty());
        input.handle_key(Key::Char('a'));
        assert!(input.is_dirty());
        input.clear_dirty();
        assert!(!input.is_dirty());
    }

    #[test]
    fn text_input_set_text() {
        let mut input = TextInput::with_max(5);
        input.set_text("hello world".to_owned());
        assert_eq!(input.text(), "hello");
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn text_input_is_empty() {
        let mut input = TextInput::new();
        assert!(input.is_empty());
        input.handle_key(Key::Char('a'));
        assert!(!input.is_empty());
    }

    #[test]
    fn text_input_clear() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.clear();
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn text_input_handles_multibyte_characters() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('日'));
        input.handle_key(Key::Char('本'));
        input.handle_key(Key::Char('語'));
        assert_eq!(input.text(), "日本語");
        assert_eq!(input.cursor(), 3);

        input.handle_key(Key::Left);
        assert_eq!(input.cursor(), 2);
        input.handle_key(Key::Backspace);
        // Backspace at cursor 2 deletes "本" (position 1), leaving "日語"
        assert_eq!(input.text(), "日語");
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn text_input_control_chars_ignored() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('\n'));
        input.handle_key(Key::Char('\r'));
        input.handle_key(Key::Char('\t'));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn text_input_handle_event_only_processes_keys() {
        let mut input = TextInput::new();
        assert!(!input.handle_event(InputEvent::Tick));
        assert!(!input.handle_event(InputEvent::Resize {
            width: 80,
            height: 24
        }));
        assert!(!input.handle_event(InputEvent::Mouse {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            pressed: true,
        }));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn action_source_display_roundtrips() {
        assert_eq!(ActionSource::Terminal.to_string(), "Terminal");
        assert_eq!(ActionSource::Script.to_string(), "Script");
        assert_eq!(ActionSource::Agent.to_string(), "Agent");
        assert_eq!(ActionSource::Replay.to_string(), "Replay");
        assert_eq!(ActionSource::Test.to_string(), "Test");
    }

    #[test]
    fn action_source_from_str_parses() {
        assert_eq!(
            "Terminal".parse::<ActionSource>().unwrap(),
            ActionSource::Terminal
        );
        assert_eq!(
            "script".parse::<ActionSource>().unwrap(),
            ActionSource::Script
        );
        assert_eq!(
            "AGENT".parse::<ActionSource>().unwrap(),
            ActionSource::Agent
        );
    }

    #[test]
    fn action_source_case_insensitive_parsing() {
        assert_eq!(
            "terminal".parse::<ActionSource>().unwrap(),
            ActionSource::Terminal
        );
        assert_eq!(
            "SCRIPT".parse::<ActionSource>().unwrap(),
            ActionSource::Script
        );
        assert_eq!(
            "agent".parse::<ActionSource>().unwrap(),
            ActionSource::Agent
        );
        assert_eq!(
            "replay".parse::<ActionSource>().unwrap(),
            ActionSource::Replay
        );
        assert_eq!("TEST".parse::<ActionSource>().unwrap(), ActionSource::Test);
    }

    #[test]
    fn action_source_invalid_returns_error() {
        assert!("unknown".parse::<ActionSource>().is_err());
        assert!("".parse::<ActionSource>().is_err());
    }

    #[test]
    fn text_input_history_records_on_enter() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('h'));
        input.handle_key(Key::Char('i'));
        assert!(input.handle_key(Key::Enter));
        assert_eq!(input.history_len(), 1);
        assert_eq!(input.history_get(0), Some("hi"));
    }

    #[test]
    fn text_input_history_navigate_up() {
        let mut input = TextInput::new();
        input.set_text("first".to_owned());
        input.handle_key(Key::Enter);
        input.set_text("second".to_owned());
        input.handle_key(Key::Enter);

        input.handle_key(Key::Up);
        assert_eq!(input.text(), "second");
        input.handle_key(Key::Up);
        assert_eq!(input.text(), "first");
    }

    #[test]
    fn text_input_history_navigate_down() {
        let mut input = TextInput::new();
        input.set_text("first".to_owned());
        input.handle_key(Key::Enter);

        input.handle_key(Key::Up);
        assert_eq!(input.text(), "first");
        input.handle_key(Key::Down);
        assert!(input.is_empty());
    }

    #[test]
    fn text_input_empty_text_not_added_to_history() {
        let mut input = TextInput::new();
        assert!(input.handle_key(Key::Enter));
        assert_eq!(input.history_len(), 0);
    }

    #[test]
    fn text_input_history_respects_max() {
        let mut input = TextInput::with_max(10).with_max_history(2);
        input.set_text("a".to_owned());
        input.handle_key(Key::Enter);
        input.set_text("b".to_owned());
        input.handle_key(Key::Enter);
        input.set_text("c".to_owned());
        input.handle_key(Key::Enter);

        assert_eq!(input.history_len(), 2);
        assert_eq!(input.history_get(0), Some("b"));
        assert_eq!(input.history_get(1), Some("c"));
    }

    #[test]
    fn text_input_clear_history() {
        let mut input = TextInput::new();
        input.set_text("test".to_owned());
        input.handle_key(Key::Enter);
        assert_eq!(input.history_len(), 1);
        input.clear_history();
        assert_eq!(input.history_len(), 0);
    }

    #[test]
    fn text_input_insert_str_at_cursor() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('c'));
        input.handle_key(Key::Left);
        input.insert_str("b");
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn text_input_insert_str_respects_max_length() {
        let mut input = TextInput::with_max(5);
        input.set_text("hel".to_owned());
        input.insert_str("lo world");
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn text_input_insert_str_empty_is_noop() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.insert_str("");
        assert_eq!(input.text(), "a");
    }

    #[test]
    fn text_input_insert_str_at_end() {
        let mut input = TextInput::new();
        input.set_text("hello".to_owned());
        input.insert_str("!");
        assert_eq!(input.text(), "hello!");
    }

    #[test]
    fn test_text_input_undo_redo() {
        let mut input = TextInput::new();
        input.handle_key(Key::Char('a'));
        input.handle_key(Key::Char('b'));
        input.handle_key(Key::Char('c'));
        assert_eq!(input.text(), "abc");

        // Undo last character insertion ('c')
        assert!(input.undo());
        assert_eq!(input.text(), "ab");

        // Undo 'b'
        assert!(input.undo());
        assert_eq!(input.text(), "a");

        // Undo 'a'
        assert!(input.undo());
        assert_eq!(input.text(), "");

        // No more undo
        assert!(!input.undo());

        // Redo 'a'
        assert!(input.redo());
        assert_eq!(input.text(), "a");

        // Redo 'b'
        assert!(input.redo());
        assert_eq!(input.text(), "ab");

        // Redo 'c'
        assert!(input.redo());
        assert_eq!(input.text(), "abc");

        // No more redo
        assert!(!input.redo());

        // Type 'd' -> should clear redo stack
        input.handle_key(Key::Char('d'));
        assert_eq!(input.text(), "abcd");
        assert!(!input.redo());

        // Test ctrl-z/ctrl-y keys via handle_key
        input.handle_key(Key::Modified {
            char: 'z',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "abc");

        input.handle_key(Key::Modified {
            char: 'y',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "abcd");

        // Test clear()/Esc undoable
        input.clear();
        assert_eq!(input.text(), "");
        assert!(input.undo());
        assert_eq!(input.text(), "abcd");
    }

    #[test]
    fn test_text_input_autocomplete() {
        let mut input = TextInput::new();
        input.set_text("run in".to_owned());

        let dict = vec!["inspect", "confirm", "info", "input", "init"];

        // Cycle 1: matches "inspect", "info", "input", "init". First match should be "inspect".
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run inspect");
        assert_eq!(input.cursor(), 11);

        // Cycle 2: next match is "info"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run info");
        assert_eq!(input.cursor(), 8);

        // Cycle 3: next match is "input"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run input");
        assert_eq!(input.cursor(), 9);

        // Cycle 4: next match is "init"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run init");
        assert_eq!(input.cursor(), 8);

        // Cycle 5: wraps around back to "inspect"
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run inspect");

        // Typing a character should break the autocomplete cycle
        input.handle_key(Key::Char('r'));
        assert_eq!(input.text(), "run inspectr");

        // Now if we hit tab/autocomplete again with "inspectr", no match.
        input.cycle_autocomplete(&dict);
        assert_eq!(input.text(), "run inspectr");
    }

    #[test]
    fn test_autocomplete_with_history() {
        let mut input = TextInput::new();
        input.set_text("super_cool_command".to_owned());
        input.handle_key(Key::Enter); // Pushes to history

        // Now clear and try prefix matching "super"
        input.set_text("su".to_owned());
        let dict = vec!["something_else"];
        input.cycle_autocomplete_with_history(&dict);
        assert_eq!(input.text(), "super_cool_command");
    }

    #[test]
    fn test_text_input_word_jumps() {
        let mut input = TextInput::new();
        input.set_text("hello brave new world".to_owned());
        assert_eq!(input.cursor(), 21); // at the end

        // Ctrl+Left (←) to jump one word left
        input.handle_key(Key::Modified {
            char: '←',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 16); // "world" starts at 16

        // Ctrl+Left again
        input.handle_key(Key::Modified {
            char: '←',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 12); // "new" starts at 12

        // Ctrl+Right (→) to jump one word right
        input.handle_key(Key::Modified {
            char: '→',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 16); // back to "world"

        // Ctrl+Right to end
        input.handle_key(Key::Modified {
            char: '→',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.cursor(), 21); // at the end again
    }

    #[test]
    fn test_text_input_delete_word() {
        let mut input = TextInput::new();
        input.set_text("hello brave new world".to_owned());

        // Ctrl+Backspace: delete "world"
        input.handle_key(Key::Modified {
            char: '\x08',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "hello brave new ");
        assert_eq!(input.cursor(), 16);

        // Ctrl+Backspace again: delete "new"
        input.handle_key(Key::Modified {
            char: '\x08',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "hello brave ");
        assert_eq!(input.cursor(), 12);

        // Ctrl+Delete: delete "brave" (word right from cursor)
        input.set_text("hello brave new world".to_owned());
        input.set_cursor(6); // at "brave"
        input.handle_key(Key::Modified {
            char: '\x7f',
            ctrl: true,
            alt: false,
            shift: false,
        });
        assert_eq!(input.text(), "hello new world");
        assert_eq!(input.cursor(), 6);
    }

    #[test]
    fn inject_priority_puts_action_at_front() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject(Move::South);
        router.inject_priority(Move::Wait);

        assert_eq!(router.pending(), 3);
        assert_eq!(router.next_action(), Some(Move::Wait));
        assert_eq!(router.next_action(), Some(Move::North));
        assert_eq!(router.next_action(), Some(Move::South));
    }

    #[test]
    fn inject_priority_from_preserves_source() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject_priority_from(Move::Wait, ActionSource::Agent);

        let queued: Vec<QueuedAction<Move>> = router.drain_queued().collect();
        assert_eq!(
            queued,
            vec![
                QueuedAction::new(Move::Wait, ActionSource::Agent),
                QueuedAction::new(Move::North, ActionSource::Script),
            ]
        );
    }

    #[test]
    fn inject_priority_counts_toward_total() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject_priority(Move::Wait);
        assert_eq!(router.total_actions_queued(), 2);
    }

    #[test]
    fn filter_pending_removes_matching_actions() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject(Move::South);
        router.inject(Move::East);

        let removed = router.filter_pending(|qa| matches!(qa.action, Move::North | Move::South));
        assert_eq!(removed, 2);
        assert_eq!(router.pending(), 1);
        assert_eq!(router.next_action(), Some(Move::East));
    }

    #[test]
    fn filter_pending_preserves_order_of_remaining() {
        let mut router = bound_router();
        router.inject(Move::North);
        router.inject(Move::South);
        router.inject(Move::East);
        router.inject(Move::West);

        router.filter_pending(|qa| matches!(qa.action, Move::South));
        let drained: Vec<Move> = router.drain().collect();
        assert_eq!(drained, vec![Move::North, Move::East, Move::West]);
    }

    #[test]
    fn filter_pending_empty_queue_returns_zero() {
        let mut router = bound_router();
        let removed = router.filter_pending(|_| true);
        assert_eq!(removed, 0);
        assert!(router.is_idle());
    }

    #[test]
    fn bindings_iter_keys_yields_all_key_bindings() {
        let b = bound_router();
        let keys: Vec<Key> = b.bindings().iter_keys().map(|(k, _)| k).collect();
        assert_eq!(keys.len(), 5);
        assert!(keys.contains(&Key::Up));
        assert!(keys.contains(&Key::Down));
        assert!(keys.contains(&Key::Left));
        assert!(keys.contains(&Key::Right));
        assert!(keys.contains(&Key::Char('.')));
    }

    #[test]
    fn bindings_iter_mouse_yields_mouse_bindings() {
        let mut b = Bindings::new();
        b.bind_mouse(MouseButton::Left, true, Move::North);
        b.bind_mouse(MouseButton::Right, false, Move::South);
        let mouse: Vec<_> = b.iter_mouse().collect();
        assert_eq!(mouse.len(), 2);
    }

    #[test]
    fn command_bindings_iter_names_and_glyphs() {
        let mut c = CommandBindings::new();
        c.bind_name("north", Move::North);
        c.bind_name("south", Move::South);
        c.bind_glyph('e', Move::East);
        c.bind_glyph('w', Move::West);

        let names: Vec<&str> = c.iter_names().map(|(n, _)| n).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"north"));
        assert!(names.contains(&"south"));

        let glyphs: Vec<char> = c.iter_glyphs().map(|(g, _)| g).collect();
        assert_eq!(glyphs.len(), 2);
        assert!(glyphs.contains(&'e'));
        assert!(glyphs.contains(&'w'));
    }

    #[test]
    fn push_bindings_saves_and_switches_context() {
        let mut router = bound_router();
        assert_eq!(router.context_depth(), 0);

        let mut menu = Bindings::new();
        menu.bind(Key::Enter, Move::Wait);
        router.push_bindings(menu);

        assert_eq!(router.context_depth(), 1);
        assert!(router.handle(InputEvent::Key {
            key: Key::Enter,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::Wait));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
    }

    #[test]
    fn pop_bindings_restores_previous_context() {
        let mut router = bound_router();

        let mut menu = Bindings::new();
        menu.bind(Key::Enter, Move::Wait);
        router.push_bindings(menu);

        assert!(router.pop_bindings());
        assert_eq!(router.context_depth(), 0);
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert_eq!(router.next_action(), Some(Move::North));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Enter,
            kind: KeyEventKind::Press
        }));
    }

    #[test]
    fn pop_bindings_returns_false_on_empty_stack() {
        let mut router = bound_router();
        assert!(!router.pop_bindings());
        assert_eq!(router.context_depth(), 0);
    }

    #[test]
    fn nested_push_bindings_supports_multiple_levels() {
        let mut router = bound_router();

        let mut level1 = Bindings::new();
        level1.bind(Key::Char('a'), Move::North);
        router.push_bindings(level1);
        assert_eq!(router.context_depth(), 1);

        let mut level2 = Bindings::new();
        level2.bind(Key::Char('b'), Move::South);
        router.push_bindings(level2);
        assert_eq!(router.context_depth(), 2);

        // Level 2 is active.
        assert!(router.handle(InputEvent::Key {
            key: Key::Char('b'),
            kind: KeyEventKind::Press
        }));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('a'),
            kind: KeyEventKind::Press
        }));

        // Pop to level 1.
        router.pop_bindings();
        assert!(router.handle(InputEvent::Key {
            key: Key::Char('a'),
            kind: KeyEventKind::Press
        }));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('b'),
            kind: KeyEventKind::Press
        }));

        // Pop back to original.
        router.pop_bindings();
        assert!(router.handle(InputEvent::Key {
            key: Key::Up,
            kind: KeyEventKind::Press
        }));
        assert!(!router.handle(InputEvent::Key {
            key: Key::Char('a'),
            kind: KeyEventKind::Press
        }));
    }

    #[test]
    fn key_display() {
        assert_eq!(format!("{}", Key::Char('a')), "a");
        assert_eq!(format!("{}", Key::Enter), "Enter");
        assert_eq!(format!("{}", Key::F(5)), "F5");
        assert_eq!(
            format!("{}", Key::modified('x', true, false, false)),
            "Ctrl+x"
        );
        assert_eq!(
            format!("{}", Key::modified('a', true, true, false)),
            "Ctrl+Alt+a"
        );
        // Empty modifiers should not produce a leading '+'
        assert_eq!(format!("{}", Key::modified('a', false, false, false)), "a");
    }

    #[test]
    fn mouse_button_display() {
        assert_eq!(format!("{}", MouseButton::Left), "Left");
        assert_eq!(format!("{}", MouseButton::Right), "Right");
    }

    #[test]
    fn scroll_direction_display() {
        assert_eq!(format!("{}", ScrollDirection::Up), "Up");
        assert_eq!(format!("{}", ScrollDirection::Down), "Down");
    }

    #[test]
    fn bindings_clear_removes_all() {
        let mut bindings = Bindings::new();
        bindings.bind(Key::Char('a'), 1);
        bindings.bind(Key::Char('b'), 2);
        bindings.bind_mouse(MouseButton::Left, true, 3);
        bindings.bind_scroll(ScrollDirection::Up, 4);
        assert_eq!(bindings.len(), 4);

        bindings.clear();
        assert!(bindings.is_empty());
        assert_eq!(bindings.len(), 0);
        assert_eq!(bindings.translate(Key::Char('a')), None);
    }

    #[test]
    fn command_bindings_clear_removes_all() {
        let mut cmds = CommandBindings::new();
        cmds.bind_name("north", 1);
        cmds.bind_glyph('n', 1);
        assert_eq!(cmds.name_count(), 1);
        assert_eq!(cmds.glyph_count(), 1);

        cmds.clear();
        assert_eq!(cmds.name_count(), 0);
        assert_eq!(cmds.glyph_count(), 0);
        assert_eq!(cmds.translate_name("north"), None);
        assert_eq!(cmds.translate_glyph('n'), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_action_source() {
        let sources = [
            ActionSource::Terminal,
            ActionSource::Script,
            ActionSource::Agent,
            ActionSource::Replay,
            ActionSource::Test,
        ];
        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let roundtrip: ActionSource = serde_json::from_str(&json).unwrap();
            assert_eq!(source, roundtrip);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_key() {
        let keys = [
            Key::Char('a'),
            Key::Enter,
            Key::Up,
            Key::F(5),
            Key::Modified {
                char: 'c',
                ctrl: true,
                alt: false,
                shift: false,
            },
        ];
        for key in keys {
            let json = serde_json::to_string(&key).unwrap();
            let roundtrip: Key = serde_json::from_str(&json).unwrap();
            assert_eq!(key, roundtrip);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_input_event() {
        let events = [
            InputEvent::Key {
                key: Key::Char('x'),
                kind: KeyEventKind::Press,
            },
            InputEvent::Mouse {
                x: 10,
                y: 5,
                button: MouseButton::Left,
                pressed: true,
            },
            InputEvent::MouseScroll {
                x: 0,
                y: 0,
                direction: ScrollDirection::Up,
            },
            InputEvent::Resize {
                width: 80,
                height: 24,
            },
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let roundtrip: InputEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, roundtrip);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_bindings() {
        #[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        enum DummyAction {
            MoveUp,
            Fire,
        }

        let mut bindings = Bindings::new();
        bindings.bind(Key::Char('w'), DummyAction::MoveUp);
        bindings.bind_mouse(MouseButton::Left, true, DummyAction::Fire);
        bindings.bind_scroll(ScrollDirection::Up, DummyAction::MoveUp);

        let json = serde_json::to_string(&bindings).unwrap();
        let roundtrip: Bindings<DummyAction> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            roundtrip.translate(Key::Char('w')),
            Some(DummyAction::MoveUp)
        );
        assert_eq!(
            roundtrip.translate_mouse(MouseButton::Left, true),
            Some(DummyAction::Fire)
        );
        assert_eq!(
            roundtrip.translate_scroll(ScrollDirection::Up),
            Some(DummyAction::MoveUp)
        );

        let mut cmd_bindings = CommandBindings::new();
        cmd_bindings.bind_name("up", DummyAction::MoveUp);
        cmd_bindings.bind_glyph('f', DummyAction::Fire);

        let cmd_json = serde_json::to_string(&cmd_bindings).unwrap();
        let cmd_roundtrip: CommandBindings<DummyAction> = serde_json::from_str(&cmd_json).unwrap();
        assert_eq!(
            cmd_roundtrip.translate_name("up"),
            Some(DummyAction::MoveUp)
        );
        assert_eq!(cmd_roundtrip.translate_glyph('f'), Some(DummyAction::Fire));
    }
}
