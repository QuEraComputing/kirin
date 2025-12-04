use quote::ToTokens;

use crate::data::PropertyAttribute;
use crate::data::attrs::utils::parse_kirin_attributes;

use super::builder::Builder;
use super::field_impl::FieldAttribute;
use super::utils::error_unknown_attribute;

#[derive(Clone, Default)]
pub struct StructAttribute {
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
    /// attributes for each field in the struct
    pub fields: Option<Vec<Option<FieldAttribute>>>,
}

impl StructAttribute {
    pub fn new<'a>(input: &'a syn::DeriveInput) -> syn::Result<Self> {
        let mut struct_attr = Self::default();
        parse_kirin_attributes(&input.attrs, |meta| {
            if meta.path.is_ident("wraps") {
                struct_attr.wraps = true;
            } else if meta.path.is_ident("crate") {
                let path: syn::Path = meta.value()?.parse()?;
                struct_attr.crate_path = Some(path);
            } else if meta.path.is_ident("type_lattice") {
                let ty: syn::Type = meta.value()?.parse()?;
                struct_attr.type_lattice = Some(ty);
            } else if meta.path.is_ident("fn") {
                match meta.value() {
                    Ok(v) => {
                        let ident: syn::Ident = v.parse()?;
                        struct_attr.builder = Builder::EnabledWithName(ident);
                    }
                    Err(_) => {
                        // just pass through, #[kirin(fn)] means enable default builder name
                        struct_attr.builder = Builder::Enabled;
                    }
                }
            } else if meta.path.is_ident("constant") {
                struct_attr.is_constant = Some(true);
            } else if meta.path.is_ident("pure") {
                struct_attr.is_pure = Some(true);
            } else if meta.path.is_ident("terminator") {
                struct_attr.is_terminator = Some(true);
            } else {
                return Err(error_unknown_attribute(&meta));
            }
            Ok(())
        })?;

        let syn::Data::Struct(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "StructAttribute can only be created from struct data",
            ));
        };

        let fields: Vec<Option<FieldAttribute>> = data
            .fields
            .iter()
            .map(|field| FieldAttribute::from_field_attrs(&field.attrs))
            .collect::<syn::Result<Vec<_>>>()?;

        // if all fields are None, set to None
        if fields.iter().all(|f| f.is_none()) {
            return Ok(struct_attr);
        }
        struct_attr.fields = Some(fields);
        Ok(struct_attr)
    }

    pub fn is_wrapper(&self) -> bool {
        self.wraps
            || self.fields.as_ref().map_or(false, |fields| {
                fields
                    .iter()
                    .any(|f| f.as_ref().map_or(false, |fa| fa.wraps))
            })
    }

    pub fn get_field_attribute(&self, index: usize) -> Option<&FieldAttribute> {
        self.fields
            .as_ref()
            .and_then(|fields| fields.get(index).and_then(|f| f.as_ref()))
    }
}

impl PropertyAttribute for StructAttribute {
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

impl std::fmt::Debug for StructAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructAttribute")
            .field("wraps", &self.wraps)
            .field(
                "crate_path",
                &self.crate_path.as_ref().map(|p| p.to_token_stream()),
            )
            .field(
                "type_lattice",
                &self.type_lattice.as_ref().map(|t| t.to_token_stream()),
            )
            .field("is_constant", &self.is_constant)
            .field("is_pure", &self.is_pure)
            .field("is_terminator", &self.is_terminator)
            .field("builder", &self.builder)
            .field("fields", &self.fields)
            .finish()
    }
}
