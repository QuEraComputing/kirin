use chumsky::span::SimpleSpan;
use kirin_ir::{BuilderSSAKind, Dialect, GetInfo, Placeholder};

use super::Spanned;
use crate::traits::{EmitContext, EmitError, EmitIR};

/// An SSA value reference with optional type annotation.
///
/// Represents syntax like:
/// - `%value` (without type)
/// - `%value: type` (with type)
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::Type as HasParser<'t>>::Output`.
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
/// `<L::Type as HasParser<'t>>::Output`.
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
/// `<L::Type as HasParser<'t>>::Output`.
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
/// Looks up the SSA value by name. In relaxed dominance mode (graph bodies),
/// creates a forward-reference placeholder if the name is not yet defined.
impl<'src, TypeOutput, IR> EmitIR<IR> for SSAValue<'src, TypeOutput>
where
    IR: Dialect,
{
    type Output = kirin_ir::SSAValue;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        ctx.resolve_ssa(self.name.value)
    }
}

/// Implementation of EmitIR for ResultValue AST nodes.
///
/// If a forward-reference placeholder already exists for this name (created by
/// `SSAValue::emit` in relaxed dominance mode), reuses it — updating its type
/// in place. Otherwise creates a new SSA value.
///
/// The SSA starts with `BuilderSSAKind::Unresolved(ResolutionInfo::Result(0))` which is
/// resolved to `BuilderSSAKind::Result(stmt, idx)` when the statement builder finalizes.
impl<'src, TypeOutput, IR> EmitIR<IR> for ResultValue<'src, TypeOutput>
where
    IR: Dialect,
    IR::Type: kirin_ir::Placeholder,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
{
    type Output = kirin_ir::ResultValue;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        // Convert the parsed type to Dialect::Type via EmitIR, or use placeholder if no type annotation
        let ty: IR::Type = self
            .ty
            .as_ref()
            .map(|t| t.emit(ctx))
            .transpose()?
            .unwrap_or_else(IR::Type::placeholder);

        // Check if a forward-reference placeholder exists for this name
        if let Some(existing) = ctx.lookup_ssa(self.name.value)
            && let Some(info) = existing.get_info_mut(ctx.stage)
            && matches!(
                info.builder_kind(),
                BuilderSSAKind::Unresolved(kirin_ir::ResolutionInfo::Result(_))
            )
        {
            // Reuse the forward-ref SSA — update type in place
            info.set_ty(ty);
            return Ok(existing.into());
        }

        // Create a new SSA value with the parsed name and type
        let ssa = ctx
            .stage
            .ssa()
            .name(self.name.value.to_string())
            .ty(ty)
            .kind(BuilderSSAKind::Unresolved(
                kirin_ir::ResolutionInfo::Result(0),
            ))
            .new();

        // Register the SSA in the symbol table for later reference
        ctx.register_ssa(self.name.value.to_string(), ssa);

        Ok(ssa.into())
    }
}
