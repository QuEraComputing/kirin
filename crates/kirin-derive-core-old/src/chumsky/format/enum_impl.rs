use quote::quote;

use crate::{chumsky::ast::ASTNodeName, prelude::*};

use super::context::DeriveHasParser;
use super::generics::GenericsImpl;
use super::step::ParseElements;

target! {
    pub struct EnumImpl
}

impl<'src> Compile<'src, DeriveHasParser, EnumImpl> for Enum<'src, DeriveHasParser> {
    fn compile(&self, ctx: &DeriveHasParser) -> EnumImpl {
        let trait_path: TraitPath = self.compile(ctx);
        let crate_path: CratePath = self.compile(ctx);
        let generics: GenericsImpl = self.compile(ctx);

        let name = self.source_ident();
        let ast_name: ASTNodeName = self.compile(ctx);
        let (_, src_ty_generics, _) = self.source().generics.split_for_impl();
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let body = self
            .variants()
            .map(|variant| {
                let variant_name = variant.source_ident();
                if let Some(wrapper) = variant.wrapper() {
                    quote! {
                        <#wrapper as #trait_path<'tokens, 'src, _AnotherLanguage>>::parser()
                            .map(|inner| #name::#variant_name { #wrapper: inner })
                    }
                } else {
                    let elements: ParseElements = variant.fields().compile(ctx);
                    quote! {
                        #elements
                    }
                }
            })
            .fold(quote! {}, |acc, item| {
                if acc.is_empty() {
                    item
                } else {
                    quote! {#acc, #item}
                }
            });

        quote! {
            impl #impl_generics #trait_path<'tokens, 'src, _AnotherLanguage> for #name #src_ty_generics #where_clause {
                type Output = #ast_name #ty_generics;
                fn parser<I: #crate_path::TokenInput<'tokens, 'src>>() -> #crate_path::Boxed<'tokens, 'tokens, I, Self::Output, #crate_path::ParserError<'tokens, 'src>> {
                    #crate_path::choice((
                        #body
                    )).boxed()
                }
            }
        }.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::debug::rustfmt;

    use super::*;

    #[test]
    fn test_enum_impl() {
        let input: syn::DeriveInput = syn::parse_quote! {
            pub enum SimpleLanguage {
                #[chumsky(format = "add {field_0} and {field_1}")]
                Add(
                    SSAValue,
                    SSAValue,
                    ResultValue,
                ),
                #[chumsky(format = "constant {field_0}")]
                Constant(
                    Value,
                    ResultValue,
                ),
                #[chumsky(format = "return {field_0}")]
                Return(SSAValue),
                #[chumsky(format = "call {field_0}")]
                Function(
                    Region,
                    ResultValue,
                ),
            }
        };

        let ctx = DeriveHasParser::builder().build();
        let Input::Enum(node) = ctx.scan(&input).unwrap() else {
            panic!("expected enum");
        };
        let token: EnumImpl = node.compile(&ctx);
        insta::assert_snapshot!(rustfmt(token.to_token_stream()));
    }
}
