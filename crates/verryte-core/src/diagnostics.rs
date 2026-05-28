use std::collections::HashMap;
use std::time::Duration;

/// Runtime performance metrics for a single ECS system.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemMetrics {
    /// Number of times the system was executed.
    pub call_count: u64,
    /// Total cumulative time spent executing this system.
    pub total_duration: Duration,
    /// Time spent during the most recent execution.
    pub last_duration: Duration,
    /// Maximum time spent in a single execution.
    pub max_duration: Duration,
}

/// A global resource that tracks execution speed of named systems.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    /// Metrics keyed by the system's unique name.
    pub systems: HashMap<String, SystemMetrics>,
}

impl Diagnostics {
    /// Create a new, empty diagnostics registry.
    pub fn new() -> Self {
        Self {
            systems: HashMap::new(),
        }
    }

    /// Record a system's execution duration.
    pub fn record(&mut self, system_name: &str, duration: Duration) {
        let metrics = self.systems.entry(system_name.to_string()).or_default();
        metrics.call_count += 1;
        metrics.total_duration += duration;
        metrics.last_duration = duration;
        if duration > metrics.max_duration {
            metrics.max_duration = duration;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_diagnostics_is_empty() {
        let diag = Diagnostics::new();
        assert!(diag.systems.is_empty());
    }

    #[test]
    fn record_single_system() {
        let mut diag = Diagnostics::new();
        diag.record("movement", Duration::from_millis(10));

        let m = diag.systems.get("movement").unwrap();
        assert_eq!(m.call_count, 1);
        assert_eq!(m.total_duration, Duration::from_millis(10));
        assert_eq!(m.last_duration, Duration::from_millis(10));
        assert_eq!(m.max_duration, Duration::from_millis(10));
    }

    #[test]
    fn record_multiple_calls_tracks_max() {
        let mut diag = Diagnostics::new();
        diag.record("ai", Duration::from_millis(5));
        diag.record("ai", Duration::from_millis(20));
        diag.record("ai", Duration::from_millis(12));

        let m = diag.systems.get("ai").unwrap();
        assert_eq!(m.call_count, 3);
        assert_eq!(m.total_duration, Duration::from_millis(37));
        assert_eq!(m.last_duration, Duration::from_millis(12));
        assert_eq!(m.max_duration, Duration::from_millis(20));
    }

    #[test]
    fn record_different_systems_independent() {
        let mut diag = Diagnostics::new();
        diag.record("render", Duration::from_millis(8));
        diag.record("physics", Duration::from_millis(3));

        assert_eq!(diag.systems.len(), 2);
        assert_eq!(diag.systems.get("render").unwrap().call_count, 1);
        assert_eq!(diag.systems.get("physics").unwrap().call_count, 1);
    }
}
