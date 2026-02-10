//! # kirin-chumsky-derive
//!
//! Derive macros for implementing chumsky parsers for Kirin dialects.
//!
//! This crate provides the following derive macros:
//! - `HasParser`: Generates AST type, parser impl, and EmitIR impl (implements `HasParser` trait)
//! - `PrettyPrint`: Implements the `PrettyPrint` trait for roundtrip-compatible printing
//!
//! Both should be derived together to get full parsing and printing support:
//!
//! # Example
//!
//! ```ignore
//! use kirin::parsers::{HasParser, PrettyPrint};
//! use kirin::ir::Dialect;
//!
//! #[derive(Dialect, HasParser, PrettyPrint)]
//! #[kirin(type_lattice = Type)]
//! #[chumsky(crate = kirin_chumsky)]
//! pub enum MyDialect {
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

/// Derives everything needed to implement the `HasParser` trait for a dialect type.
///
/// This macro generates:
/// 1. An AST type (e.g., `MyDialectAST`) for representing parsed syntax
/// 2. An implementation of `HasRecursiveParser` (which auto-implements `HasParser`)
/// 3. An implementation of `EmitIR` for converting AST to IR nodes
///
/// After deriving, the type can be used with `MyDialect::parser()` to get a parser,
/// or with `parse::<MyDialect>(input, context)` to parse and emit IR.
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
/// use kirin::parsers::HasParser;
/// use kirin::ir::Dialect;
///
/// #[derive(Dialect, HasParser)]
/// #[kirin(type_lattice = Type)]
/// pub enum ArithOps {
///     #[chumsky(format = "{res} = add {lhs}, {rhs}")]
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
///     
///     #[chumsky(format = "return {0}")]
///     Return(SSAValue),
/// }
///
/// // Now you can use ArithOps::parser() or parse::<ArithOps>(...)
/// ```
#[proc_macro_derive(HasParser, attributes(kirin, chumsky, wraps))]
pub fn derive_has_parser(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input = match kirin_chumsky_format::parse_derive_input(&ast) {
        Ok(ir) => ir,
        Err(err) => return err.write_errors().into(),
    };

    let ast_generator = kirin_chumsky_format::GenerateAST::new(&ir_input);
    let parser_generator = kirin_chumsky_format::GenerateHasDialectParser::new(&ir_input);
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

/// Derives the `PrettyPrint` trait for a dialect type.
///
/// This macro generates an implementation of `PrettyPrint` that produces
/// output matching the format strings specified in `#[chumsky(format = "...")]`
/// attributes. This ensures roundtrip compatibility with the parser.
///
/// The generated implementation mirrors the parser's format string, printing
/// field values with appropriate formatting based on the format options.
///
/// # Example
///
/// ```ignore
/// #[derive(PrettyPrint)]
/// #[kirin(type_lattice = Type)]
/// pub enum ArithOps {
///     #[chumsky(format = "{res:name} = add {lhs}, {rhs}")]
///     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
/// }
///
/// // Output for Add { res: %x, lhs: %a, rhs: %b } would be:
/// // %x = add %a, %b
/// ```
#[proc_macro_derive(PrettyPrint, attributes(kirin, chumsky, wraps))]
pub fn derive_pretty_print(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input = match kirin_chumsky_format::parse_derive_input(&ast) {
        Ok(ir) => ir,
        Err(err) => return err.write_errors().into(),
    };

    let generator = kirin_chumsky_format::GeneratePrettyPrint::new(&ir_input);
    generator.generate(&ir_input).into()
}
