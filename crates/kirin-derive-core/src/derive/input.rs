use crate::ir::{self, BuilderOptions, StandardLayout};

/// Metadata extracted from the derive input.
///
/// This struct contains the key information needed for code generation,
/// extracted from the original `Input<L>` for convenient access.
#[derive(Clone, Debug)]
pub struct InputMeta {
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub crate_path: Option<syn::Path>,
    pub ir_type: syn::Path,
    pub builder: Option<BuilderOptions>,
    pub is_enum: bool,
}

impl InputMeta {
    pub fn from_input(input: &ir::Input<StandardLayout>) -> Self {
        Self {
            name: input.name.clone(),
            generics: input.generics.clone(),
            crate_path: input.attrs.crate_path.clone(),
            ir_type: input.attrs.ir_type.clone(),
            builder: input.attrs.builder.clone(),
            is_enum: matches!(input.data, ir::Data::Enum(_)),
        }
    }

    /// Creates a path builder for generating fully qualified paths.
    pub fn path_builder<'a>(&'a self, default_crate_path: &'a syn::Path) -> PathBuilder<'a> {
        PathBuilder {
            input: self,
            default_crate_path,
        }
    }

    /// Alias for `path_builder` for backwards compatibility.
    #[deprecated(since = "0.2.0", note = "Use `path_builder` instead")]
    pub fn builder<'a>(&'a self, default_crate_path: &'a syn::Path) -> PathBuilder<'a> {
        self.path_builder(default_crate_path)
    }
}

/// Builder for generating fully qualified paths.
///
/// This helper resolves paths relative to the crate path specified
/// in the derive attributes, or falls back to a default.
pub struct PathBuilder<'a> {
    input: &'a InputMeta,
    default_crate_path: &'a syn::Path,
}

impl PathBuilder<'_> {
    pub fn full_crate_path(&self) -> syn::Path {
        self.input
            .crate_path
            .clone()
            .unwrap_or_else(|| self.default_crate_path.clone())
    }

    pub fn full_trait_path(&self, trait_path: &syn::Path) -> syn::Path {
        self.full_path(trait_path)
    }

    pub fn full_path(&self, suffix: &syn::Path) -> syn::Path {
        let mut path = self.full_crate_path();
        path.segments.extend(suffix.segments.clone());
        path
    }
}

// Type aliases for backwards compatibility
#[deprecated(since = "0.2.0", note = "Use `InputMeta` instead")]
pub type InputContext = InputMeta;

#[deprecated(since = "0.2.0", note = "Use `PathBuilder` instead")]
pub type InputBuilder<'a> = PathBuilder<'a>;
