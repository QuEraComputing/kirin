use crate::node::{LinkedList, Module, ModuleInfo};
use crate::{BuilderStageInfo, Dialect, Statement, StatementParent, Symbol};

#[bon::bon]
impl<L: Dialect> BuilderStageInfo<L> {
    /// Create a module, optionally named and with initial members.
    ///
    /// # Panics
    ///
    /// If the stage already has a module named `name`.
    #[builder(finish_fn = new)]
    pub fn module(&mut self, name: Option<Symbol>, members: Option<Vec<Statement>>) -> Module {
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
    /// Panics where [`try_add_module_member`](Self::try_add_module_member)
    /// returns `Err`.
    pub fn add_module_member(&mut self, module: Module, member: Statement) {
        self.try_add_module_member(module, member)
            .unwrap_or_else(|message| panic!("{message}"));
    }

    /// Validate membership before attaching the statement. A failed append
    /// leaves the module and the statement unchanged.
    pub fn try_add_module_member(
        &mut self,
        module: Module,
        member: Statement,
    ) -> Result<(), String> {
        let tail = self
            .modules
            .get(module)
            .ok_or_else(|| format!("module {module:?} is deleted or not in this stage"))?
            .members
            .tail;
        let info = self
            .nodes
            .statements
            .get(member)
            .ok_or_else(|| format!("statement {member:?} is deleted or not in this stage"))?;
        if info.parent.is_some() || info.node.prev.is_some() || info.node.next.is_some() {
            return Err(format!("statement {member:?} is already attached"));
        }
        if info.definition.is_terminator() {
            return Err("a terminator cannot be a module member".into());
        }
        if info.definition.successors().next().is_some() {
            return Err("a statement with successors cannot be a module member".into());
        }
        let info = &mut self.nodes.statements[member];
        info.parent = Some(StatementParent::Module(module));
        info.node.prev = tail;
        if let Some(tail) = tail {
            self.nodes.statements[tail].node.next = Some(member);
        }
        let members = &mut self.modules[module].members;
        members.head.get_or_insert(member);
        members.tail = Some(member);
        members.len += 1;
        Ok(())
    }
}
