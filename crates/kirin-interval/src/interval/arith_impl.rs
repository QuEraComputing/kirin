use kirin_ir::HasTop;

use super::Interval;

impl kirin_arith::CheckedDiv for Interval {
    fn checked_div(self, _rhs: Self) -> Option<Self> {
        Some(Interval::top())
    }
}

impl kirin_arith::CheckedRem for Interval {
    fn checked_rem(self, _rhs: Self) -> Option<Self> {
        Some(Interval::top())
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
