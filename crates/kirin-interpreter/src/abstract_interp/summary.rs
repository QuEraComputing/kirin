use kirin_ir::Lattice;

use crate::result::AnalysisResult;

/// A single context-sensitive summary entry in the cache.
#[derive(Debug, Clone)]
pub struct SummaryEntry<V> {
    /// Argument abstract values this entry was computed for.
    pub args: Vec<V>,
    /// The analysis result for this context.
    pub result: AnalysisResult<V>,
    /// Whether this entry has been invalidated. Invalidated entries are
    /// skipped during lookup but retained until garbage-collected.
    pub invalidated: bool,
}

impl<V> SummaryEntry<V> {
    /// Create a new non-invalidated summary entry.
    pub fn new(args: Vec<V>, result: AnalysisResult<V>) -> Self {
        Self {
            args,
            result,
            invalidated: false,
        }
    }
}

/// Per-function summary cache supporting multiple call contexts.
///
/// Each function may have:
/// - An optional **fixed** summary that is always returned and never
///   overwritten by analysis.
/// - Zero or more **computed** entries, each for a different call context
///   (argument abstract values). Lookup finds the tightest (most specific)
///   non-invalidated entry whose args subsume the query.
/// - At most one **tentative** entry used during recursive fixpoint iteration.
#[derive(Debug, Clone)]
pub struct SummaryCache<V> {
    /// User-provided fixed summary. Not subject to invalidation.
    fixed: Option<AnalysisResult<V>>,
    /// Computed and seed entries, possibly for multiple call contexts.
    ///
    /// Lookup is a linear scan (`find_best_match`). This is fine for the
    /// expected cardinality (single-digit contexts per function). If profiling
    /// shows this is hot, consider a lattice-height–based index — note that
    /// `BTreeMap` does not apply here because `Vec<V>` under subsumption is a
    /// partial order, not a total order.
    entries: Vec<SummaryEntry<V>>,
    /// Tentative entry during recursive fixpoint (at most one active).
    tentative: Option<SummaryEntry<V>>,
}

impl<V> Default for SummaryCache<V> {
    fn default() -> Self {
        Self {
            fixed: None,
            entries: Vec::new(),
            tentative: None,
        }
    }
}

impl<V> SummaryCache<V> {
    /// Set a fixed (user-provided) summary. Not subject to invalidation.
    pub fn set_fixed(&mut self, result: AnalysisResult<V>) {
        self.fixed = Some(result);
    }

    /// Push a new computed entry.
    pub fn push_entry(&mut self, args: Vec<V>, result: AnalysisResult<V>) {
        self.entries.push(SummaryEntry::new(args, result));
    }

    /// Set the tentative summary for recursive fixpoint.
    pub fn set_tentative(&mut self, args: Vec<V>, result: AnalysisResult<V>) {
        self.tentative = Some(SummaryEntry::new(args, result));
    }

    /// Promote the tentative summary to a computed entry.
    pub fn promote_tentative(&mut self, args: Vec<V>, result: AnalysisResult<V>) {
        self.tentative = None;
        self.push_entry(args, result);
    }

    /// Invalidate all computed entries. Returns the number invalidated.
    pub fn invalidate(&mut self) -> usize {
        let mut count = 0;
        for entry in &mut self.entries {
            if !entry.invalidated {
                entry.invalidated = true;
                count += 1;
            }
        }
        self.tentative = None;
        count
    }

    /// Remove invalidated entries. Returns `true` if the cache is now empty.
    pub fn gc(&mut self) -> bool {
        self.entries.retain(|e| !e.invalidated);
        self.is_empty()
    }

    /// Whether this cache has no entries at all.
    pub fn is_empty(&self) -> bool {
        self.fixed.is_none() && self.entries.is_empty() && self.tentative.is_none()
    }

    /// Get the fixed summary, if any.
    pub fn fixed(&self) -> Option<&AnalysisResult<V>> {
        self.fixed.as_ref()
    }

    /// Iterate over all computed entries.
    pub fn entries(&self) -> impl Iterator<Item = &SummaryEntry<V>> {
        self.entries.iter()
    }

    /// Get the tentative result (for recursive fixpoint), if any.
    pub fn tentative_result(&self) -> Option<&AnalysisResult<V>> {
        self.tentative.as_ref().map(|t| &t.result)
    }
}

impl<V: Lattice + Clone> SummaryCache<V> {
    /// Find the tightest non-invalidated entry whose args subsume `query_args`.
    ///
    /// "Tightest" means: among all matching entries, the one whose args are
    /// pointwise subsumed by every other match (i.e. the most specific).
    pub fn find_best_match(&self, query_args: &[V]) -> Option<&SummaryEntry<V>> {
        let mut best: Option<&SummaryEntry<V>> = None;
        for entry in &self.entries {
            if entry.invalidated {
                continue;
            }
            if entry.args.len() != query_args.len() {
                continue;
            }
            let subsumes = query_args
                .iter()
                .zip(entry.args.iter())
                .all(|(q, cached)| q.is_subseteq(cached));
            if !subsumes {
                continue;
            }
            best = Some(match best {
                None => entry,
                Some(current) => {
                    // Pick the entry with tighter (more specific) args
                    let tighter = entry
                        .args
                        .iter()
                        .zip(current.args.iter())
                        .all(|(e, b)| e.is_subseteq(b));
                    if tighter { entry } else { current }
                }
            });
        }
        best
    }

    /// Look up: returns fixed summary if present, otherwise best match.
    pub fn lookup(&self, query_args: &[V]) -> Option<&AnalysisResult<V>> {
        if let Some(ref fixed) = self.fixed {
            return Some(fixed);
        }
        self.find_best_match(query_args).map(|e| &e.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_interval::Interval;
    use kirin_ir::HasTop;

    #[test]
    fn empty_cache_is_empty() {
        let cache = SummaryCache::<Interval>::default();
        assert!(cache.is_empty());
        assert!(cache.fixed().is_none());
        assert!(cache.tentative_result().is_none());
        assert_eq!(cache.entries().count(), 0);
    }

    #[test]
    fn fixed_summary_always_returned() {
        let mut cache = SummaryCache::<Interval>::default();
        let fixed = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(42)),
        );
        cache.set_fixed(fixed);

        // lookup always returns fixed regardless of query args
        assert_eq!(
            cache.lookup(&[Interval::top()]).unwrap().return_value(),
            Some(&Interval::constant(42))
        );
        assert_eq!(
            cache.lookup(&[]).unwrap().return_value(),
            Some(&Interval::constant(42))
        );
        assert!(!cache.is_empty());
    }

    #[test]
    fn fixed_takes_priority_over_computed() {
        let mut cache = SummaryCache::<Interval>::default();
        let computed = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 10)),
        );
        cache.push_entry(vec![Interval::top()], computed);

        let fixed = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(99)),
        );
        cache.set_fixed(fixed);

        // lookup returns fixed even though computed matches
        assert_eq!(
            cache.lookup(&[Interval::top()]).unwrap().return_value(),
            Some(&Interval::constant(99))
        );
    }

    #[test]
    fn find_best_match_returns_tightest() {
        let mut cache = SummaryCache::<Interval>::default();

        // Wide entry: args = [0, 100]
        let wide = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 100)),
        );
        cache.push_entry(vec![Interval::new(0, 100)], wide);

        // Narrow entry: args = [0, 10]
        let narrow = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 10)),
        );
        cache.push_entry(vec![Interval::new(0, 10)], narrow);

        // Query with [0, 5] — both entries subsume it, but narrow is tighter
        let best = cache.find_best_match(&[Interval::new(0, 5)]).unwrap();
        assert_eq!(best.result.return_value(), Some(&Interval::new(0, 10)));
    }

    #[test]
    fn find_best_match_returns_none_when_not_subsumed() {
        let mut cache = SummaryCache::<Interval>::default();
        let entry = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 10)),
        );
        cache.push_entry(vec![Interval::new(0, 10)], entry);

        // Query with [50, 100] — not subsumed by [0, 10]
        assert!(cache.find_best_match(&[Interval::new(50, 100)]).is_none());
    }

    #[test]
    fn find_best_match_skips_invalidated() {
        let mut cache = SummaryCache::<Interval>::default();
        let entry = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 10)),
        );
        cache.push_entry(vec![Interval::top()], entry);

        cache.invalidate();

        assert!(cache.find_best_match(&[Interval::new(0, 5)]).is_none());
    }

    #[test]
    fn find_best_match_arity_mismatch_skipped() {
        let mut cache = SummaryCache::<Interval>::default();
        let entry = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(1)),
        );
        cache.push_entry(vec![Interval::top(), Interval::top()], entry); // 2 args

        // Query with 1 arg — arity mismatch, should return None
        assert!(cache.find_best_match(&[Interval::top()]).is_none());
    }

    #[test]
    fn invalidate_returns_count() {
        let mut cache = SummaryCache::<Interval>::default();
        let r = AnalysisResult::new(Default::default(), Default::default(), None);
        cache.push_entry(vec![], r.clone());
        cache.push_entry(vec![], r.clone());
        cache.push_entry(vec![], r);

        assert_eq!(cache.invalidate(), 3);
        // Second invalidate returns 0 (already invalidated)
        assert_eq!(cache.invalidate(), 0);
    }

    #[test]
    fn gc_removes_invalidated() {
        let mut cache = SummaryCache::<Interval>::default();
        let r = AnalysisResult::new(Default::default(), Default::default(), None);
        cache.push_entry(vec![], r);

        cache.invalidate();
        assert!(!cache.is_empty()); // entries still present (invalidated)

        cache.gc();
        assert!(cache.is_empty()); // entries removed
    }

    #[test]
    fn gc_keeps_valid_entries() {
        let mut cache = SummaryCache::<Interval>::default();
        let r1 = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(1)),
        );
        let r2 = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(2)),
        );
        cache.push_entry(vec![Interval::new(0, 10)], r1);
        cache.push_entry(vec![Interval::new(20, 30)], r2);

        cache.gc(); // nothing invalidated, so nothing removed
        assert_eq!(cache.entries().count(), 2);
    }

    #[test]
    fn tentative_set_and_promote() {
        let mut cache = SummaryCache::<Interval>::default();
        assert!(cache.tentative_result().is_none());

        let r = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(5)),
        );
        cache.set_tentative(vec![], r);

        assert!(cache.tentative_result().is_some());
        assert_eq!(
            cache.tentative_result().unwrap().return_value(),
            Some(&Interval::constant(5))
        );
        assert!(!cache.is_empty()); // tentative counts

        let promoted = AnalysisResult::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(5)),
        );
        cache.promote_tentative(vec![], promoted);

        assert!(cache.tentative_result().is_none());
        assert_eq!(cache.entries().count(), 1);
    }

    #[test]
    fn invalidate_clears_tentative() {
        let mut cache = SummaryCache::<Interval>::default();
        let r = AnalysisResult::new(Default::default(), Default::default(), None);
        cache.set_tentative(vec![], r);
        assert!(cache.tentative_result().is_some());

        cache.invalidate();
        assert!(cache.tentative_result().is_none());
    }
}
