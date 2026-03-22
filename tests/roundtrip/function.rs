use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_function::{Bind, Lambda, Return};
use kirin_test_languages::{CallableLanguage, SimpleType};
use kirin_test_utils::roundtrip;

// --- Split signature projection tests ---

// Dialect using split signature projections: {sig:inputs} and {sig:return}
// separately parse and print the Signature's input types and return type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum SplitSigLanguage {
    #[chumsky(format = "fn {:name}({sig:inputs}) -> {sig:return} {body}")]
    Function {
        body: Region,
        sig: Signature<SimpleType>,
    },
    #[wraps]
    Return(Return<SimpleType>),
}

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

// --- Tests from lambda_print.rs ---

// Lambda (Region-containing) works with #[wraps] delegation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
enum LambdaLanguage {
    #[wraps]
    Lambda(Lambda<SimpleType>),
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

// --- Split signature projection roundtrip tests ---

#[test]
fn test_split_sig_pipeline_multiple_params() {
    let input = r#"
stage @A fn @main(i32, i64) -> i32;

specialize @A fn @main(i32, i64) -> i32 {
  ^bb0(%x: i32, %y: i64) {
    ret %x;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<SplitSigLanguage>(input);
}

#[test]
fn test_split_sig_pipeline_single_param() {
    let input = r#"
stage @A fn @main(i32) -> i32;

specialize @A fn @main(i32) -> i32 {
  ^bb0(%x: i32) {
    ret %x;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<SplitSigLanguage>(input);
}

#[test]
fn test_split_sig_pipeline_many_params() {
    // Exercises split signature with three input types and a different return type
    let input = r#"
stage @A fn @compute(i32, i64, f32) -> f64;

specialize @A fn @compute(i32, i64, f32) -> f64 {
  ^bb0(%x: i32, %y: i64, %z: f32) {
    ret %x;
  }
}
"#;
    roundtrip::assert_pipeline_roundtrip::<SplitSigLanguage>(input);
}
