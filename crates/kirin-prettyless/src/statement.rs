use crate::printer::{ArenaDoc, Printer};
use prettyless::{DocAllocator, DocBuilder};
use kirin_ir::*;

pub trait PrettyPrint<L: Dialect + PrettyPrint<L> + From<Self>>: Dialect {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a, L>) -> ArenaDoc<'a>;
}

impl<'a, L: Dialect + PrettyPrint<L>> Printer<'a, L> {
    pub fn print_statement(&'a self, statement: Statement) -> ArenaDoc<'a> {
        let info = statement.expect_info(self.context);
        info.definition().pretty_print(self)
    }
}

impl<'a, L: Dialect> Printer<'a, L> {
    pub fn print_statement_default(&'a self, statement: Statement) -> ArenaDoc<'a> {
        let info = statement.expect_info(self.context);
        let def = info.definition();
        let doc = self.arena.text(format!("statement {:?}", def));
        doc
    }
}
