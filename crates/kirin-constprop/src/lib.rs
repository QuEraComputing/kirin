//! Constant propagation for Kirin pipelines.
//!
//! This crate is intentionally small: it provides the constant-propagation
//! *lattice* ([`ConstPropValue`]) and a ready-to-run analysis alias
//! ([`ConstProp`]). All traversal, loop fixpoints, and interprocedural
//! summarization live in the engine
//! ([`AbstractInterpreter`](kirin_interpreter::AbstractInterpreter)); all
//! transfer rules live in the dialect crates' ordinary `Interpretable` impls,
//! which are generic over the value domain and therefore apply to
//! [`ConstPropValue`] unchanged.
//!
//! ```ignore
//! use kirin_constprop::ConstProp;
//! use kirin_interpreter::engine::CrossStageLinker;
//!
//! let mut analysis = ConstProp::<Stage, MyError>::new(&pipeline)
//!     .with_linker(CrossStageLinker);
//! let result = analysis.analyze_by_name("source", "abs", [7.into()])?;
//! ```

mod context;
mod value;

pub use context::{CallCtx, ConstPropContext};
pub use value::{ConstPropValue, PartialStruct, PartialTuple};

/// Constant propagation as an [`AbstractInterpreter`](kirin_interpreter::AbstractInterpreter)
/// instantiated at the [`ConstPropValue`] lattice, with bounded arg-tuple
/// context sensitivity ([`ConstPropContext`]) so recursion over distinct
/// constants stays precise (e.g. `factorial(Const(5)) → Const(120)`) while
/// remaining sound and terminating on unknown/over-budget arguments.
pub type ConstProp<'ir, S, E, Lk = kirin_interpreter::SameStageLinker> =
    kirin_interpreter::AbstractInterpreter<'ir, S, ConstPropValue, E, Lk, ConstPropContext>;
