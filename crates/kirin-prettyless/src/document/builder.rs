use std::borrow::Cow;
use std::cell::Cell;

use kirin_ir::{Dialect, GetInfo, GlobalSymbol, Id, InternTable, Item, SSAInfo, StageInfo};
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
    /// The name of the enclosing function, if any.
    ///
    /// Uses `Cell` for interior mutability because `print_specialized_function`
    /// and `print_staged_function` receive `&'a self` (shared reference) but
    /// need to set the function name context before delegating to the dialect's
    /// `PrettyPrint` impl.
    pub(super) function_name: Cell<Option<GlobalSymbol>>,
    /// The return type string of the enclosing function, if any.
    ///
    /// Set by the framework printer for `{:return}` context projections.
    pub(super) return_type_text: Cell<Option<String>>,
    /// The full signature text `Type, Type) -> RetType` for `{:signature}`.
    ///
    /// Set by the framework printer. `print_function_signature()` prepends `fn @name(`.
    pub(super) signature_text: Cell<Option<String>>,
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
            function_name: Cell::new(None),
            return_type_text: Cell::new(None),
            signature_text: Cell::new(None),
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
            function_name: Cell::new(None),
            return_type_text: Cell::new(None),
            signature_text: Cell::new(None),
        }
    }

    /// Returns a reference to the global symbol table, if available.
    pub fn global_symbols(&self) -> Option<&'a InternTable<String, GlobalSymbol>> {
        self.global_symbols
    }

    /// Set the enclosing function name for format-string projections.
    ///
    /// When set, dialect `PrettyPrint` impls can call
    /// [`Document::print_function_name`] to render `@name` for the enclosing
    /// function. This is used by `{:name}` projections in generated
    /// format-string pretty printers.
    pub fn set_function_name(&self, name: Option<GlobalSymbol>) {
        self.function_name.set(name);
    }

    /// Returns the enclosing function name, if one has been set.
    pub fn function_name(&self) -> Option<GlobalSymbol> {
        self.function_name.get()
    }

    /// Set the return type text for `{:return}` context projections.
    pub fn set_return_type_text(&self, text: Option<String>) {
        self.return_type_text.set(text);
    }

    /// Returns the return type text, if set.
    pub fn return_type_text(&self) -> Option<String> {
        self.return_type_text.take()
    }

    /// Set the full signature text for `{:signature}` context projections.
    pub fn set_signature_text(&self, text: Option<String>) {
        self.signature_text.set(text);
    }

    /// Returns the full signature text, if set.
    pub fn signature_text(&self) -> Option<String> {
        self.signature_text.take()
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

    /// Resolve the display name of an SSA value.
    ///
    /// Returns the symbol-table name if one exists, otherwise falls back
    /// to the numeric ID. The returned string is the bare name without
    /// a `%` prefix.
    pub fn ssa_name<V>(&self, value: V) -> String
    where
        V: Copy + GetInfo<L, Info = Item<SSAInfo<L>>>,
        Id: From<V>,
    {
        let info = value.expect_info(self.stage);
        if let Some(name_sym) = info.name() {
            if let Some(resolved) = self.stage.symbol_table().resolve(name_sym) {
                return resolved.clone();
            }
        }
        format!("{}", Id::from(value).raw())
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
