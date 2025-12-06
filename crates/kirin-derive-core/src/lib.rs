pub mod builder;
pub mod check;
pub mod data;
pub mod dialect;
pub mod empty;
pub mod field;
pub mod from;
pub mod name;
pub mod utils;

pub mod prelude {
    pub use crate::builder::Builder;
    pub use crate::check::CheckInfo;
    pub use crate::name::NameInfo;
    pub use crate::data::*;
    pub use crate::dialect::DialectInfo;
    pub use crate::empty::Empty;
    pub use crate::field::FieldIterInfo;
    pub use crate::from::FromInfo;
    pub use crate::utils::*;
    pub use crate::{
        derive_builder, derive_check, derive_dialect, derive_empty, derive_field_iter,
        derive_field_iter_mut, derive_from, derive_name,
    };
}

#[cfg(test)]
mod tests;
