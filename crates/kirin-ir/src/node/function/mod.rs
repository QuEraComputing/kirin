//! Three-level function refinement hierarchy.
//!
//! Functions in Kirin are organized into a three-level hierarchy that reflects
//! how programs evolve through compilation stages and specialization:
//!
//! ```text
//! Function              (stage-independent identity)
//!   └─ StagedFunction        (one per compilation stage)
//!       └─ SpecializedFunction    (one per signature specialization)
//! ```
//!
//! - [`Function`] / [`FunctionInfo`] — A logical function identity, independent
//!   of any compilation stage. It maps a name to the set of staged versions
//!   produced as the function moves through the pipeline.
//!
//! - [`StagedFunction`] / [`StagedFunctionInfo`] — A function compiled to a
//!   specific stage (e.g. parsed, optimized, lowered). Carries the generic
//!   signature and owns all specializations for that stage. Stages are *not*
//!   necessarily sequential — a user may program a low-level stage directly.
//!
//! - [`SpecializedFunction`] / [`SpecializedFunctionInfo`] — A concrete
//!   instantiation of a staged function for a particular (possibly narrower)
//!   signature. Owns the IR body. Dispatch selects the most specific
//!   non-invalidated specialization via [`SignatureSemantics`].
//!
//! Each level can be *invalidated* (staged or specialized) when the function is
//! redefined; invalidated entries are kept for backedge tracking but excluded
//! from dispatch and compilation.

mod compile_stage;
mod generic;
mod specialized;
mod staged;

pub use compile_stage::CompileStage;
pub use generic::{Function, FunctionInfo};
pub use specialized::{SpecializedFunction, SpecializedFunctionInfo};
pub use staged::{
    StagedFunction, StagedFunctionInfo, StagedNamePolicy, UniqueLiveSpecializationError,
};
