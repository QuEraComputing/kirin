//! Abstract Syntax Tree types for Kirin chumsky parsers.
//!
//! These types represent the parsed syntax elements before they are
//! converted to the IR representation.

use chumsky::span::SimpleSpan;
use kirin_ir::Dialect;

use crate::traits::{HasParser, HasRecursiveParser, LanguageParser, WithAbstractSyntaxTree};

/// A value with an associated span.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: SimpleSpan,
}

impl<T: Copy> Copy for Spanned<T> {}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> Spanned<T> {
    /// Creates a new spanned value.
    pub fn new(value: T, span: SimpleSpan) -> Self {
        Self { value, span }
    }

    /// Maps the inner value using the provided function.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }
}

/// An SSA value reference with optional type annotation.
///
/// Represents syntax like:
/// - `%value` (without type)
/// - `%value: type` (with type)
#[derive(Debug)]
pub struct SSAValue<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The name of the SSA value (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The optional type annotation.
    pub ty: Option<<L::TypeLattice as HasParser<'tokens, 'src>>::Output>,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for SSAValue<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for SSAValue<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            ty: self.ty.clone(),
        }
    }
}

/// A result value (left-hand side of an SSA assignment).
///
/// Represents syntax like: `%result` in `%result = add %a, %b`
#[derive(Debug)]
pub struct ResultValue<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The name of the result value (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The optional type annotation (often inferred).
    pub ty: Option<<L::TypeLattice as HasParser<'tokens, 'src>>::Output>,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for ResultValue<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for ResultValue<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            ty: self.ty.clone(),
        }
    }
}

/// The type portion of an SSA value annotation.
///
/// Used when the type is specified separately from the SSA value name,
/// for example in `add %a, %b -> bool` where `bool` is the result type.
#[derive(Debug)]
pub struct TypeofSSAValue<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The type value.
    pub ty: <L::TypeLattice as HasParser<'tokens, 'src>>::Output,
    /// The span of the type in the source.
    pub span: SimpleSpan,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for TypeofSSAValue<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for TypeofSSAValue<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            ty: self.ty.clone(),
            span: self.span,
        }
    }
}

/// The name portion of an SSA value.
///
/// Used when only the name is needed, not the full SSA value with type.
#[derive(Debug, Clone, PartialEq)]
pub struct NameofSSAValue<'src> {
    /// The name of the SSA value (without the `%` prefix).
    pub name: &'src str,
    /// The span of the name in the source.
    pub span: SimpleSpan,
}

/// A block label.
///
/// Represents syntax like: `^bb0`
#[derive(Debug, Clone, PartialEq)]
pub struct BlockLabel<'src> {
    /// The name of the block (without the `^` prefix).
    pub name: Spanned<&'src str>,
}

/// A block argument.
///
/// Represents syntax like: `%arg: i32`
#[derive(Debug)]
pub struct BlockArgument<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The name of the argument (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The type of the argument.
    pub ty: Spanned<<L::TypeLattice as HasParser<'tokens, 'src>>::Output>,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for BlockArgument<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for BlockArgument<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            ty: self.ty.clone(),
        }
    }
}

/// A block header containing the label and arguments.
///
/// Represents syntax like: `^bb0(%arg0: i32, %arg1: f64)`
#[derive(Debug)]
pub struct BlockHeader<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The block label.
    pub label: BlockLabel<'src>,
    /// The block arguments.
    pub arguments: Vec<Spanned<BlockArgument<'tokens, 'src, L>>>,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for BlockHeader<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && self.arguments == other.arguments
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for BlockHeader<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            arguments: self.arguments.clone(),
        }
    }
}

/// A basic block containing a header and statements.
///
/// Represents syntax like:
/// ```ignore
/// ^bb0(%arg: i32) {
///     %x = add %arg, %arg;
///     return %x;
/// }
/// ```
#[derive(Debug)]
pub struct Block<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The block header with label and arguments.
    pub header: Spanned<BlockHeader<'tokens, 'src, L>>,
    /// The statements in the block.
    pub statements: Vec<Spanned<<L as HasRecursiveParser<'tokens, 'src, L>>::Output>>,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for Block<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
    <L as HasRecursiveParser<'tokens, 'src, L>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header && self.statements == other.statements
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for Block<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            statements: self.statements.clone(),
        }
    }
}

/// A region containing multiple blocks.
///
/// Represents syntax like:
/// ```ignore
/// {
///     ^entry(%arg: i32) { ... };
///     ^bb1() { ... };
/// }
/// ```
#[derive(Debug)]
pub struct Region<'tokens, 'src: 'tokens, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    /// The blocks in the region.
    pub blocks: Vec<Spanned<Block<'tokens, 'src, L>>>,
}

impl<'tokens, 'src: 'tokens, L> PartialEq for Region<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
    <L::TypeLattice as HasParser<'tokens, 'src>>::Output: PartialEq,
    <L as HasRecursiveParser<'tokens, 'src, L>>::Output: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.blocks == other.blocks
    }
}

impl<'tokens, 'src: 'tokens, L> Clone for Region<'tokens, 'src, L>
where
    L: LanguageParser<'tokens, 'src>,
{
    fn clone(&self) -> Self {
        Self {
            blocks: self.blocks.clone(),
        }
    }
}

/// A function type signature.
///
/// Represents syntax like: `(i32, f64) -> (bool, i32)`
#[derive(Debug, Clone)]
pub struct FunctionType<T> {
    /// The input parameter types.
    pub input_types: Vec<Spanned<T>>,
    /// The output return types.
    pub output_types: Vec<Spanned<T>>,
}

impl<T: PartialEq> PartialEq for FunctionType<T> {
    fn eq(&self, other: &Self) -> bool {
        self.input_types == other.input_types && self.output_types == other.output_types
    }
}

// === WithAbstractSyntaxTree implementations for kirin_ir types ===

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for kirin_ir::SSAValue
where
    'src: 'tokens,
    L: Dialect + LanguageParser<'tokens, 'src>,
{
    type AbstractSyntaxTreeNode = SSAValue<'tokens, 'src, L>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for kirin_ir::ResultValue
where
    'src: 'tokens,
    L: Dialect + LanguageParser<'tokens, 'src>,
{
    type AbstractSyntaxTreeNode = ResultValue<'tokens, 'src, L>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for kirin_ir::Block
where
    'src: 'tokens,
    L: Dialect + LanguageParser<'tokens, 'src>,
{
    type AbstractSyntaxTreeNode = Block<'tokens, 'src, L>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for kirin_ir::Successor
where
    'src: 'tokens,
    L: Dialect + LanguageParser<'tokens, 'src>,
{
    type AbstractSyntaxTreeNode = BlockLabel<'src>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for kirin_ir::Region
where
    'src: 'tokens,
    L: Dialect + LanguageParser<'tokens, 'src>,
{
    type AbstractSyntaxTreeNode = Region<'tokens, 'src, L>;
}
