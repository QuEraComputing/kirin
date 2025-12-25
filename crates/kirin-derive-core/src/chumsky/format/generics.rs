use crate::prelude::*;

pub struct GenericsImpl(syn::Generics);

impl<'src, T, L> Compile<'src, L, GenericsImpl> for T
where
    T: WithGenerics + WithUserCratePath,
    L: DeriveWithCratePath,
{
    fn compile(&self, ctx: &L) -> GenericsImpl {
        let crate_path: CratePath = self.compile(ctx);
        let mut generics = self.generics().clone();
        for param in generics.type_params_mut() {
            param.bounds.push(
                syn::parse_quote!(#crate_path::WithAbstractSyntaxTree<'tokens, 'src, _AnotherLanguage>),
            );
            param.bounds.push(syn::parse_quote!('tokens));

            let name = param.ident.clone();
            param.bounds.push(
                syn::parse_quote!(
                    #crate_path::HasParser<
                        'tokens, 'src, _AnotherLanguage,
                        Output =
                            <#name as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, _AnotherLanguage>>::AbstractSyntaxTreeNode>
                ),
            )
        }
        generics.params.push(syn::parse_quote!('tokens));
        generics.params.push(syn::parse_quote!('src: 'tokens));
        generics.params.push(syn::parse_quote! {
            _AnotherLanguage: Dialect<TypeLattice: HasParser<'tokens, 'src, _AnotherLanguage>>
                + HasParser<'tokens, 'src, _AnotherLanguage>
                + 'tokens
        });
        GenericsImpl(generics)
    }
}

impl ToTokens for GenericsImpl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl std::ops::Deref for GenericsImpl {
    type Target = syn::Generics;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
