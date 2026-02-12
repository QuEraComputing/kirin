use crate::{Document, PrettyPrint, ScanResultWidth};
use bat::PrettyPrinter;
use kirin_ir::*;

/// Print a string through the bat pager with shared config (language, line numbers, etc.).
pub(crate) fn print_str(s: &str) {
    PrettyPrinter::new()
        .input_from_bytes(s.as_bytes())
        .language("llvm")
        .line_numbers(true)
        .grid(true)
        .paging_mode(bat::PagingMode::Always)
        .print()
        .unwrap();
}

impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::Type: std::fmt::Display,
{
    pub fn pager<N>(&'a mut self, node: &N) -> Result<(), std::fmt::Error>
    where
        N: ScanResultWidth<L> + PrettyPrint,
    {
        let rendered = self.render(node)?;
        print_str(&rendered);
        Ok(())
    }
}
