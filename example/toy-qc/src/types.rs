use kirin::prelude::*;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint)]
pub enum QubitType {
    #[chumsky(format = "Qubit")]
    Qubit,
}

impl Placeholder for QubitType {
    fn placeholder() -> Self {
        QubitType::Qubit
    }
}
