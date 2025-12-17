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
    fn crate_path(&self) -> &syn::Path;

    // fn emit_inner<S, E>(&self, input: &'src syn::DeriveInput) -> syn::Result<TokenStream>
    // where
    //     S: Compile<'src, Self, impl ToTokens>,
    //     E: Compile<'src, Self, impl ToTokens>,
    // {
    //     let dialect = Dialect::from_context(self, input)?;
    //     let fi: DataImpl<S, E> = DataImpl::compile(self, &dialect)?;
    //     Ok(fi.to_token_stream())
    // }
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

    fn crate_path(&self) -> &syn::Path {
        panic!("crate_path called on unit context")
    }
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

    fn print(self, input: &'src syn::DeriveInput) -> String {
        let file = syn::parse_file(&self.emit(input).to_string()).unwrap();
        prettyplease::unparse(&file)
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
