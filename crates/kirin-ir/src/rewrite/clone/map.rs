use rustc_hash::FxHashMap;

use crate::{Block, CFG, DiGraph, SSAValue, Statement, UnGraph};

/// What a copy maps each original id to, like MLIR's `IRMapping`.
///
/// [`Rewriter::clone_statement`](crate::Rewriter::clone_statement) and
/// [`Rewriter::clone_cfg`](crate::Rewriter::clone_cfg) record every value,
/// block and statement they copy. Before a copy, [`IRMap::map_value`] can
/// replace a value the copy reads from outside; other outside values are read
/// as they are. Results and block arguments are always copied, overwriting
/// their entries.
///
/// A reused map keeps earlier entries: a statement copied after another reads
/// the other's copied results.
#[derive(Clone, Debug, Default)]
pub struct IRMap {
    values: FxHashMap<SSAValue, SSAValue>,
    blocks: FxHashMap<Block, Block>,
    statements: FxHashMap<Statement, Statement>,
    cfgs: FxHashMap<CFG, CFG>,
    digraphs: FxHashMap<DiGraph, DiGraph>,
    ungraphs: FxHashMap<UnGraph, UnGraph>,
}

impl IRMap {
    /// Make the next copies read `new` in place of `old`, a value from outside
    /// them.
    pub fn map_value(&mut self, old: impl Into<SSAValue>, new: impl Into<SSAValue>) {
        self.values.insert(old.into(), new.into());
    }

    /// The value a copy reads in place of `old`, if any.
    pub fn value(&self, old: impl Into<SSAValue>) -> Option<SSAValue> {
        self.values.get(&old.into()).copied()
    }

    /// The copy of `old`, if a copy made one.
    pub fn block(&self, old: Block) -> Option<Block> {
        self.blocks.get(&old).copied()
    }

    /// The copy of `old`, if a copy made one.
    pub fn statement(&self, old: Statement) -> Option<Statement> {
        self.statements.get(&old).copied()
    }

    /// `value`'s replacement, or `value` itself if it has none.
    pub(super) fn lookup_or_original(&self, value: SSAValue) -> SSAValue {
        self.value(value).unwrap_or(value)
    }

    pub(super) fn map_block(&mut self, old: Block, new: Block) {
        self.blocks.insert(old, new);
    }

    pub(super) fn map_statement(&mut self, old: Statement, new: Statement) {
        self.statements.insert(old, new);
    }

    pub(super) fn map_cfg(&mut self, old: CFG, new: CFG) {
        self.cfgs.insert(old, new);
    }

    pub(super) fn cfg(&self, old: CFG) -> Option<CFG> {
        self.cfgs.get(&old).copied()
    }

    pub(super) fn map_digraph(&mut self, old: DiGraph, new: DiGraph) {
        self.digraphs.insert(old, new);
    }

    pub(super) fn digraph(&self, old: DiGraph) -> Option<DiGraph> {
        self.digraphs.get(&old).copied()
    }

    pub(super) fn map_ungraph(&mut self, old: UnGraph, new: UnGraph) {
        self.ungraphs.insert(old, new);
    }

    pub(super) fn ungraph(&self, old: UnGraph) -> Option<UnGraph> {
        self.ungraphs.get(&old).copied()
    }
}
