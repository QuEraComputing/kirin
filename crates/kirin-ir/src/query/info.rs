use crate::{
    Context, Language, LinkedList, SSAInfo, SSAValue,
    node::{
        Block, BlockInfo, LinkedListNode, Region, RegionInfo, SpecializedFunction,
        SpecializedFunctionInfo, StagedFunction, StagedFunctionInfo, StatementId, StatementInfo,
    },
};

pub trait ParentInfo<L: Language> {
    type ParentPtr: Copy + PartialEq;
    /// Get a reference to the parent pointer.
    fn get_parent(&self) -> &Option<Self::ParentPtr>;
    /// Get a mutable reference to the parent pointer.
    fn get_parent_mut(&mut self) -> &mut Option<Self::ParentPtr>;
}

impl<L: Language> ParentInfo<L> for StatementInfo<L> {
    type ParentPtr = Block;
    fn get_parent(&self) -> &Option<Self::ParentPtr> {
        &self.parent
    }

    fn get_parent_mut(&mut self) -> &mut Option<Self::ParentPtr> {
        &mut self.parent
    }
}

impl<L: Language> ParentInfo<L> for BlockInfo<L> {
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

impl<L: Language> LinkedListInfo for BlockInfo<L> {
    type Ptr = StatementId;
    fn get_linked_list(&self) -> &LinkedList<Self::Ptr> {
        &self.statements
    }

    fn get_linked_list_mut(&mut self) -> &mut LinkedList<Self::Ptr> {
        &mut self.statements
    }
}

impl<L: Language> LinkedListInfo for RegionInfo<L> {
    type Ptr = Block;
    fn get_linked_list(&self) -> &LinkedList<Self::Ptr> {
        &self.blocks
    }

    fn get_linked_list_mut(&mut self) -> &mut LinkedList<Self::Ptr> {
        &mut self.blocks
    }
}

pub trait LinkedListElem<L: Language> {
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

impl<L: Language> LinkedListElem<L> for StatementInfo<L> {
    type Ptr = StatementId;
    fn get_node(&self) -> &LinkedListNode<Self::Ptr> {
        &self.node
    }

    fn get_node_mut(&mut self) -> &mut LinkedListNode<Self::Ptr> {
        &mut self.node
    }
}

impl<L: Language> LinkedListElem<L> for BlockInfo<L> {
    type Ptr = Block;
    fn get_node(&self) -> &LinkedListNode<Self::Ptr> {
        &self.node
    }

    fn get_node_mut(&mut self) -> &mut LinkedListNode<Self::Ptr> {
        &mut self.node
    }
}

pub trait Info<L: Language> {
    type InfoType;
    /// Get a reference to the context info for the given node pointer.
    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType>;
    /// Get a mutable reference to the context info for the given node pointer.
    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType>;
    /// Get a reference to the context info for the given node pointer, panicking if not found.
    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType;
    /// Get a mutable reference to the context info for the given node pointer, panicking if not found.
    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType;
}

impl<T, L: Language> Info<L> for T
where
    T: Into<SSAValue> + Copy,
{
    type InfoType = SSAInfo<L>;
    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType> {
        let ssa_value: SSAValue = (*self).into();
        context.ssas.get(ssa_value.id())
    }
    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType> {
        let ssa_value: SSAValue = (*self).into();
        context.ssas.get_mut(ssa_value.id())
    }
    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType {
        let ssa_value: SSAValue = (*self).into();
        self.get_info(context)
            .expect(format!("SSAInfo not found for SSAValue id {}", ssa_value.id()).as_str())
    }
    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType {
        let ssa_value: SSAValue = (*self).into();
        self.get_info_mut(context)
            .expect(format!("SSAInfo not found for SSAValue id {}", ssa_value.id()).as_str())
    }
}

impl<L: Language> Info<L> for StatementId {
    type InfoType = StatementInfo<L>;

    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType> {
        context.statements.get(self.id())
    }

    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType> {
        context.statements.get_mut(self.id())
    }

    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType {
        self.get_info(context)
            .expect(format!("StatementInfo not found for Statement id {}", self.id()).as_str())
    }

    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType {
        self.get_info_mut(context)
            .expect(format!("StatementInfo not found for Statement id {}", self.id()).as_str())
    }
}

impl<L: Language> Info<L> for Block {
    type InfoType = BlockInfo<L>;

    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType> {
        context.blocks.get(self.id())
    }

    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType> {
        context.blocks.get_mut(self.id())
    }

    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType {
        self.get_info(context)
            .expect(format!("BlockInfo not found for Block id {}", self.id()).as_str())
    }

    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType {
        self.get_info_mut(context)
            .expect(format!("BlockInfo not found for Block id {}", self.id()).as_str())
    }
}

impl<L: Language> Info<L> for Region {
    type InfoType = RegionInfo<L>;

    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType> {
        context.regions.get(self.id())
    }

    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType> {
        context.regions.get_mut(self.id())
    }

    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType {
        self.get_info(context)
            .expect(format!("RegionInfo not found for Region id {}", self.id()).as_str())
    }

    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType {
        self.get_info_mut(context)
            .expect(format!("RegionInfo not found for Region id {}", self.id()).as_str())
    }
}

impl<L: Language> Info<L> for StagedFunction {
    type InfoType = StagedFunctionInfo<L>;

    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType> {
        context.staged_functions.get(self.id())
    }

    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType> {
        context.staged_functions.get_mut(self.id())
    }

    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType {
        self.get_info(context).expect(
            format!(
                "StagedFunctionInfo not found for StagedFunction id {}",
                self.id()
            )
            .as_str(),
        )
    }

    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType {
        self.get_info_mut(context).expect(
            format!(
                "StagedFunctionInfo not found for StagedFunction id {}",
                self.id()
            )
            .as_str(),
        )
    }
}

impl<L: Language> Info<L> for SpecializedFunction {
    type InfoType = SpecializedFunctionInfo<L>;

    fn get_info<'a>(&self, context: &'a Context<L>) -> Option<&'a Self::InfoType> {
        let (staged_fn, spec_idx) = self.id();
        staged_fn
            .get_info(context)
            .and_then(|f| f.specializations().get(spec_idx))
    }

    fn get_info_mut<'a>(&self, context: &'a mut Context<L>) -> Option<&'a mut Self::InfoType> {
        let (staged_fn, spec_idx) = self.id();
        staged_fn
            .get_info_mut(context)
            .and_then(|f| f.specializations_mut().get_mut(spec_idx))
    }

    fn expect_info<'a>(&self, context: &'a Context<L>) -> &'a Self::InfoType {
        self.get_info(context).expect(
            format!(
                "SpecializedFunctionInfo not found for SpecializedFunction id {:?}",
                self.id()
            )
            .as_str(),
        )
    }

    fn expect_info_mut<'a>(&self, context: &'a mut Context<L>) -> &'a mut Self::InfoType {
        self.get_info_mut(context).expect(
            format!(
                "SpecializedFunctionInfo not found for SpecializedFunction id {:?}",
                self.id()
            )
            .as_str(),
        )
    }
}
