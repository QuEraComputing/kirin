use crate::PrettyPrint;

use kirin_ir::*;
use super::config::Config;
use prettyless::{Arena, DocAllocator, DocBuilder};

pub struct Printer<'a, L: Dialect> {
    config: Config,
    pub arena: Arena<'a>,
    pub context: &'a Context<L>,
}

pub type ArenaDoc<'a> = DocBuilder<'a, Arena<'a>>;

impl<'a, L: Dialect + PrettyPrint<L>> Printer<'a, L> {
    pub fn new(config: Config, context: &'a Context<L>) -> Self {
        Self {
            config,
            arena: Arena::new(),
            context,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Utility
impl<'a, L: Dialect + PrettyPrint<L>> Printer<'a, L> {
    pub fn indent(&'a self, doc: ArenaDoc<'a>) -> ArenaDoc<'a> {
        doc.nest(self.config.tab_spaces as isize)
    }

    pub fn block_indent(
        &'a self,
        doc: ArenaDoc<'a>,
    ) -> ArenaDoc<'a> {
        self.indent(self.arena.line_() + doc) + self.arena.line_()
    }
}
