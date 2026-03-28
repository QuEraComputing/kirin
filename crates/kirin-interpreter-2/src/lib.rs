pub mod control;
pub mod cursor;
pub mod effect;
mod error;
mod frame;
mod frame_stack;
mod from_constant;
mod interpretable;
pub mod interpreter;
mod lift;
mod machine;
mod product_value;
mod projection;
pub mod result;
mod seed;
mod stage_access;
mod total;
mod value_store;

pub use effect::Cursor;
pub use error::{InterpreterError, MissingEntryError, StageResolutionError};
pub use frame::Frame;
pub use frame_stack::FrameStack;
pub use from_constant::FromConstant;
pub use interpretable::Interpretable;
pub use interpreter::Interpreter;
pub use lift::Lift;
pub use machine::{ConsumeEffect, Machine};
pub use product_value::ProductValue;
pub use projection::{ProjectMachine, ProjectMachineMut};
pub use seed::{Args, BlockSeed};
pub use stage_access::StageAccess;
pub use total::Total;
pub use value_store::ValueStore;

/// Convenience alias for the interpreter's machine effect type.
pub type InterpreterEffect<'ir, I> = <<I as Interpreter<'ir>>::Machine as Machine<'ir>>::Effect;
/// Convenience alias for the interpreter's machine stop type.
pub type InterpreterStop<'ir, I> = <<I as Interpreter<'ir>>::Machine as Machine<'ir>>::Stop;
/// Convenience alias for the interpreter's machine seed type.
pub type InterpreterSeed<'ir, I> = <<I as Interpreter<'ir>>::Machine as Machine<'ir>>::Seed;

/// Essentials for dialect authors implementing machine-based semantics.
pub mod prelude {
    pub use crate::{
        ConsumeEffect, Cursor, FromConstant, Interpretable, Lift, Machine, ProductValue,
        ProjectMachine, ProjectMachineMut, StageAccess, ValueStore, control, effect, interpreter,
        result,
    };
}

#[cfg(test)]
mod tests;
