use crate::arena::{GetInfo, Id, Item};
use crate::identifier;
use crate::{Dialect, node::linked_list::LinkedListNode};

use super::block::Block;

identifier! {
    /// An Id reference to statement in arena.
    struct StatementId
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatementInfo<L: Dialect> {
    pub(crate) node: LinkedListNode<StatementId>,
    pub(crate) parent: Option<Block>,
    pub(crate) definition: L,
}

impl<'a, L: Dialect> From<&'a StatementInfo<L>> for &'a LinkedListNode<StatementId> {
    fn from(info: &'a StatementInfo<L>) -> Self {
        &info.node
    }
}

impl StatementId {
    pub fn id(&self) -> Id {
        self.0
    }
}

impl StatementId {
    pub fn results<'a, L: Dialect>(
        &self,
        context: &'a crate::Context<L>,
    ) -> <L as crate::HasResults<'a>>::Iter {
        self.expect_info(context).definition.results()
    }

    pub fn arguments<'a, L: Dialect>(
        &self,
        context: &'a crate::Context<L>,
    ) -> <L as crate::HasArguments<'a>>::Iter {
        self.expect_info(context).definition.arguments()
    }

    pub fn parent<'a, L: Dialect>(&self, context: &'a crate::Context<L>) -> &'a Option<Block> {
        &self.expect_info(context).parent
    }

    pub fn next<'a, L: Dialect>(&self, context: &'a crate::Context<L>) -> &'a Option<StatementId> {
        &self.expect_info(context).node.next
    }

    pub fn prev<'a, L: Dialect>(&self, context: &'a crate::Context<L>) -> &'a Option<StatementId> {
        &self.expect_info(context).node.prev
    }

    pub fn definition<'a, L: Dialect>(&self, context: &'a crate::Context<L>) -> &'a L {
        &self.expect_info(context).definition
    }
}

impl<L: Dialect> GetInfo<L> for StatementId {
    type Info = Item<StatementInfo<L>>;

    fn get_info<'a>(&self, context: &'a crate::Context<L>) -> Option<&'a Self::Info> {
        context.statements.get(*self)
    }

    fn get_info_mut<'a>(
            &self,
            context: &'a mut crate::Context<L>,
        ) -> Option<&'a mut Self::Info> {
        context.statements.get_mut(*self)
    }
}
