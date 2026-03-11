use kirin_ir::{Dialect, Statement};

use super::{EmitContext, EmitError, EmitIR, HasParser, ParseError, parse_ast};

/// A dialect that can parse text and emit IR in one step.
///
/// This replaces the old `HasParserEmitIR` + `HasDialectEmitIR` witness traits.
/// Downstream developers implement this trait to plug into `ParseStatementText`
/// and `ParsePipelineText` without needing `#[derive(HasParser)]`.
///
/// # Three implementation paths
///
/// 1. **Derive**: `#[derive(HasParser)]` generates this automatically.
/// 2. **Marker**: Implement `SimpleParseEmit` for non-recursive dialects
///    (no `Block`/`Region` fields) to get a blanket impl for free.
/// 3. **Manual**: Implement directly for full control over parse+emit.
pub trait ParseEmit<L: Dialect = Self>: Dialect {
    /// Parse input text and emit a single IR statement.
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, Vec<ParseError>>;
}

/// Marker trait for dialects whose `HasParser::Output` directly implements `EmitIR`.
///
/// Provides a blanket `ParseEmit` impl. Only works for non-recursive dialects
/// (no `Block`/`Region` fields) — recursive types cause E0275 due to the
/// `for<'t> <L as HasParser<'t>>::Output: EmitIR<L>` bound.
pub trait SimpleParseEmit: Dialect {}

impl<L> ParseEmit<L> for L
where
    L: SimpleParseEmit,
    for<'t> L: HasParser<'t>,
    for<'t> <L as HasParser<'t>>::Output: EmitIR<L, Output = Statement>,
{
    fn parse_and_emit(
        input: &str,
        ctx: &mut EmitContext<'_, L>,
    ) -> Result<Statement, Vec<ParseError>> {
        let ast = parse_ast::<L>(input)?;
        ast.emit(ctx).map_err(|e| {
            vec![ParseError {
                message: e.to_string(),
                span: chumsky::span::SimpleSpan::from(0..0),
            }]
        })
    }
}
