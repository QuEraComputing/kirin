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

// impl SimpleLanguage {
//     pub fn op_add<L: Language + From<Self>>(
//         arena: &mut Arena<L>,
//         arg_0: impl Into<SSAValue>,
//         arg_1: impl Into<SSAValue>,
//     ) -> AddRef
//     where
//         L::TypeLattice: From<SimpleTypeLattice>,
//     {
//         let arg_0 = arg_0.into();
//         let arg_1 = arg_1.into();
//         let id = StatementId(arena.statements.len());
//         let result_id = ResultValue(arena.ssas.len());
//         let ssa = SSAInfo::new(
//             result_id.into(),
//             None,
//             L::TypeLattice::from(Float),
//             SSAKind::Result(id, 0),
//         );
//         arena.ssas.push(ssa);
//         arena.statements.push(StatementInfo {
//             node: LinkedListNode::new(id),
//             parent: None,
//             definition: SimpleLanguage::Add(arg_0, arg_1, result_id).into(),
//         });
//         AddRef {
//             id,
//             arg_0,
//             arg_1,
//             result_0: result_id,
//         }
//     }

//     pub fn op_constant<T>(arena: &mut Arena<Self>, value: T) -> ConstantRef
//     where
//         T: Into<Value>,
//     {
//         let value: Value = value.into();
//         let parent = StatementId(arena.statements.len());
//         let result_id = ResultValue(arena.ssas.len());
//         let ssa = SSAInfo::new(
//             result_id.into(),
//             None,
//             value.type_of(),
//             SSAKind::Result(parent, 0),
//         );
//         arena.ssas.push(ssa);
//         arena.statements.push(StatementInfo {
//             node: LinkedListNode::new(parent),
//             parent: None,
//             definition: SimpleLanguage::Constant(value, result_id),
//         });
//         ConstantRef {
//             id: parent,
//             result_0: result_id,
//         }
//     }

//     pub fn op_function(arena: &mut Arena<Self>, body: Region) -> FunctionRef {
//         let parent = StatementId(arena.statements.len());
//         let result_id = ResultValue(arena.ssas.len());
//         let ssa = SSAInfo::new(
//             result_id.into(),
//             None,
//             SimpleTypeLattice::Any,
//             SSAKind::Result(parent, 0),
//         );
//         arena.ssas.push(ssa);
//         arena.statements.push(StatementInfo {
//             node: LinkedListNode::new(parent),
//             parent: None,
//             definition: SimpleLanguage::Function(body, result_id),
//         });
//         FunctionRef {
//             id: parent,
//             body,
//             result_0: result_id,
//         }
//     }

//     pub fn op_return(arena: &mut Arena<Self>, arg: impl Into<SSAValue>) -> ReturnRef {
//         let arg = arg.into();
//         let parent = StatementId(arena.statements.len());
//         arena.statements.push(StatementInfo {
//             node: LinkedListNode::new(parent),
//             parent: None,
//             definition: SimpleLanguage::Return(arg),
//         });
//         ReturnRef {
//             id: parent,
//             arg_0: arg,
//         }
//     }
// }

// pub struct AddRef {
//     pub id: StatementId,
//     pub arg_0: SSAValue,
//     pub arg_1: SSAValue,
//     pub result_0: ResultValue,
// }

// pub struct ConstantRef {
//     pub id: StatementId,
//     pub result_0: ResultValue,
// }

// pub struct ReturnRef {
//     pub id: StatementId,
//     pub arg_0: SSAValue,
// }

// pub struct FunctionRef {
//     pub id: StatementId,
//     pub body: Region,
//     pub result_0: ResultValue,
// }

// impl From<AddRef> for StatementId {
//     fn from(add: AddRef) -> Self {
//         add.id
//     }
// }

// impl From<ConstantRef> for StatementId {
//     fn from(constant: ConstantRef) -> Self {
//         constant.id
//     }
// }

// impl From<FunctionRef> for StatementId {
//     fn from(function: FunctionRef) -> Self {
//         function.id
//     }
// }

// impl From<ReturnRef> for StatementId {
//     fn from(ret: ReturnRef) -> Self {
//         ret.id
//     }
// }

impl Language for SimpleLanguage {
    type TypeLattice = SimpleTypeLattice;
}
