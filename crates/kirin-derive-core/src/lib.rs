pub mod check;
pub mod data;
pub mod field;
pub mod field_mut;
pub mod from;
mod utils;

pub mod prelude {
    pub use crate::check::CheckTraitInfo;
    pub use crate::data::*;
    pub use crate::field::AccessorTraitInfo;
    pub use crate::field_mut::AccessorMutTraitInfo;
    pub use crate::from::FromTraitInfo;
    pub use crate::utils::*;
    pub use crate::{
        derive_accessor, derive_accessor_mut, derive_check, derive_from, generate_derive,
    };
}

#[cfg(test)]
mod tests;
