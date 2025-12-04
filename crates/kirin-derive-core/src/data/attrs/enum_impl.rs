use crate::data::PropertyAttribute;
use crate::data::attrs::utils::{error_unknown_attribute, parse_kirin_attributes};

use super::builder::Builder;
use super::field_impl::FieldAttribute;

#[derive(Clone, Default)]
pub struct EnumAttribute {
    /// name for the enum statement, e.g a dialect name
    pub name: Option<syn::Expr>,
    /// whether the instruction wraps another instruction
    pub wraps: bool,
    /// path to the kirin crate
    pub crate_path: Option<syn::Path>,
    /// type lattice for the dialect's SSAValue/ResultValue to use
    /// this is only allowed on the type level attribute
    /// e.g #[kirin(type_lattice = ...)]
    /// this is different from the field-level `type` attribute
    /// e.g #[kirin(type = ...)]
    /// which specifies the type of the SSAValue/ResultValue field
    pub type_lattice: Option<syn::Type>,
    /// whether the instruction is constant
    pub is_constant: Option<bool>,
    /// whether the instruction is pure
    pub is_pure: Option<bool>,
    /// whether the instruction is a terminator
    pub is_terminator: Option<bool>,
    /// options for the builder method to generate
    pub builder: Builder,
}

impl EnumAttribute {
    pub fn new<'a>(input: &'a syn::DeriveInput) -> syn::Result<Self> {
        let mut enum_attr = Self::default();
        parse_kirin_attributes(&input.attrs, |meta| {
            if meta.path.is_ident("wraps") {
                enum_attr.wraps = true;
            } else if meta.path.is_ident("name") {
                let expr: syn::Expr = meta.value()?.parse()?;
                enum_attr.name = Some(expr);
            } else if meta.path.is_ident("crate") {
                let path: syn::Path = meta.value()?.parse()?;
                enum_attr.crate_path = Some(path);
            } else if meta.path.is_ident("type_lattice") {
                let ty: syn::Type = meta.value()?.parse()?;
                enum_attr.type_lattice = Some(ty);
            } else if meta.path.is_ident("fn") {
                match meta.value() {
                    Ok(v) => {
                        let ident: syn::Ident = v.parse()?;
                        enum_attr.builder = Builder::EnabledWithName(ident);
                    }
                    Err(_) => {
                        // just pass through, #[kirin(fn)] means enable default builder name
                        enum_attr.builder = Builder::Enabled;
                    }
                }
            } else if meta.path.is_ident("constant") {
                enum_attr.is_constant = Some(true);
            } else if meta.path.is_ident("pure") {
                enum_attr.is_pure = Some(true);
            } else if meta.path.is_ident("terminator") {
                enum_attr.is_terminator = Some(true);
            } else {
                return Err(error_unknown_attribute(&meta));
            }
            Ok(())
        })?;
        Ok(enum_attr)
    }
}

impl PropertyAttribute for EnumAttribute {
    fn is_constant(&self) -> Option<bool> {
        self.is_constant
    }

    fn is_pure(&self) -> Option<bool> {
        self.is_pure
    }

    fn is_terminator(&self) -> Option<bool> {
        self.is_terminator
    }
}

#[derive(Clone, Default)]
pub struct VariantAttribute {
    /// name for the variant statement
    pub name: Option<syn::Expr>,
    /// whether the instruction wraps another instruction
    pub wraps: bool,
    /// whether the instruction is constant
    pub is_constant: Option<bool>,
    /// whether the instruction is pure
    pub is_pure: Option<bool>,
    /// whether the instruction is a terminator
    pub is_terminator: Option<bool>,
    /// options for the builder method to generate
    pub builder: Builder,
    /// field attributes for each field in the variant
    pub fields: Option<Vec<Option<FieldAttribute>>>,
}

impl VariantAttribute {
    pub fn is_wrapper(&self) -> bool {
        self.wraps
            || self.fields.as_ref().map_or(false, |fields| {
                fields.iter().any(|f_attr_opt| {
                    if let Some(f_attr) = f_attr_opt {
                        f_attr.wraps
                    } else {
                        false
                    }
                })
            })
    }

    pub fn get_field_attribute(&self, index: usize) -> Option<&FieldAttribute> {
        if let Some(fields) = &self.fields {
            return fields.get(index).and_then(|f_attr_opt| f_attr_opt.as_ref());
        }
        None
    }

    pub fn new(variant: &syn::Variant) -> syn::Result<Self> {
        let mut variant_attr = Self::default();
        parse_kirin_attributes(&variant.attrs, |meta| {
            if meta.path.is_ident("wraps") {
                variant_attr.wraps = true;
            } else if meta.path.is_ident("name") {
                let expr: syn::Expr = meta.value()?.parse()?;
                variant_attr.name = Some(expr);
            } else if meta.path.is_ident("fn") {
                match meta.value() {
                    Ok(v) => {
                        let ident: syn::Ident = v.parse()?;
                        variant_attr.builder = Builder::EnabledWithName(ident);
                    }
                    Err(_) => {
                        // just pass through, #[kirin(fn)] means enable default builder name
                        variant_attr.builder = Builder::Enabled;
                    }
                }
            } else if meta.path.is_ident("constant") {
                variant_attr.is_constant = Some(true);
            } else if meta.path.is_ident("pure") {
                variant_attr.is_pure = Some(true);
            } else if meta.path.is_ident("terminator") {
                variant_attr.is_terminator = Some(true);
            } else {
                return Err(error_unknown_attribute(&meta));
            }
            Ok(())
        })?;
        let fields = variant
            .fields
            .iter()
            .map(|field| FieldAttribute::from_field_attrs(&field.attrs))
            .collect::<syn::Result<Vec<_>>>()?;
        variant_attr.fields = Some(fields);
        Ok(variant_attr)
    }
}

impl PropertyAttribute for VariantAttribute {
    fn is_constant(&self) -> Option<bool> {
        self.is_constant
    }

    fn is_pure(&self) -> Option<bool> {
        self.is_pure
    }

    fn is_terminator(&self) -> Option<bool> {
        self.is_terminator
    }
}
