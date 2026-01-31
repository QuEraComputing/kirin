//! Core traits for Kirin chumsky parsers

use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_ir::Dialect;
use kirin_lexer::Token;
use std::fmt::Debug;

/// An alias for token input types used in Kirin Chumsky parsers.
///
/// This trait is automatically implemented for any type that implements
/// `chumsky::input::ValueInput` with the appropriate token and span types.
pub trait TokenInput<'tokens, 'src: 'tokens>:
    chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>
{
}

impl<'tokens, 'src: 'tokens, I> TokenInput<'tokens, 'src> for I where
    I: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>
{
}

/// Standard error type for Kirin chumsky parsers.
pub type ParserError<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, SimpleSpan>>;

/// Type alias for a boxed parser.
pub type BoxedParser<'tokens, 'src, I, O> =
    Boxed<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>;

/// Type alias for a recursive parser handle.
pub type RecursiveParser<'tokens, 'src, I, O> =
    Recursive<Direct<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>>;

/// Trait for types that have an associated parser.
///
/// This trait is used for types whose parser does not require recursive parsing,
/// such as type lattices or simple syntax constructs.
///
/// # Example
///
/// ```ignore
/// impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for MyType {
///     type Output = MyTypeAST<'src>;
///     fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
///     where
///         I: TokenInput<'tokens, 'src>,
///     {
///         // ... parser implementation ...
///     }
/// }
/// ```
pub trait HasParser<'tokens, 'src: 'tokens> {
    /// The output type of the parser.
    type Output: Clone + Debug + PartialEq;

    /// Returns a parser for this type.
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>;
}

/// Trait for types that have a recursive parser.
///
/// This trait is used for dialect objects that may contain nested syntax elements
/// like blocks or regions, which require recursive parsing.
///
/// The `Language` type parameter represents the top-level language being parsed,
/// which may be a composition of multiple dialects.
///
/// # Example
///
/// ```ignore
/// impl<'tokens, 'src: 'tokens, L> HasRecursiveParser<'tokens, 'src, L> for MyDialect
/// where
///     L: Dialect + HasRecursiveParser<'tokens, 'src, L>,
///     L::TypeLattice: HasParser<'tokens, 'src>,
/// {
///     type Output = MyDialectAST<'tokens, 'src, L>;
///     fn recursive_parser<I>(
///         language: RecursiveParser<'tokens, 'src, I, L::Output>,
///     ) -> BoxedParser<'tokens, 'src, I, Self::Output>
///     where
///         I: TokenInput<'tokens, 'src>,
///     {
///         // ... parser implementation using language for nested parsing ...
///     }
/// }
/// ```
pub trait HasRecursiveParser<'tokens, 'src: 'tokens, Language> {
    /// The output type of the recursive parser.
    type Output: Clone + Debug + PartialEq;

    /// Returns a recursive parser for this type.
    ///
    /// The `language` parameter is a recursive parser handle that can be used
    /// to parse nested language constructs (like statements within blocks).
    fn recursive_parser<I>(
        language: RecursiveParser<
            'tokens,
            'src,
            I,
            <Language as HasRecursiveParser<'tokens, 'src, Language>>::Output,
        >,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
        Language: HasRecursiveParser<'tokens, 'src, Language>;
}

/// Trait for types that have an associated abstract syntax tree type.
///
/// This trait maps IR types (like `kirin_ir::SSAValue`) to their corresponding
/// AST types used during parsing.
///
/// # Example
///
/// ```ignore
/// impl<'tokens, 'src: 'tokens, L> WithAbstractSyntaxTree<'tokens, 'src, L> for kirin_ir::SSAValue
/// where
///     L: Dialect,
///     L::TypeLattice: HasParser<'tokens, 'src>,
/// {
///     type AbstractSyntaxTreeNode = ast::SSAValue<'tokens, 'src, L>;
/// }
/// ```
pub trait WithAbstractSyntaxTree<'tokens, 'src: 'tokens, Language> {
    /// The AST node type corresponding to this IR type.
    type AbstractSyntaxTreeNode: Debug + Clone;
}

/// Marker trait for a language that can be parsed with chumsky.
///
/// A language is a dialect that:
/// 1. Has a type lattice with a parser (`HasParser`)
/// 2. Has a recursive parser (`HasRecursiveParser`)
///
/// Types implementing this trait automatically get an implementation of `HasParser`
/// that uses `chumsky::recursive` to handle nested parsing.
pub trait LanguageParser<'tokens, 'src: 'tokens>:
    Dialect<TypeLattice: HasParser<'tokens, 'src>> + HasRecursiveParser<'tokens, 'src, Self>
{
}

impl<'tokens, 'src: 'tokens, L> LanguageParser<'tokens, 'src> for L where
    L: Dialect<TypeLattice: HasParser<'tokens, 'src>> + HasRecursiveParser<'tokens, 'src, Self>
{
}

/// Blanket implementation of `HasParser` for types that implement `LanguageParser`.
///
/// This allows using `MyLanguage::parser()` instead of manually setting up
/// the recursive parser.
impl<'tokens, 'src: 'tokens, L> HasParser<'tokens, 'src> for L
where
    L: LanguageParser<'tokens, 'src> + 'tokens,
{
    type Output = <L as HasRecursiveParser<'tokens, 'src, L>>::Output;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        chumsky::recursive::recursive(|language| L::recursive_parser(language)).boxed()
    }
}

// === Implementations for standard library types ===

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for std::marker::PhantomData<T>
where
    'src: 'tokens,
{
    type AbstractSyntaxTreeNode = std::marker::PhantomData<T>;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for Vec<T>
where
    'src: 'tokens,
    T: WithAbstractSyntaxTree<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = Vec<T::AbstractSyntaxTreeNode>;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for Option<T>
where
    'src: 'tokens,
    T: WithAbstractSyntaxTree<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = Option<T::AbstractSyntaxTreeNode>;
}

// === Implementations for primitive types ===

macro_rules! impl_with_ast_identity {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for $ty
            where
                'src: 'tokens,
            {
                type AbstractSyntaxTreeNode = $ty;
            }
        )*
    };
}

impl_with_ast_identity!(u8, u16, u32, u64, u128, usize);
impl_with_ast_identity!(i8, i16, i32, i64, i128, isize);
impl_with_ast_identity!(f32, f64);
impl_with_ast_identity!(bool, char, String);
