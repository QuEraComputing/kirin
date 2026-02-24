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
            debug_assert_eq!(self_args.len(), other_args.len());
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
