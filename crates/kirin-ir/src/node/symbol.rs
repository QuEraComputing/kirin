use crate::intern::InternKey;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Symbol(usize);

impl From<usize> for Symbol {
    fn from(id: usize) -> Self {
        Symbol(id)
    }
}

impl From<Symbol> for usize {
    fn from(symbol: Symbol) -> Self {
        symbol.0
    }
}

impl InternKey for Symbol {}
