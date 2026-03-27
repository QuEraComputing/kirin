mod lifted;

pub type Effect<V> = kirin_interpreter_2::effect::Flow<V>;
pub type Machine<V> = kirin_interpreter_2::effect::Stateless<V>;

#[cfg(test)]
mod tests;
