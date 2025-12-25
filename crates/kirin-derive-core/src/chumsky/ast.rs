use bon::Builder;
use quote::{format_ident, quote};

use crate::prelude::*;

use super::attrs::{ChumskyEnumOptions, ChumskyStructOptions, ChumskyVariantOptions};

#[derive(Clone, Builder)]
pub struct DeriveAST {
    #[builder(default = syn::parse_quote!(kirin::parsers))]
    pub default_crate_path: syn::Path,
    #[builder(default = syn::parse_quote!(WithAbstractSyntaxTree))]
    pub trait_path: syn::Path,
}

impl Layout for DeriveAST {
    type EnumAttr = ChumskyEnumOptions;
    type StructAttr = ChumskyStructOptions;
    type VariantAttr = ChumskyVariantOptions;
    type FieldAttr = ();
    type FieldExtra = ();
    type StatementExtra = ();
}

impl DeriveWithCratePath for DeriveAST {
    fn default_crate_path(&self) -> &syn::Path {
        &self.default_crate_path
    }
}

impl DeriveTrait for DeriveAST {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl Emit<'_> for DeriveAST {
    type EnumImpl = EnumImpl;
    type StructImpl = StructImpl;
}

target! {
    pub struct StructImpl;
}

impl<'src> Compile<'src, DeriveAST, StructImpl> for Struct<'src, DeriveAST> {
    fn compile(&self, ctx: &DeriveAST) -> StructImpl {
        let trait_path: TraitPath = self.compile(ctx);

        let name: ASTNodeName = self.compile(ctx);
        let body: ASTNodeFields = self.fields().compile(ctx);
        let generics: GenericsImpl = self.compile(ctx);

        let src_name = self.source_ident();
        let (_, src_ty_generics, _) = self.source().generics.split_for_impl();
        let (impl_generics, ty_generics, where_clause) = generics.0.split_for_impl();
        quote! {
            #[automatically_derived]
            pub struct #name #generics #body

            #[automatically_derived]
            impl #impl_generics #trait_path<'tokens, 'src, _AnotherLanguage> for #src_name #src_ty_generics #where_clause {
                type AbstractSyntaxTreeNode = #name #ty_generics;
            }
        }
        .into()
    }
}

target! {
    pub struct EnumImpl;
}

impl<'src> Compile<'src, DeriveAST, EnumImpl> for Enum<'src, DeriveAST> {
    fn compile(&self, ctx: &DeriveAST) -> EnumImpl {
        let trait_path: TraitPath = self.compile(ctx);
        let name: ASTNodeName = self.compile(ctx);
        let generics: GenericsImpl = self.compile(ctx);

        let src_name = self.source_ident();
        let (_, src_ty_generics, _) = self.source().generics.split_for_impl();
        let (impl_generics, ty_generics, where_clause) = generics.0.split_for_impl();

        let variant_names = self.variant_names();
        let variant_impls: Vec<ASTNodeFields> =
            self.variants().map(|v| v.fields().compile(ctx)).collect();
        quote! {
            #[automatically_derived]
            pub enum #name #generics {
                #( #variant_names #variant_impls ),*
            }

            #[automatically_derived]
            impl #impl_generics #trait_path<'tokens, 'src, _AnotherLanguage> for #src_name #src_ty_generics #where_clause {
                type AbstractSyntaxTreeNode = #name #ty_generics;
            }
        }
        .into()
    }
}

target! {
    pub struct ASTNodeFields;
}

impl<'src> Compile<'src, DeriveAST, ASTNodeFields> for Fields<'_, 'src, DeriveAST> {
    fn compile(&self, ctx: &DeriveAST) -> ASTNodeFields {
        let crate_path: CratePath = self.compile(ctx);

        if let Some(f) = self.wrapper() {
            let ty = &f.source().ty;
            return quote! {
                {
                    #f: <#ty as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, _AnotherLanguage>>::AbstractSyntaxTreeNode
                }
            }
            .into();
        }

        let field_impls = self.iter().map(|f| {
            let ty = &f.source().ty;
            quote! {
                #f: <#ty as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, _AnotherLanguage>>::AbstractSyntaxTreeNode
            }
        });
        quote! {{
            #( #field_impls ),*
        }}
        .into()
    }
}

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
        }
        generics.params.push(syn::parse_quote!('tokens));
        generics.params.push(syn::parse_quote!('src: 'tokens));
        generics.params.push(syn::parse_quote!(_AnotherLanguage: Dialect<TypeLattice: HasParser<'tokens, 'src, _AnotherLanguage>> + HasParser<'tokens, 'src, _AnotherLanguage>));
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

target! {
    pub struct ASTNodeName;
}

impl<'src, T, L> Compile<'src, L, ASTNodeName> for T
where
    T: SourceIdent,
    L: DeriveTrait,
{
    fn compile(&self, _ctx: &L) -> ASTNodeName {
        let ident = self.source_ident();
        let name = format_ident!("{}SyntaxTree", ident);
        quote! { #name }.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derivate_ast_struct_impl() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = MyLattice)]
            struct MyStruct<T> {
                a: SSAValue,
                b: Vec<SSAValue>,
                c: Option<SSAValue>,
                d: ResultValue,
                e: Vec<ResultValue>,
                f: Option<ResultValue>,
                g: Block,
                h: Vec<Block>,
                i: Option<Block>,
                j: Region,
                k: Vec<Region>,
                l: Option<Region>,
                m: Successor,
                n: Vec<Successor>,
                o: Option<Successor>,
                p: u32,
                q: MyType,
                l: T,
            }
        };
        insta::assert_snapshot!(DeriveAST::builder().build().print(&input));
    }

    #[test]
    fn test_derive_simple() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[chumsky(format = "my_statement {condition} then={then_block} else={else_block}")]
            struct MyStatement {
                condition: SSAValue,
                then_block: Block,
                else_block: Block,
            }
        };
        insta::assert_snapshot!(DeriveAST::builder().build().print(&input));
    }
}
