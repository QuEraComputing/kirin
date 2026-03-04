use kirin_ir::{Dialect, GetInfo, SSAKind};

use super::Spanned;
use crate::traits::{EmitContext, EmitIR};

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
/// `<L::Type as HasParser<'tokens, 'src>>::Output`.
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
/// `<L::Type as HasParser<'tokens, 'src>>::Output`.
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

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        let block = ctx
            .lookup_block(self.name.value)
            .unwrap_or_else(|| panic!("Undefined block: ^{}", self.name.value));
        kirin_ir::Successor::from_block(block)
    }
}

/// Emit a single block AST node into the IR, reusing an existing block ID if
/// the name was already registered (e.g. by a two-pass Region emit).
fn emit_block<'src, TypeOutput, StmtOutput, IR>(
    block_ast: &Block<'src, TypeOutput, StmtOutput>,
    ctx: &mut EmitContext<'_, IR>,
) -> kirin_ir::Block
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::Type>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    // Collect argument info for registration
    // Convert TypeOutput to Dialect::Type using EmitIR
    let arg_info: Vec<_> = block_ast
        .header
        .value
        .arguments
        .iter()
        .enumerate()
        .map(|(idx, arg)| {
            let name = arg.value.name.value.to_string();
            let ty: IR::Type = arg.value.ty.value.emit(ctx);
            (name, ty, idx)
        })
        .collect();

    // Create placeholder SSAs for block arguments so they can be referenced
    // in statement emission. These use BuilderBlockArgument kind.
    for (name, ty, idx) in &arg_info {
        let ssa = ctx
            .stage
            .ssa()
            .name(name.clone())
            .ty(ty.clone())
            .kind(SSAKind::BuilderBlockArgument(*idx))
            .new();
        ctx.register_ssa(name.clone(), ssa);
    }

    // Emit all statements in the block and check which are terminators
    let statements: Vec<_> = block_ast
        .statements
        .iter()
        .map(|stmt_ast| {
            let stmt = stmt_ast.value.emit(ctx);
            let is_terminator = stmt
                .get_info(ctx.stage)
                .expect("statement should exist")
                .definition()
                .is_terminator();
            (stmt, is_terminator)
        })
        .collect();

    // Build the block with arguments and statements
    let block_name = block_ast.header.value.label.name.value.to_string();
    let mut builder = ctx.stage.block().name(block_name);

    for (name, ty, _) in arg_info {
        builder = builder.argument(ty).arg_name(name);
    }

    // Add statements, handling terminators specially
    for (stmt, is_terminator) in statements {
        if is_terminator {
            builder = builder.terminator(stmt);
        } else {
            builder = builder.stmt(stmt);
        }
    }

    let block = builder.new();

    // Register the block only if not already registered (two-pass Region
    // creates stubs first, so the name may already be present).
    let block_label = block_ast.header.value.label.name.value;
    if ctx.lookup_block(block_label).is_none() {
        ctx.register_block(block_label.to_string(), block);
    }

    block
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

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        emit_block(self, ctx)
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

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
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
            .map(|block_ast| emit_block(&block_ast.value, ctx))
            .collect();

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
        builder.new()
    }
}
