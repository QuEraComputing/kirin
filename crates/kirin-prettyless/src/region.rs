// use kirin_ir::*;
// use prettyless::DocAllocator;

// use crate::{PrettyPrint, printer::{ArenaDoc, Printer}};

// impl<'a, L: Dialect + PrettyPrint<L>> Printer<'a, L> {
//     pub fn print_region(
//         &'a self,
//         region: Region,
//     ) -> ArenaDoc<'a> {
//         let mut inner = self.arena.nil();
//         for block in region.blocks(self.context) {
//             let block_doc = self.print_block(block);
//             inner += self.arena.hardline() + block_doc;
//         }
//         inner.enclose("region {\n", "\n}")
//     }
// }
