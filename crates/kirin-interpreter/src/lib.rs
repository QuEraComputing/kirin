mod abstract_interp;
mod block_eval;
mod call;
mod control;
pub mod dispatch;
mod error;
pub mod ext;
mod frame;
mod frame_stack;
mod interpretable;
mod interpreter;
mod result;
mod scheduler;
mod stack;
mod stage;
mod stage_access;
mod value;
mod value_store;
mod widening;

pub use abstract_interp::{
    AbstractInterpreter, FixpointState, SummaryCache, SummaryEntry, SummaryInserter,
};
pub use block_eval::BlockEvaluator;
pub use call::{CallSemantics, SSACFGRegion};
pub use control::{Args, ConcreteExt, Continuation};
pub use error::{InterpreterError, MissingEntryError, StageResolutionError};
pub use ext::InterpreterExt;
pub use frame::Frame;
pub use frame_stack::FrameStack;
pub use interpretable::Interpretable;
pub use interpreter::Interpreter;
pub use result::AnalysisResult;
pub use scheduler::DedupScheduler;
pub use stack::StackInterpreter;
pub use stage::Staged;
pub use stage_access::StageAccess;
pub use value::{AbstractValue, BranchCondition};
pub use value_store::ValueStore;
pub use widening::WideningStrategy;

/// Essentials for dialect authors implementing operational semantics.
pub mod prelude {
    pub use crate::{
        BranchCondition, CallSemantics, Continuation, Interpretable, Interpreter, InterpreterError,
        InterpreterExt, SSACFGRegion,
    };
}

/// Types for abstract interpretation and fixpoint analysis.
pub mod analysis {
    pub use crate::{
        AbstractValue, AnalysisResult, DedupScheduler, FixpointState, SummaryCache, SummaryEntry,
        WideningStrategy,
    };
}

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::CallSemantics;
