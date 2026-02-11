mod rustfmt;
mod simple_ir_type;
#[cfg(feature = "simple-language")]
mod simple_language;
mod simple_type;
mod ssa;
mod unit_type;
mod value;

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
