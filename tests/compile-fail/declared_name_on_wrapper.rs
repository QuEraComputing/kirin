use kirin::ir::{Dialect, Symbol};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
#[wraps]
struct Invalid(#[kirin(declared_name)] Symbol);

fn main() {}
