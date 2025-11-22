use crate::node::*;
use crate::query::Info;
use crate::{Arena, Language};

pub struct BlockBuilder<'a, L: Language> {
    arena: &'a mut Arena<L>,
    parent: Option<Region>,
    arguments: Vec<(L::Type, Option<String>)>,
    statements: Vec<StatementId>,
    terminator: Option<StatementId>,
}

impl<'a, L: Language> BlockBuilder<'a, L> {
    pub(crate) fn from_arena(arena: &'a mut Arena<L>) -> Self {
        BlockBuilder {
            arena,
            parent: None,
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

    /// Add an argument to the block.
    pub fn argument<T: Into<L::Type>>(mut self, ty: T) -> Self {
        self.arguments.push((ty.into(), None));
        self
    }

    /// Add an argument with a name to the block.
    pub fn argument_with_name<T: Into<L::Type>, S: Into<String>>(mut self, name: S, ty: T) -> Self {
        self.arguments.push((ty.into(), Some(name.into())));
        self
    }

    /// Add a statement to the block.
    pub fn stmt(mut self, stmt: impl Into<StatementId>) -> Self {
        let stmt = stmt.into();
        let info = stmt.expect_info(self.arena);

        info.definition.is_terminator().then(|| {
            panic!(
                "Cannot add terminator statement {:?} as a regular statement in block",
                info.definition
            )
        });
        self.statements.push(stmt);
        self
    }

    /// Set the terminator statement of the block.
    pub fn terminator(mut self, term: impl Into<StatementId>) -> Self {
        let term = term.into();
        let info = term.expect_info(self.arena);

        let _ = info.definition.is_terminator() || {
            panic!(
                "Statement {:?} is not a terminator and cannot be set as block terminator",
                info.definition
            )
        };
        self.terminator = Some(term.into());
        self
    }

    /// Finalize the block and add it to the arena.
    pub fn new(self) -> Block {
        let id = Block(self.arena.blocks.len());
        let args = self
            .arguments
            .into_iter()
            .enumerate()
            .map(|(index, (ty, name))| {
                let arg = BlockArgument(self.arena.ssas.len());
                let ssa = SSAInfo::new(
                    arg.into(),
                    name.map(|n| self.arena.symbols.borrow_mut().intern(n)),
                    ty,
                    SSAKind::BlockArgument(id, index),
                );
                self.arena.ssas.push(ssa);
                arg
            })
            .collect::<Vec<_>>();

        for &stmt_id in &self.statements {
            let info = &mut self.arena.statements[stmt_id.0];
            for arg in info.definition.arguments_mut() {
                let ssa_info = self
                    .arena
                    .ssas
                    .get_mut(arg.0)
                    .expect(format!("undefined SSAValue {}", arg.0).as_str());

                if let SSAKind::BuilderBlockArgument(arg_index) = ssa_info.kind {
                    arg.0 = args[arg_index].0;
                    ssa_info.kind = SSAKind::Deleted;
                }
            }
        }

        let block = BlockInfo::builder()
            .maybe_parent(self.parent)
            .node(LinkedListNode::new(id))
            .arguments(args)
            .statements(self.arena.link_statements(&self.statements))
            .maybe_terminator(self.terminator)
            .new();
        self.arena.blocks.push(block);
        id
    }
}
