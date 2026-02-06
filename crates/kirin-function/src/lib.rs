use kirin::prelude::*;
pub use kirin_test_utils::UnitType;

/// A simple function body statement for testing.
///
/// This is a structural container holding the function body Region.
/// Name, signature, and return type live on StagedFunction/SpecializedFunction,
/// not on the body statement. See `Context::specialize()` for design rationale.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type_lattice = UnitType)]
#[chumsky(format = "{body}")]
pub struct SimpleFunction {
    body: Region,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin::pretty::{Config, Document};

    #[test]
    fn test_simple_function() {
        let mut context: Context<SimpleFunction> = Context::default();
        let region = context.region().new();
        let function = SimpleFunction::new(&mut context, region);
        let config = Config::default();
        let doc = Document::new(config, &context);
        let arena_doc = doc.print_statement(&function.id);
        let mut output = String::new();
        arena_doc
            .render_fmt(80, &mut output)
            .expect("render failed");
        println!("{}", output);
    }
}
