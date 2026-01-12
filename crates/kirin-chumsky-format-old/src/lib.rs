mod ast;

use attrs::{ChumskyFieldAttrs, ChumskyGlobalAttrs, ChumskyStatementAttrs};
use kirin_derive_core_2::ir::Layout;

pub mod attrs;
pub mod parser;
pub mod parse;

#[derive(Debug, Clone)]
pub struct ChumskyLayout;

impl Layout for ChumskyLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = ChumskyGlobalAttrs;
    type ExtraStatementAttrs = ChumskyStatementAttrs;
    type ExtraFieldAttrs = ChumskyFieldAttrs;
}
