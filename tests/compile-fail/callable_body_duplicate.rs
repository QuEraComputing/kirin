use kirin::ir::{Block, CFG, Dialect};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Invalid {
    #[kirin(callable_body)]
    first: CFG,
    #[kirin(callable_body)]
    second: Block,
}

fn main() {}
