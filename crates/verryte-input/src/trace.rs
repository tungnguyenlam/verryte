//! Replayable sequences of sourced actions.

use crate::action::{ActionRecord, ActionSource, QueuedAction};

/// A replayable sequence of sourced actions.
///
/// Traces keep control-plane provenance attached to actions while still
/// replaying through [`InputRouter`](super::InputRouter)'s normal pending
/// queue. This is useful for recording a terminal session, storing an agent
/// plan, or turning a failing test into a reproducible script without creating
/// another action path.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound = "A: serde::Serialize + serde::de::DeserializeOwned")
)]
pub struct ActionTrace<A> {
    steps: Vec<QueuedAction<A>>,
}

impl<A> ActionTrace<A> {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn from_steps<I: IntoIterator<Item = QueuedAction<A>>>(steps: I) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    pub fn push(&mut self, action: A, source: ActionSource) {
        self.steps.push(QueuedAction::new(action, source));
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, QueuedAction<A>> {
        self.steps.iter()
    }

    pub fn steps(&self) -> &[QueuedAction<A>] {
        &self.steps
    }

    pub fn extend(&mut self, other: ActionTrace<A>) {
        self.steps.extend(other.steps);
    }

    pub fn into_steps(self) -> Vec<QueuedAction<A>> {
        self.steps
    }

    /// Create a trace from a list of records.
    pub fn from_records(records: &[ActionRecord<A>]) -> Self
    where
        A: Clone,
    {
        Self {
            steps: records
                .iter()
                .map(|r| QueuedAction::new(r.action.clone(), r.source))
                .collect(),
        }
    }

    /// Create a trace from a history of queued actions.
    pub fn from_history(history: &[QueuedAction<A>]) -> Self
    where
        A: Clone,
    {
        Self {
            steps: history.to_vec(),
        }
    }

    /// Serialize the action trace to a detailed string using a custom
    /// formatter for action values.
    ///
    /// Each step is written on its own line in the format:
    /// `Source:ActionString`.
    pub fn to_detailed_string<F>(&self, mut format_action: F) -> String
    where
        F: FnMut(&A) -> String,
    {
        let mut out = String::new();
        for step in &self.steps {
            out.push_str(&format!(
                "{}:{}\n",
                step.source,
                format_action(&step.action)
            ));
        }
        out
    }

    /// Deserialize an action trace from a detailed string using a custom
    /// action parser.
    ///
    /// The string should have one action per line in the format:
    /// `Source:ActionString`. Empty lines and lines starting with `#` are
    /// ignored as comments.
    pub fn from_detailed_string<F>(s: &str, mut parse_action: F) -> Result<Self, String>
    where
        F: FnMut(&str) -> Option<A>,
    {
        let mut steps = Vec::new();
        for (line_idx, line) in s.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (source_str, action_str) = trimmed.split_once(':').ok_or_else(|| {
                format!("invalid line {} (missing ':'): {:?}", line_idx + 1, trimmed)
            })?;
            let source = source_str
                .trim()
                .parse::<ActionSource>()
                .map_err(|e| format!("invalid source on line {}: {}", line_idx + 1, e))?;
            let action = parse_action(action_str.trim()).ok_or_else(|| {
                format!(
                    "unrecognized action on line {}: {:?}",
                    line_idx + 1,
                    action_str
                )
            })?;
            steps.push(QueuedAction::new(action, source));
        }
        Ok(Self { steps })
    }

    /// Save the action trace to a file on disk.
    pub fn save_to_file<P, F>(&self, path: P, format_action: F) -> Result<(), String>
    where
        P: AsRef<std::path::Path>,
        F: FnMut(&A) -> String,
    {
        let path = path.as_ref();
        let s = self.to_detailed_string(format_action);
        std::fs::write(path, s)
            .map_err(|e| format!("failed to write action trace to {:?}: {}", path, e))
    }

    /// Load an action trace from a file on disk.
    pub fn load_from_file<P, F>(path: P, parse_action: F) -> Result<Self, String>
    where
        P: AsRef<std::path::Path>,
        F: FnMut(&str) -> Option<A>,
    {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read action trace from {:?}: {}", path, e))?;
        Self::from_detailed_string(&content, parse_action)
    }
}

impl<A: Clone> ActionTrace<A> {
    pub fn from_actions<I: IntoIterator<Item = A>>(actions: I, source: ActionSource) -> Self {
        Self {
            steps: actions
                .into_iter()
                .map(|action| QueuedAction::new(action, source))
                .collect(),
        }
    }
}

impl<A> Default for ActionTrace<A> {
    fn default() -> Self {
        Self::new()
    }
}
