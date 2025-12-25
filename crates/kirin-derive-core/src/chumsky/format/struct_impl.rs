use quote::quote;

use crate::{
    chumsky::ast::{ASTNodeName, GenericsImpl},
    prelude::*,
};

use super::context::DeriveHasParser;
use super::step::ParseElements;

target! {
    pub struct StructImpl
}

impl<'src> Compile<'src, DeriveHasParser, StructImpl> for Struct<'src, DeriveHasParser> {
    fn compile(&self, ctx: &DeriveHasParser) -> StructImpl {
        let trait_path: TraitPath = self.compile(ctx);
        let crate_path: CratePath = self.compile(ctx);
        let generics: GenericsImpl = self.compile(ctx);
        let name: ASTNodeName = self.compile(ctx);
        let (_, src_ty_generics, _) = self.source().generics.split_for_impl();
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let body = if let Some(wrapper) = self.wrapper() {
            let ty_wrapper = &wrapper.source().ty;
            quote! {
                <#ty_wrapper as #trait_path>::parser()
                    .map(|inner| #name { #wrapper: inner })
                    .boxed()
            }
        } else {
            let elements: ParseElements = self.fields().compile(ctx);
            quote! {
                #elements.boxed()
            }
        };

        quote! {
            impl #impl_generics #trait_path for #name #src_ty_generics #where_clause {
                type Output = #name #ty_generics;
                fn parser<I: #crate_path::TokenInput<'tokens, 'src>>() -> #crate_path::Boxed<'tokens, 'tokens, I, Self::Output, #crate_path::ParserError<'tokens, 'src>> {
                    #body
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
    fn test_struct_impl_compile() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[chumsky(format = "my_statement {condition} then={then_block} else={else_block}")]
            struct MyStatement {
                condition: SSAValue,
                then_block: Block,
                else_block: Block,
            }
        };

        let derive_parser = DeriveHasParser::builder().build();
        let Input::Struct(data) = derive_parser.scan(&input).unwrap() else {
            panic!("expected struct");
        };
        let output: StructImpl = data.compile(&derive_parser);
        insta::assert_snapshot!(rustfmt(output.to_token_stream()));
    }
}
