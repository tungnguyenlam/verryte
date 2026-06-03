use std::collections::HashMap;
use std::fmt;
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
    /// Minimum time spent in a single execution (excluding the first call).
    pub min_duration: Option<Duration>,
}

impl SystemMetrics {
    /// Return the average duration across all recorded calls.
    pub fn avg_duration(&self) -> Duration {
        if self.call_count == 0 {
            return Duration::ZERO;
        }
        self.total_duration / self.call_count as u32
    }
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
        match metrics.min_duration {
            None => metrics.min_duration = Some(duration),
            Some(current) if duration < current => metrics.min_duration = Some(duration),
            _ => {}
        }
    }

    /// Reset all metrics to their initial state, keeping registered system names.
    pub fn reset(&mut self) {
        self.systems.values_mut().for_each(|m| {
            *m = SystemMetrics::default();
        });
    }

    /// Remove all metrics entirely, returning the count of removed systems.
    pub fn clear(&mut self) -> usize {
        let count = self.systems.len();
        self.systems.clear();
        count
    }

    /// Remove metrics for a specific system. Returns `true` if the system existed.
    pub fn remove_system(&mut self, system_name: &str) -> bool {
        self.systems.remove(system_name).is_some()
    }

    /// Return system names and metrics sorted by `last_duration` descending (slowest first).
    ///
    /// Useful for performance overlays that need pre-sorted data each frame
    /// without re-sorting the underlying `HashMap`.
    pub fn sorted_by_duration(&self) -> Vec<(&str, &SystemMetrics)> {
        let mut entries: Vec<_> = self
            .systems
            .iter()
            .map(|(name, metrics)| (name.as_str(), metrics))
            .collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.1.last_duration));
        entries
    }

    /// Return system names and metrics sorted by `max_duration` descending (worst-case first).
    pub fn sorted_by_max_duration(&self) -> Vec<(&str, &SystemMetrics)> {
        let mut entries: Vec<_> = self
            .systems
            .iter()
            .map(|(name, metrics)| (name.as_str(), metrics))
            .collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.1.max_duration));
        entries
    }

    /// Return the total number of recorded systems.
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    /// Return the aggregate call count across all systems.
    pub fn total_calls(&self) -> u64 {
        self.systems.values().map(|m| m.call_count).sum()
    }

    /// Return the aggregate duration across all systems.
    pub fn total_duration(&self) -> Duration {
        self.systems.values().map(|m| m.total_duration).sum()
    }

    /// Return system names and metrics sorted by `avg_duration` descending (slowest average first).
    ///
    /// Useful for identifying systems with consistently poor performance
    /// (as opposed to `sorted_by_duration` which sorts by last execution).
    pub fn sorted_by_avg_duration(&self) -> Vec<(&str, &SystemMetrics)> {
        let mut entries: Vec<_> = self
            .systems
            .iter()
            .map(|(name, metrics)| (name.as_str(), metrics))
            .collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.1.avg_duration()));
        entries
    }

    /// Return system names and metrics sorted by `call_count` descending (most called first).
    pub fn sorted_by_call_count(&self) -> Vec<(&str, &SystemMetrics)> {
        let mut entries: Vec<_> = self
            .systems
            .iter()
            .map(|(name, metrics)| (name.as_str(), metrics))
            .collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.1.call_count));
        entries
    }

    /// Return a serializable snapshot of the diagnostics state.
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let mut systems = Vec::with_capacity(self.systems.len());
        for (name, metrics) in &self.systems {
            systems.push(SystemMetricSnapshot {
                name: name.clone(),
                call_count: metrics.call_count,
                total_duration_ns: metrics.total_duration.as_nanos() as u64,
                last_duration_ns: metrics.last_duration.as_nanos() as u64,
                max_duration_ns: metrics.max_duration.as_nanos() as u64,
                min_duration_ns: metrics.min_duration.map(|d| d.as_nanos() as u64),
                avg_duration_ns: metrics.avg_duration().as_nanos() as u64,
            });
        }
        // Consistent ordering for the snapshot
        systems.sort_by(|a, b| a.name.cmp(&b.name));

        DiagnosticsSnapshot {
            total_calls: self.total_calls(),
            total_duration_ns: self.total_duration().as_nanos() as u64,
            systems,
        }
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sorted = self.sorted_by_avg_duration();
        writeln!(f, "Diagnostics ({} systems):", self.systems.len())?;
        for (name, metrics) in sorted {
            writeln!(
                f,
                "  {}: {} calls, avg {:?}, max {:?}, last {:?}",
                name,
                metrics.call_count,
                metrics.avg_duration(),
                metrics.max_duration,
                metrics.last_duration
            )?;
        }
        Ok(())
    }
}

/// A serializable snapshot of the Diagnostics resource.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiagnosticsSnapshot {
    pub total_calls: u64,
    pub total_duration_ns: u64,
    pub systems: Vec<SystemMetricSnapshot>,
}

/// A serializable snapshot of a single system's metrics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemMetricSnapshot {
    pub name: String,
    pub call_count: u64,
    pub total_duration_ns: u64,
    pub last_duration_ns: u64,
    pub max_duration_ns: u64,
    pub min_duration_ns: Option<u64>,
    pub avg_duration_ns: u64,
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

    #[test]
    fn system_metrics_avg_duration() {
        let mut diag = Diagnostics::new();
        diag.record("ai", Duration::from_millis(10));
        diag.record("ai", Duration::from_millis(20));
        diag.record("ai", Duration::from_millis(30));

        let m = diag.systems.get("ai").unwrap();
        assert_eq!(m.avg_duration(), Duration::from_millis(20));
    }

    #[test]
    fn system_metrics_avg_duration_zero_calls() {
        let m = SystemMetrics::default();
        assert_eq!(m.avg_duration(), Duration::ZERO);
    }

    #[test]
    fn system_metrics_min_duration() {
        let mut diag = Diagnostics::new();
        diag.record("ai", Duration::from_millis(10));
        diag.record("ai", Duration::from_millis(5));
        diag.record("ai", Duration::from_millis(15));

        let m = diag.systems.get("ai").unwrap();
        assert_eq!(m.min_duration, Some(Duration::from_millis(5)));
        assert_eq!(m.max_duration, Duration::from_millis(15));
    }

    #[test]
    fn diagnostics_reset_preserves_systems() {
        let mut diag = Diagnostics::new();
        diag.record("movement", Duration::from_millis(10));
        diag.record("ai", Duration::from_millis(5));

        diag.reset();

        assert_eq!(diag.systems.len(), 2);
        let m = diag.systems.get("movement").unwrap();
        assert_eq!(m.call_count, 0);
        assert_eq!(m.total_duration, Duration::ZERO);
    }

    #[test]
    fn diagnostics_clear_removes_all() {
        let mut diag = Diagnostics::new();
        diag.record("a", Duration::from_millis(1));
        diag.record("b", Duration::from_millis(2));

        let removed = diag.clear();
        assert_eq!(removed, 2);
        assert!(diag.systems.is_empty());
    }

    #[test]
    fn diagnostics_remove_system() {
        let mut diag = Diagnostics::new();
        diag.record("a", Duration::from_millis(1));
        diag.record("b", Duration::from_millis(2));

        assert!(diag.remove_system("a"));
        assert!(!diag.remove_system("c"));
        assert_eq!(diag.systems.len(), 1);
        assert!(diag.systems.contains_key("b"));
    }

    #[test]
    fn diagnostics_sorted_by_duration() {
        let mut diag = Diagnostics::new();
        diag.record("fast", Duration::from_millis(1));
        diag.record("slow", Duration::from_millis(50));
        diag.record("medium", Duration::from_millis(10));

        let sorted = diag.sorted_by_duration();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].0, "slow");
        assert_eq!(sorted[1].0, "medium");
        assert_eq!(sorted[2].0, "fast");
    }

    #[test]
    fn diagnostics_sorted_by_max_duration() {
        let mut diag = Diagnostics::new();
        diag.record("spike", Duration::from_millis(100));
        diag.record("spike", Duration::from_millis(1));
        diag.record("steady", Duration::from_millis(10));
        diag.record("steady", Duration::from_millis(10));

        let sorted = diag.sorted_by_max_duration();
        assert_eq!(sorted[0].0, "spike");
        assert_eq!(sorted[0].1.max_duration, Duration::from_millis(100));
    }

    #[test]
    fn diagnostics_sorted_empty() {
        let diag = Diagnostics::new();
        assert!(diag.sorted_by_duration().is_empty());
    }

    #[test]
    fn diagnostics_system_count_and_total_calls() {
        let mut diag = Diagnostics::new();
        assert_eq!(diag.system_count(), 0);
        assert_eq!(diag.total_calls(), 0);

        diag.record("a", Duration::from_millis(1));
        diag.record("b", Duration::from_millis(2));
        diag.record("a", Duration::from_millis(3));

        assert_eq!(diag.system_count(), 2);
        assert_eq!(diag.total_calls(), 3);
    }

    #[test]
    fn diagnostics_total_duration() {
        let mut diag = Diagnostics::new();
        assert_eq!(diag.total_duration(), Duration::ZERO);

        diag.record("a", Duration::from_millis(5));
        diag.record("b", Duration::from_millis(10));
        assert_eq!(diag.total_duration(), Duration::from_millis(15));
    }

    #[test]
    fn diagnostics_sorted_by_avg_duration() {
        let mut diag = Diagnostics::new();
        // "steady" averages 10ms (10+10)/2
        diag.record("steady", Duration::from_millis(10));
        diag.record("steady", Duration::from_millis(10));
        // "spiky" averages 50ms (1+99)/2
        diag.record("spiky", Duration::from_millis(1));
        diag.record("spiky", Duration::from_millis(99));

        let sorted = diag.sorted_by_avg_duration();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].0, "spiky");
        assert_eq!(sorted[1].0, "steady");
    }

    #[test]
    fn diagnostics_sorted_by_call_count() {
        let mut diag = Diagnostics::new();
        diag.record("rare", Duration::from_millis(1));
        diag.record("frequent", Duration::from_millis(1));
        diag.record("frequent", Duration::from_millis(1));
        diag.record("frequent", Duration::from_millis(1));

        let sorted = diag.sorted_by_call_count();
        assert_eq!(sorted[0].0, "frequent");
        assert_eq!(sorted[0].1.call_count, 3);
        assert_eq!(sorted[1].0, "rare");
        assert_eq!(sorted[1].1.call_count, 1);
    }

    #[test]
    fn test_diagnostics_snapshot() {
        let mut diag = Diagnostics::new();
        diag.record("b_system", Duration::from_millis(20));
        diag.record("a_system", Duration::from_millis(10));
        diag.record("a_system", Duration::from_millis(30));

        let snap = diag.snapshot();
        assert_eq!(snap.total_calls, 3);
        assert_eq!(snap.total_duration_ns, 60_000_000); // 20 + 10 + 30 = 60ms
        assert_eq!(snap.systems.len(), 2);

        // Assert systems are sorted alphabetically by name
        assert_eq!(snap.systems[0].name, "a_system");
        assert_eq!(snap.systems[0].call_count, 2);
        assert_eq!(snap.systems[0].avg_duration_ns, 20_000_000);

        assert_eq!(snap.systems[1].name, "b_system");
        assert_eq!(snap.systems[1].call_count, 1);
        assert_eq!(snap.systems[1].avg_duration_ns, 20_000_000);
    }
}
