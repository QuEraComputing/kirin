use kirin_ir::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ArithInstruction<T> {
    Add(SSAValue, Vec<SSAValue>, ResultValue, T),
    Sub(SSAValue, Vec<SSAValue>, ResultValue, T),
    Mul(SSAValue, Vec<SSAValue>, ResultValue),
    Div(SSAValue, Vec<SSAValue>, ResultValue),
}

impl<'a, T> HasArguments<'a> for ArithInstruction<T> {
    type Iter = std::iter::Chain<
        std::iter::Once<&'a SSAValue>,
        std::slice::Iter<'a, SSAValue>,
    >;

    fn arguments(&'a self) -> Self::Iter {
        match self {
            ArithInstruction::Add(arg1, arg2, ..) |
            ArithInstruction::Sub(arg1, arg2, ..) |
            ArithInstruction::Mul(arg1, arg2, ..) |
            ArithInstruction::Div(arg1, arg2, ..) => {
                std::iter::once(arg1).chain(arg2.iter())
            }
        }
    }
}

pub enum Wrapper<T> {
    InstA(ArithInstruction<T>),
    InstB(ArithInstruction<T>),
}

impl<'a, T> HasArguments<'a> for Wrapper<T> {
    type Iter = <ArithInstruction<T> as HasArguments<'a>>::Iter;

    fn arguments(&'a self) -> Self::Iter {
        match self {
            Wrapper::InstA(inst) | Wrapper::InstB(inst) => {
                <ArithInstruction<T> as HasArguments>::arguments(inst)
            },
        }
    }
}

pub enum ArgumentIter<'a, T> {
    InstA(<ArithInstruction<T> as HasArguments<'a>>::Iter),
    InstB(<ArithInstruction<T> as HasArguments<'a>>::Iter),
}
impl<'a, T> Iterator for ArgumentIter<'a, T> {
    type Item = &'a SSAValue;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ArgumentIter::InstA(iter) => iter.next(),
            ArgumentIter::InstB(iter) => iter.next(),
        }
    }
}