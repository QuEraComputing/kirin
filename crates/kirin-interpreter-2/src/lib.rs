pub mod control;
mod cursor;
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
pub use projection::{LiftEffect, LiftStop, ProjectMachine, ProjectMachineMut};
pub use seed::{BlockSeed, DiGraphSeed, ExecutionSeed, RegionSeed, UnGraphSeed};
pub use stage_access::StageAccess;
pub use value_store::ValueStore;

/// Essentials for dialect authors implementing machine-based semantics.
pub mod prelude {
    pub use crate::{
        ConsumeEffect, Cursor, FromConstant, Interpretable, Lift, LiftEffect, LiftStop, Machine,
        ProductValue, ProjectMachine, ProjectMachineMut, StageAccess, ValueStore, control, effect,
        interpreter, result,
    };
}

#[cfg(test)]
mod tests;
