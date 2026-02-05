//! PrettyPrint implementations for IR types.

use kirin_ir::{
    Dialect, GetInfo, Item, ResultValue, SSAInfo, SSAValue, SpecializedFunction, StagedFunction,
    Successor, Symbol,
};
use prettyless::DocAllocator;

use crate::{ArenaDoc, Document, PrettyPrint, PrettyPrintName, PrettyPrintType};

impl PrettyPrint for ResultValue {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context());
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context().symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrint for SSAValue {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context());
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context().symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrint for Successor {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}

impl PrettyPrint for Symbol {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        // Look up the symbol name from the context's symbol table
        if let Some(name) = doc.context().symbol_table().borrow().resolve(*self) {
            doc.text(format!("@{}", name))
        } else {
            // Fallback: print as raw ID if not found
            doc.text(format!("@<{}>", usize::from(*self)))
        }
    }
}

impl PrettyPrintName for SSAValue {
    fn pretty_print_name<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info = self.expect_info(doc.context());
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context().symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrintName for ResultValue {
    fn pretty_print_name<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a> {
        let info: &Item<SSAInfo<L>> = self.expect_info(doc.context());
        if let Some(name) = info.name() {
            if let Some(resolved_name) = doc.context().symbol_table().borrow().resolve(name) {
                return doc.text(format!("%{}", resolved_name));
            }
        }
        doc.text(self.to_string())
    }
}

impl PrettyPrintType for SSAValue {
    fn pretty_print_type<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info = self.expect_info(doc.context());
        doc.text(format!("{}", info.ty()))
    }
}

impl PrettyPrintType for ResultValue {
    fn pretty_print_type<'a, L: Dialect>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        let info: &Item<SSAInfo<L>> = self.expect_info(doc.context());
        doc.text(format!("{}", info.ty()))
    }
}

impl PrettyPrint for SpecializedFunction {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        doc.print_specialized_function(self)
    }
}

impl PrettyPrint for StagedFunction {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        doc.print_staged_function(self)
    }
}
