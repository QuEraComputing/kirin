use super::index::FieldIndex;
/// Metadata for a `#[wraps]` delegation field.
///
/// When an enum variant has `#[wraps]`, it delegates to an inner type
/// (usually another dialect's statement). The wrapper tracks which field
/// holds the inner type and its `syn::Type`.
#[derive(Debug, Clone)]
pub struct Wrapper {
    /// Position and name of the wrapped field.
    pub field: FieldIndex,
    /// The inner type being delegated to.
    pub ty: syn::Type,
    /// Extra source types that should lift/project through this wrapped type.
    pub lift_project_from: Vec<syn::Path>,
}

/// Parsed options from a `#[wraps(...)]` attribute.
#[derive(Debug, Clone, Default)]
pub struct WrapperOptions {
    pub lift_project_from: Vec<syn::Path>,
}

impl WrapperOptions {
    pub fn from_attrs(attrs: &[syn::Attribute]) -> darling::Result<Option<Self>> {
        let mut combined: Option<Self> = None;

        for attr in attrs.iter().filter(|attr| attr.path().is_ident("wraps")) {
            let options = Self::from_attr(attr)?;
            match &mut combined {
                Some(existing) => existing.extend(options),
                None => combined = Some(options),
            }
        }

        Ok(combined)
    }

    pub fn extend(&mut self, other: Self) {
        self.lift_project_from.extend(other.lift_project_from);
    }

    fn from_attr(attr: &syn::Attribute) -> darling::Result<Self> {
        match &attr.meta {
            syn::Meta::Path(_) => Ok(Self::default()),
            syn::Meta::List(_) => {
                let mut options = Self::default();
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("lift_project_from") {
                        let content;
                        syn::parenthesized!(content in meta.input);
                        let paths =
                            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated(
                                &content,
                            )?;
                        options.lift_project_from.extend(paths);
                        Ok(())
                    } else {
                        Err(meta.error("unsupported #[wraps] option"))
                    }
                })
                .map_err(darling::Error::from)?;
                Ok(options)
            }
            syn::Meta::NameValue(_) => Err(darling::Error::custom(
                "expected #[wraps] or #[wraps(lift_project_from(...))]",
            )
            .with_span(attr)),
        }
    }
}

impl Wrapper {
    /// Create a wrapper from a field's position and `syn::Field` definition.
    pub fn new(index: usize, f: &syn::Field) -> Self {
        Self::new_with_options(index, f, WrapperOptions::default())
    }

    pub fn new_with_options(index: usize, f: &syn::Field, options: WrapperOptions) -> Self {
        Self {
            field: FieldIndex::new(f.ident.clone(), index),
            ty: f.ty.clone(),
            lift_project_from: options.lift_project_from,
        }
    }

    /// For enum Lang<L: Dialect> { Op(L), ... }, this function identifies
    /// variants like `Op(L)` where the wrapped type is a bare type param.
    ///
    /// The derive treats such a variant specially, for two reasons:
    /// - In the generated impls, `L` is only known to implement what its
    ///   bounds say (`Dialect` and its supertraits), so delegating another
    ///   trait to it, such as `HasSignature`, needs a `where` clause.
    /// - `L` may be any type, including a sibling variant's wrapped type
    ///   (`L = VariantTy<T>` makes `VariantTy<L::Type>` equal `L`), so `From<L>`
    ///   would overlap with that sibling's `From`. For this case, we skip
    ///   generating a `From` impl.
    pub fn wraps_bare_type_param(&self, generics: &syn::Generics) -> bool {
        let syn::Type::Path(type_path) = &self.ty else {
            return false;
        };
        type_path.qself.is_none()
            && type_path
                .path
                .get_ident()
                .is_some_and(|ident| generics.type_params().any(|param| param.ident == *ident))
    }
}
