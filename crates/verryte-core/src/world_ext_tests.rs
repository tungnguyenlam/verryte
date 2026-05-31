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
