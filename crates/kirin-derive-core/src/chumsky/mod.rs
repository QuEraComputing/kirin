/// derive macro for generating AST types to be used with chumsky parsers
pub mod ast;
/// helper attribute definitions for parse related derive macros
pub mod attrs;
/// derive parser definitions for given statement definitions
pub mod parser;

pub mod prelude {
    pub use super::ast::DeriveAST;
}
