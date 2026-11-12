use crate::arena::{GetInfo, GetInfoMut, Id, Item};
use crate::{Dialect, identifier};

use super::block::StatementIter;
use super::linked_list::LinkedList;
use super::stmt::Statement;
use super::symbol::GlobalSymbol;

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
    /// A pipeline-global name, so the same name refers to this module from
    /// any stage. Unique among the stage's modules.
    pub(crate) name: Option<GlobalSymbol>,
    pub(crate) members: LinkedList<Statement>,
}

impl ModuleInfo {
    pub fn id(&self) -> Module {
        self.id
    }

    pub fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    /// Summary of the member chain: first member, last member, and count.
    pub fn members(&self) -> &LinkedList<Statement> {
        &self.members
    }
}

impl<L: Dialect> GetInfo<L> for Module {
    type Info = Item<ModuleInfo>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.modules.get(*self)
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
