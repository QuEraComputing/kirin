mod blocks;
mod collection;
mod comptime;
mod index;
mod regions;
mod successors;
mod value;
mod wrapper;

pub use blocks::{Block, Blocks};
pub use collection::Collection;
pub use comptime::{CompileTimeValue, CompileTimeValues};
pub use index::FieldIndex;
pub use regions::{Region, Regions};
pub use successors::{Successor, Successors};
pub use value::{Argument, Arguments, Result, Results, Value};
pub use wrapper::Wrapper;

/// The category of an IR field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldCategory {
    /// SSAValue input field
    Argument,
    /// ResultValue output field
    Result,
    /// Block field (owned control flow block)
    Block,
    /// Successor field (branch target)
    Successor,
    /// Region field (nested scope)
    Region,
    /// Compile-time value field
    Value,
}

/// Common information about a field, used for iteration across all field categories.
#[derive(Debug, Clone)]
pub struct FieldInfo<'a> {
    /// The field index and optional identifier
    pub field: &'a FieldIndex,
    /// The collection type (Single, Vec, Option)
    pub collection: &'a Collection,
    /// The category of this field
    pub category: FieldCategory,
}
