pub mod check;
pub mod data;
pub mod field;
mod utils;

pub mod prelude {
    pub use crate::check::CheckTraitInfo;
    pub use crate::data::*;
    pub use crate::{generate_derive, derive_accessor, derive_check};
    pub use crate::field::AccessorTraitInfo;
    pub use crate::utils::*;
}

#[cfg(test)]
mod tests;
