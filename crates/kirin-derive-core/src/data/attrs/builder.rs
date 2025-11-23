#[derive(Clone)]
pub enum Builder {
    Enabled,
    EnabledWithName(syn::Ident),
    Disabled,
}

impl Default for Builder {
    fn default() -> Self {
        Builder::Disabled
    }
}

impl Builder {
    pub fn is_enabled(&self) -> bool {
        match self {
            Builder::Enabled | Builder::EnabledWithName(_) => true,
            Builder::Disabled => false,
        }
    }

    pub fn builder_name(&self, default: &syn::Ident) -> syn::Ident {
        match self {
            Builder::EnabledWithName(name) => name.clone(),
            _ => default.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub struct FieldBuilder {
    /// whether to generate builder method to use `.into()` conversion
    pub into: bool,
    /// initial value expression for the field, if present,
    /// builder method will use it instead of passing as argument
    pub init: Option<syn::Expr>,
    /// type expression for the SSAValue/ResultValue in the builder
    pub ty: Option<syn::Expr>,
}
