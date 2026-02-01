//! Core traits for Kirin chumsky parsers

use chumsky::input::Stream;
use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_ir::{Context, Dialect};
use kirin_lexer::{Logos, Token};
use std::collections::HashMap;
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
/// **Important**: The `Output` type must implement `EmitIR<Language>`, which ensures
/// that parsed AST nodes can be converted to IR. When implementing a custom parser,
/// you must first implement `EmitIR` for your AST type.
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
pub trait HasRecursiveParser<'tokens, 'src: 'tokens, Language: Dialect> {
    /// The output type of the recursive parser.
    ///
    /// This type must implement `EmitIR<Language>` to enable conversion from AST to IR.
    /// The `Language` type must implement `Dialect` when the trait is used.
    type Output: Clone + Debug + PartialEq;

    /// Returns a recursive parser for this type.
    ///
    /// The `language` parameter is a recursive parser handle that can be used
    /// to parse nested language constructs (like statements within blocks).
    ///
    /// The `TypeLattice: HasParser` bound ensures that the language's type lattice
    /// can be parsed. This bound is on the method rather than the trait to allow
    /// implementing `HasRecursiveParser` for types where the `TypeLattice` parser
    /// is only available in certain contexts.
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
        Language: HasRecursiveParser<'tokens, 'src, Language>,
        Language::TypeLattice: HasParser<'tokens, 'src>;
}

/// Marker trait for a language that can be parsed with chumsky.
///
/// A language is a dialect that:
/// 1. Has a type lattice with a parser (`HasParser`) - checked at function level
/// 2. Has a recursive parser (`HasRecursiveParser`)
///
/// Types implementing this trait automatically get an implementation of `HasParser`
/// that uses `chumsky::recursive` to handle nested parsing.
///
/// Note: The `TypeLattice: HasParser` bound is enforced at the function level
/// (in `recursive_parser`) rather than at the trait level to avoid circular
/// trait resolution when using the blanket `impl HasParser for LanguageParser`.
///
/// Note: For roundtrip support, dialects should also derive `PrettyPrint`.
/// Use `#[derive(HasParser, PrettyPrint)]` to get both parser and printer.
pub trait LanguageParser<'tokens, 'src: 'tokens>:
    Dialect + HasRecursiveParser<'tokens, 'src, Self> + Sized
{
}

impl<'tokens, 'src: 'tokens, L> LanguageParser<'tokens, 'src> for L where
    L: Dialect + HasRecursiveParser<'tokens, 'src, Self> + Sized
{
}

// Note: HasParser is now generated directly by the derive macro instead of
// using a blanket impl. This avoids circular trait resolution issues when
// checking TypeLattice: HasParser bounds.

/// A parse error with location information.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// The error message.
    pub message: String,
    /// The span where the error occurred.
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
///
/// This is a convenience function that wraps the common parsing boilerplate:
/// tokenization, stream creation, and error handling. It returns the AST
/// representation without converting to IR.
///
/// For most use cases, prefer [`parse`] which combines parsing and IR emission.
///
/// # Example
///
/// ```ignore
/// use kirin_chumsky::parse_ast;
///
/// // Define your dialect with HasParser and PrettyPrint derives
/// #[derive(Dialect, HasParser, PrettyPrint)]
/// #[kirin(type_lattice = MyType)]
/// #[chumsky(crate = kirin_chumsky)]
/// enum MyLang {
///     #[chumsky(format = "{res:name} = add {lhs} {rhs}")]
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
/// }
///
/// // Parse a string to get the AST
/// let ast = parse_ast::<MyLang>("%x = add %a %b")?;
/// ```
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
///
/// This is a convenience function that combines parsing and IR emission:
/// 1. Tokenizes the input string
/// 2. Parses tokens into an AST
/// 3. Emits IR from the AST
///
/// The function creates a fresh [`EmitContext`] for the emission, which means
/// any SSA values referenced in the input must be defined within the same input.
/// For more control over name resolution (e.g., when parsing multiple statements
/// that reference previously defined SSAs), use [`parse_ast`] and [`EmitContext`]
/// directly.
///
/// # Example
///
/// ```ignore
/// use kirin_chumsky::parse;
/// use kirin_ir::Context;
///
/// // Define your dialect with HasParser and PrettyPrint derives
/// #[derive(Dialect, HasParser, PrettyPrint)]
/// #[kirin(type_lattice = MyType)]
/// #[chumsky(crate = kirin_chumsky)]
/// enum MyLang {
///     #[chumsky(format = "{res:name} = add {lhs} {rhs}")]
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
/// }
///
/// let mut context: Context<MyLang> = Context::default();
/// // Parse and emit IR directly
/// let statement = parse::<MyLang>("%x = add %a %b", &mut context)?;
/// ```
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

// === EmitIR trait and EmitContext ===

/// Context for emitting IR from parsed AST, tracking name mappings.
///
/// This struct maintains symbol tables for SSA values and blocks,
/// allowing name resolution during IR emission. Names are preserved
/// in the generated IR for roundtrip fidelity (e.g., `%x` in source
/// becomes an SSA with `name = Some("x")`).
pub struct EmitContext<'a, L: Dialect> {
    /// The IR context used for building nodes.
    pub context: &'a mut Context<L>,
    /// Maps SSA names (e.g., "x" from "%x") to SSAValue handles.
    ssa_names: HashMap<String, kirin_ir::SSAValue>,
    /// Maps block names (e.g., "bb0" from "^bb0") to Block handles.
    block_names: HashMap<String, kirin_ir::Block>,
}

impl<'a, L: Dialect> EmitContext<'a, L> {
    /// Creates a new emit context wrapping the given IR context.
    pub fn new(context: &'a mut Context<L>) -> Self {
        Self {
            context,
            ssa_names: HashMap::new(),
            block_names: HashMap::new(),
        }
    }

    /// Looks up an SSA value by its name.
    ///
    /// Returns `None` if the name has not been registered.
    pub fn lookup_ssa(&self, name: &str) -> Option<kirin_ir::SSAValue> {
        self.ssa_names.get(name).copied()
    }

    /// Registers an SSA value with the given name.
    ///
    /// This should be called when emitting a result value or block argument
    /// so that subsequent uses of the same name can resolve to the correct handle.
    pub fn register_ssa(&mut self, name: String, ssa: kirin_ir::SSAValue) {
        self.ssa_names.insert(name, ssa);
    }

    /// Looks up a block by its label name.
    ///
    /// Returns `None` if the block has not been registered.
    pub fn lookup_block(&self, name: &str) -> Option<kirin_ir::Block> {
        self.block_names.get(name).copied()
    }

    /// Registers a block with the given label name.
    ///
    /// This should be called when emitting a block so that branch targets
    /// can resolve to the correct block handle.
    pub fn register_block(&mut self, name: String, block: kirin_ir::Block) {
        self.block_names.insert(name, block);
    }
}

/// Trait for emitting IR nodes from parsed AST nodes.
///
/// This trait provides a way to convert parsed AST representations
/// into actual IR nodes using the Context builder methods. It uses
/// the `EmitContext` to track name-to-handle mappings for SSA values
/// and blocks.
///
/// # Example
///
/// ```ignore
/// impl<L> EmitIR<L> for MyDialectAST<'_, '_, L>
/// where
///     L: Dialect + From<MyDialect>,
/// {
///     type Output = kirin_ir::Statement;
///
///     fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Self::Output {
///         match self {
///             MyDialectAST::Add { lhs, rhs, res } => {
///                 let lhs_ir = lhs.emit(ctx);
///                 let rhs_ir = rhs.emit(ctx);
///                 // ... create statement using ctx.context builders ...
///             }
///         }
///     }
/// }
/// ```
pub trait EmitIR<L: Dialect> {
    /// The IR type this AST node emits.
    type Output;

    /// Emit this AST node as IR.
    ///
    /// This method converts the AST node into its corresponding IR representation,
    /// using the `EmitContext` for name resolution and the underlying `Context`
    /// for node creation.
    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Self::Output;
}

/// Emits an AST node as IR using a fresh emit context.
///
/// This is a convenience function that creates an `EmitContext`, emits the AST,
/// and returns the result.
///
/// # Example
///
/// ```ignore
/// use kirin_chumsky::emit;
///
/// let ast = parse::<MyDialect>(source)?;
/// let statement = emit(&ast, &mut context);
/// ```
pub fn emit<L, T>(ast: &T, context: &mut Context<L>) -> T::Output
where
    L: Dialect,
    T: EmitIR<L>,
{
    let mut emit_ctx = EmitContext::new(context);
    ast.emit(&mut emit_ctx)
}
