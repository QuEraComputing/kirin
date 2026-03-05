use crate::ir::{self, BuilderOptions, Layout};

/// Extracted metadata from an [`Input`](crate::ir::Input): name, generics,
/// crate path, IR type, and whether it's an enum.
///
/// Use [`path_builder`](Self::path_builder) to construct fully-qualified
/// paths for generated trait references.
#[derive(Clone, Debug)]
pub struct InputMeta {
    /// The type name of the derive target.
    pub name: syn::Ident,
    /// Generic parameters from the derive target.
    pub generics: syn::Generics,
    /// User-specified crate path override from `#[kirin(crate = ...)]`.
    pub crate_path: Option<syn::Path>,
    /// The IR type path (e.g., `kirin_ir`).
    pub ir_type: syn::Path,
    /// Builder options from `#[kirin(builder(...))]`, if present.
    pub builder: Option<BuilderOptions>,
    /// Whether the derive target is an enum (true) or struct (false).
    pub is_enum: bool,
}

impl InputMeta {
    /// Extract metadata from a parsed [`Input`](ir::Input).
    pub fn from_input<L: Layout>(input: &ir::Input<L>) -> Self {
        Self {
            name: input.name.clone(),
            generics: input.generics.clone(),
            crate_path: input.attrs.crate_path.clone(),
            ir_type: input.attrs.ir_type.clone(),
            builder: input.attrs.builder.clone(),
            is_enum: matches!(input.data, ir::Data::Enum(_)),
        }
    }

    /// Create a [`PathBuilder`] that resolves paths against the user's crate
    /// configuration, falling back to `default_crate_path`.
    pub fn path_builder<'a>(&'a self, default_crate_path: &'a syn::Path) -> PathBuilder<'a> {
        PathBuilder {
            input: self,
            default_crate_path,
        }
    }
}

/// Constructs fully-qualified paths relative to the user's crate configuration.
///
/// Respects `#[kirin(crate = ...)]` overrides, falling back to the provided
/// default crate path.
pub struct PathBuilder<'a> {
    input: &'a InputMeta,
    default_crate_path: &'a syn::Path,
}

impl PathBuilder<'_> {
    /// Return the effective crate path, preferring the user override.
    pub fn full_crate_path(&self) -> syn::Path {
        self.input
            .crate_path
            .clone()
            .unwrap_or_else(|| self.default_crate_path.clone())
    }

    /// Resolve a trait path to its fully-qualified form under the crate root.
    pub fn full_trait_path(&self, trait_path: &syn::Path) -> syn::Path {
        self.full_path(trait_path)
    }

    /// Resolve a relative path by prepending the crate root.
    ///
    /// Absolute paths (with leading `::`) are returned unchanged.
    pub fn full_path(&self, suffix: &syn::Path) -> syn::Path {
        // If the suffix is already absolute (has leading `::`) return it as-is.
        if suffix.leading_colon.is_some() {
            return suffix.clone();
        }
        let mut path = self.full_crate_path();
        path.segments.extend(suffix.segments.clone());
        path
    }
}
