pub mod dialects;
pub use kirin_chumsky as parsers;
pub use kirin_ir as ir;
pub use kirin_prettyless as pretty;

#[cfg(feature = "interpreter")]
pub use kirin_interpreter as interpreter;

pub mod prelude {
    pub use kirin_chumsky::prelude::*;
    pub use kirin_ir::*;
}
