use kirin::prelude::{CompileTimeValue, HasBottom, PrettyPrint, Typeof};
use kirin_interpreter::dialect::{
    DenseBackward, DenseBackwardInterp, Interpretable, SparseBackward, SparseBackwardInterp,
    SparseForward, SparseForwardEffect, SparseForwardInterp,
};

use crate::Constant;

/// Classic (weak) per-point liveness: kill the result (constants have no
/// operands to gen).
impl<I, T, Ty> Interpretable<I, DenseBackward> for Constant<T, Ty>
where
    I: DenseBackwardInterp,
    T: CompileTimeValue + Typeof<Ty> + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.transfer_classic(self)
    }
}

/// Backward demand: purity-aware neededness. `Constant` is `#[kirin(pure)]`
/// and has no operands, so this only records result demand.
impl<I, T, Ty> Interpretable<I, SparseBackward> for Constant<T, Ty>
where
    I: SparseBackwardInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue + Typeof<Ty> + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.transfer_ordinary(self)
    }
}

impl<I, T, Ty> Interpretable<I, SparseForward> for Constant<T, Ty>
where
    I: SparseForwardInterp,
    I::Value: TryFrom<T>,
    I::Error: From<<I::Value as TryFrom<T>>::Error>,
    T: CompileTimeValue + Typeof<Ty> + Clone + PrettyPrint,
    Ty: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        let value = I::Value::try_from(self.value.clone()).map_err(I::Error::from)?;
        interp.write(self.result, value)?;
        Ok(SparseForwardEffect::Next)
    }
}
