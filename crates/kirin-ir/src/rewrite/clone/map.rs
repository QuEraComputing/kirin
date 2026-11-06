use rustc_hash::FxHashMap;

use crate::{Block, CFG, SSAValue, Statement};

/// What a copy maps each original id to, like MLIR's `IRMapping`.
///
/// [`Rewriter::clone_statement`](crate::Rewriter::clone_statement) and
/// [`Rewriter::clone_cfg`](crate::Rewriter::clone_cfg) record every value,
/// block and statement they copy. Before a copy, values can be mapped with
/// [`IRMap::map_value`]: an operand inside the copy that reads a mapped value
/// reads its replacement instead, and a block argument that is mapped is not
/// copied. Operands reading other values from outside the copy keep reading
/// the original.
///
/// A map keeps what each copy records, so reusing it makes the next copy read
/// the previous copy's values, and skip the block arguments the previous copy
/// made.
#[derive(Clone, Debug, Default)]
pub struct IRMap {
    values: FxHashMap<SSAValue, SSAValue>,
    blocks: FxHashMap<Block, Block>,
    statements: FxHashMap<Statement, Statement>,
    cfgs: FxHashMap<CFG, CFG>,
}

impl IRMap {
    /// Make the next copies read `new` wherever they would read `old`.
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
}
