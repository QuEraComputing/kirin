//! Lexical scope tracking: maps Python variable names to SSA values.
//!
//! This is the analog of the text parser's `EmitContext` ssa-scope stack and of
//! the Python front-end's `Frame.defs`.

use kirin::prelude::SSAValue;
use rustc_hash::FxHashMap;

/// A stack of name → SSA scopes. The innermost (top) scope is consulted first.
pub struct Frame {
    scopes: Vec<FxHashMap<String, SSAValue>>,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            scopes: vec![FxHashMap::default()],
        }
    }

    /// Enter a nested scope (e.g. an `if`/`for` body).
    pub fn push(&mut self) {
        self.scopes.push(FxHashMap::default());
    }

    /// Leave the innermost scope.
    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    /// Bind `name` to `value` in the innermost scope.
    pub fn define(&mut self, name: &str, value: SSAValue) {
        self.scopes
            .last_mut()
            .expect("scope stack is never empty")
            .insert(name.to_string(), value);
    }

    /// Resolve `name` to its most recent SSA value, searching inner→outer.
    pub fn lookup(&self, name: &str) -> Option<SSAValue> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}
