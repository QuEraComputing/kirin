use chumsky::span::SimpleSpan;
use kirin_ir::{CompileStage, DeclaredName, Dialect, HasStageInfo, Module, Program};

use crate::{ChumskyError, EmitContext, EmitError, ParseEmit, ParseError, parse_ast};

use super::syntax::ModuleText;

/// Parse one `module @name { statement; ... }` into an existing stage.
///
/// Members have independent SSA scopes. Named members must be unique within
/// the module. Use the function text API for definitions with projected
/// names (`{:name}`): standalone module parsing does not register them or
/// preserve names supplied by the function registry.
/// On error the stage is unchanged and the module name remains available.
pub trait ParseModuleText<L: Dialect> {
    fn parse_module(&mut self, stage: CompileStage, input: &str) -> Result<Module, ChumskyError>;
}

impl<L, S> ParseModuleText<L> for Program<S>
where
    L: Dialect + ParseEmit<L> + DeclaredName,
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
        let mut pending = info.clone();
        let module = pending.with_builder(|builder| {
            let module = builder.module().maybe_name(name).new();
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
                let reject = |message: &str| {
                    ChumskyError::Parse(vec![ParseError {
                        message: message.to_string(),
                        span,
                    }])
                };
                let mut ctx = EmitContext::new(builder);
                let member =
                    L::parse_and_emit(&input[span.start..span.end], &mut ctx).map_err(shift)?;
                ctx.stage
                    .try_add_module_member(module, member)
                    .map_err(|msg| reject(&msg))?;
            }
            Ok::<_, ChumskyError>(module)
        })?;
        *info = pending;
        Ok(module)
    }
}
