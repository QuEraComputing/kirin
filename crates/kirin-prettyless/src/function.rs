use crate::{PrettyPrint, printer::{ArenaDoc, Printer}};
use kirin_ir::*;

impl<'a, L: Dialect + PrettyPrint<L>> Printer<'a, L> {
    pub fn print_specialized_function(&'a self, f: SpecializedFunction) -> ArenaDoc<'a> {
        let info = f.expect_info(self.context);
        let body = info.body();
        self.print_statement(*body)
    }
}
