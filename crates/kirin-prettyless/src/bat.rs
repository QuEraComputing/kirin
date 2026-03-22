use crate::{Document, PrettyPrint};
use bat::PrettyPrinter;
use kirin_ir::Dialect;

/// Print a string through the bat pager with shared config (language, line numbers, etc.).
pub(crate) fn print_str(s: &str) -> Result<(), std::io::Error> {
    PrettyPrinter::new()
        .input_from_bytes(s.as_bytes())
        .language("llvm")
        .line_numbers(true)
        .grid(true)
        .paging_mode(bat::PagingMode::Always)
        .print()
        .map_err(std::io::Error::other)?;
    Ok(())
}

impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::Type: std::fmt::Display,
{
    pub fn pager<N>(&'a mut self, node: &N) -> Result<(), crate::RenderError>
    where
        N: PrettyPrint,
    {
        let rendered = self.render(node)?;
        print_str(&rendered)?;
        Ok(())
    }
}
