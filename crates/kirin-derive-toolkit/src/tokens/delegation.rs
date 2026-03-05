use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Delegated trait method call: `<WrapperTy as Trait>::method(field)`.
///
/// Renders a fully-qualified method call that delegates to the inner
/// wrapper type's trait implementation.
///
/// ```ignore
/// let call = DelegationCall {
///     wrapper_ty: quote!(InnerOp),
///     trait_path: quote!(IsPure),
///     trait_method: format_ident!("is_pure"),
///     field: quote!(&self.0),
/// };
/// // Produces: `<InnerOp as IsPure>::is_pure(&self.0)`
/// ```
pub struct DelegationCall {
    /// The wrapper/inner type to delegate to.
    pub wrapper_ty: TokenStream,
    /// The trait containing the method.
    pub trait_path: TokenStream,
    /// The method name to call.
    pub trait_method: syn::Ident,
    /// The expression passed as the argument (typically a field access).
    pub field: TokenStream,
}

impl ToTokens for DelegationCall {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper_ty = &self.wrapper_ty;
        let trait_path = &self.trait_path;
        let trait_method = &self.trait_method;
        let field = &self.field;
        tokens.extend(quote! { <#wrapper_ty as #trait_path>::#trait_method(#field) });
    }
}

/// Delegated associated type reference: `<WrapperTy as Trait<G>>::AssocType`.
///
/// Renders a fully-qualified path to an associated type on the wrapper
/// type's trait implementation.
pub struct DelegationAssocType {
    /// The wrapper/inner type to delegate to.
    pub wrapper_ty: TokenStream,
    /// The trait containing the associated type.
    pub trait_path: TokenStream,
    /// Generic arguments on the trait (e.g., `<'ir, L>`).
    pub trait_generics: TokenStream,
    /// The associated type name to reference.
    pub assoc_type_ident: syn::Ident,
}

impl ToTokens for DelegationAssocType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let wrapper_ty = &self.wrapper_ty;
        let trait_path = &self.trait_path;
        let trait_generics = &self.trait_generics;
        let assoc_type_ident = &self.assoc_type_ident;
        tokens.extend(quote! { <#wrapper_ty as #trait_path #trait_generics>::#assoc_type_ident });
    }
}
