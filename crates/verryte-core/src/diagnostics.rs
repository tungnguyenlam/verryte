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
