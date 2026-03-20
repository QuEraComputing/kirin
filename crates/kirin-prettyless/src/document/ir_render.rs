use std::collections::HashSet;

use kirin_ir::{
    Block, DiGraph, Dialect, GetInfo, GlobalSymbol, Id, Item, Port, Region, SSAInfo, SSAValue,
    Signature, SpecializedFunction, StagedFunction, Statement, UnGraph,
};
use petgraph::visit::IntoNodeReferences;
use prettyless::DocAllocator;

use crate::{ArenaDoc, PrettyPrint};

use super::builder::Document;

// Methods for printing IR nodes that need L: PrettyPrint bound
impl<'a, L: Dialect + PrettyPrint> Document<'a, L>
where
    L::Type: std::fmt::Display,
{
    /// Print an SSA value reference as `%name`.
    ///
    /// Resolves the name via [`Document::ssa_name`] and prepends `%`.
    pub fn print_ssa_ref<V>(&'a self, value: V) -> ArenaDoc<'a>
    where
        V: Copy + GetInfo<L, Info = Item<SSAInfo<L>>>,
        Id: From<V>,
    {
        self.text(format!("%{}", self.ssa_name(value)))
    }

    /// Print the type of an SSA value.
    pub fn print_ssa_type<V>(&'a self, value: V) -> ArenaDoc<'a>
    where
        V: Copy + GetInfo<L, Info = Item<SSAInfo<L>>>,
        Id: From<V>,
    {
        let info = value.expect_info(self.stage);
        self.text(format!("{}", info.ty()))
    }

    /// Pretty print a statement by printing its definition.
    ///
    /// Prints `%name1, %name2 = ` before the dialect body for statements
    /// that produce results. The dialect's `pretty_print` only handles the
    /// dialect-specific body (keyword, operands, types).
    pub fn print_statement(&'a self, stmt: &Statement) -> ArenaDoc<'a> {
        let stmt_info = stmt.expect_info(self.stage);
        let def = stmt_info.definition();

        let results: Vec<_> = stmt.results::<L>(self.stage).collect();
        if results.is_empty() {
            def.pretty_print(self)
        } else {
            let mut result_doc = self.nil();
            for (i, rv) in results.iter().enumerate() {
                if i > 0 {
                    result_doc += self.text(", ");
                }
                result_doc += self.print_ssa_ref(**rv);
            }
            result_doc + self.text(" = ") + def.pretty_print(self)
        }
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
                let name = self.ssa_name(*arg);
                let info = arg.expect_info(self.stage);
                args_doc += self.text(format!("%{}: {}", name, info.ty()));
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

    /// Pretty print a list of ports.
    ///
    /// Edge ports (`ports[..edge_count]`) are printed as `(%name: Type, ...)`.
    /// If capture ports (`ports[edge_count..]`) are present, they are appended as
    /// ` capture(%name: Type, ...)`.
    pub fn print_ports(&'a self, ports: &[Port], edge_count: usize) -> ArenaDoc<'a> {
        let edge_ports = &ports[..edge_count];
        let capture_ports = &ports[edge_count..];

        if edge_ports.is_empty() && capture_ports.is_empty() {
            return self.nil();
        }

        let print_port_list = |port_slice: &[Port]| -> ArenaDoc<'a> {
            let mut doc = self.nil();
            for (i, port) in port_slice.iter().enumerate() {
                if i > 0 {
                    doc += self.text(", ");
                }
                let name = self.ssa_name(*port);
                let info: &Item<SSAInfo<L>> = port.expect_info(self.stage);
                doc += self.text(format!("%{}: {}", name, info.ty()));
            }
            doc
        };

        let edge_doc = print_port_list(edge_ports).enclose("(", ")");

        if capture_ports.is_empty() {
            edge_doc
        } else {
            let capture_doc = print_port_list(capture_ports).enclose("(", ")");
            if edge_ports.is_empty() {
                self.text("()") + self.text(" capture") + capture_doc
            } else {
                edge_doc + self.text(" capture") + capture_doc
            }
        }
    }

    /// Pretty print a directed graph body.
    pub fn print_digraph(&'a self, digraph: &DiGraph) -> ArenaDoc<'a> {
        let info = digraph.expect_info(self.stage);

        // Header: digraph ^name(ports) {
        let graph_name = info
            .name()
            .and_then(|name_sym| {
                self.stage
                    .symbol_table()
                    .resolve(name_sym)
                    .map(|s| format!("^{}", s))
            })
            .unwrap_or_else(|| format!("{}", digraph));

        let mut header = self.text("digraph ") + self.text(graph_name);
        header += self.print_ports(info.ports(), info.edge_count());

        // Body: nodes + yield
        let mut inner = self.nil();
        let mut first = true;
        for (_idx, stmt) in info.graph().node_references() {
            if !first {
                inner += self.line_();
            }
            inner += self.print_statement(stmt) + self.text(";");
            first = false;
        }

        // Yield line
        if !info.yields().is_empty() {
            if !first {
                inner += self.line_();
            }
            let yield_doc = self.list(info.yields().iter(), ", ", |ssa| self.print_ssa_ref(*ssa));
            inner += self.text("yield ") + yield_doc + self.text(";");
        }

        header + self.text(" {") + self.block_indent(inner) + self.line_() + self.text("}")
    }

    /// Pretty print an undirected graph body.
    pub fn print_ungraph(&'a self, ungraph: &UnGraph) -> ArenaDoc<'a> {
        let info = ungraph.expect_info(self.stage);

        // Header: ungraph ^name(ports) {
        let graph_name = info
            .name()
            .and_then(|name_sym| {
                self.stage
                    .symbol_table()
                    .resolve(name_sym)
                    .map(|s| format!("^{}", s))
            })
            .unwrap_or_else(|| format!("{}", ungraph));

        let mut header = self.text("ungraph ") + self.text(graph_name);
        header += self.print_ports(info.ports(), info.edge_count());

        // Body: interleave edge statements with node statements.
        // For each node, print any unprinted edge statements whose results it uses,
        // then print the node itself.
        let edge_stmts = info.edge_statements();

        // Build a map from result SSAValues to their edge statement
        let edge_result_to_stmt: std::collections::HashMap<SSAValue, Statement> = edge_stmts
            .iter()
            .flat_map(|&edge_stmt| {
                edge_stmt
                    .results::<L>(self.stage)
                    .map(move |rv| (SSAValue::from(*rv), edge_stmt))
            })
            .collect();

        let mut printed_edges: HashSet<Statement> = HashSet::new();
        let mut inner = self.nil();
        let mut first = true;

        for (_idx, node_stmt) in info.graph().node_references() {
            // Find edge statements used by this node
            for arg in node_stmt.arguments::<L>(self.stage) {
                if let Some(&edge_stmt) = edge_result_to_stmt.get(arg)
                    && printed_edges.insert(edge_stmt)
                {
                    if !first {
                        inner += self.line_();
                    }
                    inner += self.text("edge ") + self.print_statement(&edge_stmt) + self.text(";");
                    first = false;
                }
            }

            // Print the node
            if !first {
                inner += self.line_();
            }
            inner += self.print_statement(node_stmt) + self.text(";");
            first = false;
        }

        // Print any remaining unprinted edge statements
        for &edge_stmt in edge_stmts {
            if printed_edges.insert(edge_stmt) {
                if !first {
                    inner += self.line_();
                }
                inner += self.text("edge ") + self.print_statement(&edge_stmt) + self.text(";");
                first = false;
            }
        }

        header + self.text(" {") + self.block_indent(inner) + self.line_() + self.text("}")
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
        let params = self.list(sig.params().iter(), ", ", |p| self.text(format!("{}", p)));
        self.text(self.function_symbol_text(name))
            + params.enclose("(", ")")
            + self.text(" -> ")
            + self.text(format!("{}", sig.ret()))
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

    // ── Projection helpers (for format-string projections like {body:ports}) ──

    /// Print only the edge port declarations: `%name: Type, %name: Type`
    ///
    /// No parentheses — the caller adds those via literal tokens in the format string.
    pub fn print_ports_only(&'a self, ports: &[Port], edge_count: usize) -> ArenaDoc<'a> {
        let edge_ports = &ports[..edge_count];
        self.print_port_list(edge_ports)
    }

    /// Print only the capture port declarations: `%name: Type, %name: Type`
    ///
    /// No parentheses or `capture` keyword — the caller adds those via literal tokens.
    pub fn print_captures_only(&'a self, ports: &[Port], edge_count: usize) -> ArenaDoc<'a> {
        let capture_ports = &ports[edge_count..];
        self.print_port_list(capture_ports)
    }

    /// Print only yield types: `Type, Type`
    ///
    /// Prints the *type* of each yield value, comma-separated.
    pub fn print_yields_only(&'a self, yields: &[SSAValue]) -> ArenaDoc<'a> {
        self.list(yields.iter(), ", ", |ssa| self.print_ssa_type(*ssa))
    }

    /// Print a DiGraph body only: statements + yield, no header, no braces.
    pub fn print_digraph_body_only(&'a self, digraph: &DiGraph) -> ArenaDoc<'a> {
        let info = digraph.expect_info(self.stage);

        let mut inner = self.nil();
        let mut first = true;
        for (_idx, stmt) in info.graph().node_references() {
            if !first {
                inner += self.line_();
            }
            inner += self.print_statement(stmt) + self.text(";");
            first = false;
        }

        if !info.yields().is_empty() {
            if !first {
                inner += self.line_();
            }
            let yield_doc = self.list(info.yields().iter(), ", ", |ssa| self.print_ssa_ref(*ssa));
            inner += self.text("yield ") + yield_doc + self.text(";");
        }

        inner
    }

    /// Print an UnGraph body only: interleaved edge/node statements, no header, no braces.
    pub fn print_ungraph_body_only(&'a self, ungraph: &UnGraph) -> ArenaDoc<'a> {
        let info = ungraph.expect_info(self.stage);
        let edge_stmts = info.edge_statements();

        let edge_result_to_stmt: std::collections::HashMap<SSAValue, Statement> = edge_stmts
            .iter()
            .flat_map(|&edge_stmt| {
                edge_stmt
                    .results::<L>(self.stage)
                    .map(move |rv| (SSAValue::from(*rv), edge_stmt))
            })
            .collect();

        let mut printed_edges: HashSet<Statement> = HashSet::new();
        let mut inner = self.nil();
        let mut first = true;

        for (_idx, node_stmt) in info.graph().node_references() {
            for arg in node_stmt.arguments::<L>(self.stage) {
                if let Some(&edge_stmt) = edge_result_to_stmt.get(arg)
                    && printed_edges.insert(edge_stmt)
                {
                    if !first {
                        inner += self.line_();
                    }
                    inner += self.text("edge ") + self.print_statement(&edge_stmt) + self.text(";");
                    first = false;
                }
            }

            if !first {
                inner += self.line_();
            }
            inner += self.print_statement(node_stmt) + self.text(";");
            first = false;
        }

        for &edge_stmt in edge_stmts {
            if printed_edges.insert(edge_stmt) {
                if !first {
                    inner += self.line_();
                }
                inner += self.text("edge ") + self.print_statement(&edge_stmt) + self.text(";");
                first = false;
            }
        }

        inner
    }

    /// Print a Region body only: blocks without outer braces.
    pub fn print_region_body_only(&'a self, region: &Region) -> ArenaDoc<'a> {
        let mut inner = self.nil();
        for block in region.blocks(self.stage) {
            inner += self.print_block(&block);
            inner += self.line_();
        }
        inner
    }

    /// Print a Block body only: statements without the header or braces.
    pub fn print_block_body_only(&'a self, block: &Block) -> ArenaDoc<'a> {
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
        inner
    }

    /// Print Block arguments only: `%name: Type, %name: Type`
    ///
    /// No parentheses — the caller adds those via literal tokens.
    pub fn print_block_args_only(&'a self, block: &Block) -> ArenaDoc<'a> {
        let block_info = block.expect_info(self.stage);
        let args = &block_info.arguments;
        let mut doc = self.nil();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                doc += self.text(", ");
            }
            let name = self.ssa_name(*arg);
            let info = arg.expect_info(self.stage);
            doc += self.text(format!("%{}: {}", name, info.ty()));
        }
        doc
    }

    /// Helper: print a comma-separated list of ports as `%name: Type, %name: Type`.
    fn print_port_list(&'a self, ports: &[Port]) -> ArenaDoc<'a> {
        let mut doc = self.nil();
        for (i, port) in ports.iter().enumerate() {
            if i > 0 {
                doc += self.text(", ");
            }
            let name = self.ssa_name(*port);
            let info: &Item<SSAInfo<L>> = port.expect_info(self.stage);
            doc += self.text(format!("%{}: {}", name, info.ty()));
        }
        doc
    }
}
