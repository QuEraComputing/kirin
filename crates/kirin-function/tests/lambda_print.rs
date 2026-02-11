use kirin::prelude::*;
use kirin::pretty::{Config, Document};
use kirin_function::{Bind, Return};
use kirin_test_utils::SimpleType;

// NOTE: Lambda (and other Region-containing types) cannot currently be used
// with #[wraps] + HasParser due to recursive trait resolution overflow (E0275).
// We inline the lambda fields here to test the format-derived parser path
// for Vec<SSAValue> + Region combos. The same limitation affects Lexical<T>
// and Lifted<T> when instantiated with parse().
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum LambdaLanguage {
    #[chumsky(format = "{res:name} = lambda {name} captures({captures}) {body} -> {res:type}")]
    Lambda {
        name: Symbol,
        captures: Vec<SSAValue>,
        body: Region,
        #[kirin(type = SimpleType::default())]
        res: ResultValue,
    },
    #[wraps]
    Bind(Bind<SimpleType>),
    #[wraps]
    Return(Return<SimpleType>),
}

fn emit_statement(
    input: &str,
    operands: &[(&str, SimpleType)],
) -> (StageInfo<LambdaLanguage>, Statement) {
    let mut stage: StageInfo<LambdaLanguage> = StageInfo::default();

    for (name, ty) in operands {
        stage
            .ssa()
            .name((*name).to_string())
            .ty(ty.clone())
            .kind(SSAKind::Test)
            .new();
    }

    let statement = parse::<LambdaLanguage>(input, &mut stage)
        .expect("lambda statement parse should succeed");
    (stage, statement)
}

fn render_statement(stage: &StageInfo<LambdaLanguage>, statement: Statement) -> String {
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

fn assert_roundtrip(input: &str, operands: &[(&str, SimpleType)]) {
    let (stage, statement) = emit_statement(input, operands);
    let rendered = render_statement(&stage, statement);

    let (reparsed_stage, reparsed_statement) = emit_statement(rendered.trim(), operands);
    let rendered_again = render_statement(&reparsed_stage, reparsed_statement);

    assert_eq!(rendered.trim(), rendered_again.trim());
}

#[test]
fn test_lambda_parse_roundtrip() {
    assert_roundtrip(
        "%f = lambda @closure captures(%x, %y) { } -> i32",
        &[("x", SimpleType::I32), ("y", SimpleType::I32)],
    );
}

#[test]
fn test_lambda_parse_roundtrip_single_capture() {
    assert_roundtrip(
        "%f = lambda @closure captures(%x) { } -> i32",
        &[("x", SimpleType::I32)],
    );
}
