use proc_macro2::TokenStream;

pub trait TraitInfo<'input>:
    Sized
    + GenerateFrom<'input, RegularStruct<'input, Self>>
    + GenerateFrom<'input, UnnamedWrapperStruct<'input, Self>>
    + GenerateFrom<'input, NamedWrapperStruct<'input, Self>>
    + GenerateFrom<'input, RegularEnum<'input, Self>>
    + GenerateFrom<'input, WrapperEnum<'input, Self>>
    + GenerateFrom<'input, EitherEnum<'input, Self>>
where
    Self: 'input,
{
    type GlobalAttributeData: Default;
    type MatchingFields: FromVariantFields<'input, Self> + FromStructFields<'input, Self>;
    /// Path to the trait being derived, relative to the crate root
    fn relative_trait_path(&self) -> &syn::Path;
    /// Default path to the crate root
    fn default_crate_path(&self) -> syn::Path;
    /// Generics for the trait being derived
    fn trait_generics(&self) -> &syn::Generics;
    /// Method name for the trait being derived
    fn method_name(&self) -> &syn::Ident;
}

// pub struct CheckerTraitInfo {
//     pub trait_path: syn::Path,
// }

// impl<'input> TraitInfo<'input> for CheckerTraitInfo {
//     type GlobalAttributeData = bool;
//     type MatchingFields = bool;
//     fn trait_path(&self) -> syn::Path {
//         self.trait_path.clone()
//     }
// }

// impl FromFields<'_, CheckerTraitInfo> for bool {
//     fn from_fields(
//             ctx: &Context<'_, CheckerTraitInfo>,
//             parent: &'_ syn::Variant,
//             fields: &'_ syn::Fields,
//         ) -> Self {
//         true
//     }
// }
pub trait GenerateFrom<'input, Data> {
    fn generate_from(&self, data: &Data) -> TokenStream;
}

/// If the statement is not a wrapper statement,
/// extract relevant info from them
pub trait FromStructFields<'input, T: TraitInfo<'input>> {
    fn from_struct_fields(
        ctx: &Context<'input, T>,
        parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self;
}

pub trait FromVariantFields<'input, T: TraitInfo<'input>> {
    fn from_variant_fields(
        ctx: &Context<'input, T>,
        parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self;
}

/// Attributes parsed from #[kirin(...)]
/// this can be used for other derive traits so we
/// always keep it across different TraitInfo implementations
/// e.g a new trait may define its own helper attribute in
/// addition to `kirin(wraps)` etc.
#[derive(Clone, Default)]
pub struct KirinAttribute {
    pub wraps: bool,
    pub crate_path: Option<syn::Path>,
    pub is_constant: Option<bool>,
    pub is_pure: Option<bool>,
    pub is_terminator: Option<bool>,
}

impl KirinAttribute {
    pub fn from_global_attrs(attrs: &Vec<syn::Attribute>) -> Self {
        let mut kirin_attr = KirinAttribute::default();
        for attr in attrs {
            if attr.path().is_ident("kirin") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("wraps") {
                        kirin_attr.wraps = true;
                    } else if meta.path.is_ident("crate") {
                        let path: syn::Path = meta.value()?.parse()?;
                        kirin_attr.crate_path = Some(path);
                    } else if meta.path.is_ident("constant") {
                        kirin_attr.is_constant = Some(true);
                    } else if meta.path.is_ident("pure") {
                        kirin_attr.is_pure = Some(true);
                    } else if meta.path.is_ident("terminator") {
                        kirin_attr.is_terminator = Some(true);
                    } else {
                        return Err(meta.error("unknown attribute inside #[kirin(...)]"));
                    }
                    Ok(())
                })
                .unwrap();
            }
        }
        kirin_attr
    }

    pub fn from_attrs(attrs: &Vec<syn::Attribute>) -> Self {
        let mut kirin_attr = KirinAttribute::default();
        for attr in attrs {
            if attr.path().is_ident("kirin") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("wraps") {
                        kirin_attr.wraps = true;
                    } else if meta.path.is_ident("crate") {
                        return Err(meta.error("the 'crate' attribute is not allowed on types"));
                    } else if meta.path.is_ident("constant") {
                        kirin_attr.is_constant = Some(true);
                    } else if meta.path.is_ident("pure") {
                        kirin_attr.is_pure = Some(true);
                    } else if meta.path.is_ident("terminator") {
                        kirin_attr.is_terminator = Some(true);
                    } else {
                        return Err(meta.error("unknown attribute inside #[kirin(...)]"));
                    }
                    Ok(())
                })
                .unwrap();
            }
        }
        kirin_attr
    }

    pub fn from_field_attrs(attrs: &[syn::Attribute]) -> Self {
        let mut kirin_attr = KirinAttribute::default();
        for attr in attrs {
            if attr.path().is_ident("kirin") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("wraps") {
                        kirin_attr.wraps = true;
                    } else if meta.path.is_ident("crate") {
                        return Err(meta.error("the 'crate' attribute is not allowed on fields"));
                    } else if meta.path.is_ident("constant") {
                        return Err(meta.error("the 'constant' attribute is not allowed on fields"));
                    } else if meta.path.is_ident("pure") {
                        return Err(meta.error("the 'pure' attribute is not allowed on fields"));
                    } else if meta.path.is_ident("terminator") {
                        return Err(
                            meta.error("the 'terminator' attribute is not allowed on fields")
                        );
                    } else {
                        return Err(meta.error("unknown attribute inside #[kirin(...)]"));
                    }
                    Ok(())
                })
                .unwrap();
            }
        }
        kirin_attr
    }
}

/// some global context for the derive
pub struct Context<'input, T: TraitInfo<'input>> {
    pub trait_info: T,
    pub input: &'input syn::DeriveInput,
    pub data: T::GlobalAttributeData,
    pub kirin_attr: KirinAttribute,
    pub generics: syn::Generics,
    pub absolute_trait_path: syn::Path,
}

impl<'input, T: TraitInfo<'input>> Context<'input, T> {
    pub fn new(trait_info: T, input: &'input syn::DeriveInput) -> Self {
        let kirin_attr = KirinAttribute::from_global_attrs(&input.attrs);
        let data = T::GlobalAttributeData::default();
        let mut generics = input.generics.clone();
        let trait_generics = trait_info.trait_generics();
        let relative_trait_path = trait_info.relative_trait_path();
        let absolute_trait_path: syn::Path = if let Some(crate_path) = &kirin_attr.crate_path {
            let mut path = crate_path.clone();
            path.segments.extend(relative_trait_path.segments.clone());
            path
        } else {
            let mut path = trait_info.default_crate_path();
            path.segments.extend(relative_trait_path.segments.clone());
            path
        };

        generics.params.extend(trait_generics.params.clone());
        Self {
            trait_info,
            input,
            data,
            kirin_attr,
            generics,
            absolute_trait_path,
        }
    }

    /// splits the generics for impl
    /// - impl_generics: generics for impl declaration
    /// - trait_ty_generics: generics for the type being implemented
    /// - input_type_generics: generics for the input type
    /// - where_clause: where clause
    pub fn split_for_impl(
        &'input self,
    ) -> (
        syn::ImplGenerics<'input>,
        syn::TypeGenerics<'input>,
        syn::TypeGenerics<'input>,
        Option<&'input syn::WhereClause>,
    ) {
        let (_, trait_ty_generics, _) = self.trait_info.trait_generics().split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (impl_generics, _, where_clause) = self.generics.split_for_impl();
        (
            impl_generics,
            trait_ty_generics,
            input_ty_generics,
            where_clause,
        )
    }
}

impl<'input, T, Data> GenerateFrom<'input, Data> for Context<'input, T>
where
    T: TraitInfo<'input> + GenerateFrom<'input, Data>,
{
    fn generate_from(&self, data: &Data) -> TokenStream {
        self.trait_info.generate_from(data)
    }
}

impl<'input, T: TraitInfo<'input> + Default> Context<'input, T> {
    pub fn from_input(input: &'input syn::DeriveInput) -> Self {
        Self::new(T::default(), input)
    }

    /// name of the type being derived
    pub fn name(&self) -> &syn::Ident {
        &self.input.ident
    }

    pub fn trait_path(&self) -> &syn::Path {
        self.trait_info.relative_trait_path()
    }

    pub fn method_name(&self) -> &syn::Ident {
        self.trait_info.method_name()
    }
}

mod enum_impl;
mod struct_impl;

pub use enum_impl::*;
pub use struct_impl::*;

pub enum DataTrait<'input, T: TraitInfo<'input>> {
    Struct(struct_impl::StructTrait<'input, T>),
    Enum(enum_impl::EnumTrait<'input, T>),
}

impl<'input, T: TraitInfo<'input>> DataTrait<'input, T> {
    pub fn new(ctx: &'input Context<'input, T>) -> Self {
        match &ctx.input.data {
            syn::Data::Struct(data) => DataTrait::Struct(struct_impl::StructTrait::new(ctx, data)),
            syn::Data::Enum(data) => DataTrait::Enum(enum_impl::EnumTrait::new(ctx, data)),
            _ => panic!("only structs and enums are supported"),
        }
    }
}

impl<'input, T> GenerateFrom<'input, DataTrait<'input, T>> for T
where
    T: TraitInfo<'input>
        + GenerateFrom<'input, struct_impl::StructTrait<'input, T>>
        + GenerateFrom<'input, enum_impl::EnumTrait<'input, T>>,
{
    fn generate_from(&self, data: &DataTrait<'input, T>) -> TokenStream {
        match data {
            DataTrait::Struct(data) => self.generate_from(data),
            DataTrait::Enum(data) => self.generate_from(data),
        }
    }
}

#[macro_export]
macro_rules! generate_derive {
    ($input:expr, $trait_info:expr) => {{
        let ctx = Context::new($trait_info, $input);
        let data = DataTrait::new(&ctx);
        ctx.generate_from(&data)
    }};
}
