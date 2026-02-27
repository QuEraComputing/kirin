use kirin::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = call {target}({args}) -> {res:type}")]
pub struct Call<T: CompileTimeValue + Default> {
    target: Symbol,
    args: Vec<SSAValue>,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue + Default> Call<T> {
    pub fn target(&self) -> Symbol {
        self.target
    }

    pub fn args(&self) -> &[SSAValue] {
        &self.args
    }

    pub fn result(&self) -> ResultValue {
        self.res
    }
}
