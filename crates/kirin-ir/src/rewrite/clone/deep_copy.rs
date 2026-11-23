use crate::arena::GetInfo;
use crate::node::PortParent;
use crate::{Body, CFG, Dialect, SSAKind, SSAValue, Statement, StatementParent};

use super::IRMap;
use super::fill::Copied;
use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::{RewriteError, Rewriter};

/// Deep copies. A copy takes two passes over the original: the first
/// allocates every CFG, graph, block, block argument, port and result the copy
/// needs, recording them in the [`IRMap`]; the second copies the statements,
/// whose operands and branch targets may name any of them, since a block may
/// branch to, or read a value from, a block listed after it. Last, the copied
/// statements' operand uses are recorded, and the copied graphs' topology is
/// built from them.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Copy `stmt` with everything nested in it, giving the copy fresh ids, and
    /// return the copy, detached: it has no parent until
    /// [`Rewriter::insert`] places it. Its results can be read before then,
    /// but a copy never placed leaves their readers dangling (see
    /// [`Rewriter::insert`]).
    ///
    /// Every operand and branch target inside the copy is mapped through
    /// `map`. An operand reading a value from outside the copy keeps reading it
    /// unless `map` gives a replacement. Results and block arguments are always
    /// copied, overwriting their entries in `map`. Names are not copied.
    /// Afterwards `map` holds every value, block and statement the copy made.
    ///
    /// Rejects, leaving the stage untouched, if `stmt` is unknown, is a
    /// terminator ([`RewriteError::UnexpectedTerminator`]: it could not be
    /// inserted), or if `map` replaces a value from outside the copy with one
    /// that is not live ([`RewriteError::UnknownValue`]). Records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// for each statement copied into the copy's bodies, outer statements
    /// first. The copy itself is recorded when [`Rewriter::insert`] places it.
    pub fn clone_statement(
        &mut self,
        stmt: Statement,
        map: &mut IRMap,
    ) -> Result<Statement, RewriteError> {
        if self.live_statement(stmt)?.definition.is_terminator() {
            return Err(RewriteError::UnexpectedTerminator);
        }
        // Reject a value read from outside the copy whose replacement in
        // `map` is not live. With no value replacements in `map`, every
        // such value is read as it is, so none can be rejected.
        if map.has_values() {
            let copied = bodies_of(&stmt.expect_info(self.stage).definition);
            self.check_replacements_live(stmt, map, Some(stmt), &copied)?;
        }

        self.reserve_statement(stmt, map);
        let mut copied = Copied::default();
        let copy = self.fill_statement(stmt, None, map, &mut copied);
        self.finish_copy(copied, map);
        self.detached.insert(copy);
        Ok(copy)
    }

    /// Copy `source` with everything nested in it, giving the copy fresh ids,
    /// and return the copy, detached: no statement owns it.
    ///
    /// Place its blocks with [`Rewriter::splice_cfg_after`], or make it the
    /// body of a new statement with [`Rewriter::create`]. A copy still
    /// detached when the `Rewriter` is dropped, including one a splice left
    /// empty, is tombstoned, leaving any reader of the values in its blocks
    /// dangling (see [`Rewriter::insert`]). Values are mapped as for
    /// [`Rewriter::clone_statement`].
    ///
    /// Rejects, leaving the stage untouched, if `source` is unknown
    /// ([`RewriteError::UnknownCfg`]), or if `map` replaces a value from
    /// outside the copy with one that is not live
    /// ([`RewriteError::UnknownValue`]). Records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// for each statement copied, outer statements first.
    pub fn clone_cfg(&mut self, source: CFG, map: &mut IRMap) -> Result<CFG, RewriteError> {
        if source.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownCfg(source));
        }
        // Reject a dead replacement for a value read from outside the copy,
        // as in `clone_statement`.
        if map.has_values() {
            for stmt in self.body_statements(Body::CFG(source)) {
                self.check_replacements_live(stmt, map, None, &[Body::CFG(source)])?;
            }
        }

        let copy = self.reserve_cfg(source, map);
        let mut copied = Copied::default();
        self.fill_body(Body::CFG(source), map, &mut copied);
        self.finish_copy(copied, map);
        self.detached_cfgs.insert(copy);
        Ok(copy)
    }

    /// Reject `stmt` if the copy would read a value from `map` that is not
    /// live, in `stmt` or anything nested in it. A value the copy defines, a
    /// result of `root` (the statement being copied, if any) or a value
    /// defined in `copied` (the bodies being copied), is skipped: copying
    /// overwrites its entry in `map` before anything reads it.
    fn check_replacements_live(
        &self,
        stmt: Statement,
        map: &IRMap,
        root: Option<Statement>,
        copied: &[Body],
    ) -> Result<(), RewriteError> {
        let definition = &stmt.expect_info(self.stage).definition;
        // A `DiGraph`'s yields are read too.
        let yields = definition
            .digraphs()
            .flat_map(|&graph| graph.expect_info(self.stage).yields().iter());
        for &read in definition.arguments().chain(yields) {
            if let Some(replacement) = map.value(read)
                && replacement.get_info(self.stage).is_none()
                && !self.defined_in(read, root, copied)
            {
                return Err(RewriteError::UnknownValue(replacement));
            }
        }
        for body in bodies_of(definition) {
            for nested in self.body_statements(body) {
                self.check_replacements_live(nested, map, root, copied)?;
            }
        }
        Ok(())
    }

    /// Whether `value` is defined by the copy: a result of `root`, or defined
    /// in one of `bodies` at any depth (a result of a statement in it, or an
    /// argument or port of it or of a body nested in it).
    fn defined_in(&self, value: SSAValue, root: Option<Statement>, bodies: &[Body]) -> bool {
        let Some(info) = value.get_info(self.stage) else {
            return false;
        };
        let place = match *info.kind() {
            SSAKind::Result(stmt, _) if Some(stmt) == root => return true,
            SSAKind::Result(stmt, _) => {
                match stmt.get_info(self.stage).and_then(|item| item.parent) {
                    Some(parent) => parent,
                    None => return false,
                }
            }
            SSAKind::BlockArgument(block, _) => StatementParent::Block(block),
            SSAKind::Port(PortParent::DiGraph(graph), _) => StatementParent::DiGraph(graph),
            SSAKind::Port(PortParent::UnGraph(graph), _) => StatementParent::UnGraph(graph),
        };
        bodies.iter().any(|&body| self.body_contains(body, place))
    }

    /// The statements directly in `body`: a block's chain then its
    /// terminator, every block's in order for a CFG, and a graph's members in
    /// member order.
    pub(super) fn body_statements(&self, body: Body) -> Vec<Statement> {
        match body {
            Body::Block(block) => self.block_statements(block),
            Body::CFG(cfg) => cfg
                .blocks(self.stage)
                .flat_map(|block| self.block_statements(block))
                .collect(),
            Body::DiGraph(graph) => graph
                .expect_info(self.stage)
                .members()
                .map(|(stmt, _)| stmt)
                .collect(),
            Body::UnGraph(graph) => graph
                .expect_info(self.stage)
                .members()
                .map(|(stmt, _)| stmt)
                .collect(),
        }
    }
}
