//! Document builder for pretty printing.

use std::{borrow::Cow, ops::Deref};

use kirin_ir::{
    Block, Context, DenseHint, Dialect, GetInfo, GlobalSymbol, Id, InternTable, Item, Region,
    SSAInfo, Signature, SpecializedFunction, StagedFunction, Statement,
};
use prettyless::{Arena, DocAllocator};

use crate::{ArenaDoc, Config, PrettyPrint, ScanResultWidth};

/// A document builder for pretty printing IR.
///
/// The `Document` struct holds configuration, an arena allocator, and context
/// needed for building pretty-printed output from IR nodes.
pub struct Document<'a, L: Dialect> {
    config: Config,
    arena: Arena<'a>,
    context: &'a Context<L>,
    global_symbols: Option<&'a InternTable<String, GlobalSymbol>>,
    result_width: DenseHint<Statement, usize>,
    max_result_width: usize,
}

impl<'a, L: Dialect> Document<'a, L> {
    /// Create a new document builder with the given configuration and context.
    ///
    /// Global symbol resolution for function names is not available.
    /// Use [`Document::with_global_symbols`] if you need to resolve
    /// [`GlobalSymbol`] names.
    pub fn new(config: Config, context: &'a Context<L>) -> Self {
        let arena = Arena::new();
        Self {
            config,
            arena,
            context,
            global_symbols: None,
            result_width: context.statement_arena().hint().dense(),
            max_result_width: 0,
        }
    }

    /// Create a new document builder with global symbol table support.
    ///
    /// The `global_symbols` table is used to resolve [`GlobalSymbol`] names
    /// (e.g., function names) that were interned via [`Pipeline::intern`](kirin_ir::Pipeline::intern).
    pub fn with_global_symbols(
        config: Config,
        context: &'a Context<L>,
        global_symbols: &'a InternTable<String, GlobalSymbol>,
    ) -> Self {
        let arena = Arena::new();
        Self {
            config,
            arena,
            context,
            global_symbols: Some(global_symbols),
            result_width: context.statement_arena().hint().dense(),
            max_result_width: 0,
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
    pub fn context(&self) -> &'a Context<L> {
        self.context
    }

    /// Set the result width for a statement.
    pub(crate) fn set_result_width(&mut self, stmt: Statement, width: usize) {
        self.result_width.insert(stmt, width);
        if width > self.max_result_width {
            self.max_result_width = width;
        }
    }

    /// Get the maximum result width.
    #[allow(dead_code)]
    pub fn max_result_width(&self) -> usize {
        self.max_result_width
    }

    /// Get the result width for a statement.
    #[allow(dead_code)]
    pub fn result_width(&self, stmt: &Statement) -> Option<usize> {
        self.result_width.get(*stmt).copied()
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

    fn build<N>(&'a mut self, node: &N) -> ArenaDoc<'a>
    where
        N: ScanResultWidth<L> + PrettyPrint,
        L: PrettyPrint,
        L::Type: std::fmt::Display,
    {
        node.scan_result_width(self);
        node.pretty_print(self)
    }

    /// Render a node to a string.
    pub fn render<N>(&'a mut self, node: &N) -> Result<String, std::fmt::Error>
    where
        N: ScanResultWidth<L> + PrettyPrint,
        L: PrettyPrint,
        L::Type: std::fmt::Display,
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
    L::Type: std::fmt::Display,
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
                let arg_info: &Item<SSAInfo<L>> = arg.expect_info(self.context);
                let name = if let Some(name_sym) = arg_info.name() {
                    self.context
                        .symbol_table()
                        .borrow()
                        .resolve(name_sym)
                        .cloned()
                        .unwrap_or_else(|| format!("{}", Id::from(*arg).raw()))
                } else {
                    format!("{}", Id::from(*arg).raw())
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

    /// Pretty print a specialized function with its full header.
    ///
    /// Renders as:
    /// ```text
    /// fn @name(Type0, Type1) -> RetType {
    ///   <body>
    /// }
    /// ```
    ///
    /// The stage prefix is derived from the context's own identity (name or
    /// stage ID), so pipeline-level rendering requires no external prefix.
    ///
    /// The function name is obtained from the parent [`StagedFunction`] and
    /// resolved via the global symbol table if available.
    pub fn print_specialized_function(&'a self, func: &SpecializedFunction) -> ArenaDoc<'a> {
        let (staged_fn, idx) = func.id();
        let staged_info = staged_fn.expect_info(self.context);
        let spec = &staged_info.specializations()[idx];
        let header = self.print_function_header(staged_info.name(), spec.signature());
        header + self.text(" ") + self.print_statement(spec.body())
    }

    /// Pretty print a staged function with all its non-invalidated specializations.
    ///
    /// Each specialization is rendered with the full header:
    /// ```text
    /// stage @A fn @name(Type0, Type1) -> RetType {
    ///   <body>
    /// }
    /// ```
    ///
    /// The stage prefix is derived from the context's identity automatically.
    /// Function names are resolved via the global symbol table if available.
    /// Invalidated specializations are excluded.
    pub fn print_staged_function(&'a self, func: &StagedFunction) -> ArenaDoc<'a> {
        let info = func.expect_info(self.context);
        let active: Vec<_> = info
            .specializations()
            .iter()
            .filter(|s| !s.is_invalidated())
            .collect();

        if active.is_empty() {
            // Extern / declaration-only staged function: just print the signature header
            return self.print_function_header(info.name(), info.signature());
        }

        let mut doc = self.nil();
        for (i, spec) in active.iter().enumerate() {
            if i > 0 {
                doc += self.line_() + self.line_();
            }
            // Build header from the specialization's own signature
            doc += self.print_function_header(info.name(), spec.signature());
            // Render the body statement (dialect provides its own formatting)
            doc += self.text(" ") + self.print_statement(spec.body());
        }
        doc
    }

    /// Print a function header line: `fn @name(T0, T1) -> Ret`
    ///
    /// The stage prefix is derived from the context's identity:
    /// - If the context has a name, prints `stage @<name> fn @...`
    /// - If the context has only a stage ID, prints `stage <id> fn @...`
    /// - Otherwise, prints `fn @...` (standalone, no pipeline)
    ///
    /// Uses the given signature for parameter types and return type.
    pub fn print_function_header(
        &'a self,
        name: Option<GlobalSymbol>,
        sig: &Signature<L::Type>,
    ) -> ArenaDoc<'a> {
        let name_str = name
            .and_then(|sym| self.global_symbols.and_then(|gs| gs.resolve(sym).cloned()))
            .unwrap_or_else(|| "<unnamed>".into());

        // Derive stage prefix from the context's own identity
        let prefix = self
            .context
            .name()
            .and_then(|sym| self.global_symbols.and_then(|gs| gs.resolve(sym).cloned()))
            .map(|n| format!("stage @{}", n))
            .or_else(|| {
                self.context
                    .stage_id()
                    .map(|id| format!("stage {}", Id::from(id).raw()))
            });

        let mut header = match prefix {
            Some(p) => self.text(p) + self.text(" fn @"),
            None => self.text("fn @"),
        };
        header += self.text(name_str) + self.text("(");

        // Parameters
        for (i, param) in sig.params.iter().enumerate() {
            if i > 0 {
                header += self.text(", ");
            }
            header += self.text(format!("{}", param));
        }
        header += self.text(") -> ") + self.text(format!("{}", sig.ret));
        header
    }
}

impl<'a, L: Dialect> Deref for Document<'a, L> {
    type Target = Arena<'a>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

/// Strip trailing whitespace from each line in the string.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_trailing_whitespace_empty() {
        assert_eq!(strip_trailing_whitespace(""), "\n");
    }

    #[test]
    fn test_strip_trailing_whitespace_no_trailing() {
        assert_eq!(strip_trailing_whitespace("hello\nworld"), "hello\nworld\n");
    }

    #[test]
    fn test_strip_trailing_whitespace_with_trailing() {
        assert_eq!(
            strip_trailing_whitespace("hello   \nworld  \n"),
            "hello\nworld\n"
        );
    }

    #[test]
    fn test_strip_trailing_whitespace_mixed() {
        assert_eq!(
            strip_trailing_whitespace("  indented  \n  also  \n"),
            "  indented\n  also\n"
        );
    }
}
