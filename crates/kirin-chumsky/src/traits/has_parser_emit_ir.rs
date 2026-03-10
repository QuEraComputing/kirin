use kirin_ir::{Dialect, Statement};

use super::{EmitContext, EmitError, HasParser};

/// Emits IR directly from a dialect parser's top-level output.
///
/// Public statement and pipeline parsing APIs use this nominal witness trait
/// instead of requiring `for<'t> <L as HasParser<'t>>::Output: EmitIR<L>`.
/// That keeps recursive emission logic on the dialect type itself, which avoids
/// re-proving self-referential associated-type obligations at every call site.
pub trait HasParserEmitIR<'t>: Dialect + HasParser<'t> {
    /// Emits a parsed top-level statement AST into IR for `Self`.
    fn emit_parsed(
        output: &<Self as HasParser<'t>>::Output,
        ctx: &mut EmitContext<'_, Self>,
    ) -> Result<Statement, EmitError>;
}
