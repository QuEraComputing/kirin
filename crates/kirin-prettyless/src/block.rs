// use kirin_ir::*;
// use prettyless::DocAllocator;

// use crate::{printer::{ArenaDoc, Printer}, statement::PrettyPrint};

// impl<'a, L: Dialect + PrettyPrint<L>> Printer<'a, L> {
//     pub fn print_block(
//         &'a self,
//         block: Block,
//     ) -> ArenaDoc<'a> {
//         let mut inner = self.arena.nil();
//         for statement in block.statements(self.context) {
//             let stmt_info = statement.expect_info(self.context);
//             let stmt_doc = stmt_info.definition().pretty_print(self);
//             inner += self.arena.hardline() + stmt_doc;
//         }
//         inner
//     }
// }

// impl<'a, L: Dialect> Printer<'a, L> {
//     pub fn print_block(
//         &'a self,
//         block: Block,
//     ) -> ArenaDoc<'a> {
//         let mut inner = self.arena.nil();
//         for statement in block.statements(self.context) {
//             let stmt_doc = self.print_statement_default(statement);
//             inner += self.arena.hardline() + stmt_doc;
//         }
//         inner
//     }
// }
