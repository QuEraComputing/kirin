use crate::ir::{self, BuilderOptions, StandardLayout};

#[derive(Clone, Debug)]
pub struct InputContext {
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub crate_path: Option<syn::Path>,
    pub type_lattice: syn::Path,
    pub builder: Option<BuilderOptions>,
    pub is_enum: bool,
}

impl InputContext {
    pub fn from_input(input: &ir::Input<StandardLayout>) -> Self {
        Self {
            name: input.name.clone(),
            generics: input.generics.clone(),
            crate_path: input.attrs.crate_path.clone(),
            type_lattice: input.attrs.type_lattice.clone(),
            builder: input.attrs.builder.clone(),
            is_enum: matches!(input.data, ir::Data::Enum(_)),
        }
    }

    pub fn builder<'a>(&'a self, default_crate_path: &'a syn::Path) -> InputBuilder<'a> {
        InputBuilder {
            input: self,
            default_crate_path,
        }
    }
}

pub struct InputBuilder<'a> {
    input: &'a InputContext,
    default_crate_path: &'a syn::Path,
}

impl InputBuilder<'_> {
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
