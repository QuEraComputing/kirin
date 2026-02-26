mod abstract_interp;
mod control;
mod error;
mod eval;
mod frame;
mod interpretable;
mod interpreter;
mod result;
mod stack;
mod value;
mod widening;

pub use abstract_interp::{AbstractInterpreter, FixpointState, SummaryCache, SummaryEntry};
pub use control::{AbstractContinuation, Args, ConcreteContinuation, ConcreteExt, Continuation};
pub use error::InterpreterError;
pub use eval::{BlockExecutor, CallSemantics, SSACFGRegion};
pub use frame::Frame;
pub use interpretable::Interpretable;
pub use interpreter::Interpreter;
pub use result::AnalysisResult;
pub use stack::StackInterpreter;
pub use value::{AbstractValue, BranchCondition};
pub use widening::WideningStrategy;

pub use smallvec::{self, SmallVec};

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter::CallSemantics;
