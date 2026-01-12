use crate::misc::{is_type, is_type_in};

#[derive(Debug, Clone, Default)]
pub enum Collection {
    #[default]
    Single,
    Vec,
    Option,
}

impl Collection {
    pub fn from_type<I>(ty: &syn::Type, name: &I) -> Option<Self>
    where
        I: ?Sized,
        syn::Ident: PartialEq<I> + PartialEq<str>,
    {
        if is_type(ty, name) {
            Some(Collection::Single)
        } else if is_type_in(ty, name, |seg| seg.ident == "Vec") {
            Some(Collection::Vec)
        } else if is_type_in(ty, name, |seg| seg.ident == "Option") {
            Some(Collection::Option)
        } else {
            None
        }
    }
}
