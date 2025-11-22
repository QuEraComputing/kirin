use crate::Language;

pub trait CompileTimeValue: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {
}

pub trait Typeof<L: Language> {
    fn type_of(&self) -> L::Type;
}
