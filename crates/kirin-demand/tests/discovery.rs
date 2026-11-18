use kirin::prelude::*;
use kirin_demand::{Demand, DemandAnalysis};
use kirin_interpreter::{Body, Callee, Linker, SameStageLinker};
use kirin_test_languages::GraphFunctionLanguage;

#[test]
fn initial_call_demand_is_independent_of_block_order() {
    for choose_first in [true, false] {
        let choose = "^choose() { cond_br %unused then=^yes() else=^no(); }";
        let calculate = "^calculate() { %x = call.named @identity(%a) -> i64; br ^choose(); }";
        let blocks = if choose_first {
            format!("{choose}\n{calculate}")
        } else {
            format!("{calculate}\n{choose}")
        };
        let source = format!(
            r#"
stage @test fn @main(i64, i64) -> i64;
stage @test fn @identity(i64) -> i64;
specialize @test fn @main(i64, i64) -> i64 cfg {{
    ^entry(%a: i64, %unused: i64) {{ br ^calculate(); }}
    {blocks}
    ^yes() {{ ret %unused; }}
    ^no() {{ ret %unused; }}
}}
specialize @test fn @identity(i64) -> i64 cfg {{
    ^entry(%value: i64) {{ ret %value; }}
}}
"#
        );
        let mut program: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
        ParseProgramText::parse(&mut program, &source).unwrap();
        let stage = program.stage_by_name("test").unwrap();
        let callee = Callee::Function(program.lookup_function_by_name("main").unwrap());
        let target = SameStageLinker.resolve(&program, stage, &callee).unwrap();
        let Body::CFG(cfg) = target.body(&program).unwrap() else {
            panic!("expected CFG")
        };
        let info = program.stage(stage).unwrap();
        let blocks: Vec<_> = cfg.blocks(info).collect();
        let (choose, calculate) = if choose_first { (1, 2) } else { (2, 1) };
        let branch = blocks[choose].terminator(info).unwrap();
        let call = blocks[calculate].statements(info).next().unwrap();
        let result = *call.definition(info).results().next().unwrap();
        // The parser requires definitions before uses in text. Build the
        // dominated cross-block use through the rewriter for either order.
        kirin_ir::Rewriter::new(program.stage_mut(stage).unwrap())
            .replace_operand(branch, 0, result)
            .unwrap();

        let analysis = DemandAnalysis::<StageInfo<GraphFunctionLanguage>>::new(&program)
            .analyze(stage, callee, [Demand::Dead])
            .unwrap();
        let inputs = analysis.response(analysis.root()).unwrap();
        assert_eq!(inputs.as_slice(), &[Demand::Live, Demand::Dead]);
        assert_eq!(analysis.contexts().count(), 2);
        assert_eq!(
            analysis.fact(analysis.root(), result.into()),
            Some(&Demand::Live)
        );
    }
}
