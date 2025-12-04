use super::block::BlockBuilder;
use super::region::RegionBuilder;

use crate::arena::GetInfo;
use crate::lattice::{FiniteLattice, Lattice};
use crate::node::*;
use crate::{Context, Dialect};

impl<L: Dialect> Context<L> {
    pub fn block(&mut self) -> BlockBuilder<L> {
        BlockBuilder::from_context(self)
    }

    pub fn region(&mut self) -> RegionBuilder<L> {
        RegionBuilder::from_context(self)
    }

    pub fn link_statements(&mut self, ptrs: &[Statement]) -> LinkedList<Statement> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_stmt = current.expect_info_mut(self);
            if let Some(next) = current_stmt.node.next {
                let info = next.expect_info(self);
                panic!("Statement already has a next node: {:?}", info.definition);
            }
            current_stmt.node.next = Some(next);

            let next_stmt = next.expect_info_mut(self);
            if let Some(prev) = next_stmt.node.prev {
                let info = prev.expect_info(self);
                panic!(
                    "Statement already has a previous node: {:?}",
                    info.definition
                );
            }
            next_stmt.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }

    pub fn link_blocks(&mut self, ptrs: &[Block]) -> LinkedList<Block> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_block = current.expect_info_mut(self);
            if let Some(next) = current_block.node.next {
                let info = next.expect_info(self);
                panic!("Block already has a next node: {:?}", info);
            }
            current_block.node.next = Some(next);

            let next_block = next.expect_info_mut(self);
            if let Some(prev) = next_block.node.prev {
                let info = prev.expect_info(self);
                panic!("Block already has a previous node: {:?}", info);
            }
            next_block.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }
}

#[bon::bon]
impl<L: Dialect> Context<L> {
    #[builder(finish_fn = new)]
    pub fn ssa(
        &mut self,
        #[builder(into)] name: Option<String>,
        ty: L::TypeLattice,
        kind: SSAKind,
    ) -> SSAValue {
        let id = self.ssas.next_id();
        let ssa = SSAInfo::new(
            id.into(),
            name.map(|n| self.symbols.borrow_mut().intern(n)),
            ty,
            kind,
        );
        self.ssas.alloc(ssa);
        id
    }

    /// create a placeholder block argument SSAValue
    pub fn block_argument(&mut self, index: usize) -> BlockArgument {
        let id: BlockArgument = self.ssas.next_id().into();
        let ssa = SSAInfo::new(
            id.into(),
            None,
            L::TypeLattice::top(),
            SSAKind::BuilderBlockArgument(index),
        );
        self.ssas.alloc(ssa);
        id
    }

    #[builder(finish_fn = new)]
    pub fn statement(&mut self, #[builder(into)] definition: L) -> Statement {
        let id = self.statements.next_id();
        let statement = StatementInfo {
            node: LinkedListNode::new(id),
            parent: None,
            definition,
        };
        self.statements.alloc(statement);
        id
    }

    #[builder(finish_fn = new)]
    pub fn staged_function(
        &mut self,
        #[builder(into)] name: Option<String>,
        params_type: Option<&[L::TypeLattice]>,
        return_type: Option<L::TypeLattice>,
        specializations: Option<Vec<SpecializedFunctionInfo<L>>>,
        backedges: Option<Vec<StagedFunction>>,
    ) -> StagedFunction {
        let id = self.staged_functions.next_id();
        let staged_function = StagedFunctionInfo {
            id,
            name: name.map(|n| self.symbols.borrow_mut().intern(n)),
            signature: params_type
                .map(|pts| Signature(pts.to_vec()))
                .unwrap_or(Signature(Vec::new())),
            return_type: return_type.unwrap_or(L::TypeLattice::top()),
            specializations: specializations.unwrap_or_default(),
            backedges: backedges.unwrap_or_default(),
        };
        self.staged_functions.alloc(staged_function);
        id
    }

    #[builder(finish_fn = new)]
    pub fn specialize(
        &mut self,
        f: StagedFunction,
        params_type: Option<&[L::TypeLattice]>,
        return_type: Option<L::TypeLattice>,
        #[builder(into)] body: Statement,
        backedges: Option<Vec<SpecializedFunction>>,
    ) -> SpecializedFunction {
        // the only way to create a staged function is through the context
        // and unless the whole context is dropped, the staged function should exist
        let staged_function_info = f.expect_info_mut(self);
        let id = SpecializedFunction(f, staged_function_info.specializations.len());

        let signature = Signature(
            params_type
                .map(|pts| pts.to_vec())
                .unwrap_or(staged_function_info.signature.0.clone()),
        );

        if !signature.is_subseteq(&staged_function_info.signature) {
            panic!(
                "Specialized function signature is not a subset of the staged function signature"
            );
        }

        let specialized_function = SpecializedFunctionInfo::builder()
            .id(id)
            .signature(signature)
            .return_type(return_type.unwrap_or(staged_function_info.return_type.clone()))
            .body(body)
            .maybe_backedges(backedges)
            .new();
        staged_function_info
            .specializations
            .push(specialized_function);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{self, SimpleLanguage};

    #[test]
    fn test_block() {
        let mut context: Context<SimpleLanguage> = Context::default();
        let staged_function = context
            .staged_function()
            .name("foo")
            .params_type(&[tests::Int])
            .return_type(tests::Int)
            .new();

        let a = SimpleLanguage::op_constant(&mut context, 1.2);
        let b = SimpleLanguage::op_constant(&mut context, 3.4);
        let c = SimpleLanguage::op_add(&mut context, a.result, b.result);
        let block_arg_x = context.block_argument(0);
        let d = SimpleLanguage::op_add(&mut context, c.result, block_arg_x);
        let ret = SimpleLanguage::op_return(&mut context, d.result);

        let block = context
            .block()
            .argument(tests::Int)
            .argument_with_name("y", tests::Float)
            .stmt(a)
            .stmt(b)
            .stmt(c)
            .stmt(d)
            .terminator(ret)
            .new();

        let body = context.region().add_block(block).new();
        let fdef = SimpleLanguage::op_function(&mut context, body);
        context.specialize().f(staged_function).body(fdef).new();

        println!("Context: {:?}", context);
    }
}
