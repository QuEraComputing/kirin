use super::FieldMember;
use super::dialect_impl::Dialect;
use super::enum_impl::DialectEnum;
use super::struct_impl::DialectStruct;
use proc_macro2::TokenStream;
use quote::ToTokens;

pub trait FromContext<'src, Ctx: Context<'src>, Node>: Sized {
    fn from_context(ctx: &Ctx, node: &'src Node) -> syn::Result<Self>;
}

impl<'src, Ctx: Context<'src>, Node> FromContext<'src, Ctx, Node> for () {
    fn from_context(_ctx: &Ctx, _node: &Node) -> syn::Result<Self> {
        Ok(())
    }
}

pub trait Source {
    type Output: ToTokens;
    fn source(&self) -> &Self::Output;
}

pub trait SourceIdent {
    fn source_ident(&self) -> &syn::Ident;
}

impl<T> SourceIdent for T
where
    T: Source<Output = syn::DeriveInput>,
{
    fn source_ident(&self) -> &syn::Ident {
        &self.source().ident
    }
}

pub trait HasGenerics {
    fn generics(&self) -> &syn::Generics;

    fn combine_generics(&self, other: &syn::Generics) -> syn::Generics {
        let mut combined = self.generics().clone();
        combined.params.extend(other.params.clone());
        combined
    }

    /// add a lifetime parameter to the generics
    fn add_lifetime(&self, lifetime: syn::Lifetime) -> syn::Generics {
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                lifetime,
            )));
        self.combine_generics(&generics)
    }
}

pub trait ContainsWrapper {
    /// does the structure contains a statement that is a wrapper around another statement
    fn contains_wrapper(&self) -> bool;
}

pub trait Wrapper<'src, Attr, E>: Source {
    /// get the wrapper field if any
    fn wrapper(&self) -> Option<FieldMember<'_, 'src, Attr, E>>;

    fn wrapper_ty(&self) -> Option<&'src syn::Type> {
        self.wrapper().map(|w| &w.src.ty)
    }

    fn expect_wrapper(&self) -> syn::Result<FieldMember<'_, 'src, Attr, E>> {
        self.wrapper().ok_or_else(|| {
            syn::Error::new_spanned(self.source(), "Expected a wrapper field but none was found")
        })
    }

    fn expect_wrapper_ty(&self) -> syn::Result<&'src syn::Type> {
        self.expect_wrapper().map(|w| &w.src.ty)
    }

    fn wrapper_tokens(&self) -> TokenStream {
        self.expect_wrapper()
            .map(|w| w.to_token_stream())
            .unwrap_or_else(|e| e.to_compile_error())
    }

    fn wrapper_ty_tokens(&self) -> TokenStream {
        self.expect_wrapper()
            .map(|w| w.src.ty.to_token_stream())
            .unwrap_or_else(|e| e.to_compile_error())
    }
}

/// The top-level node in the procedural macro derivation, i.e.
/// `DialectStruct`, `DialectEnum`
pub trait TopLevel<'src, Ctx: Context<'src>> {
    /// get the global attributes for the derivation
    fn attrs_global(&self) -> &Ctx::AttrGlobal;
}

pub trait Context<'src>: Sized {
    /// Extra data for the helper attribute per statement or global
    type AttrGlobal: darling::FromDeriveInput;
    type AttrStatement: darling::FromDeriveInput + darling::FromVariant;
    type AttrField: darling::FromField;
    /// Extra data for each field of a statement
    type FieldExtra: FromContext<'src, Self, syn::Field>;
    /// Extra data for each statement in the dialect
    type StatementExtra: FromContext<'src, Self, syn::DeriveInput>
        + FromContext<'src, Self, syn::Variant>;

    fn helper_attribute() -> &'static str {
        "kirin"
    }
}

impl Context<'_> for () {
    type AttrGlobal = ();
    type AttrStatement = ();
    type AttrField = ();
    type FieldExtra = ();
    type StatementExtra = ();

    fn helper_attribute() -> &'static str {
        "kirin"
    }
}

pub trait AttrCratePath {
    fn crate_path(&self) -> Option<&syn::Path>;
}

impl AttrCratePath for () {
    fn crate_path(&self) -> Option<&syn::Path> {
        None
    }
}

pub trait AllowCratePath<'src>: Context<'src, AttrGlobal: AttrCratePath> {
    /// get the default crate path to use for the derivation
    /// if the derive macro allows specifying a crate path via global
    /// attribute, this will be overridden
    fn crate_path(&self) -> &syn::Path;
    fn absolute_crate_path(&self, path: &syn::Path) -> syn::Path {
        if path.leading_colon.is_some() {
            path.clone()
        } else {
            let mut new_path = self.crate_path().clone();
            new_path.segments.extend(path.segments.clone());
            new_path
        }
    }
}

/// Context for deriving trait implementations
pub trait TraitContext<'src>: Context<'src> {
    /// get the relative path to the trait being implemented
    /// the relative path is relative to the crate path
    /// either specified by the user or defaulted
    fn trait_path(&self) -> &syn::Path;
}

pub trait Emit<'src>: Context<'src> + Compile<'src, Dialect<'src, Self>, Self::Output> {
    type Output: ToTokens;
    /// Emit the field iterator implementation for the given derive input
    fn emit(&self, input: &'src syn::DeriveInput) -> TokenStream {
        match Dialect::from_context(self, input) {
            Ok(dialect) => {
                let fi: Self::Output = self.compile(&dialect);
                fi.to_token_stream()
            }
            Err(e) => e.to_compile_error(),
        }
    }

    #[cfg(feature = "debug")]
    fn print(self, input: &'src syn::DeriveInput) -> String {
        use super::debug::rustfmt;
        let source = self.emit(input).to_string();
        match syn::parse_file(&source) {
            Ok(_) => rustfmt(source),
            Err(_) => {
                // report_syn_error(&err, &source, "generated");
                rustfmt(source);
                panic!("Failed to parse generated code")
            }
        }
    }
}

/// Compile a data node into an intermediate representation
/// that implements ToTokens
pub trait Compile<'src, Input, Output: ToTokens>: Context<'src> {
    fn compile(&self, node: &Input) -> Output;
}

impl<'src, Ctx: Context<'src>> Compile<'src, Dialect<'src, Ctx>, TokenStream> for Ctx
where
    Ctx: Compile<'src, DialectStruct<'src, Ctx>, TokenStream>,
    Ctx: Compile<'src, DialectEnum<'src, Ctx>, TokenStream>,
{
    fn compile(&self, node: &Dialect<'src, Ctx>) -> TokenStream {
        match node {
            Dialect::Struct(s) => self.compile(s),
            Dialect::Enum(e) => self.compile(e),
        }
    }
}
