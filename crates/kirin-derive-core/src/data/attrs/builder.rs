use quote::ToTokens;

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

impl std::fmt::Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Builder::Enabled => f.debug_tuple("Enabled").finish(),
            Builder::EnabledWithName(name) => f
                .debug_tuple("EnabledWithName")
                .field(&name.to_token_stream())
                .finish(),
            Builder::Disabled => f.debug_tuple("Disabled").finish(),
        }
    }
}

#[derive(Clone, Default)]
pub struct FieldBuilder {
    /// whether to generate builder method to use `.into()` conversion
    pub into: bool,
    /// default value expression for the field, if present,
    /// builder method will use it instead of passing as argument
    pub default: Option<syn::Expr>,
    /// type expression for the SSAValue/ResultValue in the builder
    pub ty: Option<syn::Expr>,
}

impl std::fmt::Debug for FieldBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldBuilder")
            .field("into", &self.into)
            .field("default", &self.default.as_ref().map(|e| e.to_token_stream()))
            .field("ty", &self.ty.as_ref().map(|e| e.to_token_stream()))
            .finish()
    }
}
