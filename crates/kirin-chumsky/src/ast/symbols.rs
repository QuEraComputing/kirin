use chumsky::span::SimpleSpan;
use kirin_ir::Dialect;
use kirin_prettyless::{ArenaDoc, DocAllocator, Document, PrettyPrint};

use super::Spanned;
use crate::traits::{EmitContext, EmitIR};

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

/// Implementation of EmitIR for SymbolName AST nodes.
///
/// This interns the symbol name and returns a Symbol.
impl<'src, IR> EmitIR<IR> for SymbolName<'src>
where
    IR: Dialect,
{
    type Output = kirin_ir::Symbol;

    fn emit(&self, ctx: &mut EmitContext<'_, IR>) -> Self::Output {
        ctx.stage.symbol_table_mut().intern(self.name.to_string())
    }
}

/// Implementation of PrettyPrint for SymbolName AST nodes.
///
/// Prints symbols with the `@` prefix: `@main`, `@foo`, etc.
impl<'src> PrettyPrint for SymbolName<'src> {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(format!("@{}", self.name))
    }
}
