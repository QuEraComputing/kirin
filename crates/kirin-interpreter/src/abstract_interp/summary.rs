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
        self.entries.push(SummaryEntry {
            args,
            result,
            invalidated: false,
        });
    }

    /// Set the tentative summary for recursive fixpoint.
    pub fn set_tentative(&mut self, args: Vec<V>, result: AnalysisResult<V>) {
        self.tentative = Some(SummaryEntry {
            args,
            result,
            invalidated: false,
        });
    }

    /// Promote the tentative summary to a computed entry.
    pub fn promote_tentative(&mut self, args: Vec<V>, result: AnalysisResult<V>) {
        self.tentative = None;
        self.entries.push(SummaryEntry {
            args,
            result,
            invalidated: false,
        });
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
