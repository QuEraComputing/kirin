mod lifted;
mod machine;
mod runtime;

pub use machine::{CallFrame, Machine};

pub type Effect<V> = kirin_interpreter_2::effect::Flow<V>;

#[cfg(test)]
mod tests;
