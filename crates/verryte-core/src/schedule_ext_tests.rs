use crate::{Schedule, World};

fn sys_a(_world: &mut World) {}
fn sys_b(_world: &mut World) {}
fn sys_c(_world: &mut World) {}

#[test]
fn test_schedule_ordering() {
    let mut schedule = Schedule::new();
    schedule.add_stage("init");
    schedule.add_named("a", sys_a);
    schedule.add_named("c", sys_c);

    // Insert B between A and C
    assert!(schedule.add_after("a", "b", sys_b));

    let systems: Vec<_> = schedule.systems().iter().map(|s| s.name).collect();
    assert_eq!(systems, vec!["a", "b", "c"]);

    let description = schedule.describe();
    assert!(description.contains("[Stage: init]"));
    assert!(description.contains("  - a"));
    assert!(description.contains("  - b"));
    assert!(description.contains("  - c"));
}

#[test]
fn test_schedule_before() {
    let mut schedule = Schedule::new();
    schedule.add_named("b", sys_b);
    schedule.add_before("b", "a", sys_a);

    let systems: Vec<_> = schedule.systems().iter().map(|s| s.name).collect();
    assert_eq!(systems, vec!["a", "b"]);
}
