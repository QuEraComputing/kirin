use kirin::prelude::*;
use kirin_interpreter_new::Interpretable;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(type = T)]
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
#[kirin(type = T)]
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
#[kirin(type = T)]
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
#[kirin(type = T)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable)]
#[wraps]
#[kirin(type = T)]
#[chumsky(format = "call")]
pub enum Call<T: CompileTimeValue> {
    Named(CallNamed<T>),
    Function(CallFunction<T>),
    Staged(CallStaged<T>),
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

pub trait CallLike<T: CompileTimeValue>: for<'a> HasArguments<'a> + for<'a> HasResults<'a> {
    type Target: Copy;

    fn target(&self) -> Self::Target;
    fn stage(&self) -> Option<CompileStage>;
}

macro_rules! impl_call_like {
    ($ty:ident, $target:ty) => {
        impl<T: CompileTimeValue> CallLike<T> for $ty<T> {
            type Target = $target;

            fn target(&self) -> Self::Target {
                self.target
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

macro_rules! impl_call_from {
    ($variant:ident, $ty:ident) => {
        impl<T: CompileTimeValue> From<$ty<T>> for Call<T> {
            fn from(value: $ty<T>) -> Self {
                Self::$variant(value)
            }
        }
    };
}

impl_call_from!(Named, CallNamed);
impl_call_from!(Function, CallFunction);
impl_call_from!(Staged, CallStaged);
impl_call_from!(Specialized, CallSpecialized);

impl<T: CompileTimeValue> Call<T> {
    pub fn build<Lang>(stage: &mut impl AsBuildStage<Lang>) -> CallBuilder<'_, Lang, T>
    where
        Lang: Dialect,
    {
        CallBuilder {
            stage: stage.as_build_stage(),
            compile_stage: None,
            marker: std::marker::PhantomData,
        }
    }
}

pub struct CallBuilder<'a, Lang, T>
where
    Lang: Dialect,
    T: CompileTimeValue,
{
    stage: &'a mut BuilderStageInfo<Lang>,
    compile_stage: Option<CompileStage>,
    marker: std::marker::PhantomData<T>,
}

impl<'a, Lang, T> CallBuilder<'a, Lang, T>
where
    Lang: Dialect,
    T: CompileTimeValue,
{
    pub fn in_stage(mut self, stage: CompileStage) -> Self {
        self.compile_stage = Some(stage);
        self
    }

    pub fn named(self, target: impl Into<Symbol>) -> CallTargetBuilder<'a, Lang, T> {
        self.with_target(CallTarget::Named(target.into()))
    }

    pub fn function(self, target: Function) -> CallTargetBuilder<'a, Lang, T> {
        self.with_target(CallTarget::Function(target))
    }

    pub fn staged(self, target: StagedFunction) -> CallTargetBuilder<'a, Lang, T> {
        self.with_target(CallTarget::Staged(target))
    }

    pub fn specialized(self, target: SpecializedFunction) -> CallTargetBuilder<'a, Lang, T> {
        self.with_target(CallTarget::Specialized(target))
    }

    fn with_target(self, target: CallTarget) -> CallTargetBuilder<'a, Lang, T> {
        CallTargetBuilder {
            stage: self.stage,
            compile_stage: self.compile_stage,
            target,
            args: Vec::new(),
            result_count: 0,
            marker: std::marker::PhantomData,
        }
    }
}

pub struct CallTargetBuilder<'a, Lang, T>
where
    Lang: Dialect,
    T: CompileTimeValue,
{
    stage: &'a mut BuilderStageInfo<Lang>,
    compile_stage: Option<CompileStage>,
    target: CallTarget,
    args: Vec<SSAValue>,
    result_count: usize,
    marker: std::marker::PhantomData<T>,
}

impl<Lang, T> CallTargetBuilder<'_, Lang, T>
where
    Lang: Dialect + LiftFrom<Call<T>>,
    Lang::Type: From<T>,
    T: CompileTimeValue + Placeholder,
{
    pub fn args(mut self, args: impl Into<Vec<SSAValue>>) -> Self {
        self.args = args.into();
        self
    }

    pub fn results(mut self, count: usize) -> Self {
        self.result_count = count;
        self
    }

    pub fn insert(self) -> call_build_result::Call {
        let statement = self.stage.statement_arena().next_id();
        let results: Vec<ResultValue> = (0..self.result_count)
            .map(|index| {
                self.stage
                    .ssa()
                    .ty(T::placeholder().into())
                    .kind(BuilderSSAKind::Result(statement, index))
                    .new()
                    .into()
            })
            .collect();
        let definition = self
            .target
            .into_call(self.compile_stage, self.args, results.clone());
        let id = self
            .stage
            .statement()
            .definition(Lang::lift_from(definition))
            .new();
        debug_assert_eq!(id, statement);
        call_build_result::Call { id, results }
    }
}

enum CallTarget {
    Named(Symbol),
    Function(Function),
    Staged(StagedFunction),
    Specialized(SpecializedFunction),
}

impl CallTarget {
    fn into_call<T: CompileTimeValue>(
        self,
        compile_stage: Option<CompileStage>,
        args: Vec<SSAValue>,
        results: Vec<ResultValue>,
    ) -> Call<T> {
        match self {
            Self::Named(target) => Call::Named(CallNamed {
                target,
                args,
                results,
                compile_stage,
                marker: std::marker::PhantomData,
            }),
            Self::Function(target) => Call::Function(CallFunction {
                target,
                args,
                results,
                compile_stage,
                marker: std::marker::PhantomData,
            }),
            Self::Staged(target) => Call::Staged(CallStaged {
                target,
                args,
                results,
                compile_stage,
                marker: std::marker::PhantomData,
            }),
            Self::Specialized(target) => Call::Specialized(CallSpecialized {
                target,
                args,
                results,
                compile_stage,
                marker: std::marker::PhantomData,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
