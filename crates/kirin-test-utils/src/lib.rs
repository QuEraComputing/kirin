pub mod lattice;
mod rustfmt;
mod ssa;
mod unit_type;

#[cfg(feature = "parser")]
pub mod parser;

#[cfg(feature = "roundtrip")]
pub mod roundtrip;

pub use rustfmt::{rustfmt, rustfmt_display};
pub use ssa::new_test_ssa;
pub use unit_type::UnitType;

/// Dump a specialized function's IR using the builtin pretty printer.
#[cfg(feature = "interpreter")]
pub fn dump_function(
    spec_fn: kirin_ir::SpecializedFunction,
    pipeline: &kirin_ir::Pipeline<kirin_ir::StageInfo<kirin_test_languages::CompositeLanguage>>,
    stage_id: kirin_ir::CompileStage,
) -> String {
    use kirin_prettyless::PrettyPrintExt;
    let stage = pipeline.stage(stage_id).unwrap();
    spec_fn.sprint(stage)
}

#[cfg(feature = "parser")]
#[macro_export]
macro_rules! parse_tokens {
    ($input:expr, $parser:expr) => {{
        use $crate::parser::Parser;
        let stream = $crate::parser::token_stream($input);
        let result = $parser.parse(stream);
        if result.has_output() {
            Ok(result.into_output().expect("parser output should exist"))
        } else {
            Err(result
                .errors()
                .map(|error| format!("{error:?}"))
                .collect::<::std::vec::Vec<_>>())
        }
    }};
}
