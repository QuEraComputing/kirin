use kirin_interpreter_new::{Completion, StandardCompletion};
use kirin_scf::interpreter_new::ScfCompletion;

#[derive(Clone, Debug, PartialEq, Eq, Completion)]
pub enum ToyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}
