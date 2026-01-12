use quote::{format_ident, quote};

use crate::{kirin::field::context::FieldsIter, prelude::*};

target! {
    /// Name of the iterator type for given statement
    pub struct Name
}

impl<'src, S> Compile<'src, FieldsIter, Name> for S
where
    S: SourceIdent,
{
    fn compile(&self, ctx: &FieldsIter) -> Name {
        let ident = format_ident!("{}{}", &self.source_ident(), &ctx.iter_name);
        Name(quote! { #ident })
    }
}
