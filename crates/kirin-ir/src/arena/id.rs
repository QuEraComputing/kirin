use std::hash::Hash;
use crate::Dialect;

/// Arena ID
/// an ID object can only be created by
/// `arena.next_id()` or `arena.insert`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Id(pub(crate) usize);

impl Id {
    /// return raw ID as usize
    pub fn raw(self) -> usize {
        self.0
    }
}

pub trait Identifier:
    Sized + Clone + Copy + Hash + std::fmt::Debug + PartialEq + Eq + From<Id> + Into<Id>
{
}

pub trait GetInfo<L: Dialect>: std::fmt::Debug {
    type Info;
    /// Get a reference to the context info for the given node pointer.
    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info>;
    /// Get a mutable reference to the context info for the given node pointer.
    fn get_info_mut<'a>(
        &self,
        context: &'a mut crate::Context<L>,
    ) -> Option<&'a mut Self::Info>;
    /// Get a reference to the context info for the given node pointer, panicking if not found.
    fn expect_info<'a>(&self, context: &'a crate::Context<L>) -> &'a Self::Info {
        self.get_info(context).unwrap_or_else(|| {
            panic!(
                "Expected to find info for ID {:?} in context, but none was found.",
                self
            )
        })
    }
    /// Get a mutable reference to the context info for the given node pointer, panicking if not found.
    fn expect_info_mut<'a>(&self, context: &'a mut crate::Context<L>) -> &'a mut Self::Info {
        self.get_info_mut(context).unwrap_or_else(|| {
            panic!(
                "Expected to find mutable info for ID {:?} in context, but none was found.",
                self
            )
        })
    }
}

#[macro_export(local_inner_macros)]
macro_rules! identifier {
    ($(#[$attr:meta])* struct $name:ident) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(pub(crate) Id);

        impl From<Id> for $name {
            fn from(value: Id) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Id {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl crate::arena::Identifier for $name {}
    };
}
