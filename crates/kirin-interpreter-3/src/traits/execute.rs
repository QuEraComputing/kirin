use crate::Interpreter;

pub trait Execute<I: Interpreter> {
    type Output;

    fn execute(self, interp: &mut I) -> Result<Self::Output, I::Error>;
}
