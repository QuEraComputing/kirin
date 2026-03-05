use crate::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) fields: Vec<FieldInfo<StandardLayout>>,
    pub(crate) build_fn_name: syn::Ident,
    pub(crate) is_wrapper: bool,
    pub(crate) wrapper_type: Option<syn::Type>,
}
