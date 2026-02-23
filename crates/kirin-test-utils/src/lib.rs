mod rustfmt;
mod simple_ir_type;
#[cfg(feature = "simple-language")]
mod simple_language;
mod simple_type;
mod ssa;
mod unit_type;
mod value;

#[cfg(feature = "interpreter")]
mod interval;
#[cfg(feature = "composite-language")]
mod composite_language;

#[cfg(feature = "parser")]
pub mod parser;

#[cfg(feature = "roundtrip")]
pub mod roundtrip;

pub use rustfmt::{rustfmt, rustfmt_display};
pub use simple_ir_type::SimpleIRType;
#[cfg(feature = "simple-language")]
pub use simple_language::SimpleLanguage;
pub use simple_type::SimpleType;
pub use ssa::new_test_ssa;
pub use unit_type::UnitType;
pub use value::Value;

#[cfg(feature = "interpreter")]
pub use interval::{Bound, Interval, interval_add, interval_mul, interval_neg, interval_sub};
#[cfg(feature = "composite-language")]
pub use composite_language::CompositeLanguage;

/// Dump a specialized function's IR using the builtin pretty printer.
#[cfg(feature = "interpreter")]
pub fn dump_function(
    spec_fn: kirin_ir::SpecializedFunction,
    pipeline: &kirin_ir::Pipeline<kirin_ir::StageInfo<CompositeLanguage>>,
    stage_id: kirin_ir::CompileStage,
) -> String {
    use kirin_prettyless::PrettyPrintExt;
    let stage = pipeline.stage(stage_id).unwrap();
    spec_fn.sprint(stage)
}

pub use SimpleIRType::*;

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
