//! Abstract Syntax Tree types for Kirin chumsky parsers.
//!
//! These types represent the parsed syntax elements before they are
//! converted to the IR representation.

use chumsky::span::SimpleSpan;
use kirin_ir::{Dialect, FiniteLattice, GetInfo, SSAKind};

use crate::traits::{EmitContext, EmitIR};

/// A value with an associated span.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: SimpleSpan,
}

impl<T: Copy> Copy for Spanned<T> {}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> Spanned<T> {
    /// Creates a new spanned value.
    pub fn new(value: T, span: SimpleSpan) -> Self {
        Self { value, span }
    }

    /// Maps the inner value using the provided function.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }
}

/// An SSA value reference with optional type annotation.
///
/// Represents syntax like:
/// - `%value` (without type)
/// - `%value: type` (with type)
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::TypeLattice as HasParser<'tokens, 'src>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct SSAValue<'src, TypeOutput> {
    /// The name of the SSA value (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The optional type annotation.
    pub ty: Option<TypeOutput>,
}

/// A result value (left-hand side of an SSA assignment).
///
/// Represents syntax like: `%result` in `%result = add %a, %b`
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::TypeLattice as HasParser<'tokens, 'src>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultValue<'src, TypeOutput> {
    /// The name of the result value (without the `%` prefix).
    pub name: Spanned<&'src str>,
    /// The optional type annotation (often inferred).
    pub ty: Option<TypeOutput>,
}

/// The type portion of an SSA value annotation.
///
/// Used when the type is specified separately from the SSA value name,
/// for example in `add %a, %b -> bool` where `bool` is the result type.
///
/// The `TypeOutput` parameter is the parsed type representation, typically
/// `<L::TypeLattice as HasParser<'tokens, 'src>>::Output`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeofSSAValue<TypeOutput> {
    /// The type value.
    pub ty: TypeOutput,
    /// The span of the type in the source.
    pub span: SimpleSpan,
}

/// The name portion of an SSA value.
///
/// Used when only the name is needed, not the full SSA value with type.
#[derive(Debug, Clone, PartialEq)]
pub struct NameofSSAValue<'src> {
    /// The name of the SSA value (without the `%` prefix).
    pub name: &'src str,
    /// The span of the name in the source.
    pub span: SimpleSpan,
}

/// A symbol name (prefixed with `@` in source).
///
/// Represents syntax like: `@main`, `@my_function`
/// Used for function names, global symbols, etc.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolName<'src> {
    /// The name of the symbol (without the `@` prefix).
    pub name: &'src str,
    /// The span of the symbol in the source.
    pub span: SimpleSpan,
}

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
/// `<L::TypeLattice as HasParser<'tokens, 'src>>::Output`.
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
/// `<L::TypeLattice as HasParser<'tokens, 'src>>::Output`.
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

/// A function type signature.
///
/// Represents syntax like: `(i32, f64) -> (bool, i32)`
#[derive(Debug, Clone)]
pub struct FunctionType<T> {
    /// The input parameter types.
    pub input_types: Vec<Spanned<T>>,
    /// The output return types.
    pub output_types: Vec<Spanned<T>>,
}

impl<T: PartialEq> PartialEq for FunctionType<T> {
    fn eq(&self, other: &Self) -> bool {
        self.input_types == other.input_types && self.output_types == other.output_types
    }
}

// === EmitIR implementations for AST types ===

/// Implementation of EmitIR for SSAValue AST nodes.
///
/// This looks up the SSA value by name in the emit context's symbol table.
/// The name must have been previously registered (e.g., when emitting a
/// ResultValue or block argument).
impl<'src, TypeOutput, IR> EmitIR<IR> for SSAValue<'src, TypeOutput>
where
    IR: Dialect,
{
    type Output = kirin_ir::SSAValue;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        ctx.lookup_ssa(self.name.value)
            .unwrap_or_else(|| panic!("Undefined SSA value: %{}", self.name.value))
    }
}

/// Implementation of EmitIR for ResultValue AST nodes.
///
/// This creates a new SSA value with the parsed name and registers it
/// in the emit context's symbol table. The created SSA has `SSAKind::BuilderResult`
/// which will be updated when the containing statement is finalized.
///
/// Note: The result index is set to 0 here. For statements with multiple results,
/// the generated code should handle setting the correct indices.
///
/// The `TypeOutput: EmitIR<IR, Output = IR::TypeLattice>` bound allows proper type
/// conversion from the parsed type AST to the IR's type lattice via the EmitIR trait.
impl<'src, TypeOutput, IR> EmitIR<IR> for ResultValue<'src, TypeOutput>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::TypeLattice>,
{
    type Output = kirin_ir::ResultValue;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        // Convert the parsed type to TypeLattice via EmitIR, or use top() if no type annotation
        let ty: IR::TypeLattice = self
            .ty
            .as_ref()
            .map(|t| t.emit(ctx))
            .unwrap_or_else(<IR::TypeLattice as FiniteLattice>::top);

        // Create a new SSA value with the parsed name and type
        let ssa = ctx
            .context
            .ssa()
            .name(self.name.value.to_string())
            .ty(ty)
            .kind(SSAKind::BuilderResult(0))
            .new();

        // Register the SSA in the symbol table for later reference
        ctx.register_ssa(self.name.value.to_string(), ssa);

        ssa.into()
    }
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
        block.into()
    }
}

/// Implementation of EmitIR for Block AST nodes.
///
/// This builds an IR block with the parsed label, arguments, and statements.
/// Block arguments are created with their parsed names and types.
///
/// The `TypeOutput: EmitIR<IR, Output = IR::TypeLattice>` bound allows proper type
/// conversion from the parsed type AST to the IR's type lattice via the EmitIR trait.
impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for Block<'src, TypeOutput, StmtOutput>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::TypeLattice>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::Block;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        // Collect argument info for registration
        // Convert TypeOutput to TypeLattice using EmitIR
        let arg_info: Vec<_> = self
            .header
            .value
            .arguments
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                let name = arg.value.name.value.to_string();
                let ty: IR::TypeLattice = arg.value.ty.value.emit(ctx);
                (name, ty, idx)
            })
            .collect();

        // Create placeholder SSAs for block arguments so they can be referenced
        // in statement emission. These use BuilderBlockArgument kind.
        for (name, ty, idx) in &arg_info {
            let ssa = ctx
                .context
                .ssa()
                .name(name.clone())
                .ty(ty.clone())
                .kind(SSAKind::BuilderBlockArgument(*idx))
                .new();
            ctx.register_ssa(name.clone(), ssa);
        }

        // Emit all statements in the block and check which are terminators
        let statements: Vec<_> = self
            .statements
            .iter()
            .map(|stmt_ast| {
                let stmt = stmt_ast.value.emit(ctx);
                let is_terminator = stmt
                    .get_info(ctx.context)
                    .expect("statement should exist")
                    .definition()
                    .is_terminator();
                (stmt, is_terminator)
            })
            .collect();

        // Build the block with arguments and statements
        let block_name = self.header.value.label.name.value.to_string();
        let mut builder = ctx.context.block().name(block_name);

        for (name, ty, _) in arg_info {
            builder = builder.argument_with_name(name, ty);
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

        // Register the block in the symbol table for successor resolution
        ctx.register_block(self.header.value.label.name.value.to_string(), block);

        block
    }
}

/// Implementation of EmitIR for Region AST nodes.
///
/// This builds an IR region containing all the parsed blocks.
///
/// The `TypeOutput: EmitIR<IR, Output = IR::TypeLattice>` bound allows proper type
/// conversion for block arguments within the region via the EmitIR trait.
impl<'src, TypeOutput, StmtOutput, IR> EmitIR<IR> for Region<'src, TypeOutput, StmtOutput>
where
    IR: Dialect,
    TypeOutput: EmitIR<IR, Output = IR::TypeLattice>,
    StmtOutput: EmitIR<IR, Output = kirin_ir::Statement>,
{
    type Output = kirin_ir::Region;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        // Emit all blocks first (this registers them in the symbol table)
        let blocks: Vec<_> = self
            .blocks
            .iter()
            .map(|block_ast| block_ast.value.emit(ctx))
            .collect();

        // Build the region with the emitted blocks
        let mut builder = ctx.context.region();
        for block in blocks {
            builder = builder.add_block(block);
        }

        // Finalize the region
        builder.new()
    }
}

/// Implementation of EmitIR for Spanned values.
///
/// This simply delegates to the inner value's EmitIR implementation.
impl<T, IR> EmitIR<IR> for Spanned<T>
where
    IR: Dialect,
    T: EmitIR<IR>,
{
    type Output = T::Output;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        self.value.emit(ctx)
    }
}

// === Type coercion helpers for generated parser code ===
//
// These functions enable type-safe coercion between associated type form and
// concrete type form for Block and Region types. This is needed because:
//
// 1. The parser returns `Block<..., <Language as HasDialectParser>::Output>`
// 2. The AST field type is `Block<..., ConcreteAST<..., Language>>`
// 3. The HasDialectParser impl defines `type Output = ConcreteAST<..., Language>`
//
// While these types are identical after type substitution, Rust's type system
// cannot prove this equality within the impl body where the association is defined.
// These helper functions use transmute to perform the coercion, which is sound
// because the types are guaranteed to be identical at monomorphization time.

/// Coerces a `Spanned<Block>` from associated type form to concrete type form.
///
/// # Safety Invariant
///
/// This function is only safe to call when `From` and `To` are the same type,
/// just expressed differently (e.g., associated type vs concrete type).
/// This is guaranteed by the macro-generated code that calls this function.
///
/// The caller must ensure that `From` equals `To` via the HasDialectParser impl,
/// i.e., `<Language as HasDialectParser<'tokens, 'src, Language>>::Output == To`.
#[inline(always)]
pub fn coerce_block_type<TypeAST, From, To, B>(block: B) -> Spanned<Block<'static, TypeAST, To>>
where
    B: CoerceSpannedBlock<TypeAST, From, To>,
{
    block.coerce()
}

/// Coerces a `Region` from associated type form to concrete type form.
///
/// # Safety Invariant
///
/// This function is only safe to call when `From` and `To` are the same type,
/// just expressed differently (e.g., associated type vs concrete type).
/// This is guaranteed by the macro-generated code that calls this function.
///
/// The caller must ensure that `From` equals `To` via the HasDialectParser impl,
/// i.e., `<Language as HasDialectParser<'tokens, 'src, Language>>::Output == To`.
#[inline(always)]
pub fn coerce_region_type<TypeAST, From, To, R>(region: R) -> Region<'static, TypeAST, To>
where
    R: CoerceRegion<TypeAST, From, To>,
{
    region.coerce()
}

/// Helper trait for coercing Spanned<Block> types.
/// This trait-based approach avoids late-bound lifetime issues.
pub trait CoerceSpannedBlock<TypeAST, From, To> {
    fn coerce(self) -> Spanned<Block<'static, TypeAST, To>>;
}

impl<'src, TypeAST, From, To> CoerceSpannedBlock<TypeAST, From, To>
    for Spanned<Block<'src, TypeAST, From>>
{
    #[inline(always)]
    fn coerce(self) -> Spanned<Block<'static, TypeAST, To>> {
        // SAFETY: This transmute is sound because:
        // 1. `From` and `To` are the same type by construction (associated type = concrete type)
        // 2. Block<..., From> and Block<..., To> have identical layout when From == To
        // 3. Spanned is a transparent wrapper that doesn't affect layout
        // 4. The compiler will verify type equality at monomorphization time
        // 5. The lifetime change from 'src to 'static is safe because we're just changing
        //    the phantom lifetime annotation - the actual data layout is unchanged.
        //
        // This coercion is necessary because Rust cannot prove that an associated type
        // equals its definition within the impl body where the definition appears.
        // At monomorphization, after type substitution, the types will be identical.
        unsafe {
            std::mem::transmute::<
                Spanned<Block<'src, TypeAST, From>>,
                Spanned<Block<'static, TypeAST, To>>,
            >(self)
        }
    }
}

/// Helper trait for coercing Region types.
/// This trait-based approach avoids late-bound lifetime issues.
pub trait CoerceRegion<TypeAST, From, To> {
    fn coerce(self) -> Region<'static, TypeAST, To>;
}

impl<'src, TypeAST, From, To> CoerceRegion<TypeAST, From, To> for Region<'src, TypeAST, From> {
    #[inline(always)]
    fn coerce(self) -> Region<'static, TypeAST, To> {
        // SAFETY: This transmute is sound because:
        // 1. `From` and `To` are the same type by construction (associated type = concrete type)
        // 2. Region<..., From> and Region<..., To> have identical layout when From == To
        // 3. The compiler will verify type equality at monomorphization time
        // 4. The lifetime change from 'src to 'static is safe because we're just changing
        //    the phantom lifetime annotation - the actual data layout is unchanged.
        //
        // This coercion is necessary because Rust cannot prove that an associated type
        // equals its definition within the impl body where the definition appears.
        // At monomorphization, after type substitution, the types will be identical.
        unsafe {
            std::mem::transmute::<Region<'src, TypeAST, From>, Region<'static, TypeAST, To>>(self)
        }
    }
}
