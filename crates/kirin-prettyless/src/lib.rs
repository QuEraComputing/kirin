use std::ops::Deref;

use kirin_ir::*;
use prettyless::{Arena, DocBuilder};

pub use prettyless::DocAllocator;
pub type ArenaDoc<'a> = DocBuilder<'a, Arena<'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Config {
    /// Number of spaces to use for each indentation level.
    pub tab_spaces: usize,
    /// Maximum width of each line.
    pub max_width: usize,
    /// Whether to include line numbers in the output.
    pub line_numbers: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_spaces: 4,
            max_width: 120,
            line_numbers: true,
        }
    }
}

impl Config {
    pub fn with_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    pub fn with_tab_spaces(mut self, spaces: usize) -> Self {
        self.tab_spaces = spaces;
        self
    }

    pub fn with_line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }
}

pub struct Printer<'a> {
    pub config: Config,
    pub arena: Arena<'a>,
}

impl<'a> Printer<'a> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            arena: Arena::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}

impl<'a> Deref for Printer<'a> {
    type Target = Arena<'a>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

pub trait PrettyPrint<L: Dialect> {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a>;
}

impl<L: Dialect> PrettyPrint<L> for StagedFunction {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let info = self.expect_info(context);
        let name = info
            .name()
            .and_then(|n| context.symbol_table().borrow().resolve(*n).cloned());
        printer.text(name.unwrap_or_else(|| "<unnamed function>".into()))
            + printer.text(format!(
                "staged function with {} specializations",
                info.specializations().len()
            ))
    }
}

impl<L: Dialect> PrettyPrint<L> for SpecializedFunction {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let info = self.expect_info(context);
        let body = info.body();
        body.pretty_print(printer, context)
    }
}

impl<L: Dialect> PrettyPrint<L> for Statement {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let stmt_info = self.expect_info(context);
        let def = stmt_info.definition();
        def.pretty_print(printer, context)
    }
}

impl<L: Dialect> PrettyPrint<L> for Successor {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, _context: &Context<L>) -> ArenaDoc<'a> {
        printer.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrint<L> for Region {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let mut inner = printer.nil();
        for block in self.blocks(context) {
            let block_doc = block.pretty_print(printer, context);
            inner += printer.hardline() + block_doc;
        }
        inner.enclose(
            printer.text("{") + printer.hardline(),
            printer.hardline() + "}",
        )
    }
}

impl<L: Dialect> PrettyPrint<L> for Block {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let mut inner = printer.nil();
        for stmt in self.statements(context) {
            let stmt_info = stmt.expect_info(context);
            let def = stmt_info.definition();
            inner += def.pretty_print(printer, context)
        }
        printer.text(format!("{}:", self)) + printer.hardline() + inner
    }
}

trait FallbackPrettyPrint<L: Dialect> {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a>;
}

impl<L: Dialect> FallbackPrettyPrint<L> for &L {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        printer.text(format!("<unprintable {}>", self.name()))
    }
}
