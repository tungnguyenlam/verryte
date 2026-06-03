/// A simple resource for tracking game messages.
///
/// By default the log grows without bound. Use [`MessageLog::with_max`] to cap
/// the number of retained messages; when the cap is reached the oldest entries
/// are dropped so that memory stays predictable for long-running sessions.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MessageLog {
    messages: Vec<String>,
    max: Option<usize>,
}

impl MessageLog {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max: None,
        }
    }

    /// Create a message log that retains at most `max` messages.
    ///
    /// When the limit is reached, the oldest messages are trimmed on each
    /// [`push`](Self::push).
    pub fn with_max(max: usize) -> Self {
        Self {
            messages: Vec::new(),
            max: Some(max),
        }
    }

    pub fn push(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
        if let Some(max) = self.max {
            if self.messages.len() > max {
                let excess = self.messages.len() - max;
                self.messages.drain(..excess);
            }
        }
    }

    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    pub fn last(&self) -> Option<&String> {
        self.messages.last()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Return the last N messages.
    pub fn tail(&self, n: usize) -> &[String] {
        let len = self.messages.len();
        let start = len.saturating_sub(n);
        &self.messages[start..]
    }

    /// Extract the last N messages from the log, removing them.
    ///
    /// Returns the removed messages in order.
    pub fn take_tail(&mut self, n: usize) -> Vec<String> {
        let len = self.messages.len();
        let start = len.saturating_sub(n);
        self.messages.drain(start..).collect()
    }

    /// Returns the configured maximum, or `None` for unbounded.
    pub fn max(&self) -> Option<usize> {
        self.max
    }

    /// Returns the current number of retained messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Retain only messages matching a predicate.
    ///
    /// Useful for clearing specific message types (e.g., removing all combat
    /// messages while keeping exploration messages).
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&str) -> bool,
    {
        self.messages.retain(|msg| keep(msg));
    }

    /// Save the message log to a file as JSON.
    #[cfg(feature = "serde")]
    pub fn save_to_file<P>(&self, path: P) -> Result<(), String>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        let s = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize message log: {}", e))?;
        std::fs::write(path, s)
            .map_err(|e| format!("failed to write message log to {:?}: {}", path, e))
    }

    /// Load a message log from a file as JSON.
    #[cfg(feature = "serde")]
    pub fn load_from_file<P>(path: P) -> Result<Self, String>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read message log from {:?}: {}", path, e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("failed to deserialize message log from {:?}: {}", path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbounded_log_grows_freely() {
        let mut log = MessageLog::new();
        for i in 0..100 {
            log.push(format!("msg {i}"));
        }
        assert_eq!(log.len(), 100);
        assert_eq!(log.max(), None);
    }

    #[test]
    fn bounded_log_drops_oldest_when_full() {
        let mut log = MessageLog::with_max(3);
        log.push("first");
        log.push("second");
        log.push("third");
        assert_eq!(log.messages(), &["first", "second", "third"]);

        log.push("fourth");
        assert_eq!(log.messages(), &["second", "third", "fourth"]);
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn bounded_log_with_max_one_keeps_only_latest() {
        let mut log = MessageLog::with_max(1);
        log.push("old");
        log.push("new");
        assert_eq!(log.messages(), &["new"]);
        assert_eq!(log.last().unwrap(), "new");
    }

    #[test]
    fn tail_returns_suffix_of_messages() {
        let mut log = MessageLog::with_max(5);
        for i in 0..5 {
            log.push(format!("m{i}"));
        }
        assert_eq!(log.tail(2), &["m3", "m4"]);
        assert_eq!(log.tail(10), log.messages());
    }

    #[test]
    fn retain_keeps_matching_messages() {
        let mut log = MessageLog::new();
        log.push("combat: hit for 5");
        log.push("explore: entered room");
        log.push("combat: missed");
        log.push("explore: found item");

        log.retain(|msg| msg.starts_with("combat"));
        assert_eq!(log.len(), 2);
        assert_eq!(log.messages(), &["combat: hit for 5", "combat: missed"]);
    }

    #[test]
    fn retain_can_clear_all() {
        let mut log = MessageLog::new();
        log.push("a");
        log.push("b");
        log.retain(|_| false);
        assert!(log.is_empty());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_message_log_file_round_trip() {
        let mut log = MessageLog::with_max(10);
        log.push("first message");
        log.push("second message");

        let path = "test_log.json";
        log.save_to_file(path).unwrap();

        let loaded = MessageLog::load_from_file(path).unwrap();
        assert_eq!(loaded.messages(), log.messages());
        assert_eq!(loaded.max(), log.max());

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn take_tail_removes_and_returns_messages() {
        let mut log = MessageLog::new();
        log.push("msg 1");
        log.push("msg 2");
        log.push("msg 3");

        let tail = log.take_tail(2);
        assert_eq!(tail, vec!["msg 2", "msg 3"]);
        assert_eq!(log.messages(), vec!["msg 1"]);

        let tail2 = log.take_tail(5);
        assert_eq!(tail2, vec!["msg 1"]);
        assert!(log.is_empty());
    }
}
