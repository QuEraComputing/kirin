//! Creating and inserting statements at an [`InsertionPoint`].
//!
//! [`Rewriter::create`](crate::Rewriter::create) builds a new statement and
//! places it in one step. [`Rewriter::insert`](crate::Rewriter::insert)
//! places a detached statement, such as a copy. Both check the insertion point
//! against the definition before changing anything, so a rejection leaves the
//! stage untouched.
//!
//! - `create`: [`InsertionPoint`] and the `create` and `insert` entry points.
//! - `placement`: checking an insertion point and placing a statement there.
//! - `build`: making the detached statement `create` places.

mod build;
mod create;
mod placement;

pub use create::InsertionPoint;
