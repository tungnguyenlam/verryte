#[cfg(feature = "serde")]
use crate::{World, WorldRegistry};

#[test]
#[cfg(feature = "serde")]
fn test_snapshot_diff() {
    let mut world1 = World::new();
    let e1 = world1.spawn();

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct Pos {
        x: i32,
        y: i32,
    }

    let mut registry = WorldRegistry::new();
    registry.register_component::<Pos>("Pos");

    world1.insert(e1, Pos { x: 1, y: 1 });
    let snap1 = registry.snapshot(&world1);

    let mut world2 = World::new();
    world2.spawn_at(e1);
    world2.insert(e1, Pos { x: 2, y: 2 });
    let e2 = world2.spawn();
    world2.insert(e2, Pos { x: 3, y: 3 });

    let snap2 = registry.snapshot(&world2);

    let diff = snap1.diff(&snap2);

    assert_eq!(diff.added_entities.len(), 1);
    assert_eq!(diff.added_entities[0].entity, e2);
    assert!(diff.changed_entities.contains_key(&e1));
    let e1_diff = &diff.changed_entities[&e1];
    assert!(e1_diff.changed_components.contains_key("Pos"));
}
