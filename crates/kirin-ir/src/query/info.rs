use crate::{
    Dialect, LinkedList,
    node::{Block, BlockInfo, LinkedListNode, Region, RegionInfo, StatementId, StatementInfo},
};

pub trait ParentInfo<L: Dialect> {
    type ParentPtr: Copy + PartialEq;
    /// Get a reference to the parent pointer.
    fn get_parent(&self) -> &Option<Self::ParentPtr>;
    /// Get a mutable reference to the parent pointer.
    fn get_parent_mut(&mut self) -> &mut Option<Self::ParentPtr>;
}

impl<L: Dialect> ParentInfo<L> for StatementInfo<L> {
    type ParentPtr = Block;
    fn get_parent(&self) -> &Option<Self::ParentPtr> {
        &self.parent
    }

    fn get_parent_mut(&mut self) -> &mut Option<Self::ParentPtr> {
        &mut self.parent
    }
}

impl<L: Dialect> ParentInfo<L> for BlockInfo<L> {
    type ParentPtr = Region;
    fn get_parent(&self) -> &Option<Self::ParentPtr> {
        &self.parent
    }

    fn get_parent_mut(&mut self) -> &mut Option<Self::ParentPtr> {
        &mut self.parent
    }
}

pub trait LinkedListInfo {
    type Ptr: Copy + PartialEq;
    /// Get a reference to the linked list.
    fn get_linked_list(&self) -> &LinkedList<Self::Ptr>;
    /// Get a mutable reference to the linked list.
    fn get_linked_list_mut(&mut self) -> &mut LinkedList<Self::Ptr>;
    /// Get a reference to the head pointer.
    fn get_head(&self) -> &Option<Self::Ptr> {
        &self.get_linked_list().head
    }
    /// Get a mutable reference to the head pointer.
    fn get_head_mut(&mut self) -> &mut Option<Self::Ptr> {
        &mut self.get_linked_list_mut().head
    }
    /// Get a reference to the tail pointer.
    fn get_tail(&self) -> &Option<Self::Ptr> {
        &self.get_linked_list().tail
    }
    /// Get a mutable reference to the tail pointer.
    fn get_tail_mut(&mut self) -> &mut Option<Self::Ptr> {
        &mut self.get_linked_list_mut().tail
    }
    /// Get the length of the linked list.
    fn get_len(&self) -> usize {
        self.get_linked_list().len
    }
    /// Get a mutable reference to the length of the linked list.
    fn get_len_mut<'a>(&'a mut self) -> &'a mut usize
    where
        <Self as LinkedListInfo>::Ptr: 'a,
    {
        &mut self.get_linked_list_mut().len
    }
}

impl<L: Dialect> LinkedListInfo for BlockInfo<L> {
    type Ptr = StatementId;
    fn get_linked_list(&self) -> &LinkedList<Self::Ptr> {
        &self.statements
    }

    fn get_linked_list_mut(&mut self) -> &mut LinkedList<Self::Ptr> {
        &mut self.statements
    }
}

impl<L: Dialect> LinkedListInfo for RegionInfo<L> {
    type Ptr = Block;
    fn get_linked_list(&self) -> &LinkedList<Self::Ptr> {
        &self.blocks
    }

    fn get_linked_list_mut(&mut self) -> &mut LinkedList<Self::Ptr> {
        &mut self.blocks
    }
}

pub trait LinkedListElem<L: Dialect> {
    type Ptr: Copy + PartialEq;
    /// Get a reference to the linked list node.
    fn get_node(&self) -> &LinkedListNode<Self::Ptr>;
    /// Get a mutable reference to the linked list node.
    fn get_node_mut(&mut self) -> &mut LinkedListNode<Self::Ptr>;

    /// Get a reference to the previous pointer.
    fn get_prev(&self) -> &Option<Self::Ptr> {
        &self.get_node().prev
    }
    /// Get a mutable reference to the previous pointer.
    fn get_prev_mut(&mut self) -> &mut Option<Self::Ptr> {
        &mut self.get_node_mut().prev
    }
    /// Get a reference to the next pointer.
    fn get_next(&self) -> &Option<Self::Ptr> {
        &self.get_node().next
    }
    /// Get a mutable reference to the next pointer.
    fn get_next_mut(&mut self) -> &mut Option<Self::Ptr> {
        &mut self.get_node_mut().next
    }
}

impl<L: Dialect> LinkedListElem<L> for StatementInfo<L> {
    type Ptr = StatementId;
    fn get_node(&self) -> &LinkedListNode<Self::Ptr> {
        &self.node
    }

    fn get_node_mut(&mut self) -> &mut LinkedListNode<Self::Ptr> {
        &mut self.node
    }
}

impl<L: Dialect> LinkedListElem<L> for BlockInfo<L> {
    type Ptr = Block;
    fn get_node(&self) -> &LinkedListNode<Self::Ptr> {
        &self.node
    }

    fn get_node_mut(&mut self) -> &mut LinkedListNode<Self::Ptr> {
        &mut self.node
    }
}
