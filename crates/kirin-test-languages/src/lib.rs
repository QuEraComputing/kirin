#[cfg(feature = "composite-language")]
mod composite_language;
#[cfg(feature = "simple-language")]
mod simple_language;

pub use kirin_test_types::SimpleType;
pub use kirin_test_types::Value;

#[cfg(feature = "composite-language")]
pub use composite_language::CompositeLanguage;
#[cfg(feature = "simple-language")]
pub use simple_language::SimpleLanguage;
