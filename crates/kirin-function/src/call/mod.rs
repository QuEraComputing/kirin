use kirin::prelude::*;

mod parser;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "$named {target}({args})[ -> {results:type}]")]
pub struct CallNamed<T: CompileTimeValue> {
    target: Symbol,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
    #[kirin(default)]
    compile_stage: Option<CompileStage>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "$function {target}({args})[ -> {results:type}]")]
pub struct CallFunction<T: CompileTimeValue> {
    target: Function,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
    #[kirin(default)]
    compile_stage: Option<CompileStage>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "$staged {target}({args})[ -> {results:type}]")]
pub struct CallStaged<T: CompileTimeValue> {
    target: StagedFunction,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
    #[kirin(default)]
    compile_stage: Option<CompileStage>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "$specialized {target}({args})[ -> {results:type}]")]
pub struct CallSpecialized<T: CompileTimeValue> {
    target: SpecializedFunction,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
    #[kirin(default)]
    compile_stage: Option<CompileStage>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[wraps]
#[kirin(builders, type = T)]
pub enum Call<T: CompileTimeValue> {
    #[chumsky(format = "call")]
    Named(CallNamed<T>),
    #[chumsky(format = "call")]
    Function(CallFunction<T>),
    #[chumsky(format = "call")]
    Staged(CallStaged<T>),
    #[chumsky(format = "call")]
    Specialized(CallSpecialized<T>),
}

#[doc(hidden)]
pub mod call_build_result {
    use kirin::prelude::*;

    pub struct Call {
        pub id: Statement,
        pub results: Vec<ResultValue>,
    }

    impl From<Call> for Statement {
        fn from(value: Call) -> Self {
            value.id
        }
    }
}

pub trait CallLike<T: CompileTimeValue> {
    type Target: Copy;

    fn target(&self) -> Self::Target;
    fn args(&self) -> &[SSAValue];
    fn results(&self) -> &[ResultValue];
    fn stage(&self) -> Option<CompileStage>;
}

macro_rules! impl_call_like {
    ($ty:ident, $target:ty) => {
        impl<T: CompileTimeValue> CallLike<T> for $ty<T> {
            type Target = $target;

            fn target(&self) -> Self::Target {
                self.target
            }

            fn args(&self) -> &[SSAValue] {
                &self.args
            }

            fn results(&self) -> &[ResultValue] {
                &self.results
            }

            fn stage(&self) -> Option<CompileStage> {
                self.compile_stage
            }
        }
    };
}

impl_call_like!(CallNamed, Symbol);
impl_call_like!(CallFunction, Function);
impl_call_like!(CallStaged, StagedFunction);
impl_call_like!(CallSpecialized, SpecializedFunction);

impl<T: CompileTimeValue> Call<T> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<Lang>(
        stage: &mut impl AsBuildStage<Lang>,
        num_results: usize,
        target: impl Into<Symbol>,
        args: impl Into<Vec<SSAValue>>,
    ) -> call_build_result::Call
    where
        Lang: Dialect + From<Self>,
        Lang::Type: From<T>,
        T: Placeholder,
    {
        let stage = stage.as_build_stage();
        let statement = stage.statement_arena().next_id();
        let results: Vec<ResultValue> = (0..num_results)
            .map(|index| {
                stage
                    .ssa()
                    .ty(T::placeholder().into())
                    .kind(BuilderSSAKind::Result(statement, index))
                    .new()
                    .into()
            })
            .collect();
        let target = target.into();
        let args = args.into();
        let id = stage
            .statement()
            .definition(Self::Named(CallNamed {
                target,
                args,
                results: results.clone(),
                compile_stage: None,
                marker: std::marker::PhantomData,
            }))
            .new();
        debug_assert_eq!(id, statement);
        call_build_result::Call { id, results }
    }

    pub fn args(&self) -> &[SSAValue] {
        match self {
            Self::Named(call) => call.args(),
            Self::Function(call) => call.args(),
            Self::Staged(call) => call.args(),
            Self::Specialized(call) => call.args(),
        }
    }

    pub fn results(&self) -> &[ResultValue] {
        match self {
            Self::Named(call) => CallLike::results(call),
            Self::Function(call) => CallLike::results(call),
            Self::Staged(call) => CallLike::results(call),
            Self::Specialized(call) => CallLike::results(call),
        }
    }

    pub fn stage(&self) -> Option<CompileStage> {
        match self {
            Self::Named(call) => call.stage(),
            Self::Function(call) => call.stage(),
            Self::Staged(call) => call.stage(),
            Self::Specialized(call) => call.stage(),
        }
    }
}

#[cfg(test)]
mod tests;
