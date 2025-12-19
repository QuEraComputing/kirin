/// traits and tools for derive macro definitions
pub mod derive;
/// code generation gadgets for derive macros
pub mod gadgets;
/// intermediate representation for derive macros and code generation
pub mod ir;
/// miscellaneous utilities
pub mod misc;
/// Kirin's built-in derive macros.
pub mod kirin;

/// debugging utilities
#[cfg(feature = "debug")]
pub mod debug;

/// commonly used items from kirin-derive-core
pub mod prelude {
    pub use crate::derive::*;
    pub use crate::gadgets::*;
    pub use crate::ir::*;
    pub use crate::misc::*;
    pub use crate::target;
}
