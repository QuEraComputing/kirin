use crate::control::Control;
use crate::env::Env;

/// Dialect op semantic contract for interpreter-8.
///
/// Analogous to `Interpretable<E>` in interpreter-7, but uses `Env` terminology
/// and the trait is named `Semantics`.
///
/// # Effect types by layer
///
/// - **Pure value ops** (`Arith`, `Cmp`, `Bitwise`, `Constant`): `type Effect = ()`
///   — only side-effect is writing SSA results.
///
/// - **Flat CF ops** (`ControlFlow`): `type Effect = Control<D::Value, D::Ext>`
///   — produces `Jump` or `Fork` but never `Ext(...)`.
///
/// - **SCF ops (concrete)**: `type Effect = Control<D::Value, D::Ext>` where
///   `D::Ext = CursorExt<D::Cursor>` — produces `Ext(Push(cursor))`.
///
/// - **Dialect wrappers**: `type Effect = Control<D::Value, D::Ext>` — converts
///   inner `()` to `Control::Advance`, maps `Control<V, Infallible>` to
///   `Control<V, D::Ext>` via `map_ext(Into::into)`.
pub trait Semantics<D: Env> {
    type Effect: Into<Control<D::Value, D::Ext>>;
    fn eval(&self, domain: &mut D) -> Result<Self::Effect, D::Error>;
}
