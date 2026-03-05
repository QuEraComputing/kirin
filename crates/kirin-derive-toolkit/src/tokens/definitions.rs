use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub struct StructDef {
    pub vis: TokenStream,
    pub name: syn::Ident,
    pub generics: TokenStream,
    pub fields: Vec<StructField>,
}

pub struct StructField {
    pub vis: TokenStream,
    pub name: syn::Ident,
    pub ty: TokenStream,
}

impl ToTokens for StructDef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let vis = &self.vis;
        let name = &self.name;
        let generics = &self.generics;
        let fields = &self.fields;
        tokens.extend(quote! {
            #[automatically_derived]
            #[doc(hidden)]
            #vis struct #name #generics {
                #(#fields),*
            }
        });
    }
}

impl ToTokens for StructField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let vis = &self.vis;
        let name = &self.name;
        let ty = &self.ty;
        tokens.extend(quote! { #vis #name: #ty });
    }
}

pub struct EnumDef {
    pub vis: TokenStream,
    pub name: syn::Ident,
    pub generics: TokenStream,
    pub variants: Vec<EnumVariant>,
}

pub struct EnumVariant {
    pub name: syn::Ident,
    pub fields: Vec<TokenStream>,
}

impl ToTokens for EnumDef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let vis = &self.vis;
        let name = &self.name;
        let generics = &self.generics;
        let variants = &self.variants;
        tokens.extend(quote! {
            #[automatically_derived]
            #[doc(hidden)]
            #vis enum #name #generics {
                #(#variants),*
            }
        });
    }
}

impl ToTokens for EnumVariant {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let fields = &self.fields;
        if fields.is_empty() {
            tokens.extend(quote! { #name });
        } else {
            tokens.extend(quote! { #name(#(#fields),*) });
        }
    }
}

pub struct ModuleDef {
    pub vis: TokenStream,
    pub name: syn::Ident,
    pub items: Vec<TokenStream>,
}

impl ToTokens for ModuleDef {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let vis = &self.vis;
        let name = &self.name;
        let items = &self.items;
        tokens.extend(quote! {
            #[automatically_derived]
            #[doc(hidden)]
            #vis mod #name {
                #(#items)*
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::rustfmt_tokens;
    use quote::format_ident;

    #[test]
    fn struct_def_output() {
        let def = StructDef {
            vis: quote! { pub },
            name: format_ident!("MyStruct"),
            generics: TokenStream::new(),
            fields: vec![
                StructField {
                    vis: quote! { pub },
                    name: format_ident!("x"),
                    ty: quote! { i32 },
                },
                StructField {
                    vis: quote! { pub },
                    name: format_ident!("y"),
                    ty: quote! { String },
                },
            ],
        };

        let output = rustfmt_tokens(&def.to_token_stream());
        assert!(output.contains("pub struct MyStruct"));
        assert!(output.contains("pub x: i32"));
        assert!(output.contains("pub y: String"));
    }

    #[test]
    fn enum_def_output() {
        let def = EnumDef {
            vis: quote! { pub },
            name: format_ident!("MyEnum"),
            generics: TokenStream::new(),
            variants: vec![
                EnumVariant {
                    name: format_ident!("A"),
                    fields: vec![],
                },
                EnumVariant {
                    name: format_ident!("B"),
                    fields: vec![quote! { i32 }],
                },
            ],
        };

        let output = rustfmt_tokens(&def.to_token_stream());
        assert!(output.contains("pub enum MyEnum"));
        assert!(output.contains("A,"));
        assert!(output.contains("B(i32)"));
    }

    #[test]
    fn module_def_output() {
        let def = ModuleDef {
            vis: quote! { pub },
            name: format_ident!("my_mod"),
            items: vec![quote! { pub fn foo() {} }],
        };

        let output = rustfmt_tokens(&def.to_token_stream());
        assert!(output.contains("pub mod my_mod"));
        assert!(output.contains("pub fn foo()"));
    }
}
