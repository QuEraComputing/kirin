use crate::data::{Alt, Emit};

mod context;
mod enum_impl;
mod extra;
mod iter;
mod struct_impl;

pub use context::FieldsIter;
pub type FieldImpl = Alt<struct_impl::StructImpl, enum_impl::EnumImpl>;

impl Emit<'_> for FieldsIter {
    type Output = FieldImpl;
}

#[cfg(test)]
mod tests;
