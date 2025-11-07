#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Symbol(usize);

#[derive(Clone, Debug, Default)]
pub struct InternTable {
    symbols: Vec<String>,
    symbol_map: std::collections::HashMap<String, Symbol>,
}

impl InternTable {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            symbol_map: std::collections::HashMap::new(),
        }
    }

    pub fn intern(&mut self, name: impl AsRef<str>) -> Symbol {
        let name = name.as_ref();
        if let Some(&symbol) = self.symbol_map.get(name) {
            return symbol;
        }
        let symbol = Symbol(self.symbols.len());
        self.symbols.push(name.to_string());
        self.symbol_map.insert(name.to_string(), symbol);
        symbol
    }

    pub fn resolve(&self, symbol: Symbol) -> Option<&str> {
        self.symbols.get(symbol.0).map(|s| s.as_str())
    }
}
