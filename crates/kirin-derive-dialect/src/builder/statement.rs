use kirin_derive_core::prelude::*;

/// Information about a statement for builder generation.
#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    /// All fields in the statement (excluding wrapper fields).
    pub(crate) fields: Vec<FieldInfo<StandardLayout>>,
    pub(crate) build_fn_name: syn::Ident,
    pub(crate) is_wrapper: bool,
    /// The wrapper type if this is a wrapper statement.
    pub(crate) wrapper_type: Option<syn::Type>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_fields_sorted() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = L)]
            struct Example {
                b: ResultValue,
                a: SSAValue,
            }
        };
        let input = ir::Input::<StandardLayout>::from_derive_input(&input).unwrap();
        let ir::Data::Struct(data) = &input.data else {
            panic!("expected struct");
        };
        let fields = data.0.collect_fields();
        let indices: Vec<_> = fields.iter().map(|f| f.index).collect();
        assert_eq!(indices, vec![0, 1]);
    }
}
