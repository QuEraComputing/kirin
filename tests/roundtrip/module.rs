//! Standalone module text uses the same dialect parsers and document renderer
//! as statements. No function wrapper or specialization is required.
use kirin::parsers::{ChumskyError, ParseModuleText};
use kirin::prelude::*;
use kirin_test_languages::GraphFunctionLanguage;

#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[kirin(type = kirin_arith::ArithType)]
enum TestLanguage {
    #[chumsky(format = "decl {name}")]
    Decl { name: Symbol },
    #[wraps]
    Other(GraphFunctionLanguage),
}

impl From<GraphFunctionLanguage> for TestLanguage {
    fn from(value: GraphFunctionLanguage) -> Self {
        Self::Other(value)
    }
}

#[derive(Debug, StageMeta, ParseDispatch, RenderDispatch)]
enum TestStage {
    #[stage(name = "decls")]
    Decls(StageInfo<TestLanguage>),
    #[stage(name = "code")]
    Code(StageInfo<GraphFunctionLanguage>),
}

/// Both stages declare a module `@a`; each keeps its own members.
const TWO_STAGE_PROGRAM: &str = r#"
stage @decls mod @a { decl @value; }
stage @code mod @a { %k = constant 1 -> i64; }
stage @code fn @f(i64) -> i64;
specialize @code fn @f(i64) -> i64 cfg { ^e(%x: i64) { ret %x; } }
"#;

#[test]
fn program_text_keeps_modules_in_their_stages() {
    let mut program: Program<TestStage> = Program::new();
    program.parse(TWO_STAGE_PROGRAM).unwrap();
    let printed = program.sprint();

    let mut reparsed: Program<TestStage> = Program::new();
    reparsed.parse(&printed).unwrap();
    assert_eq!(reparsed.sprint(), printed);

    let decls_id = reparsed.stage_by_name("decls").unwrap();
    let module = reparsed
        .resolve_module::<TestLanguage>("a", decls_id)
        .unwrap();
    let Some(TestStage::Decls(decls)) = reparsed.stage(decls_id) else {
        panic!("@decls holds TestLanguage");
    };
    let value = decls.symbol_table().lookup("value").unwrap();
    let members: Vec<_> = module.members(decls).collect();
    assert_eq!(members.len(), 1);
    assert!(matches!(members[0].definition(decls), TestLanguage::Decl { name } if *name == value));

    let code_id = reparsed.stage_by_name("code").unwrap();
    let module = reparsed
        .resolve_module::<GraphFunctionLanguage>("a", code_id)
        .unwrap();
    let Some(TestStage::Code(code)) = reparsed.stage(code_id) else {
        panic!("@code holds GraphFunctionLanguage");
    };
    assert_eq!(module.members(code).count(), 1);
    assert!(reparsed.lookup_function_by_name("f").is_some());
}

#[test]
fn program_text_rejects_duplicate_module_in_a_stage() {
    let src = "stage @decls mod @a {}\nstage @decls mod @a {}";
    let mut program: Program<TestStage> = Program::new();
    let error = program.parse(src).unwrap_err();
    assert!(error.to_string().contains("duplicate module @a"), "{error}");
    let span = error.span.unwrap();
    assert_eq!(span.start, src.rfind("@a").unwrap());
}

#[test]
fn named_module_roundtrip() {
    let mut program: Program<StageInfo<TestLanguage>> = Program::new();
    let id = program.add_stage().stage(StageInfo::default()).new();
    let module = program.parse_module(id, "mod @a { decl @value; }").unwrap();
    // Module and member names are stage-local, so no global symbols are needed.
    let printed = module
        .render(program.stage(id).unwrap())
        .into_string()
        .unwrap();

    let mut reparsed: Program<StageInfo<TestLanguage>> = Program::new();
    let id = reparsed.add_stage().stage(StageInfo::default()).new();
    reparsed.parse_module(id, &printed).unwrap();
    let module = reparsed.resolve_module::<TestLanguage>("a", id).unwrap();
    let stage = reparsed.stage(id).unwrap();
    let value = stage.symbol_table().lookup("value").unwrap();
    let members: Vec<_> = module.members(stage).collect();
    assert_eq!(members.len(), 1);
    assert!(matches!(members[0].definition(stage), TestLanguage::Decl { name } if *name == value));
}

#[test]
fn builder_attaches_each_statement_once() {
    let mut stage = BuilderStageInfo::<TestLanguage>::default();
    let module = stage.module().new();
    let name = stage.symbol_table_mut().intern("value".into());
    let first = stage
        .statement()
        .definition(TestLanguage::Decl { name })
        .new();
    let second = stage
        .statement()
        .definition(TestLanguage::Decl { name })
        .new();
    stage.add_module_member(module, first);
    assert!(stage.try_add_module_member(module, first).is_err());
    let sibling = stage.module().members(vec![second]).new();
    let stage = stage.finalize().unwrap();
    assert_eq!(module.members(&stage).collect::<Vec<_>>(), [first]);
    assert_eq!(sibling.members(&stage).collect::<Vec<_>>(), [second]);
}

#[test]
fn invalid_members_leave_stage_retryable() {
    let mut program: Program<StageInfo<TestLanguage>> = Program::new();
    let id = program.add_stage().stage(StageInfo::default()).new();
    for (members, message) in [
        (
            "%x = constant 1 -> i64; %y = add %x, %x -> i64;",
            "undefined SSA",
        ),
        ("ret;", "terminator"),
    ] {
        let err = program
            .parse_module(id, &format!("mod @retry {{ {members} }}"))
            .unwrap_err();
        assert!(err.to_string().contains(message), "{err}");
        let stage = program.stage(id).unwrap();
        assert_eq!(stage.module_arena().iter().count(), 0);
        assert_eq!(stage.statement_arena().iter().count(), 0);
    }
    program.parse_module(id, "mod @retry { decl @x; }").unwrap();
}

#[test]
fn empty_and_anonymous_modules_roundtrip() {
    for input in ["mod @empty {}", "mod {}"] {
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
        .parse_module(stage_id, "mod @a { %x = constant 1 -> i64; }")
        .unwrap();
    program
        .parse_module(stage_id, "mod @b { %x = constant 2 -> i64; }")
        .unwrap();
    let error = program.parse_module(stage_id, "mod @a {}").unwrap_err();
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
        .parse_module(stage_id, "mod @c { %y = add %x, %x -> i64; }")
        .unwrap_err();
    assert!(error.to_string().contains("undefined SSA"), "{error}");
}

#[test]
fn module_syntax_errors_and_member_spans() {
    for input in [
        "mod @bad { %x = constant 1 -> i64 }", // missing semicolon
        "mod @bad {",                          // unclosed container
        "mod @bad {} trailing",
        "mod @bad { %x = graph_eval (]; }", // mismatched delimiters
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
    let input = "mod @bad {\n  unknown_statement;\n}";
    let ChumskyError::Parse(errors) = program.parse_module(stage_id, input).unwrap_err() else {
        panic!("expected dialect syntax error");
    };
    assert!(
        errors
            .iter()
            .all(|e| e.span.start >= input.find("unknown_statement").unwrap())
    );
}
