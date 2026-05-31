use crate::World;

#[test]
fn test_component_change_detection() {
    let mut world = World::new();
    let e = world.spawn();

    struct Pos {
        x: i32,
    }

    world.insert(e, Pos { x: 1 });
    let tick1 = world.read_tick();

    assert!(world.is_added::<Pos>(e, 0));
    assert!(!world.is_added::<Pos>(e, tick1));

    world.increment_tick();
    let tick2 = world.read_tick();
    {
        let pos = world.get_mut::<Pos>(e).unwrap();
        pos.x = 2;
    }

    assert!(world.is_changed::<Pos>(e, tick1));
    assert!(!world.is_changed::<Pos>(e, tick2));
}
