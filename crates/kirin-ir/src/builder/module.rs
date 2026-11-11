use crate::node::{LinkedList, Module, ModuleInfo};
use crate::{BuilderStageInfo, Dialect, GlobalSymbol, Statement, StatementParent};

#[bon::bon]
impl<L: Dialect> BuilderStageInfo<L> {
    /// Create a module, optionally named and with initial members.
    ///
    /// # Panics
    ///
    /// If the stage already has a module named `name`.
    #[builder(finish_fn = new)]
    pub fn module(
        &mut self,
        name: Option<GlobalSymbol>,
        members: Option<Vec<Statement>>,
    ) -> Module {
        if let Some(name) = name {
            assert!(
                self.module_by_name(name).is_none(),
                "Stage already has a module named `{name:?}`"
            );
        }
        let id = self.modules.next_id();
        let _ = self.modules.alloc(ModuleInfo {
            id,
            name,
            members: LinkedList::new(),
        });
        for member in members.unwrap_or_default() {
            self.add_module_member(id, member);
        }
        id
    }
}

impl<L: Dialect> BuilderStageInfo<L> {
    /// Append `member` to `module`.
    ///
    /// # Panics
    ///
    /// If `member` already has a parent or is linked to another statement.
    pub fn add_module_member(&mut self, module: Module, member: Statement) {
        let tail = self.modules[module].members.tail;
        let info = &mut self.nodes.statements[member];
        assert!(
            info.parent.is_none(),
            "Statement `{member:?}` already has a parent"
        );
        assert!(
            info.node.prev.is_none() && info.node.next.is_none(),
            "Statement `{member:?}` is already linked to another statement"
        );
        info.parent = Some(StatementParent::Module(module));
        info.node.prev = tail;
        if let Some(tail) = tail {
            self.nodes.statements[tail].node.next = Some(member);
        }
        let members = &mut self.modules[module].members;
        members.head.get_or_insert(member);
        members.tail = Some(member);
        members.len += 1;
    }
}
