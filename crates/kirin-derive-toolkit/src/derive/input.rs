use crate::ir::{self, BuilderOptions, Layout};

/// Extracted metadata from an [`Input`](crate::ir::Input): name, generics,
/// crate path, IR type, and whether it's an enum.
///
/// Use [`path_builder`](Self::path_builder) to construct fully-qualified
/// paths for generated trait references.
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
