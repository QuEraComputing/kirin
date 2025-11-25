use std::ops::Deref;

use crate::data::StatementFields;

use super::either::EitherEnum;
use super::regular::RegularEnum;
use super::variant_either::EitherVariant;
use super::variant_regular::RegularVariant;
use super::variant_wrapper::WrapperVariant;
use super::wrapper::WrapperEnum;

pub struct VariantIter<'a, T> {
    pub parent: &'a T,
    pub current_index: usize,
    pub total_variants: usize,
}

impl<'a, T> Iterator for VariantIter<'a, T> {
    type Item = VariantRef<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.total_variants {
            let variant_ref = VariantRef {
                parent: self.parent,
                variant_index: self.current_index,
            };
            self.current_index += 1;
            Some(variant_ref)
        } else {
            None
        }
    }
}

pub struct VariantRef<'a, T> {
    pub parent: &'a T,
    pub variant_index: usize,
}

impl<'a, 'input, T> VariantRef<'a, RegularEnum<'input, T>>
where
    T: StatementFields<'input>,
{
    pub fn variant(&'a self) -> &'a RegularVariant<'input, T> {
        &self.parent.variants[self.variant_index]
    }
}

impl<'a, 'input, T> VariantRef<'a, WrapperEnum<'input, T>> {
    pub fn variant(&'a self) -> &'a WrapperVariant<'input, T> {
        &self.parent.variants[self.variant_index]
    }
}

impl<'a, 'input, T> VariantRef<'a, EitherEnum<'input, T>>
where
    T: StatementFields<'input>,
{
    pub fn variant(&'a self) -> &'a EitherVariant<'input, T> {
        &self.parent.variants[self.variant_index]
    }
}

impl<'a, 'input, T> Deref for VariantRef<'a, RegularEnum<'input, T>>
where
    T: StatementFields<'input>,
{
    type Target = RegularVariant<'input, T>;

    fn deref(&self) -> &Self::Target {
        &self.parent.variants[self.variant_index]
    }
}

impl<'a, 'input, T> Deref for VariantRef<'a, WrapperEnum<'input, T>> {
    type Target = WrapperVariant<'input, T>;

    fn deref(&self) -> &Self::Target {
        &self.parent.variants[self.variant_index]
    }
}

impl<'a, 'input, T> Deref for VariantRef<'a, EitherEnum<'input, T>>
where
    T: StatementFields<'input>,
{
    type Target = EitherVariant<'input, T>;

    fn deref(&self) -> &Self::Target {
        &self.parent.variants[self.variant_index]
    }
}
