mod either;
mod general;
mod regular;
mod variant_either;
mod variant_ref;
mod variant_regular;
mod variant_wrapper;
mod wrapper;

pub use either::EitherEnum;
pub use general::Enum;
pub use regular::RegularEnum;
pub use variant_either::EitherVariant;
pub use variant_ref::VariantRef;
pub use variant_regular::RegularVariant;
pub use variant_wrapper::{NamedWrapperVariant, UnnamedWrapperVariant, WrapperVariant};
pub use wrapper::WrapperEnum;
