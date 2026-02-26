use crate::{Continuation, Interpreter};
use kirin_ir::Dialect;

/// Dialect execution hook. Each dialect implements this to define how its
/// operations affect interpreter state and which continuation happens next.
///
/// Dialect impls construct [`Continuation`] variants directly:
/// `Continuation::Continue`, `Continuation::Jump(block, args)`, etc.
///
/// The `L` parameter identifies the top-level language (dialect enum) that
/// this dialect is composed into. All impls are generic over `L`, so the
/// same dialect can be reused across different languages. Sub-types that
/// need stage information (e.g. `FunctionBody<T>`) use `L` to call
/// [`Interpreter::active_stage_info`] for the correct [`kirin_ir::StageInfo`].
pub trait Interpretable<'ir, I, L: Dialect>: Dialect
where
    I: Interpreter<'ir>,
{
    fn interpret(&self, interpreter: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>;
}
