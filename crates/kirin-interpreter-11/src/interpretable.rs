use crate::control::Control;
use crate::env::Env;

/// Trait for dialect operations that can be evaluated by an interpreter.
///
/// Pure ops (arith, cmp, bitwise, constant) implement this generically:
/// ```ignore
/// impl<E: Env> Interpretable<E> for MyOp { ... }
/// ```
///
/// Mode-specific ops (SCF) use split impls bounded on `E::Mode`:
/// ```ignore
/// impl<E: Env<Mode = ConcreteMode<C>>> Interpretable<E> for scf::If<T> { ... }
/// impl<E: Env<Mode = AbstractMode<C>>> Interpretable<E> for scf::If<T> { ... }
/// ```
pub trait Interpretable<E: Env> {
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
