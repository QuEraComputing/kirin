use std::collections::VecDeque;
use std::hash::Hash;

use rustc_hash::FxHashSet;

/// FIFO scheduler with O(1) membership deduplication.
///
/// All pushes deduplicate: an item already in the queue is silently skipped.
/// Use this for worklist algorithms where duplicate enqueues waste iterations.
#[derive(Debug)]
pub struct DedupScheduler<W>
where
    W: Eq + Hash + Clone,
{
    queue: VecDeque<W>,
    in_queue: FxHashSet<W>,
}

impl<W> Default for DedupScheduler<W>
where
    W: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            in_queue: FxHashSet::default(),
        }
    }
}

impl<W> DedupScheduler<W>
where
    W: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Enqueue work, deduplicating against items already in the queue.
    /// Returns `true` if the item was enqueued, `false` if already present.
    pub fn push_unique(&mut self, work: W) -> bool {
        if !self.in_queue.insert(work.clone()) {
            return false;
        }
        self.queue.push_back(work);
        true
    }

    /// Dequeue the next work item.
    pub fn pop(&mut self) -> Option<W> {
        let item = self.queue.pop_front()?;
        self.in_queue.remove(&item);
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_scheduler() {
        let mut scheduler = DedupScheduler::new();
        assert!(scheduler.push_unique(1usize));
        assert!(!scheduler.push_unique(1usize));
        assert!(scheduler.push_unique(2usize));

        let mut popped = Vec::new();
        while let Some(v) = scheduler.pop() {
            popped.push(v);
        }
        assert_eq!(popped, vec![1, 2]);
    }
}
