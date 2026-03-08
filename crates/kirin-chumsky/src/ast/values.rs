use chumsky::span::SimpleSpan;
use kirin_ir::{Dialect, SSAKind};

use super::Spanned;
use crate::traits::{EmitContext, EmitError, EmitIR};

/// An SSA value reference with optional type annotation.
///
/// Represents syntax like:
/// - `%value` (without type)
/// - `%value: type` (with type)
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::Type as HasParser<'tokens, 'src>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct SSAValue<'src, TypeOutput> {
    /// The name of the SSA value (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The optional type annotation.
    pub ty: Option<TypeOutput>,
}

/// A result value (left-hand side of an SSA assignment).
///
/// Represents syntax like: `%result` in `%result = add %a, %b`
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::Type as HasParser<'tokens, 'src>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultValue<'src, TypeOutput> {
    /// The name of the result value (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The optional type annotation (often inferred).
    pub ty: Option<TypeOutput>,
}

/// The type portion of an SSA value annotation.
///
/// Used when the type is specified separately from the SSA value name,
/// for example in `add %a, %b -> bool` where `bool` is the result type.
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::Type as HasParser<'tokens, 'src>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeofSSAValue<TypeOutput> {
    /// The type value.
    pub ty: TypeOutput,
    /// The span of the type in the source.
    pub span: SimpleSpan,
}

/// The name portion of an SSA value.
///
/// Used when only the name is needed, not the full SSA value with type.
#[derive(Debug, Clone, PartialEq)]
pub struct NameofSSAValue<'src> {
    /// The name of the SSA value (without the `%` prefix).
    pub name: &'src str,
    /// The span of the name in the source.
    pub span: SimpleSpan,
}

/// Implementation of EmitIR for SSAValue AST nodes.
///
/// This looks up the SSA value by name in the emit context's symbol table.
/// The name must have been previously registered (e.g., when emitting a
/// ResultValue or block argument).
impl<'src, TypeOutput, IR> EmitIR<IR> for SSAValue<'src, TypeOutput>
where
    IR: Dialect,
{
    type Output = kirin_ir::SSAValue;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        ctx.lookup_ssa(self.name.value)
            .ok_or_else(|| EmitError::UndefinedSSA(self.name.value.to_string()))
    }
}

/// Implementation of EmitIR for ResultValue AST nodes.
///
/// This creates a new SSA value with the parsed name and registers it
/// in the emit context's symbol table. The created SSA has `SSAKind::BuilderResult`
/// which will be updated when the containing statement is finalized.
///
/// Note: The result index is set to 0 here. For statements with multiple results,
/// the generated code should handle setting the correct indices.
///
/// The `TypeOutput: EmitIR<IR, Output = IR::Type>` bound allows proper type
/// conversion from the parsed type AST to the IR's type lattice via the EmitIR trait.
impl<'src, TypeOutput, IR> EmitIR<IR> for ResultValue<'src, TypeOutput>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
{
    type Output = kirin_ir::ResultValue;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        // Convert the parsed type to Dialect::Type via EmitIR, or use default if no type annotation
        let ty: IR::Type = self
            .ty
            .as_ref()
            .map(|t| t.emit(ctx))
            .transpose()?
            .unwrap_or_default();

        // Create a new SSA value with the parsed name and type
        let ssa = ctx
            .stage
            .ssa()
            .name(self.name.value.to_string())
            .ty(ty)
            .kind(SSAKind::BuilderResult(0))
            .new();

        // Register the SSA in the symbol table for later reference
        ctx.register_ssa(self.name.value.to_string(), ssa);

        Ok(ssa.into())
    }
}
