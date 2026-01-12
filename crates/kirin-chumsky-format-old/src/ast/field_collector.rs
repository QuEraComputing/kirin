use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::parse::FormatOption;

use super::{FormatUsage, SyntaxField, SyntaxFieldKind};

#[derive(Debug)]
pub struct CollectedField {
    pub index: usize,
    pub occurrence: usize,
    pub ident: Option<syn::Ident>,
    pub collection: kirin_derive_core_2::ir::fields::Collection,
    pub syntax: SyntaxField,
}

impl CollectedField {
    pub fn render_type(&self) -> TokenStream {
        let base = self.syntax.render_base_type();
        match self.collection {
            kirin_derive_core_2::ir::fields::Collection::Single => base,
            kirin_derive_core_2::ir::fields::Collection::Vec => quote::quote! { Vec<#base> },
            kirin_derive_core_2::ir::fields::Collection::Option => {
                quote::quote! { Option<#base> }
            }
        }
    }
}

pub struct FieldCollector {
    crate_path: syn::Path,
    format_usage: FormatUsage,
    fields: Vec<CollectedField>,
    occurrences: std::collections::HashMap<usize, usize>,
}

impl FieldCollector {
    pub fn new(crate_path: syn::Path, format_usage: FormatUsage) -> Self {
        Self {
            crate_path,
            format_usage,
            fields: Vec::new(),
            occurrences: std::collections::HashMap::new(),
        }
    }

    pub fn finish(mut self) -> Vec<CollectedField> {
        self.fields.sort_by_key(|f| (f.index, f.occurrence));
        self.fields
    }

    fn add_field(
        &mut self,
        field: &kirin_derive_core_2::ir::fields::FieldIndex,
        collection: kirin_derive_core_2::ir::fields::Collection,
        kind: SyntaxFieldKind,
        ty: TokenStream,
        suffix: Option<&str>,
    ) {
        let occurrence = self
            .occurrences
            .entry(field.index)
            .and_modify(|v| *v += 1)
            .or_insert(0)
            .to_owned();

        let ident = make_ident(field, suffix);

        self.fields.push(CollectedField {
            index: field.index,
            occurrence,
            ident,
            collection,
            syntax: SyntaxField {
                crate_path: self.crate_path.clone(),
                ty,
                kind,
            },
        });
    }
}

impl<'ir> kirin_derive_core_2::scan::Scan<'ir, crate::ChumskyLayout> for FieldCollector {
    fn scan_argument(
        &mut self,
        argument: &'ir kirin_derive_core_2::ir::fields::Argument,
    ) -> darling::Result<()> {
        let usages = self.format_usage.for_index(argument.field.index).to_vec();
        if !self.format_usage.has_format() && usages.is_empty() {
            self.add_field(
                &argument.field,
                argument.collection.clone(),
                SyntaxFieldKind::SSAValue,
                quote::quote! { SSAValue },
                None,
            );
        } else {
            for usage in usages {
                match usage {
                    FormatOption::Default => self.add_field(
                        &argument.field,
                        argument.collection.clone(),
                        SyntaxFieldKind::SSAValue,
                        quote::quote! { SSAValue },
                        None,
                    ),
                    FormatOption::Name => self.add_field(
                        &argument.field,
                        kirin_derive_core_2::ir::fields::Collection::Single,
                        SyntaxFieldKind::NameofSSAValue,
                        quote::quote! { SSAValue },
                        Some("name"),
                    ),
                    FormatOption::Type => self.add_field(
                        &argument.field,
                        kirin_derive_core_2::ir::fields::Collection::Single,
                        SyntaxFieldKind::TypeofSSAValue,
                        quote::quote! { SSAValue },
                        Some("type"),
                    ),
                }
            }
        }
        Ok(())
    }

    fn scan_result(
        &mut self,
        result: &'ir kirin_derive_core_2::ir::fields::Result,
    ) -> darling::Result<()> {
        let usages = self.format_usage.for_index(result.field.index).to_vec();
        if !self.format_usage.has_format() && usages.is_empty() {
            self.add_field(
                &result.field,
                result.collection.clone(),
                SyntaxFieldKind::ResultValue,
                quote::quote! { ResultValue },
                None,
            );
        } else {
            for usage in usages {
                if matches!(usage, FormatOption::Type) {
                    self.add_field(
                        &result.field,
                        kirin_derive_core_2::ir::fields::Collection::Single,
                        SyntaxFieldKind::TypeofResultValue,
                        quote::quote! { ResultValue },
                        Some("type"),
                    );
                }
            }
        }
        Ok(())
    }

    fn scan_block(
        &mut self,
        block: &'ir kirin_derive_core_2::ir::fields::Block,
    ) -> darling::Result<()> {
        self.add_field(
            &block.field,
            block.collection.clone(),
            SyntaxFieldKind::Block,
            quote::quote! { Block },
            None,
        );
        Ok(())
    }

    fn scan_successor(
        &mut self,
        successor: &'ir kirin_derive_core_2::ir::fields::Successor,
    ) -> darling::Result<()> {
        self.add_field(
            &successor.field,
            successor.collection.clone(),
            SyntaxFieldKind::Successor,
            quote::quote! { Successor },
            None,
        );
        Ok(())
    }

    fn scan_region(
        &mut self,
        region: &'ir kirin_derive_core_2::ir::fields::Region,
    ) -> darling::Result<()> {
        self.add_field(
            &region.field,
            region.collection.clone(),
            SyntaxFieldKind::Region,
            quote::quote! { Region },
            None,
        );
        Ok(())
    }

    fn scan_comptime_value(
        &mut self,
        value: &'ir kirin_derive_core_2::ir::fields::CompileTimeValue<crate::ChumskyLayout>,
    ) -> darling::Result<()> {
        if value.default.is_none() {
            self.add_field(
                &value.field,
                kirin_derive_core_2::ir::fields::Collection::Single,
                SyntaxFieldKind::AsIs,
                wrap_with_chumsky_output(value.ty.to_token_stream()),
                None,
            );
        }
        Ok(())
    }
}

fn make_ident(
    field: &kirin_derive_core_2::ir::fields::FieldIndex,
    suffix: Option<&str>,
) -> Option<syn::Ident> {
    match suffix {
        Some(suffix) => {
            let base = field
                .ident
                .as_ref()
                .cloned()
                .unwrap_or_else(|| quote::format_ident!("field_{}", field.index));
            Some(quote::format_ident!("{}_{}", base, suffix))
        }
        None => field.ident.clone(),
    }
}

fn wrap_with_chumsky_output(ty: TokenStream) -> TokenStream {
    quote::quote! {
        <#ty as ::kirin_chumsky_2::WithChumskyParser<'tokens, 'src>>::Output
    }
}
