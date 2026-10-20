use crate::Dialect;
use std::hash::Hash;

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

/// Look up the info a node id points at.
///
/// Read-only. Mutable lookup lives in the crate-private [`GetInfoMut`], because
/// modifying nodes from outside kirin-ir should exclusively be done using
/// [`Rewriter`](crate::Rewriter). In other words, outside this crate, IR is
/// read through this trait [`GetInfo`] and written through the `Rewriter`.
pub trait GetInfo<L: Dialect>: std::fmt::Debug {
    type Info;
    /// Get a reference to the context info for the given node pointer.
    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info>;
    /// Get a reference to the context info for the given node pointer, panicking if not found.
    fn expect_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> &'a Self::Info {
        self.get_info(stage).unwrap_or_else(|| {
            panic!(
                "Expected to find info for ID {:?} in stage, but none was found.",
                self
            )
        })
    }
}

/// Mutable node lookup, the write half of [`GetInfo`].
///
/// Crate-private: reaching a node's info mutably bypasses every guarantee the
/// mutation layer makes given that there is no maintenance of the derived mirrors.
pub(crate) trait GetInfoMut<L: Dialect>: GetInfo<L> {
    /// Get a mutable reference to the context info for the given node pointer.
    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info>;
}

#[macro_export(local_inner_macros)]
macro_rules! identifier {
    ($(#[$attr:meta])* struct $name:ident) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Hash, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(pub(crate) Id);

        // Name(id) instead of the default Name(Id(id))
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::write!(f, "{}({})", std::stringify!($name), self.0.raw())
            }
        }

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

        impl $crate::arena::Identifier for $name {}
    };
}
