pub mod lattice;
mod rustfmt;
mod ssa;

#[cfg(feature = "parser")]
pub mod parser;

#[cfg(feature = "roundtrip")]
pub mod roundtrip;

pub use kirin_test_types::UnitType;
pub use rustfmt::{rustfmt, rustfmt_display};
pub use ssa::new_test_ssa;

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
