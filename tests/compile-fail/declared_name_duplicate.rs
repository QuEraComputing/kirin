use kirin::ir::{Dialect, Symbol};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Invalid {
    #[kirin(declared_name)]
    first: Symbol,
    #[kirin(declared_name)]
    second: Symbol,
}

fn main() {}
