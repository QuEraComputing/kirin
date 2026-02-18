use fxhash::FxHashMap;
use kirin_ir::{Block, SSAValue};

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
    pub(crate) fn new(
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
}
