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
    pub ty_lattice: Option<syn::Type>,
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
    pub fn new<'a>(input: &'a syn::DeriveInput) -> Self
    {
        let mut struct_attr = Self::default();
        parse_kirin_attributes(&input.attrs, |meta| {
            if meta.path.is_ident("wraps") {
                struct_attr.wraps = true;
            } else if meta.path.is_ident("crate") {
                let path: syn::Path = meta.value()?.parse()?;
                struct_attr.crate_path = Some(path);
            } else if meta.path.is_ident("type_lattice") {
                let ty: syn::Type = meta.value()?.parse()?;
                struct_attr.ty_lattice = Some(ty);
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
        })
        .unwrap();

        let syn::Data::Struct(data) = &input.data else {
            panic!("StructAttribute can only be created from struct data");
        };

        let fields: Vec<Option<FieldAttribute>> = data
            .fields
            .iter()
            .map(|field| FieldAttribute::from_field_attrs(&field.attrs))
            .collect();

        // if all fields are None, set to None
        if fields.iter().all(|f| f.is_none()) {
            return struct_attr;
        }
        struct_attr.fields = Some(fields);
        struct_attr
    }

    pub fn is_wrapper(&self) -> bool {
        self.wraps
            || self.fields.as_ref().map_or(false, |fields| {
                fields
                    .iter()
                    .any(|f| f.as_ref().map_or(false, |fa| fa.wraps))
            })
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
