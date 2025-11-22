use crate::{language::Language, node::linked_list::LinkedListNode, query::Info};

use super::block::Block;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementId(pub(crate) usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementInfo<L: Language> {
    pub(crate) node: LinkedListNode<StatementId>,
    pub(crate) parent: Option<Block>,
    pub(crate) definition: L,
}

impl<'a, L: Language> From<&'a StatementInfo<L>> for &'a LinkedListNode<StatementId> {
    fn from(info: &'a StatementInfo<L>) -> Self {
        &info.node
    }
}

impl StatementId {
    pub fn id(&self) -> usize {
        self.0
    }
}

impl StatementId {
    pub fn results<'a, L: Language>(
        &self,
        arena: &'a crate::Arena<L>,
    ) -> <L as crate::HasResults<'a>>::Iter {
        self.expect_info(arena).definition.results()
    }

    pub fn arguments<'a, L: Language>(
        &self,
        arena: &'a crate::Arena<L>,
    ) -> <L as crate::HasArguments<'a>>::Iter {
        self.expect_info(arena).definition.arguments()
    }

    pub fn parent<'a, L: Language>(&self, arena: &'a crate::Arena<L>) -> &'a Option<Block> {
        &self.expect_info(arena).parent
    }

    pub fn next<'a, L: Language>(&self, arena: &'a crate::Arena<L>) -> &'a Option<StatementId> {
        &self.expect_info(arena).node.next
    }

    pub fn prev<'a, L: Language>(&self, arena: &'a crate::Arena<L>) -> &'a Option<StatementId> {
        &self.expect_info(arena).node.prev
    }

    pub fn definition<'a, L: Language>(&self, arena: &'a crate::Arena<L>) -> &'a L {
        &self.expect_info(arena).definition
    }
}
