mod interpret;
mod runtime;

pub use runtime::Runtime;

pub type Effect = kirin_interpreter_2::effect::Flow<std::convert::Infallible>;
pub type Machine = kirin_interpreter_2::effect::Stateless<std::convert::Infallible>;
