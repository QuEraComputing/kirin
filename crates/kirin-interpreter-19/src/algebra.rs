use kirin_ir::Dialect;

use crate::control::{Control, CursorExt};

pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

pub trait Project<Local>: Sized {
    fn try_project(self) -> Result<Local, Self>;
}

pub trait LiftInto<T>: Sized {
    fn lift_into(self) -> T;
}

pub trait ProjectInto<T>: Sized {
    fn project_into(self) -> Result<T, Self>;
}

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
