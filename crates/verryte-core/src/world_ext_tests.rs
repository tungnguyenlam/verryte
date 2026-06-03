use crate::World;

#[test]
fn test_resource_change_detection() {
    let mut world = World::new();
    struct MyRes(#[allow(dead_code)] u32);

    world.insert_resource(MyRes(10));
    let tick1 = world.read_tick();
    assert_eq!(world.resource_tick::<MyRes>(), Some(tick1));

    // Resource is dirty since tick 0
    assert!(world.is_resource_dirty::<MyRes>(0));
    // Resource is NOT dirty since tick 1
    assert!(!world.is_resource_dirty::<MyRes>(tick1));

    // Mutate resource
    world.increment_tick();
    let tick2 = world.read_tick();
    {
        let _res = world.resource_mut::<MyRes>().unwrap();
    }

    assert_eq!(world.resource_tick::<MyRes>(), Some(tick2));
    assert!(world.is_resource_dirty::<MyRes>(tick1));
}

#[test]
fn test_world_tag_helpers() {
    let mut world = World::new();
    let e1 = world.spawn_with_tag("player");
    let e2 = world.spawn_with_tag("enemy");
    let e3 = world.spawn_with_tag("enemy");

    assert!(world.has_tag(e1, "player"));
    assert!(!world.has_tag(e1, "enemy"));
    assert!(world.has_tag(e2, "enemy"));

    let enemies = world.find_entities_with_tag("enemy");
    assert_eq!(enemies.len(), 2);
    assert!(enemies.contains(&e2));
    assert!(enemies.contains(&e3));

    let despawned = world.despawn_all_with_tag("enemy");
    assert_eq!(despawned, 2);
    assert!(!world.is_alive(e2));
    assert!(!world.is_alive(e3));
    assert!(world.is_alive(e1));
}

#[test]
fn test_world_find_where() {
    let mut world = World::new();
    #[derive(Debug, PartialEq)]
    struct Score(u32);

    let e1 = world.spawn();
    world.insert(e1, Score(10));
    let e2 = world.spawn();
    world.insert(e2, Score(20));

    let found = world.find_where(|s: &Score| s.0 > 15);
    assert_eq!(found, Some((e2, &Score(20))));

    let not_found = world.find_where(|s: &Score| s.0 > 25);
    assert!(not_found.is_none());

    let found_mut = world.find_mut_where(|s: &Score| s.0 < 15);
    assert_eq!(found_mut.map(|(e, s)| (e, s.0)), Some((e1, 10)));
}

#[test]
fn test_world_has_component() {
    let mut world = World::new();
    struct CompA;
    struct CompB;

    let e = world.spawn();
    world.insert(e, CompA);

    assert!(world.has_component::<CompA>(e));
    assert!(!world.has_component::<CompB>(e));
}
