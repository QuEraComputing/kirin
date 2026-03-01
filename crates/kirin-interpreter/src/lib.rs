mod abstract_interp;
mod control;
mod error;
mod eval;
mod frame;
mod frame_stack;
mod interpretable;
mod interpreter;
mod result;
mod scheduler;
mod stack;
mod stage;
mod value;
mod widening;

pub use abstract_interp::{AbstractInterpreter, FixpointState, SummaryCache, SummaryEntry};
pub use control::{AbstractContinuation, Args, ConcreteContinuation, ConcreteExt, Continuation};
pub use error::InterpreterError;
pub use eval::{CallSemantics, SSACFGRegion};
pub use frame::Frame;
pub use frame_stack::FrameStack;
pub use interpretable::Interpretable;
pub use interpreter::Interpreter;
pub use result::AnalysisResult;
pub use scheduler::DedupScheduler;
pub use stack::StackInterpreter;
pub use stage::{InStage, WithStage};
pub use value::{AbstractValue, BranchCondition};
pub use widening::WideningStrategy;

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::EvalCall as CallSemantics;
