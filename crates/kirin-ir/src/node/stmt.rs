use crate::arena::{GetInfo, Id, Item};
use crate::identifier;
use crate::{Dialect, node::linked_list::LinkedListNode};

use super::block::Block;

identifier! {
    /// An Id reference to statement in arena.
    struct Statement
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementInfo<L: Dialect> {
    pub(crate) node: LinkedListNode<Statement>,
    pub(crate) parent: Option<Block>,
    pub(crate) definition: L,
}

impl<L: Dialect> StatementInfo<L> {
    pub fn definition(&self) -> &L {
        &self.definition
    }
}

impl<'a, L: Dialect> From<&'a StatementInfo<L>> for &'a LinkedListNode<Statement> {
    fn from(info: &'a StatementInfo<L>) -> Self {
        &info.node
    }
}

impl Statement {
    pub fn id(&self) -> Id {
        self.0
    }
}

impl Statement {
    pub fn results<'a, L: Dialect>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> <L as crate::HasResults<'a>>::Iter {
        self.expect_info(stage).definition.results()
    }

    pub fn arguments<'a, L: Dialect>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> <L as crate::HasArguments<'a>>::Iter {
        self.expect_info(stage).definition.arguments()
    }

    pub fn regions<'a, L: Dialect>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> <L as crate::HasRegions<'a>>::Iter {
        self.expect_info(stage).definition.regions()
    }

    pub fn blocks<'a, L: Dialect>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> <L as crate::HasBlocks<'a>>::Iter {
        self.expect_info(stage).definition.blocks()
    }

    pub fn successors<'a, L: Dialect>(
        &self,
        stage: &'a crate::StageInfo<L>,
    ) -> <L as crate::HasSuccessors<'a>>::Iter {
        self.expect_info(stage).definition.successors()
    }

    pub fn parent<'a, L: Dialect>(&self, stage: &'a crate::StageInfo<L>) -> &'a Option<Block> {
        &self.expect_info(stage).parent
    }

    pub fn next<'a, L: Dialect>(&self, stage: &'a crate::StageInfo<L>) -> &'a Option<Statement> {
        &self.expect_info(stage).node.next
    }

    pub fn prev<'a, L: Dialect>(&self, stage: &'a crate::StageInfo<L>) -> &'a Option<Statement> {
        &self.expect_info(stage).node.prev
    }

    pub fn definition<'a, L: Dialect>(&self, stage: &'a crate::StageInfo<L>) -> &'a L {
        &self.expect_info(stage).definition
    }
}

impl<L: Dialect> GetInfo<L> for Statement {
    type Info = Item<StatementInfo<L>>;

    fn get_info<'a>(&self, stage: &'a crate::StageInfo<L>) -> Option<&'a Self::Info> {
        stage.statements.get(*self)
    }

    fn get_info_mut<'a>(&self, stage: &'a mut crate::StageInfo<L>) -> Option<&'a mut Self::Info> {
        stage.statements.get_mut(*self)
    }
}
