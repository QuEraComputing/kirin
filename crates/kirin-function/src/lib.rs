use kirin::prelude::*;
use kirin::pretty::PrettyPrint;
pub use kirin_test_utils::UnitType;

/// A simple function type for testing.
///
/// Uses `UnitType` as the type lattice since this dialect doesn't need
/// complex type annotations.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type_lattice = T)]
#[chumsky(format = "fn {name}{ty}{body}")]
pub struct SimpleFunction<T: TypeLattice + PrettyPrint> {
    name: u32,
    ty: T,
    body: Region,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin::pretty::{Config, Document};

    #[test]
    fn test_simple_function() {
        let mut context: Context<SimpleFunction<UnitType>> = Context::default();
        let region = context.region().new();
        let function = SimpleFunction::new(&mut context, 0, UnitType, region);
        let config = Config::default();
        let doc = Document::new(config, &context);
        // Use doc.print_statement() instead of calling pretty_print directly,
        // since `function` is the build result (containing the statement ID),
        // not the dialect struct itself.
        let arena_doc = doc.print_statement(&function.id);
        let mut output = String::new();
        arena_doc
            .render_fmt(80, &mut output)
            .expect("render failed");
        println!("{}", output);
    }
}
