use kirin::ir::{Block, CFG, Dialect};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Optional {
    #[kirin(callable_body)]
    body: Option<CFG>,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Multiple {
    #[kirin(callable_body)]
    body: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Unsupported {
    #[kirin(callable_body)]
    value: i64,
}

fn main() {}
