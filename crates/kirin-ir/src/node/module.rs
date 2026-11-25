use rustc_hash::FxHashMap;

use crate::arena::{Arena, GetInfo, GetInfoMut, Id, Item};
use crate::{Dialect, Symbol, identifier};

use super::block::StatementIter;
use super::linked_list::LinkedList;
use super::stmt::Statement;

identifier! {
    /// A unique identifier for a module: a non-callable container of
    /// top-level statements within one stage.
    struct Module
}

/// The arena record for a [`Module`].
///
/// A module owns an ordered list of member statements, such as function
/// definitions. Each member has the module as its `parent` and is chained to
/// its neighbours through `prev`/`next`, the same way a block chains its
/// non-terminator statements. A module is not a statement and is never
/// executed; specialization records keep pointing at the member statements.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModuleInfo {
    pub(crate) id: Module,
    /// A stage-local name, unique among the stage's modules.
    pub(crate) name: Option<Symbol>,
    pub(crate) members: LinkedList<Statement>,
}

impl ModuleInfo {
    pub fn id(&self) -> Module {
        self.id
    }

    pub fn name(&self) -> Option<Symbol> {
        self.name
    }

    /// Summary of the member chain: first member, last member, and count.
    pub fn members(&self) -> &LinkedList<Statement> {
        &self.members
    }
}

/// A stage's modules, indexed by name.
///
/// [`alloc`](Self::alloc) is the only way to add a module, and it keeps
/// `by_name` in step with the arena, so a name lookup or duplicate check is
/// O(1). Nothing changes a module's `name` after allocation. Only the arena
/// is serialized; deserialization rebuilds the index.
#[derive(Clone, Debug, Default)]
pub(crate) struct Modules {
    arena: Arena<Module, ModuleInfo>,
    by_name: FxHashMap<Symbol, Module>,
}

impl Modules {
    pub(crate) fn arena(&self) -> &Arena<Module, ModuleInfo> {
        &self.arena
    }

    pub(crate) fn get_mut(&mut self, module: Module) -> Option<&mut Item<ModuleInfo>> {
        self.arena.get_mut(module)
    }

    pub(crate) fn by_name(&self, name: Symbol) -> Option<Module> {
        self.by_name.get(&name).copied()
    }

    /// Allocate an empty module, or return `Err(name)` if the stage already
    /// has a module with that name.
    pub(crate) fn alloc(&mut self, name: Option<Symbol>) -> Result<Module, Symbol> {
        if let Some(name) = name
            && self.by_name.contains_key(&name)
        {
            return Err(name);
        }
        let id = self.arena.alloc_with_id(|id| ModuleInfo {
            id,
            name,
            members: LinkedList::new(),
        });
        if let Some(name) = name {
            self.by_name.insert(name, id);
        }
        Ok(id)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Modules {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.arena, serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Modules {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let arena = <Arena<Module, ModuleInfo> as serde::Deserialize>::deserialize(deserializer)?;
        let mut by_name = FxHashMap::default();
        for info in arena.iter() {
            if let Some(name) = info.name
                && by_name.insert(name, info.id).is_some()
            {
                return Err(serde::de::Error::custom("duplicate module name"));
            }
        }
        Ok(Self { arena, by_name })
    }
}

impl<L: Dialect> GetInfo<L> for Module {
    type Info = Item<ModuleInfo>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.modules.arena().get(*self)
    }
}

impl<L: Dialect> GetInfoMut<L> for Module {
    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.modules.get_mut(*self)
    }
}

impl Module {
    /// The module's member statements, in order.
    pub fn members<'a, L: Dialect>(&self, stage: &'a crate::StageInfo<L>) -> StatementIter<'a, L> {
        StatementIter::over(self.expect_info(stage).members, stage)
    }
}
