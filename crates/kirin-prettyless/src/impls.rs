//! PrettyPrint implementations for IR types.

use kirin_ir::{
    Dialect, GlobalSymbol, ResultValue, SSAValue, SpecializedFunction, StagedFunction, Successor,
    Symbol,
};
use prettyless::DocAllocator;

use crate::{ArenaDoc, Document, PrettyPrint, PrettyPrintViaDisplay};

impl PrettyPrint for ResultValue {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_ssa_ref(*self)
    }

    fn pretty_print_name<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_ssa_ref(*self)
    }

    fn pretty_print_type<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_ssa_type(*self)
    }
}

impl PrettyPrint for SSAValue {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_ssa_ref(*self)
    }

    fn pretty_print_name<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_ssa_ref(*self)
    }

    fn pretty_print_type<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_ssa_type(*self)
    }
}

impl PrettyPrintViaDisplay for Successor {}

impl PrettyPrint for Symbol {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        // Look up the symbol name from the context's symbol table
        if let Some(name) = doc.stage().symbol_table().resolve(*self) {
            doc.text(format!("@{}", name))
        } else {
            // Fallback: print as raw ID if not found
            doc.text(format!("@<{}>", usize::from(*self)))
        }
    }
}

impl PrettyPrint for GlobalSymbol {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        if let Some(gs) = doc.global_symbols()
            && let Some(name) = gs.resolve(*self)
        {
            return doc.text(format!("@{}", name));
        }
        doc.text(format!("@<global:{}>", usize::from(*self)))
    }
}

impl<T: PrettyPrint> PrettyPrint for Vec<T> {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.list(self.iter(), ", ", |item| item.pretty_print(doc))
    }
}

impl<T: PrettyPrint> PrettyPrint for Option<T> {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            Some(value) => value.pretty_print(doc),
            None => doc.nil(),
        }
    }
}

impl PrettyPrint for SpecializedFunction {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_specialized_function(self)
    }
}

impl PrettyPrint for StagedFunction {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.print_staged_function(self)
    }
}

// ============================================================================
// PrettyPrint implementations for builtin types
// ============================================================================

// Integer types use Display for pretty printing
macro_rules! impl_pretty_print_via_display {
    ($($ty:ty),*) => { $(impl PrettyPrintViaDisplay for $ty {})* };
}

impl_pretty_print_via_display!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

// Macro to reduce boilerplate for floating point types (need decimal point)
macro_rules! impl_pretty_print_float {
    ($($ty:ty),*) => {
        $(
            impl PrettyPrint for $ty {
                fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
                    &self,
                    doc: &'a Document<'a, L>,
                    _namespace: &[&str],
                ) -> ArenaDoc<'a>
                where
                    L::Type: std::fmt::Display,
                {
                    // Handle special float values explicitly so output roundtrips
                    // through the parser (NaN and Inf have no decimal representation).
                    if self.is_nan() {
                        return doc.text("nan");
                    }
                    if self.is_infinite() {
                        return doc.text(if self.is_sign_positive() { "inf" } else { "-inf" });
                    }
                    // Ensure we always print as a float (with decimal point)
                    if self.fract() == 0.0 {
                        doc.text(format!("{:.1}", self))
                    } else {
                        doc.text(self.to_string())
                    }
                }
            }
        )*
    };
}

// Implement for all floating point types
impl_pretty_print_float!(f32, f64);

impl PrettyPrint for bool {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(if *self { "true" } else { "false" })
    }
}

impl PrettyPrint for String {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        // Print as quoted string with proper escaping
        doc.text(format!("{:?}", self))
    }
}
