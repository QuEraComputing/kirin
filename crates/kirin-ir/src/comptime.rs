pub trait CompileTimeValue: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {}

pub trait Typeof<Ty> {
    fn type_of(&self) -> Ty;
}

impl<T> CompileTimeValue for T where T: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {}
