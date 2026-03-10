use kirin_ir::{Dialect, GetInfo, Id};

use super::Spanned;
use crate::traits::{EmitContext, EmitError, EmitIR};

/// A block label.
///
/// Represents syntax like: `^bb0`
#[derive(Debug, Clone, PartialEq)]
pub struct BlockLabel<'src> {
    /// The name of the block (without the `^` prefix).
    pub name: Spanned<&'src str>,
}

/// A block argument.
///
/// Represents syntax like: `%arg: i32`
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::Type as HasParser<'t>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockArgument<'src, TypeOutput> {
    /// The name of the argument (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The type of the argument.
    pub ty: Spanned<TypeOutput>,
}

/// A block header containing the label and arguments.
///
/// Represents syntax like: `^bb0(%arg0: i32, %arg1: f64)`
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::Type as HasParser<'t>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockHeader<'src, TypeOutput> {
    /// The block label.
    pub label: BlockLabel<'src>,
    /// The block arguments.
    pub arguments: Vec<Spanned<BlockArgument<'src, TypeOutput>>>,
}

/// A basic block containing a header and statements.
///
/// Represents syntax like:
/// ```ignore
/// ^bb0(%arg: i32) {
///     %x = add %arg, %arg;
///     return %x;
/// }
/// ```
///
/// The `TypeOutput` parameter is the parsed type representation.
/// The `StmtOutput` parameter is the parsed statement representation.
#[derive(Debug, Clone, PartialEq)]
pub struct Block<'src, TypeOutput, StmtOutput> {
    /// The block header with label and arguments.
    pub header: Spanned<BlockHeader<'src, TypeOutput>>,
    /// The statements in the block.
    pub statements: Vec<Spanned<StmtOutput>>,
}

/// A region containing multiple blocks.
///
/// Represents syntax like:
/// ```ignore
/// {
///     ^entry(%arg: i32) { ... };
///     ^bb1() { ... };
/// }
/// ```
///
/// The `TypeOutput` parameter is the parsed type representation.
/// The `StmtOutput` parameter is the parsed statement representation.
#[derive(Debug, Clone, PartialEq)]
pub struct Region<'src, TypeOutput, StmtOutput> {
    /// The blocks in the region.
    pub blocks: Vec<Spanned<Block<'src, TypeOutput, StmtOutput>>>,
}

/// Implementation of EmitIR for BlockLabel AST nodes.
///
/// This looks up the block by name and returns it as a Successor.
/// The block must have been previously registered in the emit context.
impl<'src, IR> EmitIR<IR> for BlockLabel<'src>
where
    IR: Dialect,
{
    type Output = kirin_ir::Successor;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        let block = ctx
            .lookup_block(self.name.value)
            .ok_or_else(|| EmitError::UndefinedBlock(self.name.value.to_string()))?;
        Ok(kirin_ir::Successor::from_block(block))
    }
}

/// Emit a single block AST node into the IR, reusing an existing block ID if
/// the name was already registered (e.g. by a two-pass Region emit).
///
/// Uses a two-phase approach: first creates the block with its arguments (to
/// get real `BlockArgument` SSAs), then emits statements and attaches them.
/// This avoids `BuilderBlockArgument` placeholders which panic when nested
/// blocks (e.g. if/else bodies) reference outer block arguments.
fn emit_block<'src, TypeOutput, StmtOutput, IR>(
    block_ast: &Block<'src, TypeOutput, StmtOutput>,
    ctx: &mut EmitContext<'_, IR>,
    emit_statement: &impl for<'ctx> Fn(
        &StmtOutput,
        &mut EmitContext<'ctx, IR>,
    ) -> Result<kirin_ir::Statement, EmitError>,
) -> Result<kirin_ir::Block, EmitError>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
{
    // Collect argument info for registration.
    let arg_info: Vec<_> = block_ast
        .header
        .value
        .arguments
        .iter()
        .map(|arg| {
            let name = arg.value.name.value.to_string();
            let ty: IR::Type = arg.value.ty.value.emit(ctx)?;
            Ok((name, ty))
        })
        .collect::<Result<Vec<_>, EmitError>>()?;

    // Phase 1: Build the block with arguments only (no statements).
    // This creates real BlockArgument SSAs that nested blocks can safely
    // reference without triggering BuilderBlockArgument resolution panics.
    let block_name = block_ast.header.value.label.name.value.to_string();
    let mut builder = ctx.stage.block().name(block_name);
    for (name, ty) in &arg_info {
        builder = builder.argument(ty.clone()).arg_name(name.clone());
    }
    let block = builder.new();

    // Read back the real BlockArgument SSAs and register them in emit context.
    let block_args: Vec<kirin_ir::SSAValue> = block
        .expect_info(ctx.stage)
        .arguments
        .iter()
        .map(|arg| kirin_ir::SSAValue::from(Id::from(*arg)))
        .collect();
    for ((name, _), ssa) in arg_info.iter().zip(block_args.iter()) {
        ctx.register_ssa(name.clone(), *ssa);
    }

    // Phase 2: Emit statements now that block arguments are real SSAs.
    let mut stmts = Vec::new();
    let mut terminator = None;
    for stmt_ast in &block_ast.statements {
        let stmt = emit_statement(&stmt_ast.value, ctx)?;
        let is_terminator = stmt
            .get_info(ctx.stage)
            .expect("statement should exist")
            .definition()
            .is_terminator();
        if is_terminator {
            terminator = Some(stmt);
        } else {
            stmts.push(stmt);
        }
    }

    // Attach statements and terminator to the already-created block.
    ctx.stage
        .attach_statements_to_block(block, &stmts, terminator);

    // Register the block only if not already registered (two-pass Region
    // creates stubs first, so the name may already be present).
    let block_label = block_ast.header.value.label.name.value;
    if ctx.lookup_block(block_label).is_none() {
        ctx.register_block(block_label.to_string(), block);
    }

    Ok(block)
}

impl<'src, TypeOutput, StmtOutput> Block<'src, TypeOutput, StmtOutput> {
    pub fn emit_with<IR>(
        &self,
        ctx: &mut EmitContext<'_, IR>,
        emit_statement: &impl for<'ctx> Fn(
            &StmtOutput,
            &mut EmitContext<'ctx, IR>,
        ) -> Result<kirin_ir::Statement, EmitError>,
    ) -> Result<kirin_ir::Block, EmitError>
    where
        IR: Dialect,
        TypeOutput: EmitIR<IR, Output = IR::Type>,
    {
        emit_block(self, ctx, emit_statement)
    }
}

/// Implementation of EmitIR for Block AST nodes.
///
/// This builds an IR block with the parsed label, arguments, and statements.
/// Block arguments are created with their parsed names and types.
///
/// The `TypeOutput: EmitIR<IR, Output = IR::Type>` bound allows proper type
/// conversion from the parsed type AST to the IR's type lattice via the EmitIR trait.
impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for Block<'src, TypeOutput, StmtOutput>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::Block;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        self.emit_with(ctx, &|stmt, ctx| stmt.emit(ctx))
    }
}

impl<'src, TypeOutput, StmtOutput> Region<'src, TypeOutput, StmtOutput> {
    pub fn emit_with<IR>(
        &self,
        ctx: &mut EmitContext<'_, IR>,
        emit_statement: &impl for<'ctx> Fn(
            &StmtOutput,
            &mut EmitContext<'ctx, IR>,
        ) -> Result<kirin_ir::Statement, EmitError>,
    ) -> Result<kirin_ir::Region, EmitError>
    where
        IR: Dialect,
        TypeOutput: EmitIR<IR, Output = IR::Type>,
    {
        // Pass 1: Create stub blocks and register their names so that forward
        // references (e.g. `br ^exit` before ^exit is defined) can resolve.
        let stub_blocks: Vec<_> = self
            .blocks
            .iter()
            .map(|block_ast| {
                let name = block_ast.value.header.value.label.name.value.to_string();
                let stub = ctx.stage.block().name(name.clone()).new();
                ctx.register_block(name, stub);
                stub
            })
            .collect();

        // Pass 2: Emit full block bodies. Successor references inside these
        // blocks resolve to the stubs created above.
        let real_blocks: Vec<_> = self
            .blocks
            .iter()
            .map(|block_ast| emit_block(&block_ast.value, ctx, emit_statement))
            .collect::<Result<Vec<_>, EmitError>>()?;

        // Swap real block data into stub arena slots so that all existing
        // Successor handles (which point to stub IDs) see the real data.
        // This also remaps statement parents and block-arg ownership to stub IDs.
        for (&stub, &real) in stub_blocks.iter().zip(real_blocks.iter()) {
            ctx.stage.remap_block_identity(stub, real);
        }

        // Build the region using the stub IDs (now containing real data).
        let mut builder = ctx.stage.region();
        for block in stub_blocks {
            builder = builder.add_block(block);
        }
        Ok(builder.new())
    }
}

/// Implementation of EmitIR for Region AST nodes.
///
/// This builds an IR region containing all the parsed blocks.
/// Uses two-pass emit to support forward block references (e.g. `br ^exit`
/// before `^exit` is defined).
///
/// The `TypeOutput: EmitIR<IR, Output = IR::Type>` bound allows proper type
/// conversion for block arguments within the region via the EmitIR trait.
impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for Region<'src, TypeOutput, StmtOutput>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::Region;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Result<Self::Output, EmitError> {
        self.emit_with(ctx, &|stmt, ctx| stmt.emit(ctx))
    }
}
