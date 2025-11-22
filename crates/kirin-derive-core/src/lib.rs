pub mod check;
pub mod data;
pub mod field;
pub mod field_mut;
mod utils;

pub mod prelude {
    pub use crate::check::CheckTraitInfo;
    pub use crate::data::*;
    pub use crate::{generate_derive, derive_accessor, derive_accessor_mut, derive_check};
    pub use crate::field::AccessorTraitInfo;
    pub use crate::field_mut::AccessorMutTraitInfo;
    pub use crate::utils::*;
}

#[cfg(test)]
mod tests;
