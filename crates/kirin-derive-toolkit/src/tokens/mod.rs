//! Typed code-block builders for generating Rust syntax.
//!
//! Instead of assembling `TokenStream` with raw `quote!` calls, these builders
//! provide structured, composable code generation with compile-time shape
//! guarantees.
//!
//! | Builder | Generates |
//! |---------|-----------|
//! | [`TraitImpl`] | `impl Trait for Type { ... }` blocks |
//! | [`InherentImpl`] | `impl Type { ... }` blocks |
//! | [`MatchExpr`] | `match subject { arm => body, ... }` expressions |
//! | [`Pattern`] | Destructuring patterns (`Foo { a, b }` or `Foo(a, b)`) |
//! | [`StructDef`], [`EnumDef`] | Type definitions |
//! | [`DelegationCall`] | Forwarding calls through `#[wraps]` fields |
//!
//! All builders implement `ToTokens` so they can be interpolated directly
//! in `quote!` expressions.

mod definitions;
mod delegation;
mod fragment;
mod inherent_impl;
mod match_expr;
mod pattern;
mod trait_impl;

pub use definitions::{EnumDef, EnumVariant, ModuleDef, StructDef, StructField};
pub use delegation::{DelegationAssocType, DelegationCall};
pub use fragment::Fragment;
pub use inherent_impl::InherentImpl;
pub use match_expr::{MatchArm, MatchExpr};
pub use pattern::Pattern;
pub use trait_impl::{AssocConst, AssocType, ImplItem, Method, TraitImpl};
