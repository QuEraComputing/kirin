use core::panic;
use quote::{format_ident, quote};
use std::collections::HashMap;

use crate::{DeriveHelperAttribute, WriteTokenStream};

pub struct DeriveAttribute(pub String);

impl DeriveAttribute {
    fn generate_variant_wrapper_arm(
        &self,
        variant: &syn::Ident,
        method_name: &syn::Ident,
        trait_path: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        quote! {
            Self::#variant ( wrapped_instruction ) => {
                <wrapped_instruction as #trait_path>::#method_name(wrapped_instruction)
            },
        }
    }

    fn generate_variant_regular_arm(
        &self,
        variant: &syn::Ident,
        attribute_info: &AttributeInfo,
        f: impl Fn(&AttributeOption) -> bool,
    ) -> proc_macro2::TokenStream {
        let value = attribute_info
            .variants
            .get(variant)
            .map_or(f(&attribute_info.global), f);
        quote! {
            Self::#variant { .. } => #value,
        }
    }

    fn generate_struct_wraps(
        &self,
        ctx: &mut crate::DeriveContext<AttributeInfo>,
        method_name: &syn::Ident,
    ) {
        let name = &ctx.input.ident;
        let trait_path = &ctx.trait_path;
        ctx.write_trait_impl(quote! {
            fn #method_name(&self) -> bool {
                let #name ( wrapped_instruction ) = self;
                <wrapped_instruction as #trait_path>::#method_name(wrapped_instruction)
            }
        });
    }

    fn generate_struct_regular(
        &self,
        ctx: &mut crate::DeriveContext<AttributeInfo>,
        method_name: &syn::Ident,
        value: bool,
    ) {
        ctx.write_trait_impl(quote! {
            fn #method_name(&self) -> bool {
                #value
            }
        });
    }

    fn generate_variant(&self, ctx: &mut crate::DeriveContext<AttributeInfo>, method_name: &str) {
        let syn::Data::Enum(data) = &ctx.input.data else {
            panic!("Variant wraps can only be applied to enums");
        };

        let attribute_info = &ctx.attribute_info;
        let method_ident = format_ident!("{}", method_name);

        let arms = data.variants.iter().map(|variant| {
            if ctx.attribute_info.global_wraps() || ctx.attribute_info.variant_wraps(&variant.ident)
            {
                self.generate_variant_wrapper_arm(&variant.ident, &method_ident, &ctx.trait_path)
            } else {
                self.generate_variant_regular_arm(&variant.ident, attribute_info, |f| {
                    match method_name {
                        "is_terminator" => f.is_terminator,
                        "is_constant" => f.is_constant,
                        "is_pure" => f.is_pure,
                        _ => panic!("Unknown method name"),
                    }
                })
            }
        });

        ctx.write_trait_impl(quote! {
            fn #method_ident(&self) -> bool {
                match self {
                    #(#arms)*
                }
            }
        });
    }

    fn generate_struct(&self, ctx: &mut crate::DeriveContext<AttributeInfo>, method_name: &str) {
        let attribute_info = &ctx.attribute_info;
        let value = match method_name {
            "is_terminator" => attribute_info.global.is_terminator,
            "is_constant" => attribute_info.global.is_constant,
            "is_pure" => attribute_info.global.is_pure,
            _ => panic!("Unknown method name"),
        };
        let method_ident = format_ident!("{}", method_name);

        if ctx.attribute_info.global_wraps() {
            self.generate_struct_wraps(ctx, &method_ident);
        } else {
            self.generate_struct_regular(ctx, &method_ident, value);
        }
    }
}

impl WriteTokenStream for DeriveAttribute {
    type HelperAttribute = AttributeInfo;
    fn write_token(&mut self, ctx: &mut crate::DeriveContext<AttributeInfo>) -> eyre::Result<()> {
        match &ctx.input.data {
            syn::Data::Struct(_) => {
                self.generate_struct(ctx, &self.0);
            }
            syn::Data::Enum(_) => {
                self.generate_variant(ctx, &self.0);
            }
            _ => panic!("Attribute can only be applied to structs or enums"),
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct AttributeInfo {
    global: AttributeOption,
    /// variant-specific attributes, empty if struct
    variants: HashMap<syn::Ident, AttributeOption>,
}

impl DeriveHelperAttribute for AttributeInfo {
    fn scan(input: &syn::DeriveInput) -> eyre::Result<Self>
    where
        Self: Sized,
    {
        let global = AttributeOption::from_attrs(&input.attrs)?;
        let mut variants = HashMap::new();
        if let syn::Data::Enum(data) = &input.data {
            for variant in &data.variants {
                let option = global.join(&AttributeOption::from_attrs(&variant.attrs)?);
                variants.insert(variant.ident.clone(), option);
            }
        }
        Ok(Self { global, variants })
    }

    fn global_wraps(&self) -> bool {
        self.global.wraps
    }

    fn variant_wraps(&self, variant: &syn::Ident) -> bool {
        if let Some(option) = self.variants.get(variant) {
            option.wraps
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct AttributeOption {
    wraps: bool,
    is_terminator: bool,
    is_constant: bool,
    is_pure: bool,
}

impl AttributeOption {
    fn from_attrs(attrs: &[syn::Attribute]) -> eyre::Result<Self> {
        let mut option = Self {
            wraps: false,
            is_terminator: false,
            is_constant: false,
            is_pure: false,
        };

        for attr in attrs {
            if attr.path().is_ident("kirin") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("wraps") {
                        option.wraps = true;
                        Ok(())
                    } else if meta.path.is_ident("is_terminator") {
                        meta.value()?.parse::<syn::LitBool>().map(|lit| {
                            option.is_terminator = lit.value;
                        })
                    } else if meta.path.is_ident("is_constant") {
                        meta.value()?.parse::<syn::LitBool>().map(|lit| {
                            option.is_constant = lit.value;
                        })
                    } else if meta.path.is_ident("is_pure") {
                        meta.value()?.parse::<syn::LitBool>().map(|lit| {
                            option.is_pure = lit.value;
                        })
                    } else {
                        Err(syn::Error::new_spanned(
                            meta.path.clone(),
                            "Unknown attribute key",
                        ))
                    }
                })?;
            }
        }

        Ok(option)
    }

    fn join(&self, other: &AttributeOption) -> Self {
        Self {
            wraps: self.wraps || other.wraps,
            is_terminator: self.is_terminator || other.is_terminator,
            is_constant: self.is_constant || other.is_constant,
            is_pure: self.is_pure || other.is_pure,
        }
    }
}
