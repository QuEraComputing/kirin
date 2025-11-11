use kirin::*;

fn main() {
    let _ctx: ir::Context<Stage> = ir::Context::default();
    println!("Hello, world!");
}

pub enum Stage {
    StageA(ir::IRContext<LangA>),
    StageB(ir::IRContext<LangB>),
}

#[derive(Clone, Debug, ir::Instruction)]
#[kirin(wraps)]
pub enum LangA {
    Arith(dialects::arith::ArithInstruction),
    Constant(dialects::constant::Constant<Value>),
    Scf(dialects::scf::SCFInstruction),
}

#[derive(Clone, Debug)]
pub enum LangB {
    Arith(dialects::arith::ArithInstruction),
    Constant(dialects::constant::Constant<Value>),
    Cf(dialects::cf::ControlFlowInstruction),
}

impl ir::Language for LangA {
    type Type = AnyType;
}

impl ir::Language for LangB {
    type Type = AnyType;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct AnyType;

impl ir::Lattice for AnyType {
    fn join(&self, _other: &Self) -> Self {
        AnyType
    }

    fn meet(&self, _other: &Self) -> Self {
        AnyType
    }

    fn is_subseteq(&self, _other: &Self) -> bool {
        true
    }
}

impl ir::FiniteLattice for AnyType {
    fn bottom() -> Self {
        AnyType
    }

    fn top() -> Self {
        AnyType
    }
}

impl ir::CompileTimeValue for AnyType {}

impl ir::TypeLattice for AnyType {}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::I32(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Value::I64(v) => {
                1u8.hash(state);
                v.hash(state);
            }
            Value::F32(v) => {
                2u8.hash(state);
                v.to_bits().hash(state);
            }
            Value::F64(v) => {
                3u8.hash(state);
                v.to_bits().hash(state);
            }
        }
    }
}

impl ir::CompileTimeValue for Value {}
