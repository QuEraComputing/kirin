use bon::Builder;
use quote::{format_ident, quote};
use syn::TraitBound;

use crate::prelude::*;

#[derive(Clone, Builder)]
pub struct DeriveAST {
    #[builder(default = syn::parse_quote!(kirin::parsers))]
    pub default_crate_path: syn::Path,
    #[builder(default = syn::parse_quote!(WithAbstractSyntaxTree))]
    pub trait_path: syn::Path,
}

impl Layout for DeriveAST {
    type EnumAttr = ();
    type StructAttr = ();
    type VariantAttr = ();
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

impl<'src> Compile<'src, Struct<'src, DeriveAST>, StructImpl> for DeriveAST {
    fn compile(&self, node: &Struct<'src, DeriveAST>) -> StructImpl {
        let name: syn::Ident = format_ident!("{}SyntaxTree", node.source_ident());
        let body: ASTNodeFields = self.compile(&node.fields());
        let generics: GenericsImpl = self.compile(node);
        quote! {
            #[automatically_derived]
            pub struct #name #generics #body
        }
        .into()
    }
}

target! {
    pub struct EnumImpl;
}

impl<'src> Compile<'src, Enum<'src, DeriveAST>, EnumImpl> for DeriveAST {
    fn compile(&self, node: &Enum<'src, DeriveAST>) -> EnumImpl {
        let name: syn::Ident = format_ident!("{}SyntaxTree", node.source_ident());
        let generics: GenericsImpl = self.compile(node);
        let variant_impls: Vec<ASTNodeFields> =
            node.variants().map(|v| self.compile(&v.fields())).collect();
        quote! {
            #[automatically_derived]
            pub enum #name #generics {
                #( #variant_impls ),*
            }
        }
        .into()
    }
}

target! {
    pub struct ASTNodeFields;
}

impl<'src> Compile<'src, Fields<'_, 'src, DeriveAST>, ASTNodeFields> for DeriveAST {
    fn compile(&self, node: &Fields<'_, 'src, DeriveAST>) -> ASTNodeFields {
        let crate_path: CratePath = self.compile(node);

        if let Some(f) = node.wrapper() {
            let ty = &f.source().ty;
            return quote! {
                {
                    #f: <#ty as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, _AnotherLanguage>>::AbstractSyntaxTreeNode
                }
            }
            .into();
        }

        let field_impls = node.iter().map(|f| {
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

target! {
    pub struct GenericsImpl;
}

impl<'src, T> Compile<'src, T, GenericsImpl> for DeriveAST
where
    T: WithGenerics + WithUserCratePath,
{
    fn compile(&self, node: &T) -> GenericsImpl {
        let crate_path: CratePath = self.compile(node);
        let mut generics = node.generics().clone();
        for param in generics.type_params_mut() {
            param.bounds.push(
                syn::parse_quote!(#crate_path::WithAbstractSyntaxTree<'tokens, 'src, _AnotherLanguage>),
            );
        }
        generics.params.push(syn::parse_quote!('tokens));
        generics.params.push(syn::parse_quote!('src: 'tokens));
        generics.params.push(syn::parse_quote!(_AnotherLanguage: Dialect<TypeLattice: HasParser<'tokens, 'src, _AnotherLanguage>> + HasParser<'tokens, 'src, _AnotherLanguage>));
        generics.to_token_stream().into()
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
}
