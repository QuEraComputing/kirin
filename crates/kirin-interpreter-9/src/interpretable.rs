use crate::control::Control;
use crate::env::Env;

/// Dialect op semantic contract for interpreter-9.
///
/// Unlike interpreter-8's `Semantics<D>` (which had an associated `Effect`
/// type), `Interpretable<E>` always returns `Control<E::Value, E::Ext>`.
///
/// # Single vs split impls
///
/// - **Pure value ops** (Arith, Cmp, Bitwise, Constant): one generic impl
///   `impl<E: Env> Interpretable<E> for PureOp` — same logic for both modes.
///
/// - **CF ops** (ControlFlow): one generic impl returning `Jump` or `Fork`.
///
/// - **SCF ops** (If, For, Yield): split impls using Mode discriminant:
///   ```rust
///   impl<E: Env<Mode = ConcreteMode<C>>, ...> Interpretable<E> for If<T> { ... }
///   impl<E: Env<Mode = AbstractMode<C>>, ...> Interpretable<E> for If<T> { ... }
///   ```
///   These are coherent because `ConcreteMode<C> ≠ AbstractMode<C>` at the
///   type level (associated type uniqueness).
pub trait Interpretable<E: Env> {
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
