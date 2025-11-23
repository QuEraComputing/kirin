pub trait PropertyAttribute {
    fn is_constant(&self) -> Option<bool>;
    fn is_pure(&self) -> Option<bool>;
    fn is_terminator(&self) -> Option<bool>;
}

mod builder;
mod enum_impl;
mod field_impl;
mod struct_impl;
mod utils;

pub use builder::{Builder, FieldBuilder};
pub use enum_impl::{EnumAttribute, VariantAttribute};
pub use field_impl::FieldAttribute;
pub use struct_impl::StructAttribute;
