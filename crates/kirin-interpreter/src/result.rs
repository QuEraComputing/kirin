use kirin_ir::{Block, Lattice, SSAValue};
use rustc_hash::FxHashMap;

/// Result of an abstract interpretation analysis run.
///
/// Stores a single flat map of all SSA values (both block arguments and
/// statement results), per-block argument SSA value IDs for visited-block
/// tracking, and the joined return value from all `Return` control flow paths.
pub struct AnalysisResult<V> {
    values: FxHashMap<SSAValue, V>,
    /// Per-block argument SSA value IDs (values live in `values`).
    block_args: FxHashMap<Block, Vec<SSAValue>>,
    return_value: Option<V>,
}

impl<V: std::fmt::Debug> std::fmt::Debug for AnalysisResult<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalysisResult")
            .field("values", &self.values)
            .field("block_args", &self.block_args)
            .field("return_value", &self.return_value)
            .finish()
    }
}

impl<V: Clone> Clone for AnalysisResult<V> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            block_args: self.block_args.clone(),
            return_value: self.return_value.clone(),
        }
    }
}

impl<V> AnalysisResult<V> {
    /// Create a bottom analysis result (no values, no return).
    pub fn bottom() -> Self {
        Self {
            values: FxHashMap::default(),
            block_args: FxHashMap::default(),
            return_value: None,
        }
    }

    pub fn new(
        values: FxHashMap<SSAValue, V>,
        block_args: FxHashMap<Block, Vec<SSAValue>>,
        return_value: Option<V>,
    ) -> Self {
        Self {
            values,
            block_args,
            return_value,
        }
    }

    /// Look up the abstract value of an SSA value after analysis.
    pub fn ssa_value(&self, value: SSAValue) -> Option<&V> {
        self.values.get(&value)
    }

    /// Iterate over a block's argument bindings: `(SSAValue, &V)` pairs.
    pub fn block_arg_values(&self, block: Block) -> Option<impl Iterator<Item = (SSAValue, &V)>> {
        let args = self.block_args.get(&block)?;
        Some(
            args.iter()
                .filter_map(|ssa| self.values.get(ssa).map(|v| (*ssa, v))),
        )
    }

    /// Get the list of visited blocks.
    pub fn visited_blocks(&self) -> impl Iterator<Item = &Block> {
        self.block_args.keys()
    }

    /// Get the joined return value from all return paths.
    pub fn return_value(&self) -> Option<&V> {
        self.return_value.as_ref()
    }

    /// Check if this result is subsumed by `other` (i.e. `self ⊑ other`).
    ///
    /// Compares return values and all block argument abstract values pointwise.
    /// A block present in `self` but absent in `other` means `other` has not
    /// yet discovered it, so subsumption fails.
    pub fn is_subseteq(&self, other: &Self) -> bool
    where
        V: Lattice,
    {
        // Check return values
        match (&self.return_value, &other.return_value) {
            (Some(a), Some(b)) if !a.is_subseteq(b) => return false,
            (Some(_), None) => return false,
            _ => {}
        }

        // Check block argument values pointwise
        for (block, self_args) in &self.block_args {
            let Some(other_args) = other.block_args.get(block) else {
                return false;
            };
            assert_eq!(
                self_args.len(),
                other_args.len(),
                "block argument count mismatch in is_subseteq"
            );
            for ssa in self_args {
                match (self.values.get(ssa), other.values.get(ssa)) {
                    (Some(a), Some(b)) if !a.is_subseteq(b) => return false,
                    (Some(_), None) => return false,
                    _ => {}
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_interval::Interval;
    use kirin_ir::{Arena, Block, TestSSAValue};
    use rustc_hash::FxHashMap;

    #[test]
    fn bottom_result_has_no_values() {
        let result = AnalysisResult::<Interval>::bottom();
        assert!(result.return_value().is_none());
        assert_eq!(result.visited_blocks().count(), 0);
    }

    #[test]
    fn ssa_value_returns_none_for_missing() {
        let result = AnalysisResult::<Interval>::bottom();
        let bogus: SSAValue = TestSSAValue(999).into();
        assert!(result.ssa_value(bogus).is_none());
    }

    #[test]
    fn block_arg_values_returns_none_for_unvisited_block() {
        let result = AnalysisResult::<Interval>::bottom();
        let mut arena: Arena<Block, ()> = Arena::default();
        let block = arena.alloc(());
        assert!(result.block_arg_values(block).is_none());
    }

    #[test]
    fn is_subseteq_both_bottom() {
        let a = AnalysisResult::<Interval>::bottom();
        let b = AnalysisResult::<Interval>::bottom();
        assert!(a.is_subseteq(&b));
    }

    #[test]
    fn is_subseteq_reflexive() {
        let result = AnalysisResult::<Interval>::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 10)),
        );
        assert!(result.is_subseteq(&result));
    }

    #[test]
    fn is_subseteq_narrower_return_subsumed() {
        let narrow = AnalysisResult::<Interval>::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(2, 5)),
        );
        let wide = AnalysisResult::<Interval>::new(
            Default::default(),
            Default::default(),
            Some(Interval::new(0, 10)),
        );
        assert!(narrow.is_subseteq(&wide));
        assert!(!wide.is_subseteq(&narrow));
    }

    #[test]
    fn is_subseteq_some_vs_none_return() {
        let with_return = AnalysisResult::<Interval>::new(
            Default::default(),
            Default::default(),
            Some(Interval::constant(1)),
        );
        let without_return = AnalysisResult::<Interval>::bottom();
        // Some(_) is NOT subsumed by None
        assert!(!with_return.is_subseteq(&without_return));
        // None IS subsumed by Some(_)
        assert!(without_return.is_subseteq(&with_return));
    }

    #[test]
    fn clone_preserves_values() {
        let mut values = FxHashMap::default();
        let ssa: SSAValue = TestSSAValue(0).into();
        values.insert(ssa, Interval::constant(42));

        let result = AnalysisResult::new(values, Default::default(), Some(Interval::constant(42)));
        let cloned = result.clone();

        assert_eq!(cloned.ssa_value(ssa), Some(&Interval::constant(42)));
        assert_eq!(cloned.return_value(), Some(&Interval::constant(42)));
    }

    #[test]
    fn visited_blocks_tracks_entries() {
        let mut arena: Arena<Block, ()> = Arena::default();
        let b0 = arena.alloc(());
        let b1 = arena.alloc(());

        let mut block_args = FxHashMap::default();
        block_args.insert(b0, vec![]);
        block_args.insert(b1, vec![]);

        let result = AnalysisResult::<Interval>::new(Default::default(), block_args, None);
        assert_eq!(result.visited_blocks().count(), 2);
    }

    #[test]
    fn block_arg_values_iterates_bindings() {
        let mut arena: Arena<Block, ()> = Arena::default();
        let block = arena.alloc(());

        let ssa0: SSAValue = TestSSAValue(10).into();
        let ssa1: SSAValue = TestSSAValue(11).into();

        let mut values = FxHashMap::default();
        values.insert(ssa0, Interval::constant(100));
        values.insert(ssa1, Interval::constant(200));

        let mut block_args = FxHashMap::default();
        block_args.insert(block, vec![ssa0, ssa1]);

        let result = AnalysisResult::new(values, block_args, None);
        let bindings: Vec<_> = result.block_arg_values(block).unwrap().collect();
        assert_eq!(bindings.len(), 2);
        assert!(bindings.contains(&(ssa0, &Interval::constant(100))));
        assert!(bindings.contains(&(ssa1, &Interval::constant(200))));
    }

    #[test]
    fn is_subseteq_with_block_args() {
        let mut arena: Arena<Block, ()> = Arena::default();
        let block = arena.alloc(());

        let ssa: SSAValue = TestSSAValue(0).into();

        let mut values_narrow = FxHashMap::default();
        values_narrow.insert(ssa, Interval::new(2, 5));
        let mut block_args = FxHashMap::default();
        block_args.insert(block, vec![ssa]);
        let narrow = AnalysisResult::new(values_narrow, block_args, None);

        let mut values_wide = FxHashMap::default();
        values_wide.insert(ssa, Interval::new(0, 10));
        let mut block_args2 = FxHashMap::default();
        block_args2.insert(block, vec![ssa]);
        let wide = AnalysisResult::new(values_wide, block_args2, None);

        assert!(narrow.is_subseteq(&wide));
        assert!(!wide.is_subseteq(&narrow));
    }

    #[test]
    fn is_subseteq_missing_block_in_other() {
        let mut arena: Arena<Block, ()> = Arena::default();
        let block = arena.alloc(());

        let mut block_args = FxHashMap::default();
        block_args.insert(block, vec![]);
        let with_block = AnalysisResult::<Interval>::new(Default::default(), block_args, None);
        let without_block = AnalysisResult::<Interval>::bottom();

        // self has block that other doesn't — not subsumed
        assert!(!with_block.is_subseteq(&without_block));
        // other has block that self doesn't — still subsumed (self is smaller)
        assert!(without_block.is_subseteq(&with_block));
    }
}
