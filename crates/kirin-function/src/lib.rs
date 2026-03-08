//! Function dialect for Kirin.
//!
//! This dialect provides operations for defining, calling, and returning from
//! functions. It supports two composition styles:
//!
//! - **[`Lexical`]**: Inline lambdas with captures. Functions are defined
//!   lexically inside a parent scope and can capture SSA values from that
//!   scope. Suited for source-level representations.
//!
//! - **[`Lifted`]**: Top-level function bindings. Lambdas are "lifted" to
//!   standalone functions and references are created via `bind`. Suited for
//!   lowered representations closer to a call graph.
//!
//! Both enums share `FunctionBody`, `Call`, and `Return` — they differ only
//! in how functions are *introduced* (inline `Lambda` vs top-level `Bind`).

use kirin::prelude::*;

pub mod bind;
pub mod body;
pub mod call;
pub mod lambda;
pub mod ret;

pub use bind::Bind;
pub use body::FunctionBody;
pub use call::Call;
pub use lambda::Lambda;
pub use ret::Return;

#[cfg(feature = "interpret")]
mod interpret_impl;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(fn, type = T)]
pub enum Lexical<T: CompileTimeValue + Default> {
    FunctionBody(FunctionBody<T>),
    Lambda(Lambda<T>),
    Call(Call<T>),
    Return(Return<T>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(fn, type = T)]
pub enum Lifted<T: CompileTimeValue + Default> {
    FunctionBody(FunctionBody<T>),
    Bind(Bind<T>),
    Call(Call<T>),
    Return(Return<T>),
}
