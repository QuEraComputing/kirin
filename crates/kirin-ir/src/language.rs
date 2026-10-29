use crate::comptime::CompileTimeValue;

// TODO: use Cow<'a, str> for name to avoid allocations in some cases

pub trait HasArguments<'a> {
    type Iter: Iterator<Item = &'a crate::SSAValue>;
    fn arguments(&'a self) -> Self::Iter;
}

pub trait HasArgumentsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasResults<'a> {
    type Iter: Iterator<Item = &'a crate::ResultValue>;
    fn results(&'a self) -> Self::Iter;
}

pub trait HasResultsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::ResultValue>;
    fn results_mut(&'a mut self) -> Self::IterMut;
}

/// Stage-local symbol attributes, in a stable order, including wrapped operations.
///
/// `#[derive(Dialect)]` collects `Symbol`, `Option<Symbol>` and `Vec<Symbol>`
/// fields and delegates through `#[wraps]`. A `Symbol` nested in any other
/// field type, such as a tuple, `Box` or custom attribute struct, is not
/// listed, so cross-stage comparisons see its raw id. Handwritten dialects
/// must expose every symbol attribute here.
pub trait HasSymbols<'a> {
    type Iter: Iterator<Item = &'a crate::Symbol>;
    fn symbols(&'a self) -> Self::Iter;
}

/// Mutable access to the same symbol attributes as [`HasSymbols`], in the
/// same order. This allows cross-stage comparisons to normalize a clone's IDs.
pub trait HasSymbolsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Symbol>;
    fn symbols_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasBlocks<'a> {
    type Iter: Iterator<Item = &'a crate::Block>;
    fn blocks(&'a self) -> Self::Iter;
}

pub trait HasBlocksMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Block>;
    fn blocks_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasSuccessors<'a> {
    type Iter: Iterator<Item = &'a crate::Successor>;
    fn successors(&'a self) -> Self::Iter;
}

pub trait HasSuccessorsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Successor>;
    fn successors_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasCFG<'a> {
    type Iter: Iterator<Item = &'a crate::CFG>;
    fn cfgs(&'a self) -> Self::Iter;
}

pub trait HasCFGMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::CFG>;
    fn cfgs_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasDigraphs<'a> {
    type Iter: Iterator<Item = &'a crate::DiGraph>;
    fn digraphs(&'a self) -> Self::Iter;
}

pub trait HasDigraphsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::DiGraph>;
    fn digraphs_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasUngraphs<'a> {
    type Iter: Iterator<Item = &'a crate::UnGraph>;
    fn ungraphs(&'a self) -> Self::Iter;
}

pub trait HasUngraphsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::UnGraph>;
    fn ungraphs_mut(&'a mut self) -> Self::IterMut;
}

pub trait IsTerminator {
    fn is_terminator(&self) -> bool;
}

pub trait IsConstant {
    fn is_constant(&self) -> bool;
}

/// Strong purity contract used by semantics-informed rewrites.
///
/// `true` means the operation has no observable behavior except through its
/// SSA results and is referentially transparent: equal operation kind,
/// semantic attributes, and operand values produce interchangeable results.
/// Consequently an instance may be erased when all of its results are
/// unobservable and may participate in common-subexpression elimination.
/// Nondeterminism, hidden-state reads, defined errors, and divergence require
/// `false`. Dialect authors are responsible for classifying their operations
/// according to the dialect's semantics.
///
/// This is distinct from [`IsSpeculatable`], which governs whether an
/// operation may be moved or evaluated earlier.
pub trait IsPure {
    fn is_pure(&self) -> bool;
}

pub trait IsSpeculatable {
    fn is_speculatable(&self) -> bool;
}

pub trait IsEdge {
    fn is_edge(&self) -> bool;
}

/// The operands a branch-like statement forwards to each successor's block
/// arguments, after MLIR's `BranchOpInterface`.
///
/// `index` is a successor position in [`HasSuccessors::successors`] order.
/// Both methods return `None` when the statement forwards nothing to that
/// successor in an editable list: it is not a branch, `index` is out of
/// range, or the edge carries no operand list.
///
/// `#[derive(Dialect)]` generates an implementation that forwards through
/// `#[wraps]` variants and returns `None` otherwise. A dialect whose
/// operations forward operands marks itself `#[kirin(branch_op)]` and
/// implements this trait by hand.
pub trait BranchOp {
    fn successor_operands(&self, index: usize) -> Option<&[crate::SSAValue]>;
    fn successor_operands_mut(&mut self, index: usize) -> Option<&mut Vec<crate::SSAValue>>;
}

/// The base trait that all dialects must implement.
///
/// Every dialect carries the full set of IR accessor and property traits as
/// supertraits. Most dialects only use a subset of these (returning empty
/// iterators for the rest), but requiring them all here means downstream code
/// can rely on a single `Dialect` bound instead of enumerating individual
/// capabilities. The `#[derive(Dialect)]` macro generates all required impls
/// automatically, so dialect authors pay no boilerplate cost.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `Dialect`",
    note = "use `#[derive(Dialect)]` to generate the required IR accessor trait implementations"
)]
pub trait Dialect:
    for<'a> HasArguments<'a>
    + for<'a> HasResults<'a>
    + for<'a> HasArgumentsMut<'a>
    + for<'a> HasResultsMut<'a>
    + for<'a> HasSymbols<'a>
    + for<'a> HasSymbolsMut<'a>
    + for<'a> HasBlocks<'a>
    + for<'a> HasBlocksMut<'a>
    + for<'a> HasSuccessors<'a>
    + for<'a> HasSuccessorsMut<'a>
    + for<'a> HasCFG<'a>
    + for<'a> HasCFGMut<'a>
    + for<'a> HasDigraphs<'a>
    + for<'a> HasDigraphsMut<'a>
    + for<'a> HasUngraphs<'a>
    + for<'a> HasUngraphsMut<'a>
    + IsTerminator
    + IsConstant
    + IsPure
    + IsSpeculatable
    + IsEdge
    + BranchOp
    + Clone
    + PartialEq
    + std::fmt::Debug
{
    type Type: CompileTimeValue;
}
