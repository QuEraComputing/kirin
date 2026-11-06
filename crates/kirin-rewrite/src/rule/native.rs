use kirin_ir::walk::WalkResult;
use kirin_ir::{Block, Dialect, RewriteError, Rewriter, Statement};

/// A rule written in Rust, with no pattern, that the driver hands each
/// statement it visits.
///
/// Add one with [`RuleSet::add_statement_rule`](crate::RuleSet::add_statement_rule),
/// which takes a recipe building the rule. The driver builds a fresh rule
/// every sweep, so anything the rule stores is gone by the next sweep.
pub trait StatementRule<L: Dialect> {
    /// Try the rule at `stmt`, editing through `rewriter`, and say how the
    /// driver goes on walking.
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        stmt: Statement,
    ) -> Result<WalkResult, RewriteError>;
}

/// A rule written in Rust, with no pattern, that the driver hands each block
/// owned by a statement it visits, such as common-subexpression elimination.
///
/// Add one with [`RuleSet::add_block_rule`](crate::RuleSet::add_block_rule),
/// which takes a recipe building the rule. The driver builds a fresh rule for
/// every block, so anything the rule stores, such as a table of the
/// expressions seen so far, is gone by the next block.
///
/// The rule walks the block itself. It may edit the block's statements,
/// and redirect the uses of the block's values wherever they are, as
/// common-subexpression elimination does. It must not edit the owning
/// statement's other blocks: the driver does not check its edits, and only
/// skips a block that no longer exists when its turn comes.
pub trait BlockRule<L: Dialect> {
    /// Run the rule over `block`, editing through `rewriter`, and say how the
    /// driver goes on walking: `Skip` leaves the statements nested in the
    /// statement owning the block unwalked.
    fn apply(
        &mut self,
        rewriter: &mut Rewriter<'_, L>,
        block: Block,
    ) -> Result<WalkResult, RewriteError>;
}

/// A native rule as a [`RuleSet`](crate::RuleSet) holds it: its name, used in
/// errors, and the recipe building a fresh rule.
pub(crate) struct NativeRule<R: ?Sized> {
    name: String,
    build: Box<dyn Fn() -> Box<R>>,
}

impl<R: ?Sized> NativeRule<R> {
    pub(crate) fn new(name: String, build: Box<dyn Fn() -> Box<R>>) -> Self {
        Self { name, build }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// A fresh rule, built by the recipe.
    pub(crate) fn build(&self) -> Box<R> {
        (self.build)()
    }
}
