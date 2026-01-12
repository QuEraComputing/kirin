use proc_macro2::TokenStream;
use quote::quote;

#[derive(Debug)]
pub struct SyntaxField {
    pub crate_path: syn::Path,
    pub ty: TokenStream,
    pub kind: SyntaxFieldKind,
}

#[derive(Debug)]
pub enum SyntaxFieldKind {
    SSAValue,
    ResultValue,
    NameofSSAValue,
    TypeofSSAValue,
    TypeofResultValue,
    Block,
    Successor,
    Region,
    AsIs,
}

impl SyntaxField {
    pub fn render_base_type(&self) -> TokenStream {
        let src_ty = &self.ty;
        let crate_path = &self.crate_path;
        match &self.kind {
            SyntaxFieldKind::SSAValue => quote! { #crate_path::SSAValue<'tokens, 'src, Language> },
            SyntaxFieldKind::ResultValue => quote! { #crate_path::ResultValue<'src> },
            SyntaxFieldKind::NameofSSAValue => quote! { #crate_path::NameofSSAValue<'tokens, 'src, Language> },
            SyntaxFieldKind::TypeofSSAValue => quote! { #crate_path::TypeofSSAValue<'tokens, 'src, Language> },
            SyntaxFieldKind::TypeofResultValue => quote! { #crate_path::TypeofSSAValue<'tokens, 'src, Language> },
            SyntaxFieldKind::Successor => quote! { #crate_path::Successor<'tokens, 'src, Language> },
            SyntaxFieldKind::Block => quote! { #crate_path::Block<'tokens, 'src, Language> },
            SyntaxFieldKind::Region => quote! { #crate_path::Region<'tokens, 'src, Language> },
            SyntaxFieldKind::AsIs => quote! { #src_ty },
        }
    }
}
