use kirin::prelude::*;
use kirin::pretty::FunctionPrintExt;
use kirin::pretty::{Config, Document};
use kirin_arith::ArithType;
use kirin_function::{Bind, Call, Return};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum CallableLanguage {
    #[chumsky(format = "{body}")]
    Function { body: Region },
    #[wraps]
    Bind(Bind<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

fn emit_statement(
    input: &str,
    operands: &[(&str, ArithType)],
) -> (StageInfo<CallableLanguage>, Statement) {
    let mut stage: StageInfo<CallableLanguage> = StageInfo::default();

    for (name, ty) in operands {
        stage
            .ssa()
            .name((*name).to_string())
            .ty(*ty)
            .kind(SSAKind::Test)
            .new();
    }

    let statement = parse::<CallableLanguage>(input, &mut stage)
        .expect("callable statement parse should succeed");
    (stage, statement)
}

fn render_statement(stage: &StageInfo<CallableLanguage>, statement: Statement) -> String {
    let dialect = statement
        .get_info(stage)
        .expect("statement should exist")
        .definition();

    let doc = Document::new(Config::default(), stage);
    let mut output = String::new();
    dialect
        .pretty_print(&doc)
        .render_fmt(80, &mut output)
        .expect("render should succeed");
    output
}

fn assert_roundtrip(input: &str, operands: &[(&str, ArithType)]) {
    let (stage, statement) = emit_statement(input, operands);
    let rendered = render_statement(&stage, statement);

    let (reparsed_stage, reparsed_statement) = emit_statement(rendered.trim(), operands);
    let rendered_again = render_statement(&reparsed_stage, reparsed_statement);

    assert_eq!(rendered.trim(), rendered_again.trim());
}

fn assert_pipeline_roundtrip(input: &str) {
    let mut parsed_pipeline: Pipeline<StageInfo<CallableLanguage>> = Pipeline::new();
    let parsed_functions = parsed_pipeline
        .parse(input)
        .expect("pipeline parse should succeed");
    assert!(
        !parsed_functions.is_empty(),
        "expected at least one parsed function"
    );
    let rendered = parsed_functions
        .iter()
        .map(|function| function.sprint(&parsed_pipeline))
        .collect::<Vec<_>>()
        .join("\n");

    let mut reparsed_pipeline: Pipeline<StageInfo<CallableLanguage>> = Pipeline::new();
    let reparsed_functions = reparsed_pipeline
        .parse(&rendered)
        .expect("rendered pipeline should reparse");
    assert_eq!(
        reparsed_functions.len(),
        parsed_functions.len(),
        "expected same number of parsed functions"
    );
    let rendered_again = reparsed_functions
        .iter()
        .map(|function| function.sprint(&reparsed_pipeline))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(rendered.trim_end(), rendered_again.trim_end());
}

#[test]
fn test_bind_roundtrip_with_multiple_captures() {
    assert_roundtrip(
        "%f = bind @closure captures(%x, %y) -> i32",
        &[("x", ArithType::I32), ("y", ArithType::I32)],
    );
}

#[test]
fn test_call_roundtrip_with_multiple_arguments() {
    assert_roundtrip(
        "%r = call @closure(%x, %y) -> i32",
        &[("x", ArithType::I32), ("y", ArithType::I32)],
    );
}

#[test]
fn test_return_roundtrip_and_terminator_property() {
    let input = "ret %x";
    let (stage, statement) = emit_statement(input, &[("x", ArithType::I32)]);
    assert_eq!(render_statement(&stage, statement).trim(), input);

    let dialect = statement
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(dialect.is_terminator(), "return should be a terminator");
}

#[test]
fn test_lowered_function_roundtrip_print_parse_print() {
    let input = r#"
stage @A fn @main(i32) -> i32;
stage @A fn @closure(i32, i32) -> i32;

specialize @A fn @closure(i32, i32) -> i32 {
  ^bb0(%capt0: i32, %arg0: i32) {
    ret %arg0;
  }
}

specialize @A fn @main(i32) -> i32 {
  ^bb0(%x: i32) {
    %f = bind @closure captures(%x) -> i32;
    %r_call = call @closure(%x, %x) -> i32;
    ret %r_call;
  }
}
"#;

    assert_pipeline_roundtrip(input);
}
