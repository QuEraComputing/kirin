use darling::{Error, FromDeriveInput, FromField, FromMeta, FromVariant};

#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(kirin), supports(struct_any))]
pub struct KirinStructOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

#[derive(Debug, Clone, FromDeriveInput)]
#[darling(attributes(kirin), supports(enum_any))]
pub struct KirinEnumOptions {
    #[darling(rename = "crate")]
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

#[derive(Debug, Clone, FromVariant)]
#[darling(attributes(kirin))]
pub struct StatementOptions {
    pub format: Option<String>,
    #[darling(rename = "fn")]
    pub builder: Option<BuilderOptions>,
    #[darling(default)]
    pub constant: bool,
    #[darling(default)]
    pub pure: bool,
    #[darling(default)]
    pub terminator: bool,
}

#[derive(Debug, Clone, FromField)]
#[darling(attributes(kirin))]
pub struct KirinFieldOptions {
    #[darling(default)]
    pub into: bool,
    pub default: Option<syn::Expr>,
    #[darling(rename = "type")]
    pub ssa_ty: Option<syn::Expr>,
}

#[derive(Debug, Clone)]
pub enum BuilderOptions {
    Enabled,
    Named(syn::Ident),
}

impl FromMeta for BuilderOptions {
    fn from_word() -> darling::Result<Self> {
        Ok(BuilderOptions::Enabled)
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(syn::ExprPath { path, .. }) => {
                if let Some(ident) = path.get_ident() {
                    Ok(BuilderOptions::Named(ident.clone()))
                } else {
                    Err(Error::custom("Expected identifier for builder name"))
                }
            }
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => {
                let ident = syn::Ident::new(&s.value(), s.span());
                Ok(BuilderOptions::Named(ident))
            }
            _ => Err(Error::custom(
                "Expected identifier or string for builder name",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    pub builder: Option<BuilderOptions>,
    pub constant: bool,
    pub pure: bool,
    pub terminator: bool,
}

impl FromDeriveInput for GlobalOptions {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        match input.data {
            syn::Data::Struct(_) => {
                let opts = KirinStructOptions::from_derive_input(input)?;
                Ok(opts.into())
            }
            syn::Data::Enum(_) => {
                let opts = KirinEnumOptions::from_derive_input(input)?;
                Ok(opts.into())
            }
            _ => Err(Error::custom(
                "Kirin can only be derived for structs and enums",
            )),
        }
    }
}

impl FromDeriveInput for StatementOptions {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let opts = KirinStructOptions::from_derive_input(input)?;
        Ok(opts.into())
    }
}

impl From<KirinStructOptions> for GlobalOptions {
    fn from(opts: KirinStructOptions) -> Self {
        GlobalOptions {
            crate_path: opts.crate_path,
            type_lattice: opts.type_lattice,
            builder: opts.builder,
            constant: opts.constant,
            pure: opts.pure,
            terminator: opts.terminator,
        }
    }
}

impl From<KirinStructOptions> for StatementOptions {
    fn from(opts: KirinStructOptions) -> Self {
        StatementOptions {
            format: opts.format,
            builder: opts.builder,
            constant: opts.constant,
            pure: opts.pure,
            terminator: opts.terminator,
        }
    }
}

impl From<KirinEnumOptions> for GlobalOptions {
    fn from(value: KirinEnumOptions) -> Self {
        GlobalOptions {
            crate_path: value.crate_path,
            type_lattice: value.type_lattice,
            builder: value.builder,
            constant: value.constant,
            pure: value.pure,
            terminator: value.terminator,
        }
    }
}
