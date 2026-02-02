//! Core traits for Kirin chumsky parsers

use chumsky::input::Stream;
use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_ir::{Context, Dialect};
use kirin_lexer::{Logos, Token};
use std::collections::HashMap;
use std::fmt::Debug;

/// An alias for token input types used in Kirin Chumsky parsers.
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

/// Trait for types that have an associated parser (non-recursive).
///
/// This is used for simple types like type lattices that don't need
/// recursive parsing.
pub trait HasParser<'tokens, 'src: 'tokens> {
    /// The output type of the parser.
    type Output: Clone + Debug + PartialEq;

    /// Returns a parser for this type.
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>;
}

/// Trait for dialect types that can be parsed with chumsky.
///
/// This trait provides recursive parsing capabilities for dialects.
/// The `Language` parameter is the top-level language being parsed,
/// which may be a composition of multiple dialects.
///
/// When a dialect is used standalone, `Language = Self`.
/// When embedded in another dialect, `Language` is the outer dialect.
///
/// Note: This trait is implemented both by the original dialect type
/// (e.g., `SimpleLang`) and by the generated AST type (e.g., `SimpleLangAST`).
/// The AST type doesn't implement `Dialect`, so we don't require `Self: Dialect`.
pub trait HasDialectParser<'tokens, 'src: 'tokens, Language: Dialect>: Sized {
    /// The AST type produced by parsing this dialect.
    type Output: Clone + Debug + PartialEq;

    /// The AST representation for type annotations.
    ///
    /// This avoids the double projection through `TypeLattice + HasParser::Output`.
    /// Typically equals `Language::TypeLattice` (types parse to themselves).
    type TypeAST: Clone + Debug + PartialEq;

    /// Returns a recursive parser for this dialect.
    ///
    /// The `language` parameter is a recursive parser handle that can be used
    /// to parse nested language constructs (like statements within blocks).
    fn recursive_parser<I>(
        language: RecursiveParser<
            'tokens,
            'src,
            I,
            <Language as HasDialectParser<'tokens, 'src, Language>>::Output,
        >,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
        Language: HasDialectParser<'tokens, 'src, Language>;
}

/// A parse error with location information.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: SimpleSpan,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Parses a source string into an AST using the given language's parser.
pub fn parse_ast<'src, L>(input: &'src str) -> Result<L::Output, Vec<ParseError>>
where
    L: HasParser<'src, 'src>,
{
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, SimpleSpan::from(span))
        })
        .collect();

    let eoi = SimpleSpan::from(input.len()..input.len());
    let stream = Stream::from_iter(tokens).map(eoi, |(t, s)| (t, s));
    let result = L::parser().parse(stream);

    match result.into_result() {
        Ok(ast) => Ok(ast),
        Err(errors) => Err(errors
            .into_iter()
            .map(|e| ParseError {
                message: e.to_string(),
                span: *e.span(),
            })
            .collect()),
    }
}

/// Parses a source string and emits IR using the given language's parser.
pub fn parse<'src, L>(
    input: &'src str,
    context: &mut Context<L>,
) -> Result<<L::Output as EmitIR<L>>::Output, Vec<ParseError>>
where
    L: Dialect + HasParser<'src, 'src>,
    L::Output: EmitIR<L>,
{
    let ast = parse_ast::<L>(input)?;
    let mut emit_ctx = EmitContext::new(context);
    Ok(ast.emit(&mut emit_ctx))
}

/// Context for emitting IR from parsed AST, tracking name mappings.
pub struct EmitContext<'a, L: Dialect> {
    pub context: &'a mut Context<L>,
    ssa_names: HashMap<String, kirin_ir::SSAValue>,
    block_names: HashMap<String, kirin_ir::Block>,
}

impl<'a, L: Dialect> EmitContext<'a, L> {
    pub fn new(context: &'a mut Context<L>) -> Self {
        Self {
            context,
            ssa_names: HashMap::new(),
            block_names: HashMap::new(),
        }
    }

    pub fn lookup_ssa(&self, name: &str) -> Option<kirin_ir::SSAValue> {
        self.ssa_names.get(name).copied()
    }

    pub fn register_ssa(&mut self, name: String, ssa: kirin_ir::SSAValue) {
        self.ssa_names.insert(name, ssa);
    }

    pub fn lookup_block(&self, name: &str) -> Option<kirin_ir::Block> {
        self.block_names.get(name).copied()
    }

    pub fn register_block(&mut self, name: String, block: kirin_ir::Block) {
        self.block_names.insert(name, block);
    }
}

/// Trait for emitting IR nodes from parsed AST nodes.
pub trait EmitIR<L: Dialect> {
    type Output;
    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Self::Output;
}

/// Marker trait for types that can be used as type AST and emit to themselves.
///
/// This is used to provide identity conversion for type lattice types
/// without running into coherence issues with blanket implementations.
pub trait TypeLatticeEmit: Clone {}

/// Blanket implementation of EmitIR for types that implement TypeLatticeEmit.
///
/// This allows `TypeAST = TypeLattice` to work without explicit conversion,
/// since the type lattice can emit to itself (identity conversion).
impl<T, L> EmitIR<L> for T
where
    L: Dialect<TypeLattice = T>,
    T: TypeLatticeEmit,
{
    type Output = T;

    fn emit(&self, _ctx: &mut EmitContext<'_, L>) -> Self::Output {
        self.clone()
    }
}

/// Emits an AST node as IR using a fresh emit context.
pub fn emit<L, T>(ast: &T, context: &mut Context<L>) -> T::Output
where
    L: Dialect,
    T: EmitIR<L>,
{
    let mut emit_ctx = EmitContext::new(context);
    ast.emit(&mut emit_ctx)
}
