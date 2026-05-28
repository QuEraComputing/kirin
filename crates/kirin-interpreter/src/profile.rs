//! Type-level "profiles" that bundle the associated types of an interpreter.
//!
//! [`InterpreterProfile`] is a marker trait — implementors are usually
//! zero-sized types — whose associated items pin the type parameters that the
//! framework would otherwise demand at every call site. Selecting one
//! implementor selects the whole bundle of types consistently. This is the
//! Rust "type family" idiom (see the Rust Book, first edition, on associated
//! types): a single trait carries a family of related associated types.
//!
//! Concrete and abstract interpreters use [`InterpreterProfile`]. The fixpoint
//! driver adds owner-summary types via [`FixpointProfile`].
//!
//! New marker bundles in this framework should follow the `XxxProfile`
//! convention so the pattern stays recognizable.

use std::hash::Hash;

use crate::Summary;

/// Bundle the type parameters shared by every interpreter.
///
/// A profile is typically an empty struct that exists only at the type level.
/// Use it like:
///
/// ```ignore
/// struct MyProfile;
/// impl InterpreterProfile for MyProfile {
///     type Stage = MyStage;
///     type Value = i64;
///     type Frame = MyFrame;
///     type Completion = MyCompletion;
///     type Error = MyError;
/// }
/// let interp = ConcreteInterpreter::<MyProfile>::new(&pipeline);
/// ```
pub trait InterpreterProfile {
    /// Pipeline stage enum (e.g. `Stage` in toy-lang).
    type Stage;
    /// Value type stored in SSA environments.
    type Value;
    /// Frame type pushed onto the interpreter's frame stack.
    type Frame;
    /// Completion type produced when a frame finishes.
    type Completion;
    /// Error type for interpreter failures.
    type Error;
}

/// Extend an [`InterpreterProfile`] with the owner-summary types needed by the
/// fixpoint driver.
pub trait FixpointProfile: InterpreterProfile {
    /// Key identifying a summary owner in the fixpoint worklist.
    type SummaryKey: Clone + Eq + Hash;
    /// Summary carried per owner.
    type Summary: Summary;
}
