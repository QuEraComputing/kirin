use super::{EmitContext, EmitError, HasDialectParser};

#[doc(hidden)]
/// Witness trait for emitting IR from a dialect's parsed AST output.
///
/// **This trait is an implementation detail of derive-generated code.**
/// Dialect authors should not implement or reference it directly. The
/// `#[derive(HasParser)]` macro generates all required impls automatically.
/// The public parsing API uses [`HasParserEmitIR`] (via `ParseStatementText`
/// and `ParsePipelineText`), which is the only emit-related trait that
/// appears in user-facing where clauses.
///
/// This is a separate trait from [`HasDialectParser`] because it is parameterized
/// by `Language`. This allows each impl to carry dialect-specific bounds at the
/// impl level (e.g., value type `HasParser`/`EmitIR` bounds, `Placeholder` bounds)
/// that cannot be expressed on `HasDialectParser`'s language-agnostic methods.
///
/// # Single lifetime parameter
///
/// This trait uses a single lifetime `'tokens` (with the supertrait
/// `HasDialectParser<'tokens>`) rather than two separate `'tokens, 'src`
/// lifetimes. This keeps wrapper emission compatible with the nominal
/// `HasParserEmitIR` witness used by the public text parsing APIs, and avoids
/// nested associated-type projections under `for<'tokens>` obligations.
/// Since the emit path always uses `'tokens = 'src`, a single lifetime suffices.
///
/// # Why not a method on HasDialectParser?
///
/// `emit_output` was originally attempted as a witness method on `HasDialectParser`
/// (like `clone_output` and `eq_output`), but it failed for two reasons:
///
/// 1. **Value type bounds**: Generic dialects like `Constant<T>` need
///    `<T as HasParser>::Output: EmitIR<Language, Output = T>` — a bound that
///    references both `Language` (a method type param) and `T` (an impl type param).
///    This bound must be on the impl, not the method, because it varies per dialect.
///
/// 2. **Associated type projections**: Wrapper struct delegation resolves
///    `Self::IrType` to a GAT projection like `<If<T> as HasDialectParser>::IrType`,
///    and the compiler cannot prove `Placeholder` for such projections.
///
/// By parameterizing this trait with `Language`, each impl carries all needed
/// bounds at the impl level, and the method's where clause stays minimal.
///
/// # E0275 avoidance
///
/// The outer enum's emission logic uses `W: HasDialectEmitIR<'tokens, Language>`
/// instead of `<W as HasDialectParser>::Output<T, L>: EmitIR<Language>`. This avoids
/// GAT projection bounds that cause E0275 with self-referential AST types.
///
/// Recursive nested statement emission is passed explicitly as a callback rather
/// than encoded in a `LanguageOutput: EmitIR<_>` impl bound. This keeps recursion
/// in normal control flow instead of the trait solver.
pub trait HasDialectEmitIR<'tokens, Language: kirin_ir::Dialect, LanguageOutput>:
    HasDialectParser<'tokens> + kirin_ir::Dialect
where
    LanguageOutput: Clone + PartialEq + 'tokens,
{
    /// Emits a concrete dialect value from parsed output.
    ///
    /// The impl-level where clause carries all dialect-specific bounds
    /// (value type bounds, `Placeholder`, `From<Self>`, etc.).
    /// The method's where clause only requires what varies per call site.
    fn emit_output<TypeOutput, EmitLanguageOutput>(
        output: &<Self as HasDialectParser<'tokens>>::Output<TypeOutput, LanguageOutput>,
        ctx: &mut EmitContext<'_, Language>,
        emit_language_output: &EmitLanguageOutput,
    ) -> Result<Self, EmitError>
    where
        TypeOutput: Clone + PartialEq + 'tokens,
        EmitLanguageOutput: for<'ctx> Fn(
            &LanguageOutput,
            &mut EmitContext<'ctx, Language>,
        ) -> Result<kirin_ir::Statement, EmitError>;
}
