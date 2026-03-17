use kirin_ir::{Dialect, GetInfo, IsEdge, Placeholder, SSAKind};

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

struct PortInfo {
    name: String,
}

/// Collect port/capture names and types eagerly from parsed block arguments.
fn collect_port_info<'src, TypeOutput, IR>(
    args: &[Spanned<super::BlockArgument<'src, TypeOutput>>],
    ctx: &mut EmitContext<'_, IR>,
) -> Result<(Vec<PortInfo>, Vec<IR::Type>), EmitError>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
{
    let mut infos = Vec::with_capacity(args.len());
    let mut types = Vec::with_capacity(args.len());
    for arg in args.iter() {
        let ty: IR::Type = arg.value.ty.value.emit(ctx)?;
        types.push(ty);
        infos.push(PortInfo {
            name: arg.value.name.value.to_string(),
        });
    }
    Ok((infos, types))
}

/// Create temporary placeholder SSAs for ports/captures and register them by name.
///
/// These placeholders are resolved when the graph builder finalizes.
fn register_placeholder_ssas<IR>(
    infos: &[PortInfo],
    types: &[IR::Type],
    resolution: fn(kirin_ir::BuilderKey) -> kirin_ir::ResolutionInfo,
    ctx: &mut EmitContext<'_, IR>,
) where
    IR: Dialect,
    IR::Type: Placeholder + Clone,
{
    for (i, info) in infos.iter().enumerate() {
        let ssa = ctx
            .stage
            .ssa()
            .name(info.name.clone())
            .ty(types[i].clone())
            .kind(SSAKind::Unresolved(resolution(
                kirin_ir::BuilderKey::Index(i),
            )))
            .new();
        ctx.register_ssa(info.name.clone(), ssa);
    }
}

/// Collect (SSAValue, name) pairs from built graph ports and their corresponding infos.
///
/// Returns a `Vec` so the caller can drop the borrow on `ctx.stage` before
/// calling `ctx.register_ssa` for each pair.
fn collect_port_ssa_names(
    ports: &[kirin_ir::Port],
    infos: &[PortInfo],
) -> Vec<(kirin_ir::SSAValue, String)> {
    ports
        .iter()
        .zip(infos.iter())
        .map(|(port, info)| (kirin_ir::SSAValue::from(*port), info.name.clone()))
        .collect()
}

/// Register collected (SSAValue, name) pairs in the emit context.
fn register_ssa_names<IR: Dialect>(
    pairs: Vec<(kirin_ir::SSAValue, String)>,
    ctx: &mut EmitContext<'_, IR>,
) {
    for (ssa, name) in pairs {
        ctx.register_ssa(name, ssa);
    }
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
        IR::Type: Placeholder + Clone,
        TypeOutput: EmitIR<IR, Output = IR::Type>,
    {
        let header = &self.header.value;
        let graph_name = header.name.value.to_string();

        // Collect all port/capture types eagerly (before borrowing ctx.stage via builder)
        let (port_infos, port_types) = collect_port_info(&header.ports, ctx)?;
        let (cap_infos, cap_types) = collect_port_info(&header.captures, ctx)?;

        // Phase 1: Create temporary port/capture SSAs and register names
        register_placeholder_ssas(
            &port_infos,
            &port_types,
            kirin_ir::ResolutionInfo::Port,
            ctx,
        );
        register_placeholder_ssas(
            &cap_infos,
            &cap_types,
            kirin_ir::ResolutionInfo::Capture,
            ctx,
        );

        // Phase 2: Emit all statements with relaxed dominance.
        // Graph bodies allow forward SSA references — a statement may reference
        // SSAs defined by later statements (e.g. cycles in signal processing graphs).
        ctx.set_relaxed_dominance(true);
        let mut node_stmts = Vec::new();
        for stmt_ast in &self.statements {
            let stmt = emit_statement(&stmt_ast.value, ctx)?;
            node_stmts.push(stmt);
        }
        ctx.set_relaxed_dominance(false);

        // Phase 3: Resolve yield references
        let mut yield_ssas = Vec::new();
        for y in &self.yields {
            let ssa = ctx
                .lookup_ssa(y.value)
                .ok_or_else(|| EmitError::UndefinedSSA(y.value.to_string()))?;
            yield_ssas.push(ssa);
        }

        // Phase 4: Build digraph using builder — it will create real port SSAs
        // and resolve Unresolved(Port/Capture) placeholders in statement operands
        let mut builder = ctx.stage.digraph().name(graph_name);

        for (info, ty) in port_infos.iter().zip(port_types.iter()) {
            builder = builder.port(ty.clone()).port_name(info.name.clone());
        }

        for (info, ty) in cap_infos.iter().zip(cap_types.iter()) {
            builder = builder.capture(ty.clone()).capture_name(info.name.clone());
        }

        for stmt in &node_stmts {
            builder = builder.node(*stmt);
        }

        for ssa in &yield_ssas {
            builder = builder.yield_value(*ssa);
        }

        let dg = builder.new();

        // Phase 5: Register real port/capture SSAs in emit context
        let dg_info = dg.expect_info(ctx.stage);
        let port_ssas = collect_port_ssa_names(dg_info.edge_ports(), &port_infos);
        let cap_ssas = collect_port_ssa_names(dg_info.capture_ports(), &cap_infos);
        register_ssa_names(port_ssas, ctx);
        register_ssa_names(cap_ssas, ctx);

        Ok(dg)
    }
}

impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for DiGraph<'src, TypeOutput, StmtOutput>
where
    IR: Dialect + IsEdge,
    IR::Type: Placeholder + Clone,
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
        IR::Type: Placeholder + Clone,
        TypeOutput: EmitIR<IR, Output = IR::Type>,
    {
        let header = &self.header.value;
        let graph_name = header.name.value.to_string();

        // Collect all port/capture types eagerly
        let (port_infos, port_types) = collect_port_info(&header.ports, ctx)?;
        let (cap_infos, cap_types) = collect_port_info(&header.captures, ctx)?;

        // Phase 1: Create temporary port/capture SSAs and register names
        register_placeholder_ssas(
            &port_infos,
            &port_types,
            kirin_ir::ResolutionInfo::Port,
            ctx,
        );
        register_placeholder_ssas(
            &cap_infos,
            &cap_types,
            kirin_ir::ResolutionInfo::Capture,
            ctx,
        );

        // Phase 2: Emit all statements with relaxed dominance, tracking edge vs node
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

        // Phase 3: Build ungraph using builder
        let mut builder = ctx.stage.ungraph().name(graph_name);

        for (info, ty) in port_infos.iter().zip(port_types.iter()) {
            builder = builder.port(ty.clone()).port_name(info.name.clone());
        }

        for (info, ty) in cap_infos.iter().zip(cap_types.iter()) {
            builder = builder.capture(ty.clone()).capture_name(info.name.clone());
        }

        for stmt in &edge_stmts {
            builder = builder.edge(*stmt);
        }

        for stmt in &node_stmts {
            builder = builder.node(*stmt);
        }

        let ug = builder.new();

        // Phase 4: Register real port/capture SSAs in emit context
        let ug_info = ug.expect_info(ctx.stage);
        let port_ssas = collect_port_ssa_names(ug_info.edge_ports(), &port_infos);
        let cap_ssas = collect_port_ssa_names(ug_info.capture_ports(), &cap_infos);
        register_ssa_names(port_ssas, ctx);
        register_ssa_names(cap_ssas, ctx);

        Ok(ug)
    }
}

impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for UnGraph<'src, TypeOutput, StmtOutput>
where
    IR: Dialect + IsEdge,
    IR::Type: Placeholder + Clone,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::UnGraph;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        self.emit_with(ctx, &|stmt, ctx| stmt.emit(ctx))
    }
}
