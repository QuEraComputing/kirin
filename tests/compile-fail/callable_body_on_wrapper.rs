use kirin::ir::{Dialect, CFG};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = kirin_test_languages::SimpleType)]
#[wraps]
struct Invalid(#[kirin(callable_body)] CFG);

fn main() {}
