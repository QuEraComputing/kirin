use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_function::{Bind, Return};
use kirin_test_languages::{CallableLanguage, SimpleType};
use kirin_test_utils::roundtrip;

// --- Tests from function_roundtrip.rs (using shared CallableLanguage) ---

#[test]
fn test_bind_roundtrip_with_multiple_captures() {
    roundtrip::assert_statement_roundtrip::<CallableLanguage>(
        "%f = bind @closure captures(%x, %y) -> i32",
        &[("x", ArithType::I32), ("y", ArithType::I32)],
    );
}

#[test]
fn test_call_roundtrip_with_multiple_arguments() {
    roundtrip::assert_statement_roundtrip::<CallableLanguage>(
        "%r = call @closure(%x, %y) -> i32",
        &[("x", ArithType::I32), ("y", ArithType::I32)],
    );
}

#[test]
fn test_return_roundtrip_and_terminator_property() {
    let input = "ret %x";
    let (stage, statement) =
        roundtrip::emit_statement::<CallableLanguage>(input, &[("x", ArithType::I32)]);
    assert_eq!(
        roundtrip::render_statement::<CallableLanguage>(&stage, statement).trim(),
        input
    );

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
    roundtrip::assert_pipeline_roundtrip::<CallableLanguage>(input);
}

// --- Tests from lambda_print.rs (LambdaLanguage must stay inline due to E0275) ---

// NOTE: Lambda (and other Region-containing types) cannot currently be used
// with #[wraps] + HasParser due to recursive trait resolution overflow (E0275).
// We inline the lambda fields here to test the format-derived parser path
// for Vec<SSAValue> + Region combos.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum LambdaLanguage {
    #[chumsky(format = "{res:name} = {.lambda} {name} captures({captures}) {body} -> {res:type}")]
    Lambda {
        name: Symbol,
        captures: Vec<SSAValue>,
        body: Region,
        #[kirin(type = SimpleType::placeholder())]
        res: ResultValue,
    },
    #[wraps]
    Bind(Bind<SimpleType>),
    #[wraps]
    Return(Return<SimpleType>),
}

#[test]
fn test_lambda_parse_roundtrip() {
    roundtrip::assert_statement_roundtrip::<LambdaLanguage>(
        "%f = lambda @closure captures(%x, %y) { } -> i32",
        &[("x", SimpleType::I32), ("y", SimpleType::I32)],
    );
}

#[test]
fn test_lambda_parse_roundtrip_single_capture() {
    roundtrip::assert_statement_roundtrip::<LambdaLanguage>(
        "%f = lambda @closure captures(%x) { } -> i32",
        &[("x", SimpleType::I32)],
    );
}
