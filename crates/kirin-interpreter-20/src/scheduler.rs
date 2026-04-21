use std::collections::VecDeque;
use std::hash::Hash;

use rustc_hash::FxHashSet;

/// O(1)-dedup FIFO worklist. Items appear at most once in the queue.
pub struct DedupScheduler<T: Hash + Eq> {
    queue: VecDeque<T>,
    set: FxHashSet<T>,
}

impl<T: Hash + Eq + Clone> Default for DedupScheduler<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Hash + Eq + Clone> DedupScheduler<T> {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            set: FxHashSet::default(),
        }
    }

    /// Push an item. Returns `true` if it was actually enqueued (not a dupe).
    pub fn push(&mut self, item: T) -> bool {
        if self.set.contains(&item) {
            return false;
        }
        self.set.insert(item.clone());
        self.queue.push_back(item);
        true
    }

    pub fn pop(&mut self) -> Option<T> {
        let item = self.queue.pop_front()?;
        self.set.remove(&item);
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}
