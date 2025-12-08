use crate::{Document, PrettyPrint, ScanResultWidth};
use bat::PrettyPrinter;
use kirin_ir::*;

impl<'a, L: Dialect> Document<'a, L> {
    pub fn pager<N>(&'a mut self, node: N) -> Result<(), std::fmt::Error>
    where
        N: ScanResultWidth<L> + PrettyPrint<L>,
    {
        let rendered = self.render(node)?;
        PrettyPrinter::new()
            .input_from_bytes(rendered.as_bytes())
            .language("llvm")
            .line_numbers(true)
            .grid(true)
            .paging_mode(bat::PagingMode::Always)
            .print()
            .unwrap();
        Ok(())
    }
}
