use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_function::{Bind, Call, Return};
use kirin_test_utils::roundtrip;

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
