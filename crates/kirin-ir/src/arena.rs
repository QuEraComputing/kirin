use std::cell::RefCell;
use std::sync::Arc;

use crate::language::Language;
use crate::node::region::RegionInfo;
use crate::{InternTable, node::*};

#[derive(Debug)]
pub struct Arena<L: Language> {
    pub(crate) staged_functions: Vec<StagedFunctionInfo<L>>,
    pub(crate) regions: Vec<RegionInfo<L>>,
    pub(crate) blocks: Vec<BlockInfo<L>>,
    pub(crate) statements: Vec<StatementInfo<L>>,
    pub(crate) ssas: Vec<SSAInfo<L>>,
    pub(crate) symbols: Arc<RefCell<InternTable<String, Symbol>>>,
}

impl<L> Default for Arena<L>
where
    L: Language,
{
    fn default() -> Self {
        Self {
            staged_functions: Vec::new(),
            regions: Vec::new(),
            blocks: Vec::new(),
            statements: Vec::new(),
            ssas: Vec::new(),
            symbols: Arc::new(RefCell::new(InternTable::default())),
        }
    }
}

impl<L> Clone for Arena<L>
where
    L: Language,
    StatementInfo<L>: Clone,
    SSAInfo<L>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            staged_functions: self.staged_functions.clone(),
            regions: self.regions.clone(),
            blocks: self.blocks.clone(),
            statements: self.statements.clone(),
            ssas: self.ssas.clone(),
            symbols: self.symbols.clone(),
        }
    }
}

impl<L: Language> Arena<L> {
    pub fn new_statement_id(&self) -> StatementId {
        StatementId(self.statements.len())
    }
}

// #[bon::bon]
// impl<L: Language> Arena<L> {
//     pub fn link_statements(&mut self, ptrs: &[StatementId]) -> LinkedList<StatementId> {
//         for window in ptrs.windows(2) {
//             let current = window[0];
//             let next = window[1];
//             let current_stmt = self
//                 .get_statement_mut(current)
//                 .expect("Invalid StatementId in given arena");
//             if let Some(next) = current_stmt.node.next {
//                 let info = self
//                     .get_statement(next)
//                     .expect("Invalid StatementId in given arena");
//                 panic!("Statement already has a next node: {:?}", info.definition);
//             }
//             current_stmt.node.next = Some(next);

//             let next_stmt = self
//                 .get_statement_mut(next)
//                 .expect("Invalid StatementId in given arena");
//             if let Some(prev) = next_stmt.node.prev {
//                 let info = self
//                     .get_statement(prev)
//                     .expect("Invalid StatementId in given arena");
//                 panic!(
//                     "Statement already has a previous node: {:?}",
//                     info.definition
//                 );
//             }
//             next_stmt.node.prev = Some(current);
//         }
//         LinkedList {
//             head: ptrs.first().copied(),
//             tail: ptrs.last().copied(),
//             len: ptrs.len(),
//         }
//     }

//     pub fn link_blocks(&mut self, ptrs: &[Block]) -> LinkedList<Block> {
//         for window in ptrs.windows(2) {
//             let current = window[0];
//             let next = window[1];
//             let current_block = self
//                 .get_block_mut(current)
//                 .expect("Invalid Block in given arena");
//             if let Some(next) = current_block.node.next {
//                 let info = self.get_block(next).expect("Invalid Block in given arena");
//                 panic!("Block already has a next node: {:?}", info);
//             }
//             current_block.node.next = Some(next);

//             let next_block = self
//                 .get_block_mut(next)
//                 .expect("Invalid Block in given arena");
//             if let Some(prev) = next_block.node.prev {
//                 let info = self.get_block(prev).expect("Invalid Block in given arena");
//                 panic!("Block already has a previous node: {:?}", info);
//             }
//             next_block.node.prev = Some(current);
//         }
//         LinkedList {
//             head: ptrs.first().copied(),
//             tail: ptrs.last().copied(),
//             len: ptrs.len(),
//         }
//     }

//     #[builder(finish_fn = new)]
//     pub fn staged_function(
//         &mut self,
//         #[builder(into)] name: Option<String>,
//         params_type: Option<&[L::Type]>,
//         return_type: Option<L::Type>,
//         specializations: Option<Vec<SpecializedFunctionInfo<L>>>,
//         backedges: Option<Vec<StagedFunction>>,
//     ) -> StagedFunction {
//         let id = StagedFunction(self.staged_functions.len());
//         let staged_function = StagedFunctionInfo {
//             id,
//             name: name.map(|n| self.symbols.borrow_mut().intern(n)),
//             signature: params_type
//                 .map(|pts| Signature(pts.to_vec()))
//                 .unwrap_or(Signature(Vec::new())),
//             return_type: return_type.unwrap_or(L::Type::top()),
//             specializations: specializations.unwrap_or_default(),
//             backedges: backedges.unwrap_or_default(),
//         };
//         self.staged_functions.push(staged_function);
//         id
//     }

//     #[builder(finish_fn = new)]
//     pub fn specialize(
//         &mut self,
//         f: StagedFunction,
//         params_type: Option<&[L::Type]>,
//         return_type: Option<L::Type>,
//         body: StatementId,
//         backedges: Option<Vec<SpecializedFunction>>,
//     ) -> SpecializedFunction {
//         // the only way to create a staged function is through the arena
//         // and unless the whole arena is dropped, the staged function should exist
//         let staged_function_info = self
//             .get_staged_function_mut(f)
//             .expect("Staged function not found");
//         let id = SpecializedFunction(f.id(), staged_function_info.specializations.len());

//         let signature = Signature(
//             params_type
//                 .map(|pts| pts.to_vec())
//                 .unwrap_or(staged_function_info.signature.0.clone()),
//         );

//         if !signature.is_subseteq(&staged_function_info.signature) {
//             panic!(
//                 "Specialized function signature is not a subset of the staged function signature"
//             );
//         }

//         let specialized_function = SpecializedFunctionInfo::builder()
//             .id(id)
//             .signature(signature)
//             .return_type(return_type.unwrap_or(staged_function_info.return_type.clone()))
//             .body(body)
//             .maybe_backedges(backedges)
//             .new();
//         staged_function_info
//             .specializations
//             .push(specialized_function);
//         id
//     }

//     #[builder(finish_fn = new)]
//     pub fn region(
//         &mut self,
//         parent: Option<StatementId>,
//         blocks: Option<LinkedList<Block>>,
//     ) -> Region {
//         let id = Region(self.regions.len());
//         let region = RegionInfo::builder()
//             .id(id)
//             .blocks(blocks.unwrap_or_default())
//             .maybe_parent(parent)
//             .new();
//         self.regions.push(region);
//         id
//     }

//     pub fn block<'a>(
//         &'a mut self
//     ) -> BlockBuilder<'a, L> {
//         BlockBuilder {
//             arena: self,
//             parent: None,
//             arguments: Vec::new(),
//             statements: Vec::new(),
//             terminator: None,
//         }
//     }

//     #[builder(finish_fn = new)]
//     pub fn ssa(
//         &mut self,
//         #[builder(into)] name: Option<String>,
//         ty: L::Type,
//         kind: SSAKind,
//     ) -> SSAValue {
//         let id = SSAValue(self.ssas.len());
//         let ssa = SSAInfo::new(
//             id.into(),
//             name.map(|n| self.symbols.borrow_mut().intern(n)),
//             ty,
//             kind,
//         );
//         self.ssas.push(ssa);
//         id
//     }

//     #[builder(finish_fn = new)]
//     pub fn statement(&mut self, #[builder(into)] definition: L) -> StatementId {
//         let id = StatementId(self.statements.len());
//         let statement = StatementInfo {
//             node: LinkedListNode::new(id),
//             parent: None,
//             definition,
//         };
//         self.statements.push(statement);
//         id
//     }
// }

// pub struct BlockBuilder<'a, L: Language> {
//     arena: &'a mut Arena<L>,
//     parent: Option<Region>,
//     arguments: Vec<(L::Type, Option<String>)>,
//     statements: Vec<StatementId>,
//     terminator: Option<StatementId>,
// }

// impl<L: Language> BlockBuilder<'_, L> {
//     pub fn parent(mut self, parent: Region) -> Self {
//         self.parent = Some(parent);
//         self
//     }

//     pub fn argument<T: Into<L::Type>>(mut self, ty: T) -> Self {
//         self.arguments.push((ty.into(), None));
//         self
//     }

//     pub fn argument_with_name<T: Into<L::Type>, S: Into<String>>(mut self, name: S, ty: T) -> Self {
//         self.arguments.push((ty.into(), Some(name.into())));
//         self
//     }

//     pub fn stmt(mut self, stmt: StatementId) -> Self {
//         self.statements.push(stmt);
//         self
//     }

//     pub fn terminator(mut self, term: StatementId) -> Self {
//         self.terminator = Some(term);
//         self
//     }

//     pub fn new(self) -> Block {
//         let id = Block(self.arena.blocks.len());
//         let args = self
//             .arguments
//             .into_iter()
//             .map(|(ty, name)| {
//                 let arg = BlockArgument(self.arena.ssas.len());
//                 let ssa = SSAInfo::new(
//                     arg.into(),
//                     name.map(|n| self.arena.symbols.borrow_mut().intern(n)),
//                     ty,
//                     SSAKind::BlockArgument(id),
//                 );
//                 self.arena.ssas.push(ssa);
//                 arg
//             })
//             .collect::<Vec<_>>();

//         let block = BlockInfo::builder()
//             .maybe_parent(self.parent)
//             .node(LinkedListNode::new(id))
//             .arguments(args)
//             .statements(self.arena.link_statements(&self.statements))
//             .maybe_terminator(self.terminator)
//             .new();
//         self.arena.blocks.push(block);
//         id
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::tests::{self, SimpleLanguage};

//     #[test]
//     fn test_add_staged_function() {
//         let mut arena: Arena<SimpleLanguage> = Arena::default();
//         let staged_function = arena
//             .staged_function()
//             .name("foo")
//             .params_type(&[tests::Int])
//             .return_type(tests::Int)
//             .new();

//         let a = SimpleLanguage::constant_f64(&mut arena, 1.2);
//         let b = SimpleLanguage::constant_f64(&mut arena, 3.4);
//         let c = SimpleLanguage::add(&mut arena, a.result_0, b.result_0);

//         let region = arena
//             .region()
//             .block()
//             .argument(tests::Int)
//             .argument_with_name("arg1", tests::Int)
//             .stmt(c.id)
//             .terminator(d)
//             .block()
//             .new();

//         let block = arena
//             .block()
//             .argument(ty)
//             .stmt(a.id)
//             .stmt(b.id)
//             .stmt(c.id)
//             .new();

//         // let const_stmt = arena
//         //     .statement()
//         //     .parent(block)
//         //     .from_id(|arena, id| {
//         //         let result = arena
//         //             .result_value()
//         //             .parent(id)
//         //             .name("const_2")
//         //             .ty(tests::Type::Float)
//         //             .new();
//         //         tests::SimpleLanguage::ConstantF64(2.0, result)
//         //     })
//         //     .new();
//         // let fetched = arena.get_staged_function(staged_function).unwrap();
//         // println!("{:?}", fetched);
//     }
// }
