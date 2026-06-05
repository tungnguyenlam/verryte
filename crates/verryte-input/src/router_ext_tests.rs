use crate::bindings::Bindings;
use crate::router::InputRouter;

#[derive(Clone, Debug, PartialEq)]
enum MyAction {
    Move,
    Attack,
}

#[test]
fn test_delayed_actions() {
    let mut router = InputRouter::new(Bindings::<MyAction>::new());

    router.inject_delayed(MyAction::Move, 1.0);
    router.inject_delayed(MyAction::Attack, 0.5);

    // No actions yet
    assert_eq!(router.pending(), 0);

    router.tick(0.3);
    assert_eq!(router.pending(), 0);

    router.tick(0.3); // Total 0.6
    assert_eq!(router.pending(), 1); // Attack is ready
    assert_eq!(router.next_action(), Some(MyAction::Attack));

    router.tick(0.5); // Total 1.1
    assert_eq!(router.pending(), 1); // Move is ready
    assert_eq!(router.next_action(), Some(MyAction::Move));
}

#[test]
fn test_history_limit() {
    let mut router = InputRouter::new(Bindings::<MyAction>::new());
    router.set_history_limit(Some(5));

    for _ in 0..10 {
        router.inject(MyAction::Move);
        router.next_action();
    }

    assert_eq!(router.history().len(), 5);
}

#[test]
fn test_bindings_profiles() {
    let mut default_bindings = Bindings::<MyAction>::new();
    default_bindings.bind(crate::key::Key::Char('w'), MyAction::Move);

    let mut router = InputRouter::new(default_bindings);
    assert_eq!(router.active_profile_name(), Some("default"));

    let mut alt_bindings = Bindings::<MyAction>::new();
    alt_bindings.bind(crate::key::Key::Char('a'), MyAction::Attack);
    router.register_profile("alt", alt_bindings);

    assert!(router.switch_profile("alt"));
    assert_eq!(router.active_profile_name(), Some("alt"));

    router.handle(crate::key::InputEvent::Key {
        key: crate::key::Key::Char('a'),
        kind: crate::key::KeyEventKind::Press,
    });
    assert_eq!(router.next_action(), Some(MyAction::Attack));
}

#[test]
fn test_action_interceptor() {
    let mut router = InputRouter::new(Bindings::<MyAction>::new());

    router.set_interceptor(|action| !matches!(action, MyAction::Attack));

    router.inject(MyAction::Move);
    router.inject(MyAction::Attack);

    assert_eq!(router.pending(), 1);
    assert_eq!(router.next_action(), Some(MyAction::Move));
    assert_eq!(router.next_action(), None);
}
