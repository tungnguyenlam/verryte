use crate::bindings::CommandBindings;

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
