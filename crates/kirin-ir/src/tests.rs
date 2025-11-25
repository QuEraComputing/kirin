use crate::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum SimpleTypeLattice {
    Any,
    Int,
    Float,
    DataType,
    Bottom,
}

pub use SimpleTypeLattice::*;

impl Lattice for SimpleTypeLattice {
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (a, b) if a == b)
    }

    fn join(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            other.clone()
        } else if other.is_subseteq(self) {
            self.clone()
        } else {
            SimpleTypeLattice::Any
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            self.clone()
        } else if other.is_subseteq(self) {
            other.clone()
        } else {
            SimpleTypeLattice::Bottom
        }
    }
}

impl FiniteLattice for SimpleTypeLattice {
    fn bottom() -> Self {
        SimpleTypeLattice::Bottom
    }

    fn top() -> Self {
        SimpleTypeLattice::Any
    }
}

impl crate::TypeLattice for SimpleTypeLattice {}

impl Typeof<SimpleTypeLattice> for i64 {
    fn type_of(&self) -> SimpleTypeLattice {
        SimpleTypeLattice::Int
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I64(i64),
    F64(f64),
}
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::I64(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Value::F64(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
        }
    }
}

impl Typeof<SimpleTypeLattice> for Value {
    fn type_of(&self) -> SimpleTypeLattice {
        match self {
            Value::I64(_) => SimpleTypeLattice::Int,
            Value::F64(_) => SimpleTypeLattice::Float,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

#[derive(Clone, Debug, PartialEq, Statement)]
#[kirin(fn, type_lattice = SimpleTypeLattice, crate = crate)]
pub enum SimpleLanguage {
    Add(
        SSAValue,
        SSAValue,
        #[kirin(type = SimpleTypeLattice::Float)] ResultValue,
    ),
    Constant(
        #[kirin(into)] Value,
        #[kirin(type = SimpleTypeLattice::Float)] ResultValue,
    ),
    #[kirin(terminator)]
    Return(SSAValue),
    Function(
        Region,
        #[kirin(type = SimpleTypeLattice::Float)] ResultValue,
    ),
}

impl Language for SimpleLanguage {
    type TypeLattice = SimpleTypeLattice;
}
