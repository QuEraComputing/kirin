/// Creates a wrapper over TokenStream to mark
/// the compile target semantically for composability.
/// See the default `kirin` module for usage examples.
/// 
/// # Examples
/// 
/// ```ignore
/// target! {
///    /// docstring for MyTarget
///    pub struct MyTarget
/// }
/// ```
#[macro_export]
macro_rules! target {
    {$(#[$attr:meta])* $v:vis struct $name:ident $(;)?} => {
        #[derive(Clone)]
        $(#[$attr])*
        $v struct $name(proc_macro2::TokenStream);

        impl std::ops::Deref for $name {
            type Target = proc_macro2::TokenStream;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl quote::ToTokens for $name {
            fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl From<$name> for proc_macro2::TokenStream {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl From<proc_macro2::TokenStream> for $name {
            fn from(value: proc_macro2::TokenStream) -> Self {
                $name(value)
            }
        }
    };
}
