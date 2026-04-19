use crate::control::Control;
use crate::env::Env;

/// Trait for dialect operations that can be evaluated by an interpreter.
///
/// Pure ops (arith, cmp, bitwise, constant) implement this generically:
/// ```ignore
/// impl<E: Env> Interpretable<E> for MyOp { ... }
/// ```
///
/// Mode-specific ops delegate to seam traits on the environment:
/// ```ignore
/// StructuredControlFlow::If(op) => env.eval_if(op),
/// Lexical::Call(op) => env.eval_call(op),
/// ```
pub trait Interpretable<E: Env> {
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
