use crate::Location;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeContext<Token> {
    entries: Vec<Token>,
}

impl<Token> NodeContext<Token> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn entries(&self) -> &[Token] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<Token> {
        self.entries
    }
}

impl<Token> NodeContext<Token>
where
    Token: Clone,
{
    pub fn push(&self, token: Token, strategy: ContextStrategy) -> Self {
        let mut entries = self.entries.clone();
        entries.push(token);

        if entries.len() > strategy.k {
            entries.drain(0..entries.len() - strategy.k);
        }

        Self { entries }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextStrategy {
    pub k: usize,
}

impl ContextStrategy {
    pub fn new(k: usize) -> Self {
        Self { k }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SummaryKey<Token = Location> {
    pub location: Location,
    pub context: NodeContext<Token>,
}

impl<Token> SummaryKey<Token> {
    pub fn new(location: Location, context: NodeContext<Token>) -> Self {
        Self { location, context }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_context_keeps_the_latest_k_tokens() {
        let strategy = ContextStrategy::new(2);
        let context = NodeContext::new()
            .push("a", strategy)
            .push("b", strategy)
            .push("c", strategy);

        assert_eq!(context.entries(), &["b", "c"]);
    }

    #[test]
    fn zero_context_collapses_all_tokens() {
        let context = NodeContext::new().push("call", ContextStrategy::new(0));

        assert!(context.entries().is_empty());
    }
}
