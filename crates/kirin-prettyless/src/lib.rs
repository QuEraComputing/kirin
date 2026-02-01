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
            tab_spaces: 2,
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

pub struct Document<'a, L: Dialect> {
    config: Config,
    arena: Arena<'a>,
    context: &'a Context<L>,
    result_width: DenseHint<Statement, usize>,
    max_result_width: usize,
}

impl<'a, L: Dialect> Document<'a, L> {
    pub fn new(config: Config, context: &'a Context<L>) -> Self {
        let arena = Arena::new();
        Self {
            config,
            arena,
            context,
            result_width: context.statement_arena().hint().dense(),
            max_result_width: 0,
        }
    }

    pub fn indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        doc.nest(self.config.tab_spaces as isize)
    }

    pub fn block_indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        self.indent(self.arena.line_() + doc)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns a reference to the IR context.
    pub fn context(&self) -> &'a Context<L> {
        self.context
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

    fn build<N>(&'a mut self, node: N) -> ArenaDoc<'a>
    where
        N: ScanResultWidth<L> + PrettyPrint<L>,
    {
        node.scan_result_width(self);
        node.pretty_print(self)
    }

    pub fn render<N>(&'a mut self, node: N) -> Result<String, std::fmt::Error>
    where
        N: ScanResultWidth<L> + PrettyPrint<L>,
    {
        let max_width = self.config.max_width;
        let arena_doc = self.build(node);
        let mut buf = String::new();
        arena_doc.render_fmt(max_width, &mut buf)?;
        Ok(strip_trailing_whitespace(&buf))
    }
}

fn strip_trailing_whitespace(s: &str) -> String {
    if s.is_empty() {
        return "\n".to_string();
    }
    let mut res = String::with_capacity(s.len());
    for line in s.lines() {
        res.push_str(line.trim_end());
        res.push('\n');
    }
    res
}

impl<'a, L: Dialect> Deref for Document<'a, L> {
    type Target = Arena<'a>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

pub trait PrettyPrint<L: Dialect> {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>;
}

pub trait ScanResultWidth<L: Dialect> {
    fn scan_result_width(&self, doc: &mut Document<'_, L>);
}

impl<L: Dialect> ScanResultWidth<L> for Statement {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        let mut len = 0;
        for result in self.results(doc.context) {
            let info = result.expect_info(doc.context);
            let mut result_len = result.to_string().len();
            if let Some(name) = info.name() {
                if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                    result_len = resolved_name.len();
                }
            }
            len += result_len + 2; // account for ", "
        }
        if len > 0 {
            len -= 2; // remove last ", "
        }

        doc.result_width.insert(*self, len);
        if len > doc.max_result_width {
            doc.max_result_width = len;
        }

        for block in self.blocks(doc.context) {
            block.scan_result_width(doc);
        }

        for region in self.regions(doc.context) {
            region.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> ScanResultWidth<L> for Block {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        for stmt in self.statements(doc.context) {
            stmt.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> ScanResultWidth<L> for Region {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        for block in self.blocks(doc.context) {
            block.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> ScanResultWidth<L> for SpecializedFunction {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        let info = self.expect_info(doc.context);
        let body = info.body();
        body.scan_result_width(doc);
    }
}

impl<L: Dialect> ScanResultWidth<L> for StagedFunction {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        let info = self.expect_info(doc.context);
        for specialization in info.specializations() {
            let body = specialization.body();
            body.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> PrettyPrint<L> for ResultValue {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrint<L> for SSAValue {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrint<L> for Successor {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        doc.text(self.to_string())
    }
}

/// Trait for types that can print their name (e.g., SSA values).
pub trait PrettyPrintName<L: Dialect> {
    /// Pretty print just the name part (e.g., `%x` for an SSA value).
    fn pretty_print_name<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>;
}

/// Trait for types that can print their type annotation.
pub trait PrettyPrintType<L: Dialect> {
    /// Pretty print just the type part (e.g., `i32` for a typed value).
    fn pretty_print_type<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>;
}

impl<L: Dialect> PrettyPrintName<L> for SSAValue {
    fn pretty_print_name<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrintName<L> for ResultValue {
    fn pretty_print_name<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl<L: Dialect> PrettyPrintType<L> for SSAValue
where
    L::TypeLattice: std::fmt::Display,
{
    fn pretty_print_type<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        doc.text(format!("{}", info.ty()))
    }
}

impl<L: Dialect> PrettyPrintType<L> for ResultValue
where
    L::TypeLattice: std::fmt::Display,
{
    fn pretty_print_type<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        doc.text(format!("{}", info.ty()))
    }
}

// Blanket impls for references - allows calling on &T when the impl is on T
impl<L: Dialect, T: PrettyPrintName<L>> PrettyPrintName<L> for &T {
    fn pretty_print_name<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        (*self).pretty_print_name(doc)
    }
}

impl<L: Dialect, T: PrettyPrintType<L>> PrettyPrintType<L> for &T {
    fn pretty_print_type<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        (*self).pretty_print_type(doc)
    }
}

impl<L: Dialect, T: PrettyPrint<L>> PrettyPrint<L> for &T {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        (*self).pretty_print(doc)
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Statement {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let stmt_info = self.expect_info(doc.context);
        let def = stmt_info.definition();
        def.pretty_print(doc)
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Block {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let mut inner = doc.nil();
        for (i, stmt) in self.statements(doc.context).enumerate() {
            if i > 0 {
                inner += doc.line_();
            }

            let mut doc_results = doc.spaces(doc.max_result_width - doc.result_width[stmt]);
            for (i, result) in stmt.results(doc.context).enumerate() {
                if i > 0 {
                    doc_results += doc.text(", ");
                }
                doc_results += result.pretty_print(doc);
            }
            if !doc_results.is_nil() {
                inner += doc_results + doc.text(" = ");
            }
            inner += stmt.pretty_print(doc);
        }
        if let Some(terminator) = self.terminator(doc.context) {
            if !inner.is_nil() {
                inner += doc.line_();
            }
            inner += terminator.pretty_print(doc)
        }
        doc.text(format!("{}:", self)) + doc.block_indent(inner)
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Region {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let mut inner = doc.nil();
        for block in self.blocks(doc.context) {
            inner += block.pretty_print(doc);
            inner += doc.line_();
        }
        doc.block_indent(inner).enclose("{", "}")
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for SpecializedFunction {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        let body = info.body();
        body.pretty_print(doc)
    }
}

impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for StagedFunction {
    fn pretty_print<'a>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        let name = info
            .name()
            .and_then(|n| doc.context.symbol_table().borrow().resolve(*n).cloned());
        doc.text(name.unwrap_or_else(|| "<unnamed function>".into()))
            + doc.text(format!(
                "staged function with {} specializations",
                info.specializations().len()
            ))
    }
}

#[cfg(feature = "bat")]
mod bat;

#[cfg(test)]
mod tests;
