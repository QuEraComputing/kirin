use crate::arena::GetInfo;
use crate::node::*;
use crate::{Context, Dialect};

pub struct BlockBuilder<'a, L: Dialect> {
    context: &'a mut Context<L>,
    parent: Option<Region>,
    name: Option<String>,
    arguments: Vec<(L::TypeLattice, Option<String>)>,
    statements: Vec<Statement>,
    terminator: Option<Statement>,
}

impl<'a, L: Dialect> BlockBuilder<'a, L> {
    pub(crate) fn from_context(context: &'a mut Context<L>) -> Self {
        BlockBuilder {
            context,
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
    pub fn argument<T: Into<L::TypeLattice>>(mut self, ty: T) -> Self {
        self.arguments.push((ty.into(), None));
        self
    }

    /// Add an argument with a name to the block.
    pub fn argument_with_name<T: Into<L::TypeLattice>, S: Into<String>>(
        mut self,
        name: S,
        ty: T,
    ) -> Self {
        self.arguments.push((ty.into(), Some(name.into())));
        self
    }

    /// Add a statement to the block.
    pub fn stmt(mut self, stmt: impl Into<Statement>) -> Self {
        let stmt = stmt.into();
        let info = stmt.expect_info(self.context);

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
    pub fn terminator(mut self, term: impl Into<Statement>) -> Self {
        let term = term.into();
        let info = term.expect_info(self.context);

        let _ = info.definition.is_terminator() || {
            panic!(
                "Statement {:?} is not a terminator and cannot be set as block terminator",
                info.definition
            )
        };
        self.terminator = Some(term.into());
        self
    }

    /// Finalize the block and add it to the context.
    pub fn new(self) -> Block {
        let id = self.context.blocks.next_id();
        let block_args = self
            .arguments
            .into_iter()
            .enumerate()
            .map(|(index, (ty, name))| {
                let arg: BlockArgument = self.context.ssas.next_id().into();
                let ssa = SSAInfo::new(
                    arg.into(),
                    name.map(|n| self.context.symbols.borrow_mut().intern(n)),
                    ty,
                    SSAKind::BlockArgument(id, index),
                );
                self.context.ssas.alloc(ssa);
                arg
            })
            .collect::<Vec<_>>();

        for &stmt_id in &self.statements {
            let info = &mut self.context.statements[stmt_id];
            for arg in info.definition.arguments_mut() {
                let ssa_info = self
                    .context
                    .ssas
                    .get(*arg)
                    .expect("SSAValue not found in context");
                if let SSAKind::BuilderBlockArgument(arg_index) = ssa_info.kind {
                    self.context.ssas.delete(*arg);
                    *arg = block_args[arg_index].into();
                }
            }
        }

        let block = BlockInfo::builder()
            .maybe_parent(self.parent)
            .maybe_name(self.name.map(|n| self.context.symbols.borrow_mut().intern(n)))
            .node(LinkedListNode::new(id))
            .arguments(block_args)
            .statements(self.context.link_statements(&self.statements))
            .maybe_terminator(self.terminator)
            .new();
        self.context.blocks.alloc(block);
        id
    }
}
