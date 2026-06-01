use crate::bindings::{Bindings, CommandBindings};
use crate::key::Key;

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
enum MyAction {
    North,
    South,
}

#[test]
fn test_command_aliases() {
    let mut cmds = CommandBindings::new();
    cmds.bind_name("north", MyAction::North);
    cmds.bind_alias("n", "north");
    cmds.bind_alias("mv_n", "north");

    assert_eq!(cmds.translate_name("north"), Some(MyAction::North));
    assert_eq!(cmds.translate_name("n"), Some(MyAction::North));
    assert_eq!(cmds.translate_name("mv_n"), Some(MyAction::North));
    assert_eq!(cmds.translate_name("unknown"), None);
}

#[test]
fn test_alias_merge() {
    let mut base = CommandBindings::new();
    base.bind_name("north", MyAction::North);
    base.bind_alias("n", "north");

    let mut overlay = CommandBindings::new();
    overlay.bind_alias("up", "north");

    base.merge(overlay);
    assert_eq!(base.translate_name("up"), Some(MyAction::North));
    assert_eq!(base.translate_name("n"), Some(MyAction::North));
}

#[test]
fn test_bindings_get_keys_for_action() {
    let mut bindings = Bindings::new();
    bindings.bind(Key::Char('w'), MyAction::North);
    bindings.bind(Key::Up, MyAction::North);
    bindings.bind(Key::Char('s'), MyAction::South);

    let mut north_keys = bindings.get_keys_for_action(&MyAction::North);
    north_keys.sort_by_key(|k| format!("{:?}", k)); // Sorting for deterministic assertions

    let mut expected_keys = vec![Key::Char('w'), Key::Up];
    expected_keys.sort_by_key(|k| format!("{:?}", k));

    assert_eq!(north_keys, expected_keys);
    assert_eq!(
        bindings.get_keys_for_action(&MyAction::South),
        vec![Key::Char('s')]
    );
}

#[test]
fn test_bindings_queries() {
    use crate::key::{MouseButton, ScrollDirection};
    let mut bindings = Bindings::new();
    bindings.bind(Key::Char('w'), MyAction::North);
    bindings.bind_mouse(MouseButton::Left, true, MyAction::North);
    bindings.bind_scroll(ScrollDirection::Up, MyAction::North);

    assert!(bindings.is_key_bound(Key::Char('w')));
    assert!(!bindings.is_key_bound(Key::Char('a')));

    assert!(bindings.is_action_bound(&MyAction::North));
    assert!(!bindings.is_action_bound(&MyAction::South));

    assert_eq!(
        bindings.get_mouse_for_action(&MyAction::North),
        vec![(MouseButton::Left, true)]
    );
    assert_eq!(
        bindings.get_scroll_for_action(&MyAction::North),
        vec![ScrollDirection::Up]
    );
}

