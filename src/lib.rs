pub mod dialects;
pub use kirin_chumsky as parsers;
pub use kirin_ir as ir;
pub use kirin_prettyless as pretty;

pub mod prelude {
    pub use kirin_chumsky::prelude::*;
    pub use kirin_ir::*;
}
