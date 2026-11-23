//! Standalone module text uses the same dialect parsers and document renderer
//! as statements. No function wrapper or specialization is required.
use kirin::parsers::{ChumskyError, ParseModuleText};
use kirin::prelude::*;
use kirin_test_languages::GraphFunctionLanguage;

#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[kirin(type = kirin_arith::ArithType)]
enum TestLanguage {
    #[chumsky(format = "item {name}")]
    NamedItem {
        #[kirin(declared_name)]
        name: Symbol,
    },
    #[wraps]
    Other(GraphFunctionLanguage),
}

impl From<GraphFunctionLanguage> for TestLanguage {
    fn from(value: GraphFunctionLanguage) -> Self {
        Self::Other(value)
    }
}

#[test]
fn named_item_lookup_after_roundtrip() {
    let mut program: Program<StageInfo<TestLanguage>> = Program::new();
    let id = program.add_stage().stage(StageInfo::default()).new();
    let module = program
        .parse_module(id, "module @a { item @value; }")
        .unwrap();
    let printed = module
        .render(program.stage(id).unwrap())
        .globals(program.global_symbols())
        .into_string()
        .unwrap();

    let mut reparsed: Program<StageInfo<TestLanguage>> = Program::new();
    let id = reparsed.add_stage().stage(StageInfo::default()).new();
    let module = reparsed.parse_module(id, &printed).unwrap();
    let stage = reparsed.stage(id).unwrap();
    let value = stage.symbol_table().lookup("value").unwrap();
    let member = module.member_by_name(stage, value).unwrap();
    assert!(matches!(member.definition(stage), TestLanguage::NamedItem { name } if *name == value));
}

#[test]
fn builder_only_finds_explicit_members() {
    let mut stage = BuilderStageInfo::<TestLanguage>::default();
    let module = stage.module().new();
    let name = stage.symbol_table_mut().intern("item".into());
    let first = stage
        .statement()
        .definition(TestLanguage::NamedItem { name })
        .new();
    let second = stage
        .statement()
        .definition(TestLanguage::NamedItem { name })
        .new();
    assert_eq!(module.member_by_name(&stage, name), None);
    stage.add_module_member(module, first);
    assert!(stage.try_add_module_member(module, second).is_err());
    let sibling = stage.module().members(vec![second]).new();
    let stage = stage.finalize().unwrap();
    assert_eq!(module.member_by_name(&stage, name), Some(first));
    assert_eq!(sibling.member_by_name(&stage, name), Some(second));
}

#[test]
fn invalid_members_leave_stage_retryable() {
    let mut program: Program<StageInfo<TestLanguage>> = Program::new();
    let id = program.add_stage().stage(StageInfo::default()).new();
    for (members, message) in [
        ("item @x; item @x;", "duplicate module member @x"),
        (
            "%x = constant 1 -> i64; %y = add %x, %x -> i64;",
            "undefined SSA",
        ),
        ("ret;", "terminator"),
    ] {
        let err = program
            .parse_module(id, &format!("module @retry {{ {members} }}"))
            .unwrap_err();
        assert!(err.to_string().contains(message), "{err}");
        let stage = program.stage(id).unwrap();
        assert_eq!(stage.module_arena().iter().count(), 0);
        assert_eq!(stage.statement_arena().iter().count(), 0);
    }
    program
        .parse_module(id, "module @retry { item @x; }")
        .unwrap();
}

#[test]
fn empty_and_anonymous_modules_roundtrip() {
    for input in ["module @empty {}", "module {}"] {
        let mut program: Program<StageInfo<TestLanguage>> = Program::new();
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
        let mut reparsed: Program<StageInfo<TestLanguage>> = Program::new();
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
    let mut program: Program<StageInfo<TestLanguage>> = Program::new();
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
        let mut program: Program<StageInfo<TestLanguage>> = Program::new();
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
    let mut program: Program<StageInfo<TestLanguage>> = Program::new();
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
