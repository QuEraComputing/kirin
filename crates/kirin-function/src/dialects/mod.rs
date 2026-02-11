use kirin::prelude::*;

pub mod bind;
pub mod body;
pub mod call;
pub mod lambda;
pub mod ret;

pub use bind::Bind;
pub use body::FunctionBody;
pub use call::Call;
pub use lambda::Lambda;
pub use ret::Return;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(fn, type = T)]
pub enum Lexical<T: CompileTimeValue + Default> {
    FunctionBody(FunctionBody<T>),
    Lambda(Lambda<T>),
    Call(Call<T>),
    Return(Return<T>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(fn, type = T)]
pub enum Lifted<T: CompileTimeValue + Default> {
    FunctionBody(FunctionBody<T>),
    Bind(Bind<T>),
    Call(Call<T>),
    Return(Return<T>),
}
