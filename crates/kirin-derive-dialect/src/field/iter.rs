use kirin_derive_core_2::prelude::*;

pub struct DeriveFieldIter;

impl<'ir> Emit<'ir, StandardLayout> for DeriveFieldIter {
    fn emit_statement(
        &mut self,
        statement: &'ir ir::Statement<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
    }
}
