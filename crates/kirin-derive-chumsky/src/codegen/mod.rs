//! Code generation for chumsky derive macros.

mod ast;
mod bounds;
mod config;
mod emit_ir;
mod helpers;
mod parser;
mod pretty_print;
pub(crate) mod type_enum;

pub(crate) use bounds::{ImplBounds, WhereClauseExt, init_where_clause};
pub(crate) use config::GeneratorConfig;
pub(crate) use helpers::{
    build_ast_generics, collect_all_value_types_needing_bounds, collect_wrapper_types,
    filter_ast_fields, format_for_statement, generate_enum_match, get_fields_in_format,
};

pub use self::ast::GenerateAST;
pub use self::emit_ir::GenerateEmitIR;
pub use self::parser::GenerateHasDialectParser;
pub use self::pretty_print::GeneratePrettyPrint;
