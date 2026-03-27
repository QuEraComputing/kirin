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

pub trait HasRegions<'a> {
    type Iter: Iterator<Item = &'a crate::Region>;
    fn regions(&'a self) -> Self::Iter;
}

pub trait HasRegionsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Region>;
    fn regions_mut(&'a mut self) -> Self::IterMut;
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

/// Structural trait for dialect operations that have a single region body.
///
/// This trait is intentionally not a supertrait of `Dialect` — it applies to
/// individual operations (e.g., `FunctionBody`, `Lambda`) that contain a single
/// `Region`, not to the dialect enum itself.  It enables shared helper functions
/// for interpreter and analysis code that operate on region-bearing operations.
pub trait HasRegionBody {
    fn region(&self) -> &crate::Region;

    fn entry_block<L: Dialect>(&self, stage: &crate::StageInfo<L>) -> Option<crate::Block> {
        self.region().blocks(stage).next()
    }
}

pub trait IsTerminator {
    fn is_terminator(&self) -> bool;
}

pub trait IsConstant {
    fn is_constant(&self) -> bool;
}

pub trait IsPure {
    fn is_pure(&self) -> bool;
}

pub trait IsSpeculatable {
    fn is_speculatable(&self) -> bool;
}

pub trait IsEdge {
    fn is_edge(&self) -> bool;
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
    + for<'a> HasBlocks<'a>
    + for<'a> HasBlocksMut<'a>
    + for<'a> HasSuccessors<'a>
    + for<'a> HasSuccessorsMut<'a>
    + for<'a> HasRegions<'a>
    + for<'a> HasRegionsMut<'a>
    + for<'a> HasDigraphs<'a>
    + for<'a> HasDigraphsMut<'a>
    + for<'a> HasUngraphs<'a>
    + for<'a> HasUngraphsMut<'a>
    + IsTerminator
    + IsConstant
    + IsPure
    + IsSpeculatable
    + IsEdge
    + Clone
    + PartialEq
    + std::fmt::Debug
{
    type Type: CompileTimeValue;
}
