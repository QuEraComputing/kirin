#![allow(unused_imports)]

mod composition;
mod language;
mod machine;
mod programs;
mod value;

pub use composition::{
    CompositeDialect, RecordingEffect, RecordingError, RecordingMachine,
    build_machine_routing_program, build_mixed_effect_program, build_mixed_error_program,
    build_pure_passthrough_program,
};
pub use language::{
    AddI64, BranchSelect, ConstI64, ForOp, FunctionDef, IfOp, JumpTo, PackTuple, ReturnOp, StopOp,
    TestDialect, UnknownValue, YieldOp,
};
pub use machine::TestMachine;
pub use programs::{
    build_branch_false_program, build_branch_nondeterministic_program, build_branch_true_program,
    build_for_program, build_for_program_missing_yield, build_for_program_overflow,
    build_if_program_false, build_if_program_missing_yield, build_if_program_nondeterministic,
    build_if_program_true, build_jump_program, build_linear_sum_program,
    build_product_return_program, build_region_yield_program, build_return_program,
    build_yield_program,
};
pub use value::{TestType, TestValue};
