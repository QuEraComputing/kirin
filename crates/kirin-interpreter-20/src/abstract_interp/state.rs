use kirin_ir::{Block, CompileStage, ResultValue, SSAValue, SpecializedFunction};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;
use std::hash::Hash;

// ---------------------------------------------------------------------------
// AnalysisResult
// ---------------------------------------------------------------------------

/// Result of an abstract interpretation run — per-block entry state and return value.
#[derive(Debug, Clone)]
pub struct AnalysisResult<V: Clone> {
    /// Abstract values at entry of each visited block.
    pub block_in: FxHashMap<Block, Vec<V>>,
    /// All SSA value bindings from the final analysis pass.
    pub ssa_values: FxHashMap<SSAValue, V>,
    /// Joined return value from all Return paths.
    pub return_value: Option<V>,
}

impl<V: Clone> AnalysisResult<V> {
    pub fn ssa_value(&self, ssa: SSAValue) -> Option<&V> {
        self.ssa_values.get(&ssa)
    }

    pub fn return_value(&self) -> Option<&V> {
        self.return_value.as_ref()
    }

    pub fn visited_blocks(&self) -> impl Iterator<Item = &Block> {
        self.block_in.keys()
    }
}

// ---------------------------------------------------------------------------
// O(1) deduplicating worklist
// ---------------------------------------------------------------------------

pub struct Worklist<T: Hash + Eq> {
    queue: VecDeque<T>,
    set: FxHashSet<T>,
}

impl<T: Hash + Eq + Clone> Default for Worklist<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Hash + Eq + Clone> Worklist<T> {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            set: FxHashSet::default(),
        }
    }

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
}

// ---------------------------------------------------------------------------
// Staged function key
// ---------------------------------------------------------------------------

pub type StagedKey = (SpecializedFunction, CompileStage);

// ---------------------------------------------------------------------------
// Intraprocedural state
// ---------------------------------------------------------------------------

pub struct FuncState<V> {
    pub block_in: FxHashMap<Block, Vec<V>>,
    pub visit_counts: FxHashMap<Block, usize>,
    pub block_worklist: Worklist<Block>,
    pub active_ssa: FxHashMap<SSAValue, V>,
}

impl<V> Default for FuncState<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> FuncState<V> {
    pub fn new() -> Self {
        Self {
            block_in: FxHashMap::default(),
            visit_counts: FxHashMap::default(),
            block_worklist: Worklist::new(),
            active_ssa: FxHashMap::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Interprocedural summary
// ---------------------------------------------------------------------------

pub struct FuncSummary<V> {
    pub input: Vec<V>,
    pub output: Option<V>,
    pub entry_block: Block,
}

// ---------------------------------------------------------------------------
// Abstract call frame
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbstractFrame {
    pub func: SpecializedFunction,
    pub stage: CompileStage,
    pub results: Vec<ResultValue>,
}
