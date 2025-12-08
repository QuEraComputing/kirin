use std::{borrow::Cow, ops::Deref};

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
    config: Config,
    arena: Arena<'a>,
    result_width: usize,
}

impl<'a> Printer<'a> {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            arena: Arena::new(),
            result_width: 0,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        doc.nest(self.config.tab_spaces as isize)
    }

    pub fn block_indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        self.indent(self.arena.line_() + doc)
    }

    pub fn list<I, U: Clone + Into<Cow<'a, str>>>(
        &'a self,
        items: impl Iterator<Item = I>,
        sep: U,
        f: impl Fn(I) -> ArenaDoc<'a>,
    ) -> ArenaDoc<'a> {
        let mut doc = self.nil();
        let mut first = true;
        for item in items {
            if !first {
                doc += self.text(sep.clone());
            }
            doc += f(item);
            first = false;
        }
        doc
    }
}

impl<'a> Deref for Printer<'a> {
    type Target = Arena<'a>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

trait ScanResultWidth<L: Dialect> {
    fn scan_result_width(&self, context: &Context<L>) -> usize;
}

macro_rules! impl_scan_result_with {
    ($name:ident) => {
        impl<L: Dialect> ScanResultWidth<L> for $name {
            fn scan_result_width(&self, context: &Context<L>) -> usize {
                let ssa_value: SSAValue = self.clone().into();
                let info = ssa_value.expect_info(context);
                if let Some(name) = info.name() {
                    if let Some(resolved_name) = context.symbol_table().borrow().resolve(name) {
                        return resolved_name.len();
                    }
                }
                ssa_value.to_string().len()
            }
        }
    };
}

impl_scan_result_with!(SSAValue);
impl_scan_result_with!(ResultValue);
impl_scan_result_with!(BlockArgument);
impl_scan_result_with!(DeletedSSAValue);

impl<L: Dialect> ScanResultWidth<L> for Statement {
    fn scan_result_width(&self, context: &Context<L>) -> usize {
        self.results(context)
            .map(|result| result.scan_result_width(context) + 2)
            .sum::<usize>()
            - 1
    }
}

impl<L: Dialect> ScanResultWidth<L> for Block {
    fn scan_result_width(&self, context: &Context<L>) -> usize {
        let mut max_width = 0;
        for stmt in self.statements(context) {
            for result in stmt.results(context) {
                let width = result.scan_result_width(context);
                if width > max_width {
                    max_width = width;
                }
            }
        }
        max_width
    }
}

impl<L: Dialect> ScanResultWidth<L> for Region {
    fn scan_result_width(&self, context: &Context<L>) -> usize {
        let mut max_width = 0;
        for block in self.blocks(context) {
            let width = block.scan_result_width(context);
            if width > max_width {
                max_width = width;
            }
        }
        max_width
    }
}

impl<L: Dialect> ScanResultWidth<L> for SpecializedFunction {
    fn scan_result_width(&self, context: &Context<L>) -> usize {
        let info = self.expect_info(context);
        let body = info.body();
        body.scan_result_width(context)
    }
}

impl<L: Dialect> ScanResultWidth<L> for StagedFunction {
    fn scan_result_width(&self, context: &Context<L>) -> usize {
        let info = self.expect_info(context);
        let mut max_width = 0;
        for specialization in info.specializations() {
            let width = specialization.body().scan_result_width(context);
            if width > max_width {
                max_width = width;
            }
        }
        max_width
    }
}

pub trait PrettyPrint<L: Dialect + PrettyPrint<L>> {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a>;
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for StagedFunction {
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

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for SpecializedFunction {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let info = self.expect_info(context);
        let body = info.body();
        body.pretty_print(printer, context)
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Statement {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let stmt_info = self.expect_info(context);
        let def = stmt_info.definition();
        def.pretty_print(printer, context)
    }
}

macro_rules! impl_ssa_pretty_print {
    ($name:ident) => {
        impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for $name {
            fn pretty_print<'a>(
                &self,
                printer: &'a Printer<'a>,
                context: &Context<L>,
            ) -> ArenaDoc<'a> {
                let info = self.expect_info(context);
                if let Some(name) = info.name() {
                    if let Some(resolved_name) = context.symbol_table().borrow().resolve(name) {
                        return printer.text(format!("%{}", resolved_name));
                    }
                }
                printer.text(self.to_string())
            }
        }
    };
}

impl_ssa_pretty_print!(SSAValue);
impl_ssa_pretty_print!(ResultValue);
impl_ssa_pretty_print!(BlockArgument);
impl_ssa_pretty_print!(DeletedSSAValue);

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Successor {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let info = self.expect_info(context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = context.symbol_table().borrow().resolve(name) {
                return printer.text(format!("^{}", resolved_name));
            }
        }
        printer.text(self.to_string())
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Region {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let mut inner = printer.nil();
        for block in self.blocks(context) {
            inner += block.pretty_print(printer, context);
            inner += printer.line_();
        }
        printer.block_indent(inner).enclose("{", "}")
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Block {
    fn pretty_print<'a>(&self, printer: &'a Printer<'a>, context: &Context<L>) -> ArenaDoc<'a> {
        let mut inner = printer.nil();
        for (i, stmt) in self.statements(context).enumerate() {
            if i > 0 {
                inner += printer.line_();
            }

            let mut doc_results = printer.nil();
            for (i, result) in stmt.results(context).enumerate() {
                if i > 0 {
                    doc_results += printer.text(", ");
                }
                doc_results += result.pretty_print(printer, context);
            }
            if !doc_results.is_nil() {
                inner += doc_results + printer.text(" = ");
            }
            inner += stmt.pretty_print(printer, context);
        }
        if let Some(terminator) = self.terminator(context) {
            if !inner.is_nil() {
                inner += printer.line_();
            }
            inner += terminator.pretty_print(printer, context)
        }
        printer.text(format!("{}:", self)) + printer.block_indent(inner)
    }
}

pub fn pretty_print_fallback<'a, L: Dialect + PrettyPrint<L>>(
    statement: &L,
    printer: &'a Printer<'a>,
    context: &Context<L>,
) -> ArenaDoc<'a> {
    let mut doc = printer.nil();
    doc += printer
        .list(statement.arguments(), ",", |arg| {
            arg.pretty_print(printer, context)
        })
        .enclose("(", ")");

    doc += printer
        .list(statement.results(), ",", |arg| {
            arg.pretty_print(printer, context)
        })
        .enclose("(", ")");
    doc
}
