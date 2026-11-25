use kirin_ir::{Dialect, GetInfo, Module};
use prettyless::DocAllocator;

use crate::{ArenaDoc, Document, PrettyPrint};

impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::Type: std::fmt::Display,
{
    /// Print `mod @name { statement; ... }`, without a stage prefix.
    /// Program text uses [`Document::print_stage_module`].
    pub fn print_module(&'a self, module: &Module) -> ArenaDoc<'a> {
        let info = module.expect_info(self.stage);
        let mut header = self.text("mod");
        if let Some(name) = info.name() {
            header += self.text(" ") + name.pretty_print(self);
        }
        let mut inner = self.nil();
        for (i, member) in module.members(self.stage).enumerate() {
            if i > 0 {
                inner += self.line_();
            }
            inner += self.print_statement(&member) + self.text(";");
        }
        header + self.text(" {") + self.block_indent(inner) + self.line_() + self.text("}")
    }
}

impl PrettyPrint for Module {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_module(self)
    }
}
