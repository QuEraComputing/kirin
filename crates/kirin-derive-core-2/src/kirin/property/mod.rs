mod context;
mod struct_impl;
mod enum_impl;

pub use context::{IsConstant, IsPure, IsTerminator, Property, SearchProperty};

use enum_impl::EnumImpl;
use struct_impl::StructImpl;

pub enum PropertyImpl<'a, 'src> {
    Struct(StructImpl<'a, 'src>),
    Enum(EnumImpl<'a, 'src>),
}
