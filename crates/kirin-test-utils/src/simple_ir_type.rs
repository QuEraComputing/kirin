use kirin_ir::{FiniteLattice, Lattice, TypeLattice, Typeof};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum SimpleIRType {
    Any,
    Int,
    Float,
    DataType,
    Bottom,
}

impl Lattice for SimpleIRType {
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (a, b) if a == b)
    }

    fn join(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            other.clone()
        } else if other.is_subseteq(self) {
            self.clone()
        } else {
            SimpleIRType::Any
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            self.clone()
        } else if other.is_subseteq(self) {
            other.clone()
        } else {
            SimpleIRType::Bottom
        }
    }
}

impl FiniteLattice for SimpleIRType {
    fn bottom() -> Self {
        SimpleIRType::Bottom
    }

    fn top() -> Self {
        SimpleIRType::Any
    }
}

impl Default for SimpleIRType {
    fn default() -> Self {
        Self::bottom()
    }
}

impl std::fmt::Display for SimpleIRType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleIRType::Any => write!(f, "any"),
            SimpleIRType::Int => write!(f, "int"),
            SimpleIRType::Float => write!(f, "float"),
            SimpleIRType::DataType => write!(f, "datatype"),
            SimpleIRType::Bottom => write!(f, "bottom"),
        }
    }
}

impl TypeLattice for SimpleIRType {}

impl Typeof<SimpleIRType> for i64 {
    fn type_of(&self) -> SimpleIRType {
        SimpleIRType::Int
    }
}
