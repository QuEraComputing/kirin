use chumsky::span::SimpleSpan;
use kirin_ir::{CompileStage, Dialect, HasStageInfo, Module, Program};

use crate::{ChumskyError, EmitContext, EmitError, ParseEmit, ParseError, parse_ast};

use super::syntax::ModuleText;

/// Parse one `module @name { statement; ... }` into an existing stage.
///
/// The name is optional. Members share a fresh SSA scope: later members can
/// use earlier results, but names from other modules are not implicitly in
/// scope. Nested bodies use the dialect's usual parser and scope rules.
///
/// This is the module counterpart of `ParseStatementText`, not a replacement
/// for the function-oriented `ParseProgramText`. It does not register member
/// functions as specializations or resolve module-qualified symbols.
/// Use the function text API for definitions with function context projections
/// such as `{:name}`: those names live in the specialization registry.
/// Like statement parsing, emission errors may leave partially built IR in
/// the stage. Container syntax and duplicate module names are checked before
/// emission; member syntax is checked by the dialect parser during emission.
pub trait ParseModuleText<L: Dialect> {
    fn parse_module(&mut self, stage: CompileStage, input: &str) -> Result<Module, ChumskyError>;
}

impl<L, S> ParseModuleText<L> for Program<S>
where
    L: Dialect + ParseEmit<L>,
    S: HasStageInfo<L>,
{
    fn parse_module(&mut self, stage: CompileStage, input: &str) -> Result<Module, ChumskyError> {
        let ast = parse_ast::<ModuleText<'_>>(input)?;
        let info = self
            .stage(stage)
            .and_then(S::try_stage_info)
            .ok_or_else(|| {
                EmitError::Custom(format!(
                    "stage {stage:?} is unavailable or has a different dialect"
                ))
            })?;
        if let Some(name) = &ast.name
            && self
                .lookup_symbol(name.name)
                .and_then(|name| info.module_by_name(name))
                .is_some()
        {
            return Err(ChumskyError::Parse(vec![ParseError {
                message: format!("duplicate module @{}", name.name),
                span: name.span,
            }]));
        }
        let name = ast.name.map(|name| self.intern(name.name));
        // The stage and dialect were checked above; interning cannot change them.
        let info = S::try_stage_info_mut(self.stage_mut(stage).unwrap()).unwrap();
        info.with_builder(|builder| {
            let module = builder.module().maybe_name(name).new();
            let mut ctx = EmitContext::new(builder);
            for span in ast.members {
                let member =
                    L::parse_and_emit(&input[span.start..span.end], &mut ctx).map_err(|error| {
                        match error {
                            ChumskyError::Parse(mut errors) => {
                                for error in &mut errors {
                                    error.span = SimpleSpan::from(
                                        error.span.start + span.start..error.span.end + span.start,
                                    );
                                }
                                ChumskyError::Parse(errors)
                            }
                            error => error,
                        }
                    })?;
                ctx.stage.add_module_member(module, member);
            }
            Ok(module)
        })
    }
}
