use quote::format_ident;

pub struct Config {
    pub accessor: syn::Ident,
    pub accessor_iter: syn::Ident,
    pub matching_type: syn::Ident,
    pub trait_path: syn::Path,
}

impl Config {
    pub fn new(accessor: impl AsRef<str>, matching_type: impl AsRef<str>, trait_path: impl AsRef<str>) -> Self {
        let accessor_str = accessor.as_ref();
        // convert accessor_str to camel case for iterator name
        let accessor_iter_str = format!("__Kirin{}Iter", to_camel_case(accessor_str));
        Self {
            accessor: format_ident!("{}", accessor_str),
            accessor_iter: format_ident!("{}", accessor_iter_str),
            matching_type: format_ident!("{}", matching_type.as_ref()),
            trait_path: syn::parse_str(trait_path.as_ref()).unwrap(),
        }
    }
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}
