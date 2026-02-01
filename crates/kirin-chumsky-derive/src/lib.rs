//! # kirin-chumsky-derive
//!
//! Derive macros for implementing chumsky parsers for Kirin dialects.
//!
//! This crate provides the following derive macros:
//! - `HasRecursiveParser`: Implements the `HasRecursiveParser` trait for parsing dialect statements
//! - `WithAbstractSyntaxTree`: Generates AST types and implements the `WithAbstractSyntaxTree` trait
//! - `EmitIR`: Implements the `EmitIR` trait for converting AST to IR nodes
//! - `DialectParser`: Combined macro that derives all three above
//!
//! # Example
//!
//! ```ignore
//! use kirin_chumsky::prelude::*;
//! use kirin_chumsky_derive::{HasRecursiveParser, WithAbstractSyntaxTree, EmitIR};
//!
//! #[derive(HasRecursiveParser, WithAbstractSyntaxTree, EmitIR)]
//! #[kirin(type_lattice = Type)]
//! #[chumsky(crate = kirin_chumsky)]
//! pub enum MyDialectAST {
//!     #[chumsky(format = "{res} = add {lhs} {rhs}")]
//!     Add(SSAValue, SSAValue, ResultValue),
//!     #[chumsky(format = "{res} = mul {lhs} {rhs}")]
//!     Mul(SSAValue, SSAValue, ResultValue),
//!     #[chumsky(format = "return {0}")]
//!     Return(SSAValue),
//! }
//! ```

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// Derives the `HasRecursiveParser` trait for a dialect type.
///
/// This macro generates a recursive parser implementation that can parse
/// statements according to the format strings specified in `#[chumsky(format = "...")]`
/// attributes.
///
/// # Attributes
///
/// - `#[chumsky(format = "...")]` - Required on each variant/struct. Specifies the syntax format.
/// - `#[chumsky(crate = path)]` - Optional. Overrides the path to the kirin-chumsky crate.
///
/// # Format String Syntax
///
/// Format strings contain:
/// - Literal tokens that must be matched exactly
/// - Field interpolations in `{field_name}` or `{index}` format
/// - Optional field options like `{field:type}` or `{field:name}`
///
/// # Example
///
/// ```ignore
/// #[derive(HasRecursiveParser)]
/// #[kirin(type_lattice = Type)]
/// pub enum ArithOps {
///     #[chumsky(format = "{res} = add {lhs}, {rhs}")]
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
///     
///     #[chumsky(format = "return {0}")]
///     Return(SSAValue),
/// }
/// ```
#[proc_macro_derive(HasRecursiveParser, attributes(kirin, chumsky, wraps))]
pub fn derive_has_recursive_parser(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input =
        match kirin_derive_core::ir::Input::<kirin_chumsky_format::ChumskyLayout>::from_derive_input(
            &ast,
        ) {
            Ok(ir) => ir,
            Err(err) => return err.write_errors().into(),
        };

    let generator = kirin_chumsky_format::GenerateHasRecursiveParser::new(&ir_input);
    generator.generate(&ir_input).into()
}

/// Derives the `WithAbstractSyntaxTree` trait for a dialect type.
///
/// This macro generates:
/// 1. An AST type with the suffix `AST` (e.g., `MyDialectAST` for `MyDialect`)
/// 2. An implementation of `WithAbstractSyntaxTree` that maps the IR type to the AST type
///
/// The generated AST type includes the appropriate lifetimes and generic parameters
/// for working with the chumsky parser infrastructure.
///
/// # Example
///
/// ```ignore
/// #[derive(WithAbstractSyntaxTree)]
/// #[kirin(type_lattice = Type)]
/// pub enum ArithOps {
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
///     Return(SSAValue),
/// }
///
/// // Generates:
/// // pub enum ArithOpsAST<'tokens, 'src: 'tokens, Language: LanguageParser<'tokens, 'src>> {
/// //     Add { res: ResultValue<'tokens, 'src, Language>, lhs: SSAValue<'tokens, 'src, Language>, ... },
/// //     Return(SSAValue<'tokens, 'src, Language>),
/// // }
/// ```
#[proc_macro_derive(WithAbstractSyntaxTree, attributes(kirin, chumsky, wraps))]
pub fn derive_with_abstract_syntax_tree(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input =
        match kirin_derive_core::ir::Input::<kirin_chumsky_format::ChumskyLayout>::from_derive_input(
            &ast,
        ) {
            Ok(ir) => ir,
            Err(err) => return err.write_errors().into(),
        };

    let generator = kirin_chumsky_format::GenerateWithAbstractSyntaxTree::new(&ir_input);
    generator.generate(&ir_input).into()
}

/// Derives the `EmitIR` trait for an AST type.
///
/// This macro generates an implementation of `EmitIR` that converts parsed AST
/// nodes into actual IR nodes using the Context builder methods.
///
/// The generated implementation:
/// 1. Emits each field by calling `EmitIR::emit` on it
/// 2. Constructs the dialect variant directly
/// 3. Creates a statement via `ctx.context.statement().definition(variant).new()`
///
/// # Constraints
///
/// The dialect type must implement `From<DialectType>` for the language's `Dialect`.
///
/// # Example
///
/// ```ignore
/// #[derive(EmitIR)]
/// #[kirin(type_lattice = Type)]
/// pub enum ArithOps {
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
/// }
/// ```
#[proc_macro_derive(EmitIR, attributes(kirin, chumsky, wraps))]
pub fn derive_emit_ir(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input =
        match kirin_derive_core::ir::Input::<kirin_chumsky_format::ChumskyLayout>::from_derive_input(
            &ast,
        ) {
            Ok(ir) => ir,
            Err(err) => return err.write_errors().into(),
        };

    let generator = kirin_chumsky_format::GenerateEmitIR::new(&ir_input);
    generator.generate(&ir_input).into()
}

/// Combined derive macro that implements both `HasRecursiveParser` and `WithAbstractSyntaxTree`.
///
/// This is a convenience macro that combines both derives into one.
///
/// # Example
///
/// ```ignore
/// #[derive(DialectParser)]
/// #[kirin(type_lattice = Type)]
/// pub enum MyDialect {
///     #[chumsky(format = "{res} = add {lhs}, {rhs}")]
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
/// }
/// ```
#[proc_macro_derive(DialectParser, attributes(kirin, chumsky, wraps))]
pub fn derive_dialect_parser(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input =
        match kirin_derive_core::ir::Input::<kirin_chumsky_format::ChumskyLayout>::from_derive_input(
            &ast,
        ) {
            Ok(ir) => ir,
            Err(err) => return err.write_errors().into(),
        };

    let ast_generator = kirin_chumsky_format::GenerateWithAbstractSyntaxTree::new(&ir_input);
    let parser_generator = kirin_chumsky_format::GenerateHasRecursiveParser::new(&ir_input);
    let emit_generator = kirin_chumsky_format::GenerateEmitIR::new(&ir_input);

    let ast_tokens = ast_generator.generate(&ir_input);
    let parser_tokens = parser_generator.generate(&ir_input);
    let emit_tokens = emit_generator.generate(&ir_input);

    let output = quote! {
        #ast_tokens
        #parser_tokens
        #emit_tokens
    };

    output.into()
}
