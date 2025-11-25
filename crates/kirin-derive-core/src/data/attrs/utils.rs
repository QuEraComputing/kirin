pub fn parse_kirin_attributes(
    attrs: &[syn::Attribute],
    logic: impl FnMut(syn::meta::ParseNestedMeta) -> syn::Result<()>,
) -> syn::Result<()> {
    for attr in attrs {
        if attr.path().is_ident("kirin") {
            return attr.parse_nested_meta(logic);
        }
    }
    Ok(())
}

pub fn error_unknown_attribute(meta: &syn::meta::ParseNestedMeta) -> syn::Error {
    if ["crate_path", "ty_lattice"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        return meta.error(format!(
            "the '{}' attribute is only allowed on the type level #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ));
    } else if ["constant", "pure", "terminator", "fn"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        return meta.error(format!(
            "the '{}' attribute is only allowed on the per statement #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ));
    } else if ["into", "default", "type"]
        .iter()
        .any(|name| meta.path.is_ident(name))
    {
        return meta.error(format!(
            "the '{}' attribute is only allowed on fields inside statements",
            meta.path.get_ident().unwrap()
        ));
    } else {
        return meta.error(format!(
            "unknown attribute '{}' for #[kirin(...)]",
            meta.path.get_ident().unwrap()
        ));
    }
}
