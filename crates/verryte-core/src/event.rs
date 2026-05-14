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

    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, E> {
        self.queue.iter()
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
}
