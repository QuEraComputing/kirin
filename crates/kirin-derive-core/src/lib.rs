mod data;
mod field;
mod check;
mod accessor;
mod derive;
mod instruction;
mod traits;

pub use accessor::FieldAccessor;
pub use derive::DeriveContext;
pub use instruction::{
    DeriveHasArguments, DeriveHasRegions, DeriveHasResults, DeriveHasSuccessors, DeriveIsConstant,
    DeriveIsPure, DeriveIsTerminator,DeriveInstruction,
};
pub use traits::{DeriveHelperAttribute, WriteTokenStream, DeriveTrait};
pub use field::{DataAccessor, AccessorInfo};

#[cfg(test)]
mod tests;


pub fn has_attr(attrs: &[syn::Attribute], attr_name: &str, option: &str) -> bool {
    let mut has_option = false;
    for attr in attrs {
        if attr.path().is_ident(attr_name) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(option) {
                    has_option = true;
                }
                Ok(())
            })
            .unwrap();
        }
    }
    has_option
}

pub fn is_attr_option_true(attrs: &[syn::Attribute], option_name: &str) -> bool {
    let mut value = false;
    attrs.iter().for_each(|attr| {
        if attr.path().is_ident("kirin") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(option_name) {
                    meta.value()?.parse::<syn::LitBool>().map(|lit| {
                        value = lit.value;
                    }).unwrap();
                }
                Ok(())
            })
            .unwrap();
        }
    });
    value
}
