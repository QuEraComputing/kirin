use kirin_ir::Dialect;

use crate::control::{Control, CursorExt};

pub use kirin_ir::{Lift, LiftInto, Project, ProjectInto};

/// Marker trait for cursor types that serve a single dialect at a single stage.
pub trait SingleStageCursorFor<L: Dialect> {}

pub fn lift_cursor_ext<C, Total>(ext: CursorExt<C>) -> CursorExt<Total>
where
    C: Lift<Total>,
{
    match ext {
        CursorExt::Push(c) => CursorExt::Push(c.lift()),
        CursorExt::Pop => CursorExt::Pop,
    }
}

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
