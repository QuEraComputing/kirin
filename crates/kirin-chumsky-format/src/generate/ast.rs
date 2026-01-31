//! Code generation for the `WithAbstractSyntaxTree` derive macro.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

/// Generator for the `WithAbstractSyntaxTree` trait implementation.
pub struct GenerateWithAbstractSyntaxTree {
    crate_path: syn::Path,
}

impl GenerateWithAbstractSyntaxTree {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>) -> Self {
        let crate_path = ir_input
            .extra_attrs
            .crate_path
            .clone()
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| syn::parse_quote!(::kirin_chumsky));
        Self { crate_path }
    }

    /// Generates the AST type and `WithAbstractSyntaxTree` implementation.
    pub fn generate(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let ast_generics = self.build_ast_generics(ir_input);

        let ast_definition = self.generate_ast_definition(ir_input, &ast_name, &ast_generics);
        let trait_impls = self.generate_derive_impls(ir_input, &ast_name, &ast_generics);
        let trait_impl = self.generate_trait_impl(ir_input, &ast_name, &ast_generics);

        quote! {
            #ast_definition
            #trait_impls
            #trait_impl
        }
    }

    fn build_ast_generics(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        let mut generics = ir_input.generics.clone();

        // Add 'tokens lifetime if not present
        let tokens_lt = syn::Lifetime::new("'tokens", proc_macro2::Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
        {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt.clone())),
            );
        }

        // Add 'src lifetime with bound 'tokens if not present
        let src_lt = syn::Lifetime::new("'src", proc_macro2::Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "src"))
        {
            let mut src_param = syn::LifetimeParam::new(src_lt.clone());
            src_param.bounds.push(tokens_lt.clone());
            generics
                .params
                .insert(1, syn::GenericParam::Lifetime(src_param));
        }

        // Add Language type parameter if not present
        let lang_ident = syn::Ident::new("Language", proc_macro2::Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let crate_path = &self.crate_path;
            let mut lang_param = syn::TypeParam::from(lang_ident.clone());
            lang_param
                .bounds
                .push(syn::parse_quote!(#crate_path::LanguageParser<'tokens, 'src>));
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }

    fn generate_ast_definition(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let (_, ty_generics, _) = ast_generics.split_for_impl();

        match &ir_input.data {
            kirin_derive_core_2::ir::Data::Struct(data) => {
                let fields = self.generate_struct_fields(&data.0, true);
                let is_tuple = self.is_tuple_style(&data.0);

                if is_tuple {
                    quote! {
                        pub struct #ast_name #ty_generics
                        where
                            Language: #crate_path::LanguageParser<'tokens, 'src>,
                        (
                            #fields
                        );
                    }
                } else {
                    quote! {
                        pub struct #ast_name #ty_generics
                        where
                            Language: #crate_path::LanguageParser<'tokens, 'src>,
                        {
                            #fields
                        }
                    }
                }
            }
            kirin_derive_core_2::ir::Data::Enum(data) => {
                let variants = self.generate_enum_variants(data);
                quote! {
                    pub enum #ast_name #ty_generics
                    where
                        Language: #crate_path::LanguageParser<'tokens, 'src>,
                    {
                        #variants
                    }
                }
            }
        }
    }

    fn generate_derive_impls(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();

        // Generate Debug impl
        let debug_impl = self.generate_debug_impl(ir_input, ast_name, ast_generics);

        // Generate Clone impl
        let clone_impl = self.generate_clone_impl(ir_input, ast_name, ast_generics);

        // Generate PartialEq impl
        let partialeq_impl = self.generate_partialeq_impl(ir_input, ast_name, ast_generics);

        quote! {
            impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src>,
            {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    #debug_impl
                }
            }

            impl #impl_generics ::core::clone::Clone for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src>,
            {
                fn clone(&self) -> Self {
                    #clone_impl
                }
            }

            impl #impl_generics ::core::cmp::PartialEq for #ast_name #ty_generics
            where
                Language: #crate_path::LanguageParser<'tokens, 'src>,
            {
                fn eq(&self, other: &Self) -> bool {
                    #partialeq_impl
                }
            }
        }
    }

    fn generate_debug_impl(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core_2::ir::Data::Struct(data) => {
                let name = ast_name.to_string();
                let is_tuple = self.is_tuple_style(&data.0);

                if is_tuple {
                    let field_count = self.count_fields(&data.0);
                    let field_names: Vec<_> = (0..field_count)
                        .map(|i| {
                            syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site())
                        })
                        .collect();
                    let patterns = quote! { Self(#(#field_names),*) };
                    let debug_fields = field_names.iter().fold(
                        quote! { f.debug_tuple(#name) },
                        |acc, field| quote! { #acc.field(&#field) },
                    );
                    quote! {
                        let #patterns = self;
                        #debug_fields.finish()
                    }
                } else {
                    let fields = self.field_list(&data.0);
                    let patterns = quote! { Self { #(#fields),* } };
                    let debug_fields =
                        fields
                            .iter()
                            .fold(quote! { f.debug_struct(#name) }, |acc, field| {
                                let field_name = field.to_string();
                                quote! { #acc.field(#field_name, &#field) }
                            });
                    quote! {
                        let #patterns = self;
                        #debug_fields.finish()
                    }
                }
            }
            kirin_derive_core_2::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let name_str = name.to_string();
                        let is_tuple = self.is_tuple_style(variant);

                        if is_tuple {
                            let field_count = self.count_fields(variant);
                            if field_count == 0 {
                                quote! {
                                    Self::#name => f.write_str(#name_str)
                                }
                            } else {
                                let field_names: Vec<_> = (0..field_count)
                                    .map(|i| {
                                        syn::Ident::new(
                                            &format!("f{}", i),
                                            proc_macro2::Span::call_site(),
                                        )
                                    })
                                    .collect();
                                let debug_fields = field_names.iter().fold(
                                    quote! { f.debug_tuple(#name_str) },
                                    |acc, field| quote! { #acc.field(&#field) },
                                );
                                quote! {
                                    Self::#name(#(#field_names),*) => #debug_fields.finish()
                                }
                            }
                        } else {
                            let fields = self.field_list(variant);
                            if fields.is_empty() {
                                quote! {
                                    Self::#name {} => f.write_str(#name_str)
                                }
                            } else {
                                let debug_fields = fields.iter().fold(
                                    quote! { f.debug_struct(#name_str) },
                                    |acc, field| {
                                        let field_name = field.to_string();
                                        quote! { #acc.field(#field_name, &#field) }
                                    },
                                );
                                quote! {
                                    Self::#name { #(#fields),* } => #debug_fields.finish()
                                }
                            }
                        }
                    })
                    .collect();

                quote! {
                    match self {
                        #(#arms),*
                    }
                }
            }
        }
    }

    fn generate_clone_impl(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core_2::ir::Data::Struct(data) => {
                let is_tuple = self.is_tuple_style(&data.0);

                if is_tuple {
                    let field_count = self.count_fields(&data.0);
                    let field_names: Vec<_> = (0..field_count)
                        .map(|i| {
                            syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site())
                        })
                        .collect();
                    let patterns = quote! { Self(#(#field_names),*) };
                    let clones = quote! { Self(#(#field_names.clone()),*) };
                    quote! {
                        let #patterns = self;
                        #clones
                    }
                } else {
                    let fields = self.field_list(&data.0);
                    let patterns = quote! { Self { #(#fields),* } };
                    let clones: Vec<_> = fields.iter().map(|f| quote! { #f: #f.clone() }).collect();
                    quote! {
                        let #patterns = self;
                        Self { #(#clones),* }
                    }
                }
            }
            kirin_derive_core_2::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let is_tuple = self.is_tuple_style(variant);

                        if is_tuple {
                            let field_count = self.count_fields(variant);
                            if field_count == 0 {
                                quote! { Self::#name => Self::#name }
                            } else {
                                let field_names: Vec<_> = (0..field_count)
                                    .map(|i| {
                                        syn::Ident::new(
                                            &format!("f{}", i),
                                            proc_macro2::Span::call_site(),
                                        )
                                    })
                                    .collect();
                                let clones = quote! { Self::#name(#(#field_names.clone()),*) };
                                quote! {
                                    Self::#name(#(#field_names),*) => #clones
                                }
                            }
                        } else {
                            let fields = self.field_list(variant);
                            if fields.is_empty() {
                                quote! { Self::#name {} => Self::#name {} }
                            } else {
                                let clones: Vec<_> =
                                    fields.iter().map(|f| quote! { #f: #f.clone() }).collect();
                                quote! {
                                    Self::#name { #(#fields),* } => Self::#name { #(#clones),* }
                                }
                            }
                        }
                    })
                    .collect();

                quote! {
                    match self {
                        #(#arms),*
                    }
                }
            }
        }
    }

    fn generate_partialeq_impl(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        _ast_generics: &syn::Generics,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core_2::ir::Data::Struct(data) => {
                let is_tuple = self.is_tuple_style(&data.0);

                if is_tuple {
                    let field_count = self.count_fields(&data.0);
                    let self_fields: Vec<_> = (0..field_count)
                        .map(|i| {
                            syn::Ident::new(&format!("s{}", i), proc_macro2::Span::call_site())
                        })
                        .collect();
                    let other_fields: Vec<_> = (0..field_count)
                        .map(|i| {
                            syn::Ident::new(&format!("o{}", i), proc_macro2::Span::call_site())
                        })
                        .collect();
                    let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                        quote! { #s == #o }
                    });
                    quote! {
                        let Self(#(#self_fields),*) = self;
                        let Self(#(#other_fields),*) = other;
                        true #(&& #comparisons)*
                    }
                } else {
                    let fields = self.field_list(&data.0);
                    let self_fields: Vec<_> = fields
                        .iter()
                        .map(|f| {
                            syn::Ident::new(&format!("s_{}", f), proc_macro2::Span::call_site())
                        })
                        .collect();
                    let other_fields: Vec<_> = fields
                        .iter()
                        .map(|f| {
                            syn::Ident::new(&format!("o_{}", f), proc_macro2::Span::call_site())
                        })
                        .collect();
                    let self_pattern: Vec<_> = fields
                        .iter()
                        .zip(&self_fields)
                        .map(|(f, s)| quote! { #f: #s })
                        .collect();
                    let other_pattern: Vec<_> = fields
                        .iter()
                        .zip(&other_fields)
                        .map(|(f, o)| quote! { #f: #o })
                        .collect();
                    let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                        quote! { #s == #o }
                    });
                    quote! {
                        let Self { #(#self_pattern),* } = self;
                        let Self { #(#other_pattern),* } = other;
                        true #(&& #comparisons)*
                    }
                }
            }
            kirin_derive_core_2::ir::Data::Enum(data) => {
                let arms: Vec<_> = data
                    .variants
                    .iter()
                    .map(|variant| {
                        let name = &variant.name;
                        let is_tuple = self.is_tuple_style(variant);

                        if is_tuple {
                            let field_count = self.count_fields(variant);
                            if field_count == 0 {
                                quote! {
                                    (Self::#name, Self::#name) => true
                                }
                            } else {
                                let self_fields: Vec<_> = (0..field_count)
                                    .map(|i| syn::Ident::new(&format!("s{}", i), proc_macro2::Span::call_site()))
                                    .collect();
                                let other_fields: Vec<_> = (0..field_count)
                                    .map(|i| syn::Ident::new(&format!("o{}", i), proc_macro2::Span::call_site()))
                                    .collect();
                                let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                                    quote! { #s == #o }
                                });
                                quote! {
                                    (Self::#name(#(#self_fields),*), Self::#name(#(#other_fields),*)) => {
                                        true #(&& #comparisons)*
                                    }
                                }
                            }
                        } else {
                            let fields = self.field_list(variant);
                            if fields.is_empty() {
                                quote! {
                                    (Self::#name {}, Self::#name {}) => true
                                }
                            } else {
                                let self_fields: Vec<_> = fields
                                    .iter()
                                    .map(|f| syn::Ident::new(&format!("s_{}", f), proc_macro2::Span::call_site()))
                                    .collect();
                                let other_fields: Vec<_> = fields
                                    .iter()
                                    .map(|f| syn::Ident::new(&format!("o_{}", f), proc_macro2::Span::call_site()))
                                    .collect();
                                let self_pattern: Vec<_> = fields.iter().zip(&self_fields)
                                    .map(|(f, s)| quote! { #f: #s })
                                    .collect();
                                let other_pattern: Vec<_> = fields.iter().zip(&other_fields)
                                    .map(|(f, o)| quote! { #f: #o })
                                    .collect();
                                let comparisons = self_fields.iter().zip(&other_fields).map(|(s, o)| {
                                    quote! { #s == #o }
                                });
                                quote! {
                                    (Self::#name { #(#self_pattern),* }, Self::#name { #(#other_pattern),* }) => {
                                        true #(&& #comparisons)*
                                    }
                                }
                            }
                        }
                    })
                    .collect();

                quote! {
                    match (self, other) {
                        #(#arms,)*
                        _ => false
                    }
                }
            }
        }
    }

    fn field_list(
        &self,
        stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>,
    ) -> Vec<syn::Ident> {
        let mut fields = Vec::new();
        for arg in stmt.arguments.iter() {
            if let Some(ident) = &arg.field.ident {
                fields.push(ident.clone());
            }
        }
        for res in stmt.results.iter() {
            if let Some(ident) = &res.field.ident {
                fields.push(ident.clone());
            }
        }
        for block in stmt.blocks.iter() {
            if let Some(ident) = &block.field.ident {
                fields.push(ident.clone());
            }
        }
        for succ in stmt.successors.iter() {
            if let Some(ident) = &succ.field.ident {
                fields.push(ident.clone());
            }
        }
        for region in stmt.regions.iter() {
            if let Some(ident) = &region.field.ident {
                fields.push(ident.clone());
            }
        }
        for value in stmt.values.iter() {
            if let Some(ident) = &value.field.ident {
                fields.push(ident.clone());
            }
        }
        fields
    }

    fn count_fields(&self, stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>) -> usize {
        stmt.arguments.iter().count()
            + stmt.results.iter().count()
            + stmt.blocks.iter().count()
            + stmt.successors.iter().count()
            + stmt.regions.iter().count()
            + stmt.values.iter().count()
    }

    fn is_tuple_style(&self, stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>) -> bool {
        stmt.arguments.iter().all(|a| a.field.ident.is_none())
            && stmt.results.iter().all(|r| r.field.ident.is_none())
            && stmt.blocks.iter().all(|b| b.field.ident.is_none())
            && stmt.successors.iter().all(|s| s.field.ident.is_none())
            && stmt.regions.iter().all(|r| r.field.ident.is_none())
            && stmt.values.iter().all(|v| v.field.ident.is_none())
    }

    fn generate_struct_fields(
        &self,
        stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>,
        with_pub: bool,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let mut fields = Vec::new();

        // Generate fields for arguments
        for arg in stmt.arguments.iter() {
            let ty = self.field_ast_type(&arg.collection, FieldKind::SSAValue);
            if let Some(ident) = &arg.field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        // Generate fields for results
        for res in stmt.results.iter() {
            let ty = self.field_ast_type(&res.collection, FieldKind::ResultValue);
            if let Some(ident) = &res.field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        // Generate fields for blocks
        for block in stmt.blocks.iter() {
            let ty = self.field_ast_type(&block.collection, FieldKind::Block);
            if let Some(ident) = &block.field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        // Generate fields for successors
        for succ in stmt.successors.iter() {
            let ty = self.field_ast_type(&succ.collection, FieldKind::Successor);
            if let Some(ident) = &succ.field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        // Generate fields for regions
        for region in stmt.regions.iter() {
            let ty = self.field_ast_type(&region.collection, FieldKind::Region);
            if let Some(ident) = &region.field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        // Generate fields for compile-time values
        for value in stmt.values.iter() {
            let original_ty = &value.ty;
            let ty = quote! { <#original_ty as #crate_path::HasParser<'tokens, 'src>>::Output };
            if let Some(ident) = &value.field.ident {
                if with_pub {
                    fields.push(quote! { pub #ident: #ty });
                } else {
                    fields.push(quote! { #ident: #ty });
                }
            } else if with_pub {
                fields.push(quote! { pub #ty });
            } else {
                fields.push(quote! { #ty });
            }
        }

        let is_tuple = self.is_tuple_style(stmt);

        if is_tuple {
            quote! { #(#fields),* }
        } else {
            quote! { #(#fields,)* }
        }
    }

    fn generate_enum_variants(
        &self,
        data: &kirin_derive_core_2::ir::DataEnum<ChumskyLayout>,
    ) -> TokenStream {
        let variants: Vec<TokenStream> = data
            .variants
            .iter()
            .map(|variant| {
                let name = &variant.name;

                // Check if this is a wrapper variant
                if let Some(wrapper) = &variant.wraps {
                    let wrapped_ty = &wrapper.ty;
                    let crate_path = &self.crate_path;
                    return quote! {
                        #name(<#wrapped_ty as #crate_path::WithAbstractSyntaxTree<'tokens, 'src, Language>>::AbstractSyntaxTreeNode)
                    };
                }

                // For enum variants, don't use `pub`
                let fields = self.generate_struct_fields(variant, false);
                let is_tuple = self.is_tuple_style(variant);

                if is_tuple {
                    quote! { #name(#fields) }
                } else {
                    quote! { #name { #fields } }
                }
            })
            .collect();

        quote! { #(#variants,)* }
    }

    fn field_ast_type(
        &self,
        collection: &kirin_derive_core_2::ir::fields::Collection,
        kind: FieldKind,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let base = match kind {
            FieldKind::SSAValue => {
                quote! { #crate_path::SSAValue<'tokens, 'src, Language> }
            }
            FieldKind::ResultValue => {
                quote! { #crate_path::ResultValue<'tokens, 'src, Language> }
            }
            FieldKind::Block => {
                // Block parser returns Spanned<Block>, so we need Spanned wrapper
                quote! { #crate_path::Spanned<#crate_path::Block<'tokens, 'src, Language>> }
            }
            FieldKind::Successor => {
                quote! { #crate_path::BlockLabel<'src> }
            }
            FieldKind::Region => {
                quote! { #crate_path::Region<'tokens, 'src, Language> }
            }
        };

        match collection {
            kirin_derive_core_2::ir::fields::Collection::Single => base,
            kirin_derive_core_2::ir::fields::Collection::Vec => quote! { Vec<#base> },
            kirin_derive_core_2::ir::fields::Collection::Option => quote! { Option<#base> },
        }
    }

    fn generate_trait_impl(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
    ) -> TokenStream {
        let crate_path = &self.crate_path;
        let name = &ir_input.name;
        let (impl_generics, ty_generics, where_clause) = ast_generics.split_for_impl();

        quote! {
            impl #impl_generics #crate_path::WithAbstractSyntaxTree<'tokens, 'src, Language> for #name
            #where_clause
            {
                type AbstractSyntaxTreeNode = #ast_name #ty_generics;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FieldKind {
    SSAValue,
    ResultValue,
    Block,
    Successor,
    Region,
}
