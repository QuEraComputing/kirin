use kirin::prelude::*;

/// The matched op an [`Erase`](crate::Erase) or [`Rewrite`](crate::Rewrite)
/// acts on, held as the op's `Statement` id so that an op without results
/// can be named too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

pub struct OpRef(pub(crate) Statement);

impl From<Statement> for OpRef {
    fn from(op: Statement) -> Self {
        Self(op)
    }
}

/// The printer shows the op's first result, as in `wildcard.erase %r`. An op
/// without results prints quoted, followed by its 0-based position in the
/// pattern body to tell identical ops apart, as in
/// `wildcard.erase 'effect %n' (#2)`. The quote collapses the op's nested
/// bodies to `{...}` (see [`Document::print_statement_abbreviated`]).
impl PrettyPrint for OpRef {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        let stage = doc.stage();
        match self.0.results(stage).next() {
            Some(&result) => doc.print_ssa_ref(result),
            None => {
                let position =
                    std::iter::successors(*self.0.prev(stage), |stmt| *stmt.prev(stage)).count();
                doc.text("'")
                    + doc.print_statement_abbreviated(&self.0)
                    + doc.text(format!("' (#{position})"))
            }
        }
    }
}
