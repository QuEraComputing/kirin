//! Symbol access must include wrapped operations and their extra attributes.

mod common;

use common::TestType;
use kirin_ir::{Dialect, HasSymbols, HasSymbolsMut, Symbol};

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = TestType, crate = kirin_ir)]
struct Named {
    target: Symbol,
}

#[derive(Clone, Debug, PartialEq, Dialect)]
#[kirin(type = TestType, crate = kirin_ir)]
enum Wrapper<L: kirin_ir::Dialect<Type = TestType>> {
    Named {
        #[wraps]
        inner: L,
        aliases: Vec<Symbol>,
        fallback: Option<Symbol>,
    },
}

#[test]
fn symbol_access_includes_wrapped_and_collection_fields() {
    let mut op = Wrapper::Named {
        inner: Named { target: 0.into() },
        aliases: vec![1.into(), 2.into()],
        fallback: Some(3.into()),
    };
    assert_eq!(
        op.symbols().copied().collect::<Vec<_>>(),
        [0.into(), 1.into(), 2.into(), 3.into()]
    );
    for symbol in op.symbols_mut() {
        *symbol = Symbol::from(usize::from(*symbol) + 10);
    }
    let Wrapper::Named {
        inner,
        aliases,
        fallback,
    } = op;
    assert_eq!(inner.target, 10.into());
    assert_eq!(aliases, [11.into(), 12.into()]);
    assert_eq!(fallback, Some(13.into()));
}
