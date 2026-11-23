use chumsky::span::SimpleSpan;
use kirin_ir::{BuilderStageInfo, CompileStage, DeclaredName, Dialect, Module, Program, StageInfo};

use crate::{ChumskyError, EmitContext, ParseEmit, ParseError, parse_ast};

use super::syntax::ModuleText;

/// Parse one `mod @name { statement; ... }` into a stage.
///
/// Like [`ParseStatementText`](crate::ParseStatementText), `Ctx` locates the
/// stage: `()` on `StageInfo<L>`, [`CompileStage`] on `Program<S>`.
///
/// The module name is stage-local. Members have independent SSA scopes, and
/// named members must be unique within the module. Use the function text API
/// for definitions with projected names (`{:name}`): standalone module
/// parsing does not register them or preserve names supplied by the function
/// registry. On error the stage is unchanged.
pub trait ParseModuleText<L: Dialect, Ctx = ()> {
    fn parse_module(&mut self, ctx: Ctx, input: &str) -> Result<Module, ChumskyError>;
}

/// Blanket convenience: when `Ctx = ()`, allow calling without the unit arg.
pub trait ParseModuleTextExt<L: Dialect>: ParseModuleText<L, ()> {
    fn parse_module(&mut self, input: &str) -> Result<Module, ChumskyError>;
}

impl<T, L> ParseModuleTextExt<L> for T
where
    L: Dialect,
    T: ParseModuleText<L, ()>,
{
    fn parse_module(&mut self, input: &str) -> Result<Module, ChumskyError> {
        <Self as ParseModuleText<L, ()>>::parse_module(self, (), input)
    }
}

/// Helper: parse module text into IR on a `BuilderStageInfo<L>`.
fn parse_module_on_builder_stage<L>(
    stage: &mut BuilderStageInfo<L>,
    input: &str,
) -> Result<Module, ChumskyError>
where
    L: Dialect + ParseEmit<L> + DeclaredName,
{
    let ast = parse_ast::<ModuleText<'_>>(input)?;
    let name = ast
        .name
        .as_ref()
        .map(|name| stage.symbol_table_mut().intern(name.name.into()));
    if let (Some(name), Some(written)) = (name, &ast.name)
        && stage.module_by_name(name).is_some()
    {
        return Err(ChumskyError::Parse(vec![ParseError {
            message: format!("duplicate module @{}", written.name),
            span: written.span,
        }]));
    }
    let module = stage.module().maybe_name(name).new();
    for span in ast.members {
        let shift = |error: ChumskyError| match error {
            ChumskyError::Parse(mut errors) => {
                for error in &mut errors {
                    error.span = SimpleSpan::from(
                        error.span.start + span.start..error.span.end + span.start,
                    );
                }
                ChumskyError::Parse(errors)
            }
            error => error,
        };
        let mut ctx = EmitContext::new(stage);
        let member = L::parse_and_emit(&input[span.start..span.end], &mut ctx).map_err(shift)?;
        ctx.stage
            .try_add_module_member(module, member)
            .map_err(|message| ChumskyError::Parse(vec![ParseError { message, span }]))?;
    }
    Ok(module)
}

impl<L> ParseModuleText<L> for StageInfo<L>
where
    L: Dialect + ParseEmit<L> + DeclaredName,
{
    fn parse_module(&mut self, (): (), input: &str) -> Result<Module, ChumskyError> {
        // Build on a copy so a failed parse leaves no partial module behind.
        let mut pending = self.clone();
        let module =
            pending.with_builder(|builder| parse_module_on_builder_stage::<L>(builder, input))?;
        *self = pending;
        Ok(module)
    }
}

impl<L, S> ParseModuleText<L, CompileStage> for Program<S>
where
    L: Dialect + ParseEmit<L> + DeclaredName,
    S: kirin_ir::HasStageInfo<L>,
{
    fn parse_module(
        &mut self,
        stage_id: CompileStage,
        input: &str,
    ) -> Result<Module, ChumskyError> {
        let stage = self
            .stage_mut(stage_id)
            .and_then(S::try_stage_info_mut)
            .ok_or_else(|| {
                ChumskyError::Parse(vec![ParseError {
                    message: format!(
                        "stage {stage_id:?} is unavailable or has a different dialect"
                    ),
                    span: SimpleSpan::from(0..0),
                }])
            })?;
        <StageInfo<L> as ParseModuleText<L>>::parse_module(stage, (), input)
    }
}
