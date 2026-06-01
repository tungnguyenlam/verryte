//! Action source, queued actions, records, and history.

/// Where a queued action came from before entering the shared game-action path.
///
/// This is metadata only: games should still apply the contained action through
/// the same systems no matter who produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ActionSource {
    Terminal,
    Script,
    Agent,
    Replay,
    Test,
}

impl std::fmt::Display for ActionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionSource::Terminal => write!(f, "Terminal"),
            ActionSource::Script => write!(f, "Script"),
            ActionSource::Agent => write!(f, "Agent"),
            ActionSource::Replay => write!(f, "Replay"),
            ActionSource::Test => write!(f, "Test"),
        }
    }
}

impl std::str::FromStr for ActionSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" => Ok(ActionSource::Terminal),
            "script" => Ok(ActionSource::Script),
            "agent" => Ok(ActionSource::Agent),
            "replay" => Ok(ActionSource::Replay),
            "test" => Ok(ActionSource::Test),
            other => Err(format!("unknown ActionSource: {other}")),
        }
    }
}

/// One pending game action plus its control-plane provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct QueuedAction<A> {
    pub action: A,
    pub source: ActionSource,
}

impl<A> QueuedAction<A> {
    pub fn new(action: A, source: ActionSource) -> Self {
        Self { action, source }
    }

    pub fn into_action(self) -> A {
        self.action
    }
}

/// A permanent record of an action that was applied to the game.
///
/// Unlike [`QueuedAction`], which is for pending work, [`ActionRecord`]
/// stores the action alongside its source, a timestamp, and optional
/// metadata (like turn number or phase) for replay and analysis.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct ActionRecord<A> {
    pub action: A,
    pub source: ActionSource,
    pub timestamp: f32,
    pub metadata: std::collections::HashMap<String, String>,
}

impl<A> ActionRecord<A> {
    pub fn new(action: A, source: ActionSource, timestamp: f32) -> Self {
        Self {
            action,
            source,
            timestamp,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// A linear history of all actions applied during a game session.
///
/// This provides the data foundation for replays, undo/redo (if actions are
/// reversible), and agent performance analysis.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct ActionHistory<A> {
    pub records: Vec<ActionRecord<A>>,
}

impl<A> Default for ActionHistory<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A> ActionHistory<A> {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn push(&mut self, record: ActionRecord<A>) {
        self.records.push(record);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Iterate over all records in order.
    pub fn iter(&self) -> std::slice::Iter<'_, ActionRecord<A>> {
        self.records.iter()
    }

    /// Get a record by index.
    pub fn get(&self, index: usize) -> Option<&ActionRecord<A>> {
        self.records.get(index)
    }

    /// Get the most recent record.
    pub fn last(&self) -> Option<&ActionRecord<A>> {
        self.records.last()
    }

    /// Filter records by source, returning matching records in order.
    pub fn by_source(&self, source: ActionSource) -> Vec<&ActionRecord<A>> {
        self.records.iter().filter(|r| r.source == source).collect()
    }

    /// Filter records by a predicate on the action.
    pub fn filter<F>(&self, predicate: F) -> Vec<&ActionRecord<A>>
    where
        F: Fn(&A) -> bool,
    {
        self.records
            .iter()
            .filter(|r| predicate(&r.action))
            .collect()
    }

    /// Get the time range of records (first timestamp, last timestamp).
    pub fn time_range(&self) -> Option<(f32, f32)> {
        let first = self.records.first()?.timestamp;
        let last = self.records.last()?.timestamp;
        Some((first, last))
    }
}

#[cfg(feature = "serde")]
impl<A> ActionHistory<A> {
    /// Save the action history to a file on disk as JSON.
    pub fn save_to_file<P>(&self, path: P) -> Result<(), String>
    where
        P: AsRef<std::path::Path>,
        A: serde::Serialize + serde::de::DeserializeOwned,
    {
        let path = path.as_ref();
        let s = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize action history: {}", e))?;
        std::fs::write(path, s)
            .map_err(|e| format!("failed to write action history to {:?}: {}", path, e))
    }

    /// Load an action history from a file on disk as JSON.
    pub fn load_from_file<P>(path: P) -> Result<Self, String>
    where
        P: AsRef<std::path::Path>,
        A: serde::Serialize + serde::de::DeserializeOwned,
    {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read action history from {:?}: {}", path, e))?;
        serde_json::from_str(&content).map_err(|e| {
            format!(
                "failed to deserialize action history from {:?}: {}",
                path, e
            )
        })
    }
}

/// A buffer that can throttle actions based on per-action cooldowns.
#[derive(Clone, Debug)]
pub struct ActionBuffer<A: Clone + Eq + std::hash::Hash> {
    cooldowns: std::collections::HashMap<A, f32>,
    timers: std::collections::HashMap<A, f32>,
}

impl<A: Clone + Eq + std::hash::Hash> ActionBuffer<A> {
    pub fn new() -> Self {
        Self {
            cooldowns: std::collections::HashMap::new(),
            timers: std::collections::HashMap::new(),
        }
    }

    pub fn set_cooldown(&mut self, action: A, duration: f32) {
        self.cooldowns.insert(action, duration);
    }

    pub fn update(&mut self, dt: f32) {
        for timer in self.timers.values_mut() {
            *timer = (*timer - dt).max(0.0);
        }
    }

    pub fn can_perform(&self, action: &A) -> bool {
        self.timers.get(action).is_none_or(|&t| t <= 0.0)
    }

    pub fn perform(&mut self, action: A) -> bool {
        if self.can_perform(&action) {
            if let Some(&duration) = self.cooldowns.get(&action) {
                self.timers.insert(action, duration);
            }
            true
        } else {
            false
        }
    }

    pub fn remaining_cooldown(&self, action: &A) -> f32 {
        self.timers.get(action).cloned().unwrap_or(0.0)
    }

    pub fn clear_cooldowns(&mut self) {
        self.timers.clear();
    }
}

impl<A: Clone + Eq + std::hash::Hash> Default for ActionBuffer<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestAction {
        Attack,
        Heal,
        Defend,
    }

    #[test]
    fn action_buffer_new_has_no_cooldowns() {
        let buf = ActionBuffer::<TestAction>::new();
        assert!(buf.can_perform(&TestAction::Attack));
        assert_eq!(buf.remaining_cooldown(&TestAction::Attack), 0.0);
    }

    #[test]
    fn action_buffer_perform_sets_cooldown() {
        let mut buf = ActionBuffer::new();
        buf.set_cooldown(TestAction::Attack, 1.0);

        assert!(buf.perform(TestAction::Attack));
        assert!(!buf.can_perform(&TestAction::Attack));
        assert_eq!(buf.remaining_cooldown(&TestAction::Attack), 1.0);
    }

    #[test]
    fn action_buffer_perform_without_cooldown_always_succeeds() {
        let mut buf = ActionBuffer::new();
        // No cooldown set
        assert!(buf.perform(TestAction::Attack));
        assert!(buf.can_perform(&TestAction::Attack));
    }

    #[test]
    fn action_buffer_update_decrements_timers() {
        let mut buf = ActionBuffer::new();
        buf.set_cooldown(TestAction::Attack, 1.0);
        buf.perform(TestAction::Attack);

        buf.update(0.5);
        assert!(!buf.can_perform(&TestAction::Attack));
        assert_eq!(buf.remaining_cooldown(&TestAction::Attack), 0.5);

        buf.update(0.5);
        assert!(buf.can_perform(&TestAction::Attack));
        assert_eq!(buf.remaining_cooldown(&TestAction::Attack), 0.0);
    }

    #[test]
    fn action_buffer_update_clamps_to_zero() {
        let mut buf = ActionBuffer::new();
        buf.set_cooldown(TestAction::Attack, 0.5);
        buf.perform(TestAction::Attack);

        buf.update(10.0); // way more than cooldown
        assert_eq!(buf.remaining_cooldown(&TestAction::Attack), 0.0);
        assert!(buf.can_perform(&TestAction::Attack));
    }

    #[test]
    fn action_buffer_different_actions_independent() {
        let mut buf = ActionBuffer::new();
        buf.set_cooldown(TestAction::Attack, 1.0);
        buf.set_cooldown(TestAction::Heal, 2.0);

        buf.perform(TestAction::Attack);
        buf.perform(TestAction::Heal);

        buf.update(1.0);
        assert!(buf.can_perform(&TestAction::Attack));
        assert!(!buf.can_perform(&TestAction::Heal));

        buf.update(1.0);
        assert!(buf.can_perform(&TestAction::Heal));
    }

    #[test]
    fn action_buffer_clear_cooldowns_resets_all() {
        let mut buf = ActionBuffer::new();
        buf.set_cooldown(TestAction::Attack, 1.0);
        buf.perform(TestAction::Attack);
        assert!(!buf.can_perform(&TestAction::Attack));

        buf.clear_cooldowns();
        assert!(buf.can_perform(&TestAction::Attack));
        assert_eq!(buf.remaining_cooldown(&TestAction::Attack), 0.0);
    }

    #[test]
    fn action_buffer_perform_returns_false_during_cooldown() {
        let mut buf = ActionBuffer::new();
        buf.set_cooldown(TestAction::Attack, 1.0);
        buf.perform(TestAction::Attack);
        assert!(!buf.perform(TestAction::Attack));
    }

    #[test]
    fn action_buffer_remaining_cooldown_unknown_action() {
        let buf = ActionBuffer::<TestAction>::new();
        assert_eq!(buf.remaining_cooldown(&TestAction::Defend), 0.0);
    }

    #[test]
    fn action_buffer_is_default() {
        let buf = ActionBuffer::<i32>::default();
        assert!(buf.can_perform(&1));
    }

    #[test]
    fn queued_action_new_and_into_action() {
        let qa = QueuedAction::new(42, ActionSource::Terminal);
        assert_eq!(qa.action, 42);
        assert_eq!(qa.source, ActionSource::Terminal);
        assert_eq!(qa.into_action(), 42);
    }

    #[test]
    fn action_record_with_metadata() {
        let record = ActionRecord::new(TestAction::Attack, ActionSource::Terminal, 1.5)
            .with_metadata("turn", "3")
            .with_metadata("phase", "combat");
        assert_eq!(record.action, TestAction::Attack);
        assert_eq!(record.source, ActionSource::Terminal);
        assert_eq!(record.timestamp, 1.5);
        assert_eq!(record.metadata.get("turn").unwrap(), "3");
        assert_eq!(record.metadata.get("phase").unwrap(), "combat");
    }

    #[test]
    fn action_history_push_len_is_empty_clear() {
        let mut history = ActionHistory::<i32>::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);

        history.push(ActionRecord::new(1, ActionSource::Terminal, 0.0));
        history.push(ActionRecord::new(2, ActionSource::Script, 1.0));
        assert_eq!(history.len(), 2);
        assert!(!history.is_empty());

        history.clear();
        assert!(history.is_empty());
    }

    #[test]
    fn action_history_iter_and_get() {
        let mut history = ActionHistory::new();
        history.push(ActionRecord::new(10, ActionSource::Terminal, 0.0));
        history.push(ActionRecord::new(20, ActionSource::Script, 1.0));
        history.push(ActionRecord::new(30, ActionSource::Agent, 2.0));

        assert_eq!(history.iter().count(), 3);
        assert_eq!(history.get(0).unwrap().action, 10);
        assert_eq!(history.get(1).unwrap().action, 20);
        assert_eq!(history.get(2).unwrap().action, 30);
        assert!(history.get(3).is_none());

        assert_eq!(history.last().unwrap().action, 30);
    }

    #[test]
    fn action_history_by_source() {
        let mut history = ActionHistory::new();
        history.push(ActionRecord::new(1, ActionSource::Terminal, 0.0));
        history.push(ActionRecord::new(2, ActionSource::Script, 0.1));
        history.push(ActionRecord::new(3, ActionSource::Terminal, 0.2));
        history.push(ActionRecord::new(4, ActionSource::Agent, 0.3));

        let terminal: Vec<_> = history.by_source(ActionSource::Terminal);
        assert_eq!(terminal.len(), 2);
        assert_eq!(terminal[0].action, 1);
        assert_eq!(terminal[1].action, 3);

        let script: Vec<_> = history.by_source(ActionSource::Script);
        assert_eq!(script.len(), 1);
        assert_eq!(script[0].action, 2);
    }

    #[test]
    fn action_history_filter() {
        let mut history = ActionHistory::new();
        history.push(ActionRecord::new(10, ActionSource::Terminal, 0.0));
        history.push(ActionRecord::new(20, ActionSource::Script, 0.1));
        history.push(ActionRecord::new(30, ActionSource::Terminal, 0.2));

        let large: Vec<_> = history.filter(|a| *a > 15);
        assert_eq!(large.len(), 2);
        assert_eq!(large[0].action, 20);
        assert_eq!(large[1].action, 30);
    }

    #[test]
    fn action_history_time_range() {
        let mut history = ActionHistory::new();
        assert!(history.time_range().is_none());

        history.push(ActionRecord::new(1, ActionSource::Terminal, 0.5));
        history.push(ActionRecord::new(2, ActionSource::Script, 2.5));
        history.push(ActionRecord::new(3, ActionSource::Agent, 1.5));

        let (start, end) = history.time_range().unwrap();
        assert_eq!(start, 0.5);
        assert_eq!(end, 1.5);
    }
}
