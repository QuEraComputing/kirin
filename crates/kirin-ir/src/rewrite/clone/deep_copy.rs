use rustc_hash::FxHashSet;

use crate::arena::GetInfo;
use crate::{Body, CFG, Dialect, SSAValue, Statement};

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
    /// that is not live ([`RewriteError::UnknownValue`]). Records no event:
    /// nothing reachable changes until the insert.
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
            let mut defined: FxHashSet<SSAValue> = {
                let definition = &stmt.expect_info(self.stage).definition;
                definition.results().map(|&result| result.into()).collect()
            };
            for body in bodies_of(&stmt.expect_info(self.stage).definition) {
                self.collect_defined(body, &mut defined);
            }
            self.check_replacements_live(stmt, map, &defined)?;
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
    /// ([`RewriteError::UnknownValue`]). Records no event.
    pub fn clone_cfg(&mut self, source: CFG, map: &mut IRMap) -> Result<CFG, RewriteError> {
        if source.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownCfg(source));
        }
        // Reject a dead replacement for a value read from outside the copy,
        // as in `clone_statement`.
        if map.has_values() {
            let mut defined = FxHashSet::default();
            self.collect_defined(Body::CFG(source), &mut defined);
            for stmt in self.body_statements(Body::CFG(source)) {
                self.check_replacements_live(stmt, map, &defined)?;
            }
        }

        let copy = self.reserve_cfg(source, map);
        let mut copied = Copied::default();
        self.fill_body(Body::CFG(source), map, &mut copied);
        self.finish_copy(copied, map);
        self.detached_cfgs.insert(copy);
        Ok(copy)
    }

    /// Add every value defined in `body` or nested in it to `defined`: block
    /// arguments, graph ports and results.
    fn collect_defined(&self, body: Body, defined: &mut FxHashSet<SSAValue>) {
        match body {
            Body::Block(block) => {
                let arguments = block.expect_info(self.stage).arguments();
                defined.extend(arguments.iter().map(|&argument| SSAValue::from(argument)));
            }
            Body::CFG(cfg) => {
                for block in cfg.blocks(self.stage) {
                    let arguments = block.expect_info(self.stage).arguments();
                    defined.extend(arguments.iter().map(|&argument| SSAValue::from(argument)));
                }
            }
            Body::DiGraph(graph) => {
                let ports = graph.expect_info(self.stage).ports();
                defined.extend(ports.iter().map(|&port| SSAValue::from(port)));
            }
            Body::UnGraph(graph) => {
                let ports = graph.expect_info(self.stage).ports();
                defined.extend(ports.iter().map(|&port| SSAValue::from(port)));
            }
        }
        for stmt in self.body_statements(body) {
            let definition = &stmt.expect_info(self.stage).definition;
            defined.extend(definition.results().map(|&result| SSAValue::from(result)));
            for nested in bodies_of(definition) {
                self.collect_defined(nested, defined);
            }
        }
    }

    /// Reject `stmt` if the copy would read a value from `map` that is not
    /// live, in `stmt` or anything nested in it. The values in `defined`,
    /// which the copy defines, are skipped: copying overwrites their entries
    /// in `map` before anything reads them.
    fn check_replacements_live(
        &self,
        stmt: Statement,
        map: &IRMap,
        defined: &FxHashSet<SSAValue>,
    ) -> Result<(), RewriteError> {
        let definition = &stmt.expect_info(self.stage).definition;
        for &operand in definition.arguments() {
            if !defined.contains(&operand)
                && let Some(replacement) = map.value(operand)
                && replacement.get_info(self.stage).is_none()
            {
                return Err(RewriteError::UnknownValue(replacement));
            }
        }
        // A `DiGraph`'s yields are read too.
        for &graph in definition.digraphs() {
            for &value in graph.expect_info(self.stage).yields() {
                if !defined.contains(&value)
                    && let Some(replacement) = map.value(value)
                    && replacement.get_info(self.stage).is_none()
                {
                    return Err(RewriteError::UnknownValue(replacement));
                }
            }
        }
        for body in bodies_of(definition) {
            for nested in self.body_statements(body) {
                self.check_replacements_live(nested, map, defined)?;
            }
        }
        Ok(())
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
