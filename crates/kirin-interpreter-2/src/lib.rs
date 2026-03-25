mod breakpoint_control;
mod control;
mod cursor;
mod error;
mod frame;
mod frame_stack;
mod fuel_control;
mod interpretable;
mod interpreter;
mod interrupt_control;
mod machine;
mod projection;
mod result;
mod seed;
mod single_stage;
mod stage_access;
mod value_store;

pub use breakpoint_control::{Breakpoint, BreakpointControl, ExecutionLocation};
pub use control::Control;
pub use error::{InterpreterError, MissingEntryError, StageResolutionError};
pub use frame::Frame;
pub use frame_stack::FrameStack;
pub use fuel_control::FuelControl;
pub use interpretable::Interpretable;
pub use interpreter::Interpreter;
pub use interrupt_control::InterruptControl;
pub use machine::{ConsumeEffect, Machine};
pub use projection::{LiftEffect, LiftStop, ProjectMachine, ProjectMachineMut};
pub use result::{RunResult, StepOutcome, StepResult, SuspendReason};
pub use seed::{BlockSeed, DiGraphSeed, ExecutionSeed, RegionSeed, UnGraphSeed};
pub use single_stage::SingleStageInterpreter;
pub use stage_access::StageAccess;
pub use value_store::ValueStore;

/// Essentials for dialect authors implementing machine-based semantics.
pub mod prelude {
    pub use crate::{
        BreakpointControl, ConsumeEffect, Control, FuelControl, Interpretable, Interpreter,
        InterruptControl, LiftEffect, LiftStop, Machine, ProjectMachine, ProjectMachineMut,
        StageAccess, ValueStore,
    };
}

#[cfg(test)]
mod tests;
