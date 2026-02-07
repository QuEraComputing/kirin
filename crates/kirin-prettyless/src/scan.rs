//! Result width scanning for proper alignment in pretty printing.

use kirin_ir::{
    Block, Dialect, GetInfo, GlobalSymbol, Region, ResultValue, SSAValue, SpecializedFunction,
    SpecializedFunctionInfo, StagedFunction, Statement, Successor,
};

use crate::Document;

/// Trait for scanning result widths in IR nodes.
///
/// This trait is used to pre-scan IR structures to calculate the maximum width
/// of result values, enabling proper alignment during pretty printing.
pub trait ScanResultWidth<L: Dialect> {
    /// Scan this node and its children to calculate result widths.
    fn scan_result_width(&self, doc: &mut Document<'_, L>);
}

impl<L: Dialect> ScanResultWidth<L> for Statement {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        let mut len = 0;
        for result in self.results(doc.stage()) {
            let info = result.expect_info(doc.stage());
            let mut result_len = result.to_string().len();
            if let Some(name) = info.name() {
                if let Some(resolved_name) = doc.stage().symbol_table().resolve(name) {
                    result_len = resolved_name.len();
                }
            }
            len += result_len + 2; // account for ", "
        }
        if len > 0 {
            len -= 2; // remove last ", "
        }

        doc.set_result_width(*self, len);

        for block in self.blocks(doc.stage()) {
            block.scan_result_width(doc);
        }

        for region in self.regions(doc.stage()) {
            region.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> ScanResultWidth<L> for Block {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        for stmt in self.statements(doc.stage()) {
            stmt.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> ScanResultWidth<L> for Region {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        for block in self.blocks(doc.stage()) {
            block.scan_result_width(doc);
        }
    }
}

impl<L: Dialect> ScanResultWidth<L> for SpecializedFunction {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        let info = self.expect_info(doc.stage());
        let body = info.body();
        body.scan_result_width(doc);
    }
}

impl<L: Dialect> ScanResultWidth<L> for StagedFunction {
    fn scan_result_width(&self, doc: &mut Document<'_, L>) {
        let info = self.expect_info(doc.stage());
        for specialization in info.specializations() {
            let spec_info: &SpecializedFunctionInfo<L> = specialization;
            let body = spec_info.body();
            body.scan_result_width(doc);
        }
    }
}

// Leaf IR nodes - no nested statements to scan

impl<L: Dialect> ScanResultWidth<L> for SSAValue {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // SSAValue is a leaf node with no nested statements
    }
}

impl<L: Dialect> ScanResultWidth<L> for ResultValue {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // ResultValue is a leaf node with no nested statements
    }
}

impl<L: Dialect> ScanResultWidth<L> for Successor {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // Successor is a leaf node with no nested statements
    }
}

impl<L: Dialect> ScanResultWidth<L> for GlobalSymbol {
    fn scan_result_width(&self, _doc: &mut Document<'_, L>) {
        // GlobalSymbol is a leaf node with no nested statements
    }
}
