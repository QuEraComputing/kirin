use crate::context::DeriveContext;
use crate::ir::{self, Layout, StandardLayout};
use crate::misc::from_str;
use crate::tokens::{Method, TraitImpl};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::Template;
use super::method_pattern::{
    AssocTypeSpec, BoolProperty, MethodSpec,
    bool_property::{PropertyKind, PropertyValueReader},
    field_collection::FieldIterKind,
};

/// Template that generates `impl Trait for Type { methods, assoc types }`.
///
/// For enums, methods automatically generate match expressions with one arm
/// per variant, where each arm's body comes from the method pattern.
pub struct TraitImplTemplate<L: Layout> {
    trait_path: syn::Path,
    default_crate_path: syn::Path,
    /// Modifies impl generics (e.g., add `'__ir`, `__InterpI`).
    generics_modifier: Option<Box<dyn Fn(&syn::Generics) -> syn::Generics>>,
    /// Trait-level generics (e.g., `<'a>`).
    trait_generics_fn: Option<Box<dyn Fn(&DeriveContext<'_, L>) -> TokenStream>>,
    /// Extra where clause.
    where_clause_fn: Option<Box<dyn Fn(&DeriveContext<'_, L>) -> Option<syn::WhereClause>>>,
    methods: Vec<MethodSpec<L>>,
    assoc_types: Vec<AssocTypeSpec<L>>,
    validate: Option<Box<dyn Fn(&DeriveContext<'_, L>) -> darling::Result<()>>>,
}

impl<L: Layout> TraitImplTemplate<L> {
    /// Create a new trait impl template for the given trait and default crate path.
    pub fn new(trait_path: syn::Path, default_crate_path: syn::Path) -> Self {
        Self {
            trait_path,
            default_crate_path,
            generics_modifier: None,
            trait_generics_fn: None,
            where_clause_fn: None,
            methods: Vec::new(),
            assoc_types: Vec::new(),
            validate: None,
        }
    }

    /// Set a function that transforms the impl-level generics (e.g., to add lifetime params).
    pub fn generics_modifier(
        mut self,
        f: impl Fn(&syn::Generics) -> syn::Generics + 'static,
    ) -> Self {
        self.generics_modifier = Some(Box::new(f));
        self
    }

    /// Set a function that computes trait-level generic arguments (e.g., `<'a, L>`).
    pub fn trait_generics(
        mut self,
        f: impl Fn(&DeriveContext<'_, L>) -> TokenStream + 'static,
    ) -> Self {
        self.trait_generics_fn = Some(Box::new(f));
        self
    }

    /// Set a function that computes an extra where clause to merge with the original.
    pub fn where_clause(
        mut self,
        f: impl Fn(&DeriveContext<'_, L>) -> Option<syn::WhereClause> + 'static,
    ) -> Self {
        self.where_clause_fn = Some(Box::new(f));
        self
    }

    /// Add a method to the trait impl, defined by a [`MethodSpec`].
    pub fn method(mut self, spec: MethodSpec<L>) -> Self {
        self.methods.push(spec);
        self
    }

    /// Add an associated type to the trait impl.
    pub fn assoc_type(mut self, spec: AssocTypeSpec<L>) -> Self {
        self.assoc_types.push(spec);
        self
    }

    /// Set a validation function that runs before code generation.
    pub fn validate(
        mut self,
        f: impl Fn(&DeriveContext<'_, L>) -> darling::Result<()> + 'static,
    ) -> Self {
        self.validate = Some(Box::new(f));
        self
    }

    fn full_trait_path(&self, ctx: &DeriveContext<'_, L>) -> syn::Path {
        ctx.meta
            .path_builder(&self.default_crate_path)
            .full_trait_path(&self.trait_path)
    }

    fn build_impl_generics(&self, ctx: &DeriveContext<'_, L>) -> syn::Generics {
        let base = &ctx.meta.generics;
        match &self.generics_modifier {
            Some(f) => f(base),
            None => base.clone(),
        }
    }
}

impl<L: Layout> Template<L> for TraitImplTemplate<L> {
    fn emit(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> {
        if let Some(validate) = &self.validate {
            validate(ctx)?;
        }

        let full_trait_path = self.full_trait_path(ctx);
        let type_name = &ctx.meta.name;
        let impl_generics = self.build_impl_generics(ctx);
        let (_, ty_generics, orig_where) = ctx.meta.generics.split_for_impl();

        let mut trait_impl =
            TraitImpl::new(impl_generics, &full_trait_path, type_name).type_generics(ty_generics);

        if let Some(f) = &self.trait_generics_fn {
            trait_impl = trait_impl.trait_generics(f(ctx));
        }

        // Merge where clauses
        let extra_where = self.where_clause_fn.as_ref().and_then(|f| f(ctx));
        let merged = crate::codegen::combine_where_clauses(extra_where.as_ref(), orig_where);
        trait_impl = trait_impl.where_clause(merged);

        // Add associated types
        for spec in &self.assoc_types {
            match spec {
                AssocTypeSpec::Fixed(name, ty) => {
                    trait_impl = trait_impl.assoc_type(name.clone(), ty);
                }
                AssocTypeSpec::PerStatement { name, compute } => {
                    // Use first statement to compute the type
                    if let Some(first_stmt_ctx) = ctx.statements.values().next() {
                        let ty = compute(ctx, first_stmt_ctx);
                        trait_impl = trait_impl.assoc_type(name.clone(), ty);
                    }
                }
            }
        }

        // Add methods
        for method_spec in &self.methods {
            let body = match &ctx.input.data {
                ir::Data::Struct(data) => {
                    let stmt_ctx = ctx
                        .statements
                        .get(&data.0.name.to_string())
                        .ok_or_else(|| darling::Error::custom("missing statement context"))?;
                    method_spec.pattern.for_struct(ctx, stmt_ctx)?
                }
                ir::Data::Enum(data) => {
                    let mut arms = Vec::new();
                    for variant in &data.variants {
                        let stmt_ctx =
                            ctx.statements
                                .get(&variant.name.to_string())
                                .ok_or_else(|| {
                                    darling::Error::custom(format!(
                                        "missing statement context for '{}'",
                                        variant.name
                                    ))
                                })?;
                        let variant_body = method_spec.pattern.for_variant(ctx, stmt_ctx)?;
                        let variant_name = &variant.name;
                        let pattern = &stmt_ctx.pattern;
                        let arm_pattern = if stmt_ctx.pattern.is_empty() {
                            quote! { Self::#variant_name }
                        } else {
                            quote! { Self::#variant_name #pattern }
                        };
                        arms.push(quote! { #arm_pattern => #variant_body });
                    }
                    if data.has_hidden_variants {
                        quote! {
                            match self {
                                #(#arms,)*
                                _ => unreachable!()
                            }
                        }
                    } else {
                        quote! {
                            match self {
                                #(#arms),*
                            }
                        }
                    }
                }
            };

            trait_impl = trait_impl.method(Method {
                name: method_spec.name.clone(),
                self_arg: method_spec.self_arg.clone(),
                params: method_spec.params.clone(),
                return_type: method_spec.return_type.clone(),
                body,
                generics: method_spec.generics.clone(),
                method_where_clause: method_spec.method_where_clause.clone(),
            });
        }

        Ok(vec![trait_impl.to_token_stream()])
    }
}

// --- Factory methods ---

/// Configuration for a boolean property trait.
pub struct BoolPropertyConfig {
    /// How the property value is read from attributes.
    pub kind: PropertyKind,
    /// Fully qualified trait name (e.g., `"IsPure"`).
    pub trait_name: &'static str,
    /// Method name on the trait (e.g., `"is_pure"`).
    pub trait_method: &'static str,
}

impl TraitImplTemplate<StandardLayout> {
    /// Create a template for a boolean property trait (e.g., `IsPure`, `IsTerminator`).
    pub fn bool_property(config: BoolPropertyConfig, default_crate_path: &str) -> Self {
        let trait_path: syn::Path = from_str(config.trait_name);
        let trait_method: syn::Ident = from_str(config.trait_method);
        let crate_path: syn::Path = from_str(default_crate_path);

        TraitImplTemplate::new(trait_path.clone(), crate_path.clone()).method(MethodSpec {
            name: trait_method.clone(),
            self_arg: quote! { &self },
            params: vec![],
            return_type: Some(quote! { bool }),
            pattern: Box::new(BoolProperty::new(
                config.kind,
                trait_path,
                trait_method,
                crate_path,
            )),
            generics: None,
            method_where_clause: None,
        })
    }

    /// Create a template for a boolean property with a custom reader.
    pub fn bool_property_with_reader(
        reader: impl PropertyValueReader + 'static,
        trait_name: &str,
        trait_method_name: &str,
        default_crate_path: &str,
    ) -> Self {
        let trait_path: syn::Path = from_str(trait_name);
        let trait_method: syn::Ident = from_str(trait_method_name);
        let crate_path: syn::Path = from_str(default_crate_path);

        TraitImplTemplate::new(trait_path.clone(), crate_path.clone()).method(MethodSpec {
            name: trait_method.clone(),
            self_arg: quote! { &self },
            params: vec![],
            return_type: Some(quote! { bool }),
            pattern: Box::new(BoolProperty::new(
                reader,
                trait_path,
                trait_method,
                crate_path,
            )),
            generics: None,
            method_where_clause: None,
        })
    }

    /// Create a marker trait impl with a `Type` associated type.
    pub fn marker(trait_path: &syn::Path, ir_type: &syn::Path) -> MarkerTemplate {
        MarkerTemplate {
            trait_path: trait_path.clone(),
            ir_type: ir_type.clone(),
        }
    }
}

/// Template for a marker trait impl (`impl Dialect for Type { type Type = IrType; }`).
pub struct MarkerTemplate {
    trait_path: syn::Path,
    ir_type: syn::Path,
}

impl Template<StandardLayout> for MarkerTemplate {
    fn emit(&self, ctx: &DeriveContext<'_, StandardLayout>) -> darling::Result<Vec<TokenStream>> {
        let ir_type = &self.ir_type;
        let result = TraitImpl::new(ctx.meta.generics.clone(), &self.trait_path, &ctx.meta.name)
            .assoc_type(
                syn::Ident::new("Type", proc_macro2::Span::call_site()),
                ir_type,
            )
            .to_token_stream();
        Ok(vec![result])
    }
}

/// Configuration for a field iterator trait.
pub struct FieldIterConfig {
    /// Which field category to iterate over (e.g., regions, blocks, successors).
    pub kind: FieldIterKind,
    /// Whether the iterator yields mutable references.
    pub mutable: bool,
    /// Fully qualified trait name (e.g., `"HasRegions"`).
    pub trait_name: &'static str,
    /// The IR type that fields must match (e.g., `"Region"`).
    pub matching_type: &'static str,
    /// Method name on the trait (e.g., `"regions"`).
    pub trait_method: &'static str,
    /// Associated type name for the iterator (e.g., `"Iter"`).
    pub trait_type_iter: &'static str,
}

impl TraitImplTemplate<StandardLayout> {
    /// Create templates for a field iterator trait (e.g., `HasArguments`).
    ///
    /// Returns a `CompositeTemplate` containing:
    /// 1. The trait impl template
    /// 2. The iterator struct/enum definition
    /// 3. The Iterator impl for the generated type
    pub fn field_iter(
        config: FieldIterConfig,
        default_crate_path: &str,
        trait_lifetime: &str,
    ) -> super::FieldIterTemplateSet {
        super::FieldIterTemplateSet::new(config, default_crate_path, trait_lifetime)
    }
}
