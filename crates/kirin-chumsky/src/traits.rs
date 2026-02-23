//! Core traits for Kirin chumsky parsers

use chumsky::input::Stream;
use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_ir::{Dialect, Pipeline, StageInfo};
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
    type Output: Clone + PartialEq;

    /// Returns a parser for this type.
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>;
}

/// Trait for dialect types that can be parsed with chumsky.
///
/// This trait provides recursive parsing capabilities for dialects.
/// The AST type is parameterized by `TypeOutput` (for type annotations) and
/// `LanguageOutput` (for nested statements in blocks/regions).
///
/// Using explicit type parameters instead of GAT projections avoids infinite
/// compilation times when the Language type is self-referential.
///
/// Note: This trait is implemented by the original dialect type (e.g., `SimpleLang`).
pub trait HasDialectParser<'tokens, 'src: 'tokens>: Sized {
    /// The AST type produced by parsing this dialect.
    ///
    /// - `TypeOutput`: The parsed representation of type annotations
    /// - `LanguageOutput`: The AST type for statements in blocks/regions
    type Output<TypeOutput, LanguageOutput>: Clone + PartialEq
    where
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;

    /// Returns a recursive parser for this dialect.
    ///
    /// The `language` parameter is a recursive parser handle that can be used
    /// to parse nested language constructs (like statements within blocks).
    ///
    /// - `TypeOutput`: The parsed type representation (e.g., from type lattice)
    /// - `LanguageOutput`: The outer language's AST type for recursive parsing
    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: RecursiveParser<'tokens, 'src, I, LanguageOutput>,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'tokens, 'src>,
        TypeOutput: Clone + PartialEq + 'tokens,
        LanguageOutput: Clone + PartialEq + 'tokens;
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

/// Extension trait for parsing a single statement from text.
///
/// Generic over `L` (the dialect) and `Ctx` (extra context needed to locate
/// the target stage). The default `Ctx = ()` applies to `StageInfo<L>` where
/// the receiver *is* the stage, so callers simply write
/// `stage.parse_statement(input)`. For `Pipeline<S>`, `Ctx` is
/// [`CompileStage`](kirin_ir::CompileStage) and callers write
/// `pipeline.parse_statement::<MyLang>(stage_id, input)`.
///
/// # Examples
///
/// ```ignore
/// use kirin_chumsky::prelude::*;
///
/// // StageInfo — dialect inferred, no extra context:
/// let mut stage: StageInfo<MyLang> = StageInfo::default();
/// let stmt = stage.parse_statement("%res = add %a, %b")?;
///
/// // Pipeline — dialect specified via turbofish, stage ID required:
/// let mut pipeline: Pipeline<StageInfo<MyLang>> = Pipeline::new();
/// let id = pipeline.add_stage().stage(StageInfo::default()).new();
/// let stmt = pipeline.parse_statement::<MyLang>(id, "%res = add %a, %b")?;
/// ```
pub trait ParseStatementText<L: Dialect, Ctx = ()> {
    fn parse_statement(
        &mut self,
        ctx: Ctx,
        input: &str,
    ) -> Result<kirin_ir::Statement, Vec<ParseError>>;
}

/// Blanket convenience: when `Ctx = ()`, allow calling without the unit arg.
///
/// This lets `StageInfo<L>` users write `stage.parse_statement(input)` instead
/// of `stage.parse_statement((), input)`.
pub trait ParseStatementTextExt<L: Dialect>: ParseStatementText<L, ()> {
    fn parse_statement(
        &mut self,
        input: &str,
    ) -> Result<kirin_ir::Statement, Vec<ParseError>>;
}

impl<T, L> ParseStatementTextExt<L> for T
where
    L: Dialect,
    T: ParseStatementText<L, ()>,
{
    fn parse_statement(
        &mut self,
        input: &str,
    ) -> Result<kirin_ir::Statement, Vec<ParseError>> {
        <Self as ParseStatementText<L, ()>>::parse_statement(self, (), input)
    }
}

/// Helper: collect pre-existing named SSAs from a stage for emit-context seeding.
fn collect_existing_ssas(stage: &StageInfo<impl Dialect>) -> Vec<(String, kirin_ir::SSAValue)> {
    let symbols = stage.symbol_table();
    stage
        .ssa_arena()
        .iter()
        .filter_map(|ssa| {
            let symbol = ssa.name()?;
            let name = symbols.resolve(symbol)?.clone();
            Some((name, ssa.id()))
        })
        .collect()
}

/// Helper: parse text into IR on a `StageInfo<L>`.
fn parse_statement_on_stage<L>(
    stage: &mut StageInfo<L>,
    input: &str,
) -> Result<kirin_ir::Statement, Vec<ParseError>>
where
    L: Dialect,
    for<'src> L: HasParser<'src, 'src>,
    for<'src> <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = kirin_ir::Statement>,
{
    let ast = parse_ast::<L>(input)?;
    let existing_ssas = collect_existing_ssas(stage);
    let mut emit_ctx = EmitContext::new(stage);
    for (name, ssa) in existing_ssas {
        emit_ctx.register_ssa(name, ssa);
    }
    Ok(ast.emit(&mut emit_ctx))
}

impl<L> ParseStatementText<L> for StageInfo<L>
where
    L: Dialect,
    for<'src> L: HasParser<'src, 'src>,
    for<'src> <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = kirin_ir::Statement>,
{
    fn parse_statement(
        &mut self,
        (): (),
        input: &str,
    ) -> Result<kirin_ir::Statement, Vec<ParseError>> {
        parse_statement_on_stage::<L>(self, input)
    }
}

impl<L, S> ParseStatementText<L, kirin_ir::CompileStage> for Pipeline<S>
where
    L: Dialect,
    S: kirin_ir::HasStageInfo<L>,
    for<'src> L: HasParser<'src, 'src>,
    for<'src> <L as HasParser<'src, 'src>>::Output: EmitIR<L, Output = kirin_ir::Statement>,
{
    fn parse_statement(
        &mut self,
        stage_id: kirin_ir::CompileStage,
        input: &str,
    ) -> Result<kirin_ir::Statement, Vec<ParseError>> {
        let stage_entry = self
            .stage_mut(stage_id)
            .ok_or_else(|| vec![ParseError {
                message: format!("stage {stage_id:?} not found in pipeline"),
                span: SimpleSpan::from(0..0),
            }])?;
        let stage = <S as kirin_ir::HasStageInfo<L>>::try_stage_info_mut(stage_entry)
            .ok_or_else(|| vec![ParseError {
                message: "stage does not contain the requested dialect".to_string(),
                span: SimpleSpan::from(0..0),
            }])?;
        parse_statement_on_stage::<L>(stage, input)
    }
}

/// Context for emitting IR from parsed AST, tracking name mappings.
pub struct EmitContext<'a, L: Dialect> {
    pub stage: &'a mut StageInfo<L>,
    ssa_names: HashMap<String, kirin_ir::SSAValue>,
    block_names: HashMap<String, kirin_ir::Block>,
}

impl<'a, L: Dialect> EmitContext<'a, L> {
    pub fn new(stage: &'a mut StageInfo<L>) -> Self {
        Self {
            stage,
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

/// Marker trait for types that can be directly parsed into themselves.
///
/// This is used to provide identity conversion for types that parse directly
/// into themselves (like type lattice types and compile-time values) without
/// running into coherence issues with blanket implementations.
pub trait DirectlyParsable: Clone {}

/// Blanket implementation of EmitIR for types that implement DirectlyParsable.
///
/// This allows types to emit to themselves (identity conversion),
/// which is useful for type lattices and compile-time value types.
impl<T, L> EmitIR<L> for T
where
    L: Dialect<Type = T>,
    T: DirectlyParsable,
{
    type Output = T;

    fn emit(&self, _ctx: &mut EmitContext<'_, L>) -> Self::Output {
        self.clone()
    }
}

impl<T, L> EmitIR<L> for Vec<T>
where
    L: Dialect,
    T: EmitIR<L>,
{
    type Output = Vec<T::Output>;

    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Self::Output {
        self.iter().map(|item| item.emit(ctx)).collect()
    }
}

impl<T, L> EmitIR<L> for Option<T>
where
    L: Dialect,
    T: EmitIR<L>,
{
    type Output = Option<T::Output>;

    fn emit(&self, ctx: &mut EmitContext<'_, L>) -> Self::Output {
        self.as_ref().map(|item| item.emit(ctx))
    }
}

