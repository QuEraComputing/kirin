use kirin::prelude::*;

// fn main() {
//     let _ctx: Context<Stage> = Context::default();
//     println!("Hello, world!");
// }

// #[derive(Clone, Debug, Hash, PartialEq, Eq)]
// pub struct AnyType;

// impl Lattice for AnyType {
//     fn join(&self, _other: &Self) -> Self {
//         AnyType
//     }

//     fn meet(&self, _other: &Self) -> Self {
//         AnyType
//     }

//     fn is_subseteq(&self, _other: &Self) -> bool {
//         true
//     }
// }

// impl FiniteLattice for AnyType {
//     fn bottom() -> Self {
//         AnyType
//     }

//     fn top() -> Self {
//         AnyType
//     }
// }

// impl CompileTimeValue for AnyType {}

// impl TypeLattice for AnyType {}

// #[derive(Clone, Debug, PartialEq)]
// pub enum Value {
//     I32(i32),
//     I64(i64),
//     F32(f32),
//     F64(f64),
// }

// impl std::hash::Hash for Value {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         match self {
//             Value::I32(v) => {
//                 0u8.hash(state);
//                 v.hash(state);
//             }
//             Value::I64(v) => {
//                 1u8.hash(state);
//                 v.hash(state);
//             }
//             Value::F32(v) => {
//                 2u8.hash(state);
//                 v.to_bits().hash(state);
//             }
//             Value::F64(v) => {
//                 3u8.hash(state);
//                 v.to_bits().hash(state);
//             }
//         }
//     }
// }

// impl CompileTimeValue for Value {}

// #[derive(Clone)]
// pub enum Stage {
//     Structured(IRContext<StageA>),
//     Flat(IRContext<StageB>),
// }

// #[derive(Clone)]
// pub enum StageA {
//     Arith(ArithInstruction),
//     Constant(Constant<Value>),
//     Cf(ControlFlowInstruction),
// }

// #[derive(Clone)]
// pub enum StageB {
//     Arith(ArithInstruction),
//     Constant(Constant<Value>),
// }

// impl Language for StageA {
//     type Type = AnyType;
// }

// impl Language for StageB {
//     type Type = AnyType;
// }

// // impl Language for SimpleLang {
// //     type Type = AnyType;
// // }

// // type Context = IRContext<SimpleLang>;
