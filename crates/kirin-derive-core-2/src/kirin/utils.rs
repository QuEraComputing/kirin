pub fn strip_path(path: &syn::Path) -> syn::Ident {
    path.segments
        .last()
        .expect("matching_type_path must have at least one segment")
        .ident
        .clone()
}

pub fn from_str<T: syn::parse::Parse>(s: impl Into<String>) -> T {
    syn::parse_str(&s.into()).unwrap()
}
