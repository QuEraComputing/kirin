use kirin::ir::{CompileTimeValue, Typeof};
use kirin_constant::Constant;

use crate::circuit::Circuit;

pub enum KernelLang {
    Circuit(Circuit),
    Constant(Constant<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    F64(f64),
    Qubit(usize),
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::F64(v) => {
                state.write_u8(0);
                state.write(&v.to_le_bytes());
            }
            Value::Qubit(id) => {
                state.write_u8(1);
                state.write_usize(*id);
            }
        }
    }
}

impl CompileTimeValue for Value {}
// impl Typeof<KernelLang> for Value {
//     fn type_of(&self) -> KernelType {
//         match self {
//             Value::F64(_) => KernelType::F64,
//             Value::Qubit(_) => KernelType::Qubit,
//         }
//     }
// }
