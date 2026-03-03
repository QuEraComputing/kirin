use kirin_ir::{
    Block, Dialect, GetInfo, GlobalSymbol, Id, Item, Region, SSAInfo, Signature,
    SpecializedFunction, StagedFunction, Statement,
};
use prettyless::DocAllocator;

use crate::{ArenaDoc, PrettyPrint};

use super::builder::Document;

// Methods for printing IR nodes that need L: PrettyPrint bound
impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::Type: std::fmt::Display,
{
    /// Pretty print a statement by printing its definition.
    pub fn print_statement(&'a self, stmt: &Statement) -> ArenaDoc<'a> {
        let stmt_info = stmt.expect_info(self.stage);
        let def = stmt_info.definition();
        def.pretty_print(self)
    }

    /// Pretty print a block with its header and statements.
    pub fn print_block(&'a self, block: &Block) -> ArenaDoc<'a> {
        let block_info = block.expect_info(self.stage);

        // Build block header with arguments: ^name(%arg0: type, %arg1: type)
        // Look up block name from symbol table, fall back to ^index
        let block_name = block_info
            .name
            .and_then(|name_sym| {
                self.stage
                    .symbol_table()
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
                let arg_info: &Item<SSAInfo<L>> = arg.expect_info(self.stage);
                let name = if let Some(name_sym) = arg_info.name() {
                    self.stage
                        .symbol_table()
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
        for (i, stmt) in block.statements(self.stage).enumerate() {
            if i > 0 {
                inner += self.line_();
            }
            inner += self.print_statement(&stmt) + self.text(";");
        }
        if let Some(terminator) = block.terminator(self.stage) {
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
        for block in region.blocks(self.stage) {
            inner += self.print_block(&block);
            inner += self.line_();
        }
        self.block_indent(inner).enclose("{", "}")
    }

    /// Pretty print a specialized function with its full header.
    ///
    /// Renders as:
    /// ```text
    /// specialize @stage fn @name(Type0, Type1) -> RetType {
    ///   <body>
    /// }
    /// ```
    pub fn print_specialized_function(&'a self, func: &SpecializedFunction) -> ArenaDoc<'a> {
        let (staged_fn, idx) = func.id();
        let staged_info = staged_fn.expect_info(self.stage);
        let spec = &staged_info.specializations()[idx];
        let header = self.print_specialize_header(staged_info.name(), spec.signature());
        header + self.text(" ") + self.print_statement(spec.body())
    }

    /// Pretty print a staged function with all its non-invalidated specializations.
    ///
    /// The staged signature is rendered as a declaration:
    /// ```text
    /// stage @A fn @name(Type0, Type1) -> RetType;
    /// ```
    ///
    /// Each active specialization is then rendered as:
    /// ```text
    /// specialize @A fn @name(Type0, Type1) -> RetType { ... }
    /// ```
    pub fn print_staged_function(&'a self, func: &StagedFunction) -> ArenaDoc<'a> {
        let info = func.expect_info(self.stage);
        let active: Vec<_> = info
            .specializations()
            .iter()
            .filter(|s| !s.is_invalidated())
            .collect();

        let mut doc = self.print_stage_header(info.name(), info.signature());
        if active.is_empty() {
            return doc;
        }

        for spec in active {
            doc += self.line_();
            doc += self.print_specialize_header(info.name(), spec.signature());
            doc += self.text(" ") + self.print_statement(spec.body());
        }
        doc
    }

    /// Print a standalone function header line: `fn @name(T0, T1) -> Ret`
    ///
    /// Uses the given signature for parameter types and return type.
    pub fn print_function_header(
        &'a self,
        name: Option<GlobalSymbol>,
        sig: &Signature<L::Type>,
    ) -> ArenaDoc<'a> {
        self.text("fn @") + self.print_fn_signature(name, sig)
    }

    fn print_stage_header(
        &'a self,
        name: Option<GlobalSymbol>,
        sig: &Signature<L::Type>,
    ) -> ArenaDoc<'a> {
        self.text("stage @")
            + self.text(self.stage_symbol_text())
            + self.text(" fn @")
            + self.print_fn_signature(name, sig)
            + self.text(";")
    }

    fn print_specialize_header(
        &'a self,
        name: Option<GlobalSymbol>,
        sig: &Signature<L::Type>,
    ) -> ArenaDoc<'a> {
        self.text("specialize @")
            + self.text(self.stage_symbol_text())
            + self.text(" fn @")
            + self.print_fn_signature(name, sig)
    }

    /// Render `name(T0, T1) -> Ret` — shared by all header variants.
    fn print_fn_signature(
        &'a self,
        name: Option<GlobalSymbol>,
        sig: &Signature<L::Type>,
    ) -> ArenaDoc<'a> {
        let params = self.list(sig.params.iter(), ", ", |p| self.text(format!("{}", p)));
        self.text(self.function_symbol_text(name))
            + params.enclose("(", ")")
            + self.text(" -> ")
            + self.text(format!("{}", sig.ret))
    }

    fn function_symbol_text(&self, name: Option<GlobalSymbol>) -> String {
        name.map(|symbol| self.resolve_global_symbol(symbol))
            .unwrap_or_else(|| "unnamed".to_string())
    }

    fn stage_symbol_text(&self) -> String {
        if let Some(name) = self.stage.name() {
            return self.resolve_global_symbol(name);
        }
        if let Some(id) = self.stage.stage_id() {
            return Id::from(id).raw().to_string();
        }
        "0".to_string()
    }

    fn resolve_global_symbol(&self, symbol: GlobalSymbol) -> String {
        self.global_symbols
            .and_then(|symbols| symbols.resolve(symbol).cloned())
            .unwrap_or_else(|| usize::from(symbol).to_string())
    }
}
