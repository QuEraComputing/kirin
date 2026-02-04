use std::{
    borrow::Cow,
    io::{Write, stdout},
    ops::Deref,
};

use kirin_ir::*;
use prettyless::{Arena, DocBuilder};

pub use prettyless::DocAllocator;
pub type ArenaDoc<'a> = DocBuilder<'a, Arena<'a>>;

pub mod prelude {
    pub use crate::{
        ArenaDoc, DocAllocator, Document, PrettyPrint, PrettyPrintExt, PrettyPrintName,
        PrettyPrintType,
    };
    pub use prettyless;
}

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

    fn build<N>(&'a mut self, node: &N) -> ArenaDoc<'a>
    where
        N: ScanResultWidth<L> + PrettyPrint,
        L: PrettyPrint,
        L::TypeLattice: std::fmt::Display,
    {
        node.scan_result_width(self);
        node.pretty_print(self)
    }

    pub fn render<N>(&'a mut self, node: &N) -> Result<String, std::fmt::Error>
    where
        N: ScanResultWidth<L> + PrettyPrint,
        L: PrettyPrint,
        L::TypeLattice: std::fmt::Display,
    {
        let max_width = self.config.max_width;
        let arena_doc = self.build(node);
        let mut buf = String::new();
        arena_doc.render_fmt(max_width, &mut buf)?;
        Ok(strip_trailing_whitespace(&buf))
    }
}

// Methods for printing IR nodes that need L: PrettyPrint bound
impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::TypeLattice: std::fmt::Display,
{
    /// Pretty print a statement by printing its definition.
    pub fn print_statement(&'a self, stmt: &Statement) -> ArenaDoc<'a> {
        let stmt_info = stmt.expect_info(self.context);
        let def = stmt_info.definition();
        def.pretty_print(self)
    }

    /// Pretty print a block with its header and statements.
    pub fn print_block(&'a self, block: &Block) -> ArenaDoc<'a> {
        let block_info = block.expect_info(self.context);

        // Build block header with arguments: ^name(%arg0: type, %arg1: type)
        // Look up block name from symbol table, fall back to ^index
        let block_name = block_info
            .name
            .and_then(|name_sym| {
                self.context
                    .symbol_table()
                    .borrow()
                    .resolve(name_sym)
                    .map(|s| format!("^{}", s))
            })
            .unwrap_or_else(|| format!("{}", block)); // Block's Display includes the ^
        let mut header = self.text(block_name);

        // Add arguments
        let args = &block_info.arguments;
        if !args.is_empty() {
            let mut args_doc = self.nil();
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    args_doc += self.text(", ");
                }
                let arg_info: &kirin_ir::Item<kirin_ir::SSAInfo<L>> = arg.expect_info(self.context);
                let name = if let Some(name_sym) = arg_info.name() {
                    self.context
                        .symbol_table()
                        .borrow()
                        .resolve(name_sym)
                        .cloned()
                        .unwrap_or_else(|| format!("{}", *arg))
                } else {
                    format!("{}", *arg)
                };
                args_doc += self.text(format!("%{}: {}", name, arg_info.ty()));
            }
            header += args_doc.enclose("(", ")");
        }

        // Build block body with statements
        let mut inner = self.nil();
        for (i, stmt) in block.statements(self.context).enumerate() {
            if i > 0 {
                inner += self.line_();
            }
            inner += self.print_statement(&stmt) + self.text(";");
        }
        if let Some(terminator) = block.terminator(self.context) {
            if !inner.is_nil() {
                inner += self.line_();
            }
            inner += self.print_statement(&terminator) + self.text(";");
        }

        header + self.text(" {") + self.block_indent(inner) + self.line_() + self.text("}")
    }

    /// Pretty print a region with its blocks.
    pub fn print_region(&'a self, region: &Region) -> ArenaDoc<'a> {
        let mut inner = self.nil();
        for block in region.blocks(self.context) {
            inner += self.print_block(&block);
            inner += self.line_();
        }
        self.block_indent(inner).enclose("{", "}")
    }

    /// Pretty print a specialized function.
    pub fn print_specialized_function(&'a self, func: &SpecializedFunction) -> ArenaDoc<'a> {
        let info = func.expect_info(self.context);
        let body = info.body();
        self.print_statement(body)
    }

    /// Pretty print a staged function.
    pub fn print_staged_function(&'a self, func: &StagedFunction) -> ArenaDoc<'a> {
        let info = func.expect_info(self.context);
        let name = info
            .name()
            .and_then(|n| self.context.symbol_table().borrow().resolve(*n).cloned());
        self.text(name.unwrap_or_else(|| "<unnamed function>".into()))
            + self.text(format!(
                "staged function with {} specializations",
                info.specializations().len()
            ))
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

/// Core trait for pretty printing values to a document.
///
/// This trait defines how a type should be rendered to a document representation.
/// The method is generic over the dialect type `L`, allowing the same implementation
/// to work with any `Document<L>`.
///
/// The bounds on `L` (`PrettyPrint` and `TypeLattice: Display`) are required because:
/// - `L: PrettyPrint` is needed to print nested Block/Region structures
/// - `TypeLattice: Display` is needed to print type annotations (`:type` format)
///
/// For IR nodes that require context (like `Statement`, `Block`, `Region`), use
/// the convenience methods provided on `Document` instead.
pub trait PrettyPrint {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display;
}

/// Extension trait providing convenience methods for pretty printing IR nodes.
///
/// This trait is automatically implemented for any type that implements both
/// `PrettyPrint` and `ScanResultWidth<L>`. All methods require a `&Context<L>`
/// parameter since IR nodes (like `Statement`, `Block`, `Region`, etc.) need to
/// look up their data from the context.
///
/// # Example
/// ```ignore
/// use kirin::pretty::{Config, PrettyPrintExt};
///
/// // Render to string with custom config
/// let output = statement.sprint_with_config(config, &context);
///
/// // Render to string with default config
/// let output = statement.sprint(&context);
/// ```
pub trait PrettyPrintExt<L: Dialect + PrettyPrint>: PrettyPrint + ScanResultWidth<L>
where
    L::TypeLattice: std::fmt::Display,
{
    /// Render to string with custom config.
    fn sprint_with_config(&self, config: Config, context: &Context<L>) -> String;

    /// Render to string with default config.
    fn sprint(&self, context: &Context<L>) -> String;

    /// Write to writer with custom config.
    fn write_with_config(&self, writer: &mut impl Write, config: Config, context: &Context<L>);

    /// Write to writer with default config.
    fn write(&self, writer: &mut impl Write, context: &Context<L>);

    /// Print to stdout with custom config.
    fn print_with_config(&self, config: Config, context: &Context<L>);

    /// Print to stdout with default config.
    fn print(&self, context: &Context<L>);

    /// Display with bat pager with custom config.
    #[cfg(feature = "bat")]
    fn bat_with_config(&self, config: Config, context: &Context<L>);

    /// Display with bat pager with default config.
    #[cfg(feature = "bat")]
    fn bat(&self, context: &Context<L>);
}

// Blanket implementation: any type that implements PrettyPrint + ScanResultWidth<L>
// automatically gets the context-aware convenience methods.
impl<L: Dialect + PrettyPrint, T: PrettyPrint + ScanResultWidth<L>> PrettyPrintExt<L> for T
where
    L::TypeLattice: std::fmt::Display,
{
    fn sprint_with_config(&self, config: Config, context: &Context<L>) -> String {
        let mut doc = Document::new(config, context);
        doc.render(self).expect("render failed")
    }

    fn sprint(&self, context: &Context<L>) -> String {
        let mut doc = Document::new(Config::default(), context);
        doc.render(self).expect("render failed")
    }

    fn write_with_config(&self, writer: &mut impl Write, config: Config, context: &Context<L>) {
        let mut doc = Document::new(config, context);
        let output = doc.render(self).expect("render failed");
        writer.write_all(output.as_bytes()).expect("write failed");
    }

    fn write(&self, writer: &mut impl Write, context: &Context<L>) {
        let mut doc = Document::new(Config::default(), context);
        let output = doc.render(self).expect("render failed");
        writer.write_all(output.as_bytes()).expect("write failed");
    }

    fn print_with_config(&self, config: Config, context: &Context<L>) {
        let mut doc = Document::new(config, context);
        let output = doc.render(self).expect("render failed");
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    fn print(&self, context: &Context<L>) {
        let mut doc = Document::new(Config::default(), context);
        let output = doc.render(self).expect("render failed");
        stdout().write_all(output.as_bytes()).expect("write failed");
    }

    #[cfg(feature = "bat")]
    fn bat_with_config(&self, config: Config, context: &Context<L>) {
        let mut doc = Document::new(config, context);
        doc.pager(self).expect("pager failed");
    }

    #[cfg(feature = "bat")]
    fn bat(&self, context: &Context<L>) {
        let mut doc = Document::new(Config::default(), context);
        doc.pager(self).expect("pager failed");
    }
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

// Leaf IR nodes - no nested statements to scan
impl<L: Dialect> ScanResultWidth<L> for SSAValue {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // SSAValue is a leaf node with no nested statements
    }
}

impl<L: Dialect> ScanResultWidth<L> for ResultValue {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // ResultValue is a leaf node with no nested statements
    }
}

impl<L: Dialect> ScanResultWidth<L> for Successor {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // Successor is a leaf node with no nested statements
    }
}

impl PrettyPrint for ResultValue {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrint for SSAValue {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrint for Successor {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}

/// Trait for types that can print their name (e.g., SSA values).
pub trait PrettyPrintName {
    /// Pretty print just the name part (e.g., `%x` for an SSA value).
    fn pretty_print_name<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>;
}

/// Trait for types that can print their type annotation.
pub trait PrettyPrintType {
    /// Pretty print just the type part (e.g., `i32` for a typed value).
    fn pretty_print_type<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display;
}

impl PrettyPrintName for SSAValue {
    fn pretty_print_name<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrintName for ResultValue {
    fn pretty_print_name<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context);
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context.symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrintType for SSAValue {
    fn pretty_print_type<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context);
        doc.text(format!("{}", info.ty()))
    }
}

impl PrettyPrintType for ResultValue {
    fn pretty_print_type<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context);
        doc.text(format!("{}", info.ty()))
    }
}

// Blanket impls for references - allows calling on &T when the impl is on T
impl<T: PrettyPrintName> PrettyPrintName for &T {
    fn pretty_print_name<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        (*self).pretty_print_name(doc)
    }
}

impl<T: PrettyPrintType> PrettyPrintType for &T {
    fn pretty_print_type<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        (*self).pretty_print_type(doc)
    }
}

impl<T: PrettyPrint> PrettyPrint for &T {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        (*self).pretty_print(doc)
    }
}


#[cfg(feature = "bat")]
mod bat;

#[cfg(test)]
mod tests;
