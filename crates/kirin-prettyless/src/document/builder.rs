use std::borrow::Cow;

use kirin_ir::{Dialect, GlobalSymbol, InternTable, StageInfo};
use prettyless::{Arena, DocAllocator};

use crate::{ArenaDoc, Config, PrettyPrint};

/// A document builder for pretty printing IR.
///
/// `Document` holds a configuration, an arena allocator, a stage reference,
/// and an optional global symbol table. It provides two categories of API:
///
/// - **Arena methods** (via `Deref<Target = Arena>`): `text()`, `nil()`,
///   `line_()`, etc. for building document fragments.
/// - **IR printing methods**: `print_statement()`, `print_block()`,
///   `print_region()`, etc. for rendering structured IR nodes.
///
/// For most use cases, prefer [`PrettyPrintExt::render`] or
/// [`PrettyPrintExt::sprint`] which construct and use a `Document` internally.
pub struct Document<'a, L: Dialect> {
    pub(super) config: Config,
    pub(super) arena: Arena<'a>,
    pub(super) stage: &'a StageInfo<L>,
    pub(super) global_symbols: Option<&'a InternTable<String, GlobalSymbol>>,
}

impl<'a, L: Dialect> Document<'a, L> {
    /// Create a new document builder with the given configuration and context.
    ///
    /// Global symbol resolution for function names is not available.
    /// Use [`Document::with_global_symbols`] if you need to resolve
    /// [`GlobalSymbol`] names.
    pub fn new(config: Config, stage: &'a StageInfo<L>) -> Self {
        let arena = Arena::new();
        Self {
            config,
            arena,
            stage,
            global_symbols: None,
        }
    }

    /// Create a new document builder with global symbol table support.
    ///
    /// The `global_symbols` table is used to resolve [`GlobalSymbol`] names
    /// (e.g., function names) that were interned via [`Pipeline::intern`](kirin_ir::Pipeline::intern).
    pub fn with_global_symbols(
        config: Config,
        stage: &'a StageInfo<L>,
        global_symbols: &'a InternTable<String, GlobalSymbol>,
    ) -> Self {
        let arena = Arena::new();
        Self {
            config,
            arena,
            stage,
            global_symbols: Some(global_symbols),
        }
    }

    /// Returns a reference to the global symbol table, if available.
    pub fn global_symbols(&self) -> Option<&'a InternTable<String, GlobalSymbol>> {
        self.global_symbols
    }

    /// Indent a document by the configured tab spaces.
    pub fn indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        doc.nest(self.config.tab_spaces as isize)
    }

    /// Create an indented block with a leading line break.
    pub fn block_indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        self.indent(self.arena.line_() + doc)
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns a reference to the IR context.
    pub fn stage(&self) -> &'a StageInfo<L> {
        self.stage
    }

    /// Build a list of items with a separator.
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

    /// Render a node to a string.
    pub fn render<N>(&'a mut self, node: &N) -> Result<String, std::fmt::Error>
    where
        N: PrettyPrint,
        L: PrettyPrint,
        L::Type: std::fmt::Display,
    {
        let max_width = self.config.max_width;
        let arena_doc = node.pretty_print(self);
        let mut buf = String::new();
        arena_doc.render_fmt(max_width, &mut buf)?;
        Ok(strip_trailing_whitespace(&buf))
    }
}

impl<'a, L: Dialect> std::ops::Deref for Document<'a, L> {
    type Target = Arena<'a>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

/// Strip trailing whitespace from each line in the string.
pub(super) fn strip_trailing_whitespace(s: &str) -> String {
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
