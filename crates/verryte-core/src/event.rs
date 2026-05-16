//! Single-frame event channels.
//!
//! `Events<E>` is the minimum useful event primitive: systems push, downstream
//! systems read, and the loop driver decides when to clear. Anything richer
//! (double-buffering, multi-reader cursors) can be layered on later without
//! breaking this API.

use std::collections::VecDeque;

pub struct Events<E> {
    queue: VecDeque<E>,
}

impl<E> Events<E> {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// Create an event channel with pre-allocated capacity.
    ///
    /// Useful when the typical per-frame event volume is known, avoiding
    /// repeated reallocations during hot loops.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
        }
    }

    pub fn send(&mut self, event: E) {
        self.queue.push_back(event);
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn drain(&mut self) -> std::collections::vec_deque::Drain<'_, E> {
        self.queue.drain(..)
    }

    /// Consume all pending events and return them as a `Vec`.
    ///
    /// Equivalent to `drain().collect()` but more ergonomic for systems that
    /// want to snapshot the event set without dealing with iterators.
    pub fn take(&mut self) -> Vec<E> {
        self.queue.drain(..).collect()
    }

    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, E> {
        self.queue.iter()
    }

    /// Peek at the oldest pending event without consuming it.
    pub fn peek(&self) -> Option<&E> {
        self.queue.front()
    }

    /// Peek at the most recently added event without consuming it.
    pub fn last(&self) -> Option<&E> {
        self.queue.back()
    }
}

impl<E> Default for Events<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Bump(u32);

    #[test]
    fn send_and_drain() {
        let mut events = Events::<Bump>::new();
        events.send(Bump(1));
        events.send(Bump(2));
        assert_eq!(events.len(), 2);
        let drained: Vec<Bump> = events.drain().collect();
        assert_eq!(drained, vec![Bump(1), Bump(2)]);
        assert!(events.is_empty());
    }

    #[test]
    fn iter_does_not_consume() {
        let mut events = Events::<Bump>::new();
        events.send(Bump(1));
        let seen: Vec<u32> = events.iter().map(|b| b.0).collect();
        assert_eq!(seen, vec![1]);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn peek_returns_oldest_without_consuming() {
        let mut events = Events::<Bump>::new();
        assert_eq!(events.peek(), None);
        events.send(Bump(10));
        events.send(Bump(20));
        assert_eq!(events.peek(), Some(&Bump(10)));
        assert_eq!(events.len(), 2, "peek does not consume");
    }

    #[test]
    fn last_returns_newest_without_consuming() {
        let mut events = Events::<Bump>::new();
        assert_eq!(events.last(), None);
        events.send(Bump(10));
        events.send(Bump(20));
        assert_eq!(events.last(), Some(&Bump(20)));
        assert_eq!(events.len(), 2, "last does not consume");
    }

    #[test]
    fn take_consumes_all_events() {
        let mut events = Events::<Bump>::new();
        events.send(Bump(1));
        events.send(Bump(2));
        events.send(Bump(3));

        let taken = events.take();
        assert_eq!(taken, vec![Bump(1), Bump(2), Bump(3)]);
        assert!(events.is_empty());
    }

    #[test]
    fn with_capacity_preallocates() {
        let events = Events::<Bump>::with_capacity(16);
        assert_eq!(events.len(), 0);
        assert!(events.is_empty());
    }
}
