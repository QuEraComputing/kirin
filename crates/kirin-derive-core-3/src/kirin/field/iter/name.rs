use quote::{format_ident, quote};

use crate::{kirin::field::context::FieldsIter, prelude::*};

target! {
    /// Name of the iterator type for given statement
    pub struct Name
}

impl<'src, S> Compile<'src, S, Name> for FieldsIter
where
    S: SourceIdent,
{
    fn compile(&self, node: &S) -> Name {
        let ident = format_ident!("{}{}", &node.source_ident(), &self.iter_name);
        Name(quote! { #ident })
    }
}
