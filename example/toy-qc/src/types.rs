use kirin::prelude::*;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, HasParser, PrettyPrint)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QubitType {
    #[chumsky(format = "Qubit")]
    Qubit,
}

impl Placeholder for QubitType {
    fn placeholder() -> Self {
        QubitType::Qubit
    }
}
