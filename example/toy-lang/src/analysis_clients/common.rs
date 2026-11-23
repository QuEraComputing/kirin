//! Test helpers shared by the analysis clients.

use kirin::prelude::*;
use kirin_interpreter::{Body, Callee, Linker, SameStageLinker};
use kirin_rewrite::Database;

use crate::language::LowLevel;
use crate::stage::Stage;

/// Every live specialization of a stage, with its number of parameters.
pub(super) fn roots<L: Dialect>(info: &StageInfo<L>) -> Vec<(SpecializedFunction, usize)> {
    info.staged_function_arena()
        .iter()
        .flat_map(|function| function.specializations())
        .filter(|specialization| !specialization.is_invalidated())
        .map(|specialization| {
            (
                specialization.id(),
                specialization.signature().params().len(),
            )
        })
        .collect()
}

pub(super) fn parse(input: &str) -> Program<Stage> {
    let mut program: Program<Stage> = Program::new();
    ParseProgramText::parse(&mut program, input).expect("the program parses");
    program
}

/// A database over `input`, with no analysis registered, and its `lowered`
/// stage.
pub(super) fn new_database(input: &str) -> (Database<Stage>, CompileStage) {
    let program = parse(input);
    let stage = program.stage_by_name("lowered").expect("the stage exists");
    (Database::new(program), stage)
}

pub(super) fn printed(input: &str) -> String {
    parse(input).sprint()
}

/// `%z`, the result of `helper`'s `add`.
pub(super) fn helper_result(program: &Program<Stage>, stage: CompileStage) -> SSAValue {
    let helper = Callee::Function(program.lookup_function_by_name("helper").unwrap());
    let target = SameStageLinker.resolve(program, stage, &helper).unwrap();
    let Body::CFG(cfg) = target.body(program).unwrap() else {
        panic!("helper has a CFG body")
    };
    let Stage::Lowered(info) = program.stage(stage).unwrap() else {
        panic!("the lowered stage holds the lowered language")
    };
    let entry = cfg.blocks(info).next().unwrap();
    let add = entry
        .statements(info)
        .find(|stmt| matches!(stmt.expect_info(info).definition(), LowLevel::Arith(_)))
        .unwrap();
    SSAValue::from(*add.definition(info).results().next().unwrap())
}
