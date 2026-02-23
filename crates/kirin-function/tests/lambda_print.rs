use kirin::prelude::*;
use kirin_function::{Bind, Return};
use kirin_test_languages::SimpleType;
use kirin_test_utils::roundtrip;

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
