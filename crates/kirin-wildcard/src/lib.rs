//! Wildcard pattern dialect for Kirin.
//!
//! A pattern is ordinary Kirin IR written in a language that composes the
//! real dialects being matched (e.g. `Arith`) with this dialect. Typed
//! statements in the pattern describe the statements to match; the
//! `wildcard.*` statements mark holes and describe the replacement.
//!
//! # Statements
//!
//! | Statement | Description |
//! |-----------|-------------|
//! | `wildcard.pattern @name block ^bb() {..}` | Holds one pattern: the match statements followed by a `rewrite` |
//! | `%x = wildcard.hole -> T` | Hole: matches any SSA value and binds it to `%x` |
//! | `%k = wildcard.native_call "log2"(%c) -> T` | Calls a registered native function: computes values for a `constraint` in a pattern body, or computes results in a replacement body |
//! | `wildcard.constraint %ok` | Fails the match unless `%ok`, computed by a `native_call`, is true |
//! | `wildcard.replace %r with %x` | Redirects every use of the matched `%r` to `%x` |
//! | `wildcard.erase 2` | Erases the matched statement at position 2 of the pattern body |
//! | `wildcard.rewrite 2 block ^rw() {..}` | Terminates a pattern body; the statement at position 2 is the root, the body holds the replacement |
//!
//! # Example
//!
//! ```text
//! wildcard.pattern @add_zero block ^bb() {
//!   %x    = wildcard.hole -> i64;
//!   %zero = constant 0 -> i64;
//!   %r    = add %x, %zero -> i64;
//!   wildcard.rewrite 2 block ^rw() {
//!     wildcard.replace %r with %x;
//!     wildcard.erase 2;
//!   };
//! };
//! ```
//!
//! The matched statements are the producers of the `rewrite` root, followed
//! through operands and stopping at holes (`wildcard.hole`). The
//! replacement body's actions run in order, so a `replace` must redirect a
//! statement's uses before an `erase` removes it, unless the pattern checks
//! that they are unused. Erasing a statement whose results are still used
//! fails the rewrite, and the pass quarantines the stage.
//!
//! `rewrite` and `erase` name their statement by its 0-based position in the
//! pattern body rather than by one of its results because kirin-ir statements
//! take only SSA values as arguments. So a statement without results could not
//! be referenced otherwise.
//! Patterns are never rewritten, so the positions stay valid.
//!
//! This crate only defines the pattern data. It does not match patterns or
//! mutate concrete IR; those responsibilities belong to `kirin-rewrite` and
//! `kirin-ir`, respectively.
//!
//! Patterns are never executed, so the dialect has no interpreter rules.
#![forbid(unsafe_code)]

mod constraint;
mod erase;
mod hole;
mod language;
mod native_call;
mod pattern;
mod replace;
mod rewrite;

pub use constraint::Constraint;
pub use erase::Erase;
pub use hole::Hole;
pub use language::PatternLanguage;
pub use native_call::NativeCall;
pub use pattern::Pattern;
pub use replace::Replace;
pub use rewrite::Rewrite;

use kirin::prelude::*;

#[cfg(test)]
mod tests;

/// Wrapper enum that composes all wildcard statements.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(builders, type = T)]
#[chumsky(format = "wildcard")]
pub enum PatternStatement<T: CompileTimeValue> {
    Pattern(Pattern<T>),
    Hole(Hole<T>),
    NativeCall(NativeCall<T>),
    Constraint(Constraint<T>),
    Replace(Replace<T>),
    Erase(Erase<T>),
    #[kirin(terminator)]
    Rewrite(Rewrite<T>),
}
