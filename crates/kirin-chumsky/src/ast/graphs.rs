use kirin_ir::{Dialect, GetInfo, IsEdge, SSAValue};

use super::Spanned;
use crate::traits::{EmitContext, EmitError, EmitIR};

/// A graph header containing the name, ports, and optional captures.
///
/// Represents syntax like: `^dg0(%p0: Type, %p1: Type) capture(%theta: f64)`
#[derive(Debug, Clone, PartialEq)]
pub struct GraphHeader<'src, TypeOutput> {
    /// The graph label name (without `^` prefix).
    pub name: Spanned<&'src str>,
    /// Edge port arguments.
    pub ports: Vec<Spanned<super::BlockArgument<'src, TypeOutput>>>,
    /// Capture arguments (optional).
    pub captures: Vec<Spanned<super::BlockArgument<'src, TypeOutput>>>,
}

/// A directed graph body.
///
/// Represents syntax like:
/// ```text
/// digraph ^dg0(%p0: Type) capture(%theta: f64) {
///   %0 = constant 1;
///   %1 = add %p0, %0;
///   yield %1;
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DiGraph<'src, TypeOutput, StmtOutput> {
    /// The graph header with name, ports, and captures.
    pub header: Spanned<GraphHeader<'src, TypeOutput>>,
    /// The statements (nodes) in the graph.
    pub statements: Vec<Spanned<StmtOutput>>,
    /// The yield values (output edges).
    pub yields: Vec<Spanned<&'src str>>,
}

/// An undirected graph body.
///
/// Represents syntax like:
/// ```text
/// ungraph ^ug0(%p0: Type) capture(%theta: f64) {
///   edge %w0 = wire;
///   node_a(%p0, %w0);
///   node_b(%theta, %w0);
/// }
/// ```
///
/// Edge statements are prefixed with `edge` keyword.
/// The parser interleaves edge and node statements — the `is_edge` flag
/// distinguishes them.
#[derive(Debug, Clone, PartialEq)]
pub struct UnGraph<'src, TypeOutput, StmtOutput> {
    /// The graph header with name, ports, and captures.
    pub header: Spanned<GraphHeader<'src, TypeOutput>>,
    /// All statements in the graph body.
    /// Each is tagged with whether it was prefixed with `edge`.
    pub statements: Vec<UnGraphStatement<'src, StmtOutput>>,
}

/// A statement inside an ungraph body, tagged as edge or node.
#[derive(Debug, Clone, PartialEq)]
pub struct UnGraphStatement<'src, StmtOutput> {
    /// Whether this statement was prefixed with the `edge` keyword.
    pub is_edge: bool,
    /// The parsed statement.
    pub stmt: Spanned<StmtOutput>,
    /// Span of the `edge` keyword, if present (for error reporting).
    pub edge_span: Option<chumsky::span::SimpleSpan>,
    _phantom: std::marker::PhantomData<&'src ()>,
}

impl<'src, StmtOutput> UnGraphStatement<'src, StmtOutput> {
    pub fn new(
        is_edge: bool,
        stmt: Spanned<StmtOutput>,
        edge_span: Option<chumsky::span::SimpleSpan>,
    ) -> Self {
        Self {
            is_edge,
            stmt,
            edge_span,
            _phantom: std::marker::PhantomData,
        }
    }
}

// --- Shared helpers for graph emit ---

/// Collect port/capture names and types eagerly from parsed block arguments.
fn collect_port_info<'src, TypeOutput, IR>(
    args: &[Spanned<super::BlockArgument<'src, TypeOutput>>],
    ctx: &mut EmitContext<'_, IR>,
) -> Result<(Vec<String>, Vec<IR::Type>), EmitError>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
{
    let mut names = Vec::with_capacity(args.len());
    let mut types = Vec::with_capacity(args.len());
    for arg in args.iter() {
        let ty: IR::Type = arg.value.ty.value.emit(ctx)?;
        types.push(ty);
        names.push(arg.value.name.value.to_string());
    }
    Ok((names, types))
}

// --- EmitIR implementations ---

impl<'src, TypeOutput, StmtOutput> DiGraph<'src, TypeOutput, StmtOutput> {
    pub fn emit_with<IR>(
        &self,
        ctx: &mut EmitContext<'_, IR>,
        emit_statement: &impl for<'ctx> Fn(
            &StmtOutput,
            &mut EmitContext<'ctx, IR>,
        ) -> Result<kirin_ir::Statement, EmitError>,
    ) -> Result<kirin_ir::DiGraph, EmitError>
    where
        IR: Dialect + IsEdge,
        IR::Type: Clone,
        TypeOutput: EmitIR<IR, Output = IR::Type>,
    {
        let header = &self.header.value;
        let graph_name = header.name.value.to_string();

        // Collect all port/capture types eagerly (before borrowing ctx.stage via builder)
        let (port_names, port_types) = collect_port_info(&header.ports, ctx)?;
        let (cap_names, cap_types) = collect_port_info(&header.captures, ctx)?;

        // Phase 1: Create the digraph with ports/captures only (no nodes/yields).
        // This produces real port SSAs immediately.
        let mut builder = ctx.stage.digraph().name(graph_name);

        for (name, ty) in port_names.iter().zip(port_types.iter()) {
            builder = builder.port(ty.clone()).port_name(name.clone());
        }

        for (name, ty) in cap_names.iter().zip(cap_types.iter()) {
            builder = builder.capture(ty.clone()).capture_name(name.clone());
        }

        let dg = builder.new();

        // Phase 2: Read back real port/capture SSAs and register them in emit context.
        // Collect SSA values first to avoid borrow conflict with ctx.
        let dg_info = dg.expect_info(ctx.stage);
        let port_ssas: Vec<SSAValue> = dg_info
            .edge_ports()
            .iter()
            .map(|p| SSAValue::from(*p))
            .collect();
        let cap_ssas: Vec<SSAValue> = dg_info
            .capture_ports()
            .iter()
            .map(|p| SSAValue::from(*p))
            .collect();
        for (ssa, name) in port_ssas.into_iter().zip(port_names.iter()) {
            ctx.register_ssa(name.clone(), ssa);
        }
        for (ssa, name) in cap_ssas.into_iter().zip(cap_names.iter()) {
            ctx.register_ssa(name.clone(), ssa);
        }

        // Phase 3: Emit all statements with relaxed dominance.
        // Graph bodies allow forward SSA references — a statement may reference
        // SSAs defined by later statements (e.g. cycles in signal processing graphs).
        ctx.set_relaxed_dominance(true);
        let mut node_stmts = Vec::new();
        for stmt_ast in &self.statements {
            let stmt = emit_statement(&stmt_ast.value, ctx)?;
            node_stmts.push(stmt);
        }
        ctx.set_relaxed_dominance(false);

        // Phase 4: Resolve yield references
        let mut yield_ssas = Vec::new();
        for y in &self.yields {
            let ssa = ctx
                .lookup_ssa(y.value)
                .ok_or_else(|| EmitError::UndefinedSSA(y.value.to_string()))?;
            yield_ssas.push(ssa);
        }

        // Phase 5: Attach nodes and yields to the already-created digraph.
        ctx.stage
            .attach_nodes_to_digraph(dg, &node_stmts, &yield_ssas);

        Ok(dg)
    }
}

impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for DiGraph<'src, TypeOutput, StmtOutput>
where
    IR: Dialect + IsEdge,
    IR::Type: Clone,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::DiGraph;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        self.emit_with(ctx, &|stmt, ctx| stmt.emit(ctx))
    }
}

impl<'src, TypeOutput, StmtOutput> UnGraph<'src, TypeOutput, StmtOutput> {
    pub fn emit_with<IR>(
        &self,
        ctx: &mut EmitContext<'_, IR>,
        emit_statement: &impl for<'ctx> Fn(
            &StmtOutput,
            &mut EmitContext<'ctx, IR>,
        ) -> Result<kirin_ir::Statement, EmitError>,
    ) -> Result<kirin_ir::UnGraph, EmitError>
    where
        IR: Dialect + IsEdge,
        IR::Type: Clone,
        TypeOutput: EmitIR<IR, Output = IR::Type>,
    {
        let header = &self.header.value;
        let graph_name = header.name.value.to_string();

        // Collect all port/capture types eagerly
        let (port_names, port_types) = collect_port_info(&header.ports, ctx)?;
        let (cap_names, cap_types) = collect_port_info(&header.captures, ctx)?;

        // Phase 1: Create the ungraph with ports/captures only (no edges/nodes).
        let mut builder = ctx.stage.ungraph().name(graph_name);

        for (name, ty) in port_names.iter().zip(port_types.iter()) {
            builder = builder.port(ty.clone()).port_name(name.clone());
        }

        for (name, ty) in cap_names.iter().zip(cap_types.iter()) {
            builder = builder.capture(ty.clone()).capture_name(name.clone());
        }

        let ug = builder.new();

        // Phase 2: Read back real port/capture SSAs and register them in emit context.
        // Collect SSA values first to avoid borrow conflict with ctx.
        let ug_info = ug.expect_info(ctx.stage);
        let port_ssas: Vec<SSAValue> = ug_info
            .edge_ports()
            .iter()
            .map(|p| SSAValue::from(*p))
            .collect();
        let cap_ssas: Vec<SSAValue> = ug_info
            .capture_ports()
            .iter()
            .map(|p| SSAValue::from(*p))
            .collect();
        for (ssa, name) in port_ssas.into_iter().zip(port_names.iter()) {
            ctx.register_ssa(name.clone(), ssa);
        }
        for (ssa, name) in cap_ssas.into_iter().zip(cap_names.iter()) {
            ctx.register_ssa(name.clone(), ssa);
        }

        // Phase 3: Emit all statements with relaxed dominance, tracking edge vs node
        ctx.set_relaxed_dominance(true);
        let mut edge_stmts = Vec::new();
        let mut node_stmts = Vec::new();
        for ug_stmt in &self.statements {
            let stmt = emit_statement(&ug_stmt.stmt.value, ctx)?;
            if ug_stmt.is_edge {
                edge_stmts.push(stmt);
            } else {
                node_stmts.push(stmt);
            }
        }
        ctx.set_relaxed_dominance(false);

        // Phase 4: Attach edges and nodes to the already-created ungraph.
        ctx.stage
            .attach_nodes_to_ungraph(ug, &edge_stmts, &node_stmts);

        Ok(ug)
    }
}

impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for UnGraph<'src, TypeOutput, StmtOutput>
where
    IR: Dialect + IsEdge,
    IR::Type: Clone,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::UnGraph;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        self.emit_with(ctx, &|stmt, ctx| stmt.emit(ctx))
    }
}
