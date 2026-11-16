//! Standalone module text uses the same dialect parsers and document renderer
//! as statements. No function wrapper or specialization is required.
use kirin::parsers::{ChumskyError, ParseModuleText};
use kirin::prelude::*;
use kirin_test_languages::GraphFunctionLanguage;

#[test]
fn module_roundtrip_preserves_members_and_nested_ssa() {
    let input = r#"module @floorplan {
        %seed = constant 1 -> i64;
        %physical = graph_eval %seed, %seed digraph ^physical(%x: i64, %y: i64) {
            %sum = add %x, %y -> i64;
            yield %sum;
        } -> i64;
        // The second section consumes the first section's result.
        %logical = graph_eval %physical, %seed digraph ^logical() capture(%x: i64, %y: i64) {
            %sum = mul %x, %y -> i64;
            yield %sum;
        } -> i64;
    }"#;
    let mut program: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
    let stage_id = program
        .add_stage()
        .stage(StageInfo::default())
        .name("test")
        .new();
    let module = program.parse_module(stage_id, input).unwrap();
    assert_eq!(
        program.resolve_module::<GraphFunctionLanguage>("floorplan", stage_id),
        Some(module)
    );
    assert_eq!(program.function_arena().iter().count(), 0);
    let stage = program.stage(stage_id).unwrap();
    let members: Vec<_> = module.members(stage).collect();
    assert_eq!(members.len(), 3);
    for member in &members {
        assert_eq!(*member.parent(stage), Some(StatementParent::Module(module)));
    }
    let physical_result: SSAValue = (*members[1].results(stage).next().unwrap()).into();
    assert_eq!(
        *members[2].arguments(stage).next().unwrap(),
        physical_result
    );
    let logical_graph = members[2].definition(stage).digraphs().next().unwrap();
    assert_eq!(logical_graph.expect_info(stage).capture_ports().len(), 2);
    let printed = module
        .render(stage)
        .globals(program.global_symbols())
        .into_string()
        .unwrap();
    assert!(printed.starts_with("module @floorplan {"), "{printed}");
    assert!(printed.contains("digraph ^physical"), "{printed}");
    assert!(printed.contains("digraph ^logical"), "{printed}");

    let mut reparsed: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
    let stage_id = reparsed.add_stage().stage(StageInfo::default()).new();
    let module = reparsed.parse_module(stage_id, &printed).unwrap();
    let reprinted = module
        .render(reparsed.stage(stage_id).unwrap())
        .globals(reparsed.global_symbols())
        .into_string()
        .unwrap();
    assert_eq!(printed, reprinted);
}

#[test]
fn empty_and_anonymous_modules_roundtrip() {
    for input in ["module @empty {}", "module {}"] {
        let mut program: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
        let stage_id = program.add_stage().stage(StageInfo::default()).new();
        let module = program.parse_module(stage_id, input).unwrap();
        let stage = program.stage(stage_id).unwrap();
        assert_eq!(module.members(stage).count(), 0);
        assert_eq!(
            module.expect_info(stage).name().is_some(),
            input.contains('@')
        );
        let printed = module
            .render(stage)
            .globals(program.global_symbols())
            .into_string()
            .unwrap();
        let mut reparsed: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
        let stage_id = reparsed.add_stage().stage(StageInfo::default()).new();
        let module = reparsed.parse_module(stage_id, &printed).unwrap();
        let reprinted = module
            .render(reparsed.stage(stage_id).unwrap())
            .globals(reparsed.global_symbols())
            .into_string()
            .unwrap();
        assert_eq!(printed, reprinted);
    }
}

#[test]
fn module_names_and_ssa_scopes_are_separate() {
    let mut program: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
    let stage_id = program.add_stage().stage(StageInfo::default()).new();
    program
        .parse_module(stage_id, "module @a { %x = constant 1 -> i64; }")
        .unwrap();
    program
        .parse_module(stage_id, "module @b { %x = constant 2 -> i64; }")
        .unwrap();
    let error = program.parse_module(stage_id, "module @a {}").unwrap_err();
    assert!(error.to_string().contains("duplicate module @a"));
    assert_eq!(
        program
            .stage(stage_id)
            .unwrap()
            .module_arena()
            .iter()
            .count(),
        2
    );
    let error = program
        .parse_module(stage_id, "module @c { %y = add %x, %x -> i64; }")
        .unwrap_err();
    assert!(error.to_string().contains("undefined SSA"), "{error}");
}

#[test]
fn module_syntax_errors_and_member_spans() {
    for input in [
        "module @bad { %x = constant 1 -> i64 }", // missing semicolon
        "module @bad {",                          // unclosed container
        "module @bad {} trailing",
        "module @bad { %x = graph_eval (]; }", // mismatched delimiters
    ] {
        let mut program: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
        let stage_id = program.add_stage().stage(StageInfo::default()).new();
        assert!(program.parse_module(stage_id, input).is_err(), "{input}");
        assert_eq!(
            program
                .stage(stage_id)
                .unwrap()
                .module_arena()
                .iter()
                .count(),
            0
        );
    }
    let mut program: Program<StageInfo<GraphFunctionLanguage>> = Program::new();
    let stage_id = program.add_stage().stage(StageInfo::default()).new();
    let input = "module @bad {\n  unknown_statement;\n}";
    let ChumskyError::Parse(errors) = program.parse_module(stage_id, input).unwrap_err() else {
        panic!("expected dialect syntax error");
    };
    assert!(
        errors
            .iter()
            .all(|e| e.span.start >= input.find("unknown_statement").unwrap())
    );
}
