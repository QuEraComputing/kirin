use chumsky::span::SimpleSpan;
use kirin_ir::{
    BuilderStageInfo, CompileStage, Dialect, Module, Program, StageInfo, Statement, Symbol,
};

use crate::{ChumskyError, EmitContext, ParseEmit, ParseError, parse_ast};

use super::syntax::ModuleText;

/// Parse one `mod @name { statement; ... }` into a stage.
///
/// Like [`ParseStatementText`](crate::ParseStatementText), `Ctx` locates the
/// stage: `()` on `StageInfo<L>`, [`CompileStage`] on `Program<S>`.
///
/// The module name is stage-local. Members have independent SSA scopes.
/// Use the function text API for definitions with projected names
/// (`{:name}`): standalone module parsing does not register them or preserve
/// names supplied by the function registry. On error no module is created, so
/// the same name can be parsed again; members emitted before the error stay
/// in the stage as detached statements.
///
/// In whole-program text the same module is written `stage @A mod @name
/// { ... }` and parsed by `ParseProgramText`.
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

/// A parsed module whose members are emitted into the stage but which is not
/// created yet.
///
/// Program text holds these until the whole parse succeeds, so a failed parse
/// creates no module. Attach it to the stage it was emitted into.
pub struct PendingModule {
    pub(crate) name: Option<Symbol>,
    /// Span of the written `@name`, relative to the module text.
    pub(crate) name_span: Option<SimpleSpan>,
    members: Vec<Statement>,
}

impl PendingModule {
    /// Check the name, then emit and check every member. On error no module
    /// exists; members emitted before the error stay as detached statements.
    pub fn emit<L>(stage: &mut StageInfo<L>, input: &str) -> Result<Self, ChumskyError>
    where
        L: Dialect + ParseEmit<L>,
    {
        stage.with_builder(|builder| Self::emit_on(builder, input))
    }

    /// Create the module and attach its members.
    pub fn attach<L: Dialect>(self, stage: &mut StageInfo<L>) -> Module {
        stage.with_builder(|builder| self.attach_on(builder))
    }

    fn emit_on<L>(stage: &mut BuilderStageInfo<L>, input: &str) -> Result<Self, ChumskyError>
    where
        L: Dialect + ParseEmit<L>,
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
        let mut members = Vec::with_capacity(ast.members.len());
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
            let member =
                L::parse_and_emit(&input[span.start..span.end], &mut ctx).map_err(shift)?;
            ctx.stage
                .check_module_member(member)
                .map_err(|message| ChumskyError::Parse(vec![ParseError { message, span }]))?;
            members.push(member);
        }
        Ok(Self {
            name,
            name_span: ast.name.map(|name| name.span),
            members,
        })
    }

    fn attach_on<L: Dialect>(self, stage: &mut BuilderStageInfo<L>) -> Module {
        stage
            .module()
            .maybe_name(self.name)
            .members(self.members)
            .new()
    }
}

impl<L> ParseModuleText<L> for StageInfo<L>
where
    L: Dialect + ParseEmit<L>,
{
    fn parse_module(&mut self, (): (), input: &str) -> Result<Module, ChumskyError> {
        self.with_builder(|builder| {
            let pending = PendingModule::emit_on(builder, input)?;
            Ok(pending.attach_on(builder))
        })
    }
}

impl<L, S> ParseModuleText<L, CompileStage> for Program<S>
where
    L: Dialect + ParseEmit<L>,
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
