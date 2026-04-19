use kirin_ir::Dialect;

use crate::control::{Control, CursorExt};

/// Inject `self` into a composed coproduct type `Total`.
pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

/// Extract a component from a composed coproduct (partial — may fail).
///
/// Returns `Ok(local)` if `self` contains the `Local` variant, or `Err(self)` to return
/// ownership back to the caller when the variant doesn't match.
pub trait Project<Local>: Sized {
    fn try_project(self) -> Result<Local, Self>;
}

/// Ergonomic alias: `self.lift_into()` instead of `Lift::<T>::lift(self)`.
pub trait LiftInto<T>: Sized {
    fn lift_into(self) -> T;
}

/// Ergonomic alias: `self.project_into()` instead of `Project::<T>::try_project(self)`.
pub trait ProjectInto<T>: Sized {
    fn project_into(self) -> Result<T, Self>;
}

// Identity impls: every type lifts/projects to itself trivially.
impl<T> Lift<T> for T {
    fn lift(self) -> T {
        self
    }
}

impl<T> Project<T> for T {
    fn try_project(self) -> Result<T, T> {
        Ok(self)
    }
}

impl<F: Lift<T>, T> LiftInto<T> for F {
    fn lift_into(self) -> T {
        self.lift()
    }
}

impl<F: Project<T>, T> ProjectInto<T> for F {
    fn project_into(self) -> Result<T, Self> {
        self.try_project()
    }
}

/// Marker trait for cursor types that serve a single dialect at a single stage.
///
/// Implementing this on cursor type `C` opts it into the blanket `CallSeam<L>` impl
/// in `kirin-function`. Multi-stage cursor types MUST NOT implement this — they
/// provide their own `CallSeam` impl with cross-stage dispatch logic.
pub trait SingleStageCursorFor<L: Dialect> {}

// ---------------------------------------------------------------------------
// Lift/Project propagate through the effect layer (CursorExt, Control).
//
// Note: Rust coherence prevents providing blanket structural impls alongside
// the identity blanket above (they overlap when C = Total). These are instead
// provided as concrete impls at each use site in dialect code.
// ---------------------------------------------------------------------------

/// Lift a `CursorExt<C>` into `CursorExt<Total>` by lifting the contained cursor.
///
/// Cannot be a blanket impl (coherence conflict with identity). Implement at use sites.
pub fn lift_cursor_ext<C, Total>(ext: CursorExt<C>) -> CursorExt<Total>
where
    C: Lift<Total>,
{
    match ext {
        CursorExt::Push(c) => CursorExt::Push(c.lift()),
        CursorExt::Pop => CursorExt::Pop,
    }
}

/// Project a `CursorExt<Total>` to `CursorExt<Local>` by projecting the inner cursor.
///
/// Cannot be a blanket impl (coherence conflict with identity). Implement at use sites.
pub fn project_cursor_ext<C, Local>(ext: CursorExt<C>) -> Result<CursorExt<Local>, CursorExt<C>>
where
    C: Project<Local>,
{
    match ext {
        CursorExt::Push(c) => match c.try_project() {
            Ok(local) => Ok(CursorExt::Push(local)),
            Err(c) => Err(CursorExt::Push(c)),
        },
        CursorExt::Pop => Ok(CursorExt::Pop),
    }
}

/// Project a `Control<V, Ext>` to `Control<V, Local>` by projecting the Ext.
///
/// Non-Ext variants project freely. Cannot be a blanket impl (coherence conflict).
pub fn project_control<V, Ext, Local>(
    ctrl: Control<V, Ext>,
) -> Result<Control<V, Local>, Control<V, Ext>>
where
    Ext: Project<Local>,
{
    match ctrl {
        Control::Advance => Ok(Control::Advance),
        Control::Return(v) => Ok(Control::Return(v)),
        Control::Yield(v) => Ok(Control::Yield(v)),
        Control::Jump(b, args) => Ok(Control::Jump(b, args)),
        Control::Fork(branches) => Ok(Control::Fork(branches)),
        Control::Call {
            callee,
            stage,
            args,
            results,
        } => Ok(Control::Call {
            callee,
            stage,
            args,
            results,
        }),
        Control::Ext(ext) => match ext.try_project() {
            Ok(local) => Ok(Control::Ext(local)),
            Err(ext) => Err(Control::Ext(ext)),
        },
    }
}
