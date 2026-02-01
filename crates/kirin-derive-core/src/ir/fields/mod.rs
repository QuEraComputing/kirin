mod collection;
mod comptime;
mod index;
mod value;
mod wrapper;

pub use collection::Collection;
pub use comptime::{CompileTimeValue, CompileTimeValues};
pub use index::FieldIndex;
pub use value::{Argument, Arguments, Result, Results, Value};
pub use wrapper::Wrapper;

/// Macro to define a simple IR field collection type.
///
/// This generates:
/// - A container struct (e.g., `Blocks`) with `data: Vec<T>`
/// - An item struct (e.g., `Block`) with `field: FieldIndex` and `collection: Collection`
/// - `add()` method that checks for the type name in field type
/// - `iter()` method to iterate over items
macro_rules! define_field_collection {
    ($container:ident, $item:ident, $type_name:literal) => {
        #[derive(Debug, Clone, Default)]
        pub struct $container {
            data: Vec<$item>,
        }

        #[derive(Debug, Clone)]
        pub struct $item {
            pub field: FieldIndex,
            pub collection: Collection,
        }

        impl $container {
            pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
                let Some(coll) = Collection::from_type(&f.ty, $type_name) else {
                    return Ok(false);
                };
                self.data.push($item {
                    field: FieldIndex::new(f.ident.clone(), index),
                    collection: coll,
                });
                Ok(true)
            }

            pub fn iter(&self) -> impl Iterator<Item = &$item> {
                self.data.iter()
            }
        }
    };
}

define_field_collection!(Blocks, Block, "Block");
define_field_collection!(Regions, Region, "Region");
define_field_collection!(Successors, Successor, "Successor");

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
