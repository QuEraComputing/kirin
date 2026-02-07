use crate::intern::InternKey;

/// A stage-local interned symbol.
///
/// Used for SSA value names, block names, and other identifiers that are
/// local to a single compilation stage's [`StageInfo`](crate::StageInfo).
/// Interned via `Context.symbols`.
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

/// A cross-stage interned symbol.
///
/// Used for function names and other identifiers that must be consistent
/// across multiple compilation stages. Interned via `Pipeline.global_symbols`.
///
/// Distinct from [`Symbol`] at the type level to prevent accidental mixing
/// of global and stage-local symbols.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct GlobalSymbol(usize);

impl From<usize> for GlobalSymbol {
    fn from(id: usize) -> Self {
        GlobalSymbol(id)
    }
}

impl From<GlobalSymbol> for usize {
    fn from(symbol: GlobalSymbol) -> Self {
        symbol.0
    }
}

impl InternKey for GlobalSymbol {}
