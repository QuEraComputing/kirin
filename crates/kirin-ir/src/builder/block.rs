use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind, ResolutionInfo, SSAValue};
use crate::node::stmt::StatementParent;
use crate::node::*;
use crate::{BuilderStageInfo, Dialect};

pub struct BlockBuilder<'a, L: Dialect> {
    stage: &'a mut BuilderStageInfo<L>,
    parent: Option<Region>,
    name: Option<String>,
    arguments: Vec<(L::Type, Option<String>)>,
    statements: Vec<Statement>,
    terminator: Option<Statement>,
}

impl<'a, L: Dialect> BlockBuilder<'a, L> {
    pub(crate) fn from_stage(stage: &'a mut BuilderStageInfo<L>) -> Self {
        BlockBuilder {
            stage,
            parent: None,
            name: None,
            arguments: Vec::new(),
            statements: Vec::new(),
            terminator: None,
        }
    }

    /// Attach the block to a parent region without pushing it to the region's block list.
    pub fn parent(mut self, parent: Region) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Set the name of this block (e.g., "entry", "loop_body").
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add an argument to the block.
    pub fn argument<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.arguments.push((ty.into(), None));
        self
    }

    /// Name the most recently added argument.
    pub fn arg_name<S: Into<String>>(mut self, name: S) -> Self {
        debug_assert!(
            !self.arguments.is_empty(),
            "arg_name called without a preceding argument()"
        );
        if let Some(last) = self.arguments.last_mut() {
            last.1 = Some(name.into());
        }
        self
    }

    /// Add an argument with a name to the block.
    #[deprecated(note = "use `.argument(ty).arg_name(name)` instead")]
    pub fn argument_with_name<T: Into<L::Type>, S: Into<String>>(mut self, name: S, ty: T) -> Self {
        self.arguments.push((ty.into(), Some(name.into())));
        self
    }

    /// Add a statement to the block.
    pub fn stmt(mut self, stmt: impl Into<Statement>) -> Self {
        let stmt = stmt.into();
        let info = &self.stage.statements[stmt];

        assert!(
            !info.definition.is_terminator(),
            "Cannot add terminator statement {:?} as a regular statement in block",
            info.definition
        );
        self.statements.push(stmt);
        self
    }

    /// Set the terminator statement of the block.
    pub fn terminator(mut self, term: impl Into<Statement>) -> Self {
        let term = term.into();
        let info = &self.stage.statements[term];

        assert!(
            info.definition.is_terminator(),
            "Statement {:?} is not a terminator and cannot be set as block terminator",
            info.definition
        );
        self.terminator = Some(term);
        self
    }

    /// Finalize the block and add it to the context.
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    pub fn new(self) -> Block {
        let id = self.stage.blocks.next_id();
        let block_args = self
            .arguments
            .into_iter()
            .enumerate()
            .map(|(index, (ty, name))| {
                let arg: BlockArgument = self.stage.ssas.next_id().into();
                let ssa = BuilderSSAInfo::new(
                    arg.into(),
                    name.map(|n| self.stage.symbols.intern(n)),
                    Some(ty),
                    BuilderSSAKind::BlockArgument(id, index),
                );
                self.stage.ssas.alloc(ssa);
                arg
            })
            .collect::<Vec<_>>();

        // Build name→index map for named block argument lookup
        let arg_name_to_index: std::collections::HashMap<crate::Symbol, usize> = block_args
            .iter()
            .enumerate()
            .filter_map(|(i, arg)| {
                let info = self.stage.ssas.get(SSAValue::from(*arg))?;
                info.name().map(|sym| (sym, i))
            })
            .collect();

        // Build replacement map: placeholder SSA -> resolved block arg SSA
        let mut replacements: std::collections::HashMap<SSAValue, SSAValue> =
            std::collections::HashMap::new();

        // Collect all statement IDs to scan (regular + terminator)
        let all_stmt_ids: Vec<Statement> = self
            .statements
            .iter()
            .copied()
            .chain(self.terminator)
            .collect();

        for &stmt_id in &all_stmt_ids {
            let info = &self.stage.statements[stmt_id];
            for arg in info.definition.arguments() {
                if replacements.contains_key(arg) {
                    continue;
                }
                let ssa_info = self
                    .stage
                    .ssas
                    .get(*arg)
                    .expect("SSAValue not found in stage");
                if let BuilderSSAKind::Unresolved(ResolutionInfo::BlockArgument(key)) =
                    ssa_info.kind
                {
                    let index = super::resolve_builder_key(
                        key,
                        block_args.len(),
                        &arg_name_to_index,
                        &self.stage.symbols,
                        "block argument",
                    );
                    replacements.insert(*arg, block_args[index].into());
                }
            }
        }
        // Apply replacements and delete placeholder SSAs
        for &old in replacements.keys() {
            self.stage.ssas.delete(old);
        }
        for &stmt_id in &self.statements {
            let info = &mut self.stage.statements[stmt_id];
            info.parent = Some(StatementParent::Block(id));
            for arg in info.definition.arguments_mut() {
                if let Some(&replacement) = replacements.get(arg) {
                    *arg = replacement;
                }
            }
        }

        if let Some(term_id) = self.terminator {
            let info = &mut self.stage.statements[term_id];
            info.parent = Some(StatementParent::Block(id));
            for arg in info.definition.arguments_mut() {
                if let Some(&replacement) = replacements.get(arg) {
                    *arg = replacement;
                }
            }
        }

        let block = BlockInfo::new(
            self.parent,
            self.name.map(|n| self.stage.symbols.intern(n)),
            LinkedListNode::new(id),
            block_args,
            Some(self.stage.link_statements(&self.statements)),
            self.terminator,
        );
        self.stage.blocks.alloc(block);
        id
    }
}
