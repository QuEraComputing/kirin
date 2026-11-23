use kirin::ir::{Dialect, Symbol};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Optional {
    #[kirin(declared_name)]
    name: Option<Symbol>,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Multiple {
    #[kirin(declared_name)]
    names: Vec<Symbol>,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
struct Unsupported {
    #[kirin(declared_name)]
    value: i64,
}

fn main() {}
