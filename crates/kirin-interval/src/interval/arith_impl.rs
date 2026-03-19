use kirin_ir::HasTop;

use super::{Interval, interval_div, interval_rem};

impl kirin_arith::CheckedDiv for Interval {
    fn checked_div(self, rhs: Self) -> Option<Self> {
        Some(interval_div(&self, &rhs))
    }
}

impl kirin_arith::CheckedRem for Interval {
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        Some(interval_rem(&self, &rhs))
    }
}

impl From<kirin_arith::ArithValue> for Interval {
    fn from(v: kirin_arith::ArithValue) -> Self {
        use kirin_arith::ArithValue;

        match v {
            ArithValue::I64(x) => Interval::constant(x),
            ArithValue::I32(x) => Interval::constant(x as i64),
            ArithValue::I16(x) => Interval::constant(x as i64),
            ArithValue::I8(x) => Interval::constant(x as i64),
            _ => Interval::top(),
        }
    }
}
