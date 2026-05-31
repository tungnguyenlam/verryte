use crate::World;

#[test]
fn test_world_inspect() {
    let mut world = World::new();
    let e1 = world.spawn();

    struct Pos {
        _x: i32,
    }
    struct Health {
        _hp: i32,
    }

    world.insert(e1, Pos { _x: 1 });
    world.insert(e1, Health { _hp: 10 });

    let inspection = world.inspect_entities();
    println!("{}", inspection);
    assert!(inspection.contains("Entity 0#1"));
    assert!(inspection.contains("Pos"));
    assert!(inspection.contains("Health"));
}
