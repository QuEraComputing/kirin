use chumsky::prelude::*;
use kirin_ir::{Dialect, Pipeline, StageInfo};

use super::emit_ir::EmitContext;
use super::has_parser::ParseError;
use super::has_parser_emit_ir::HasParserEmitIR;

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
    fn parse_statement(&mut self, input: &str) -> Result<kirin_ir::Statement, Vec<ParseError>>;
}

impl<T, L> ParseStatementTextExt<L> for T
where
    L: Dialect,
    T: ParseStatementText<L, ()>,
{
    fn parse_statement(&mut self, input: &str) -> Result<kirin_ir::Statement, Vec<ParseError>> {
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
    for<'t> L: HasParserEmitIR<'t>,
{
    let ast = super::has_parser::parse_ast::<L>(input)?;
    let existing_ssas = collect_existing_ssas(stage);
    let mut emit_ctx = EmitContext::new(stage);
    for (name, ssa) in existing_ssas {
        emit_ctx.register_ssa(name, ssa);
    }
    L::emit_parsed(&ast, &mut emit_ctx).map_err(|e| {
        vec![ParseError {
            message: e.to_string(),
            span: SimpleSpan::from(0..0),
        }]
    })
}

impl<L> ParseStatementText<L> for StageInfo<L>
where
    L: Dialect,
    for<'t> L: HasParserEmitIR<'t>,
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
    for<'t> L: HasParserEmitIR<'t>,
{
    fn parse_statement(
        &mut self,
        stage_id: kirin_ir::CompileStage,
        input: &str,
    ) -> Result<kirin_ir::Statement, Vec<ParseError>> {
        let stage_entry = self.stage_mut(stage_id).ok_or_else(|| {
            vec![ParseError {
                message: format!("stage {stage_id:?} not found in pipeline"),
                span: SimpleSpan::from(0..0),
            }]
        })?;
        let stage =
            <S as kirin_ir::HasStageInfo<L>>::try_stage_info_mut(stage_entry).ok_or_else(|| {
                vec![ParseError {
                    message: "stage does not contain the requested dialect".to_string(),
                    span: SimpleSpan::from(0..0),
                }]
            })?;
        parse_statement_on_stage::<L>(stage, input)
    }
}
