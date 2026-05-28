use kirin_interpreter::{Completion, StandardCompletion};
use kirin_scf::interpreter::ScfCompletion;

#[derive(Clone, Debug, PartialEq, Eq, Completion)]
pub enum ToyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}
