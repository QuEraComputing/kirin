use crate::{CompileStage, Dialect, HasStageInfo, Pipeline, StageInfo, StageMeta};

/// Immutable stage action executed after resolving a concrete stage dialect.
///
/// Implement this trait for each dialect in your stage container's
/// `S::Languages` type tuple.
pub trait StageAction<S, L>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output;
    type Error;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error>;
}

/// Mutable stage action executed after resolving a concrete stage dialect.
///
/// Implement this trait for each dialect in your stage container's
/// `S::Languages` type tuple.
pub trait StageActionMut<S, L>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output;
    type Error;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &mut StageInfo<L>,
    ) -> Result<Self::Output, Self::Error>;
}

/// Recursive dispatcher over `S::Languages` for immutable stage access.
///
/// This trait is implemented for `()` and nested tuples `(L, Tail)` and is
/// intended to be used by [`Pipeline::dispatch_stage`].
pub trait StageDispatch<S, A, R, E>
where
    S: StageMeta,
{
    fn dispatch(stage: &S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E>;
}

impl<S, A, R, E> StageDispatch<S, A, R, E> for ()
where
    S: StageMeta,
{
    fn dispatch(_stage: &S, _stage_id: CompileStage, _action: &mut A) -> Result<Option<R>, E> {
        Ok(None)
    }
}

impl<S, L, Tail, A, R, E> StageDispatch<S, A, R, E> for (L, Tail)
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    A: StageAction<S, L, Output = R, Error = E>,
    Tail: StageDispatch<S, A, R, E>,
{
    fn dispatch(stage: &S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E> {
        if let Some(stage_info) = <S as HasStageInfo<L>>::try_stage_info(stage) {
            return action.run(stage_id, stage_info).map(Some);
        }
        <Tail as StageDispatch<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}

/// Recursive dispatcher over `S::Languages` for mutable stage access.
///
/// This trait is implemented for `()` and nested tuples `(L, Tail)` and is
/// intended to be used by [`Pipeline::dispatch_stage_mut`].
pub trait StageDispatchMut<S, A, R, E>
where
    S: StageMeta,
{
    fn dispatch(stage: &mut S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E>;
}

impl<S, A, R, E> StageDispatchMut<S, A, R, E> for ()
where
    S: StageMeta,
{
    fn dispatch(_stage: &mut S, _stage_id: CompileStage, _action: &mut A) -> Result<Option<R>, E> {
        Ok(None)
    }
}

impl<S, L, Tail, A, R, E> StageDispatchMut<S, A, R, E> for (L, Tail)
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    A: StageActionMut<S, L, Output = R, Error = E>,
    Tail: StageDispatchMut<S, A, R, E>,
{
    fn dispatch(stage: &mut S, stage_id: CompileStage, action: &mut A) -> Result<Option<R>, E> {
        if let Some(stage_info) = <S as HasStageInfo<L>>::try_stage_info_mut(stage) {
            return action.run(stage_id, stage_info).map(Some);
        }
        <Tail as StageDispatchMut<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}

impl<S> Pipeline<S>
where
    S: StageMeta,
{
    /// Resolve `stage_id`, dispatch to the first matching dialect in
    /// `S::Languages`, and run `action`.
    ///
    /// Returns `Ok(None)` when `stage_id` does not exist or no dialect in
    /// `S::Languages` matches the concrete stage variant.
    pub fn dispatch_stage<A, R, E>(
        &self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>
    where
        S::Languages: StageDispatch<S, A, R, E>,
    {
        let Some(stage) = self.stage(stage_id) else {
            return Ok(None);
        };
        <S::Languages as StageDispatch<S, A, R, E>>::dispatch(stage, stage_id, action)
    }

    /// Mutable variant of [`Self::dispatch_stage`].
    ///
    /// Returns `Ok(None)` when `stage_id` does not exist or no dialect in
    /// `S::Languages` matches the concrete stage variant.
    pub fn dispatch_stage_mut<A, R, E>(
        &mut self,
        stage_id: CompileStage,
        action: &mut A,
    ) -> Result<Option<R>, E>
    where
        S::Languages: StageDispatchMut<S, A, R, E>,
    {
        let Some(stage) = self.stage_mut(stage_id) else {
            return Ok(None);
        };
        <S::Languages as StageDispatchMut<S, A, R, E>>::dispatch(stage, stage_id, action)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::{
        Block, GlobalSymbol, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasRegions,
        HasRegionsMut, HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut, Id, IsConstant,
        IsPure, IsSpeculatable, IsTerminator, Region, ResultValue, SSAValue, StagedNamePolicy,
        Successor,
    };

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
    enum TestType {
        #[default]
        Any,
    }

    macro_rules! impl_empty_dialect_traits {
        ($dialect:ty) => {
            impl<'a> HasArguments<'a> for $dialect {
                type Iter = std::iter::Empty<&'a SSAValue>;

                fn arguments(&'a self) -> Self::Iter {
                    std::iter::empty()
                }
            }

            impl<'a> HasArgumentsMut<'a> for $dialect {
                type IterMut = std::iter::Empty<&'a mut SSAValue>;

                fn arguments_mut(&'a mut self) -> Self::IterMut {
                    std::iter::empty()
                }
            }

            impl<'a> HasResults<'a> for $dialect {
                type Iter = std::iter::Empty<&'a ResultValue>;

                fn results(&'a self) -> Self::Iter {
                    std::iter::empty()
                }
            }

            impl<'a> HasResultsMut<'a> for $dialect {
                type IterMut = std::iter::Empty<&'a mut ResultValue>;

                fn results_mut(&'a mut self) -> Self::IterMut {
                    std::iter::empty()
                }
            }

            impl<'a> HasBlocks<'a> for $dialect {
                type Iter = std::iter::Empty<&'a Block>;

                fn blocks(&'a self) -> Self::Iter {
                    std::iter::empty()
                }
            }

            impl<'a> HasBlocksMut<'a> for $dialect {
                type IterMut = std::iter::Empty<&'a mut Block>;

                fn blocks_mut(&'a mut self) -> Self::IterMut {
                    std::iter::empty()
                }
            }

            impl<'a> HasSuccessors<'a> for $dialect {
                type Iter = std::iter::Empty<&'a Successor>;

                fn successors(&'a self) -> Self::Iter {
                    std::iter::empty()
                }
            }

            impl<'a> HasSuccessorsMut<'a> for $dialect {
                type IterMut = std::iter::Empty<&'a mut Successor>;

                fn successors_mut(&'a mut self) -> Self::IterMut {
                    std::iter::empty()
                }
            }

            impl<'a> HasRegions<'a> for $dialect {
                type Iter = std::iter::Empty<&'a Region>;

                fn regions(&'a self) -> Self::Iter {
                    std::iter::empty()
                }
            }

            impl<'a> HasRegionsMut<'a> for $dialect {
                type IterMut = std::iter::Empty<&'a mut Region>;

                fn regions_mut(&'a mut self) -> Self::IterMut {
                    std::iter::empty()
                }
            }

            impl IsTerminator for $dialect {
                fn is_terminator(&self) -> bool {
                    false
                }
            }

            impl IsConstant for $dialect {
                fn is_constant(&self) -> bool {
                    false
                }
            }

            impl IsPure for $dialect {
                fn is_pure(&self) -> bool {
                    true
                }
            }

            impl IsSpeculatable for $dialect {
                fn is_speculatable(&self) -> bool {
                    true
                }
            }
        };
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct LangA;
    impl_empty_dialect_traits!(LangA);
    impl Dialect for LangA {
        type Type = TestType;
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct LangB;
    impl_empty_dialect_traits!(LangB);
    impl Dialect for LangB {
        type Type = TestType;
    }

    #[derive(Debug)]
    enum TestStage {
        A(StageInfo<LangA>),
        B(StageInfo<LangB>),
    }

    impl HasStageInfo<LangA> for TestStage {
        fn try_stage_info(&self) -> Option<&StageInfo<LangA>> {
            match self {
                TestStage::A(stage) => Some(stage),
                TestStage::B(_) => None,
            }
        }

        fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<LangA>> {
            match self {
                TestStage::A(stage) => Some(stage),
                TestStage::B(_) => None,
            }
        }
    }

    impl HasStageInfo<LangB> for TestStage {
        fn try_stage_info(&self) -> Option<&StageInfo<LangB>> {
            match self {
                TestStage::A(_) => None,
                TestStage::B(stage) => Some(stage),
            }
        }

        fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<LangB>> {
            match self {
                TestStage::A(_) => None,
                TestStage::B(stage) => Some(stage),
            }
        }
    }

    impl StageMeta for TestStage {
        type Languages = (LangA, (LangB, ()));

        fn stage_name(&self) -> Option<GlobalSymbol> {
            match self {
                TestStage::A(stage) => stage.name(),
                TestStage::B(stage) => stage.name(),
            }
        }

        fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
            match self {
                TestStage::A(stage) => stage.set_name(name),
                TestStage::B(stage) => stage.set_name(name),
            }
        }

        fn stage_id(&self) -> Option<CompileStage> {
            match self {
                TestStage::A(stage) => stage.stage_id(),
                TestStage::B(stage) => stage.stage_id(),
            }
        }

        fn set_stage_id(&mut self, id: Option<CompileStage>) {
            match self {
                TestStage::A(stage) => stage.set_stage_id(id),
                TestStage::B(stage) => stage.set_stage_id(id),
            }
        }

        fn from_stage_name(stage_name: &str) -> Result<Self, String> {
            match stage_name {
                "a" => Ok(TestStage::A(StageInfo::<LangA>::default())),
                "b" => Ok(TestStage::B(StageInfo::<LangB>::default())),
                _ => Err(format!("unknown stage '{stage_name}'")),
            }
        }

        fn declared_stage_names() -> &'static [&'static str] {
            &["a", "b"]
        }
    }

    struct IdentifyStage;

    impl StageAction<TestStage, LangA> for IdentifyStage {
        type Output = &'static str;
        type Error = Infallible;

        fn run(
            &mut self,
            stage_id: CompileStage,
            stage: &StageInfo<LangA>,
        ) -> Result<Self::Output, Self::Error> {
            assert_eq!(stage.stage_id(), Some(stage_id));
            Ok("A")
        }
    }

    impl StageAction<TestStage, LangB> for IdentifyStage {
        type Output = &'static str;
        type Error = Infallible;

        fn run(
            &mut self,
            stage_id: CompileStage,
            stage: &StageInfo<LangB>,
        ) -> Result<Self::Output, Self::Error> {
            assert_eq!(stage.stage_id(), Some(stage_id));
            Ok("B")
        }
    }

    struct SetPolicy;

    impl StageActionMut<TestStage, LangA> for SetPolicy {
        type Output = &'static str;
        type Error = Infallible;

        fn run(
            &mut self,
            stage_id: CompileStage,
            stage: &mut StageInfo<LangA>,
        ) -> Result<Self::Output, Self::Error> {
            assert_eq!(stage.stage_id(), Some(stage_id));
            stage.set_staged_name_policy(StagedNamePolicy::MultipleDispatch);
            Ok("A")
        }
    }

    impl StageActionMut<TestStage, LangB> for SetPolicy {
        type Output = &'static str;
        type Error = Infallible;

        fn run(
            &mut self,
            stage_id: CompileStage,
            stage: &mut StageInfo<LangB>,
        ) -> Result<Self::Output, Self::Error> {
            assert_eq!(stage.stage_id(), Some(stage_id));
            stage.set_staged_name_policy(StagedNamePolicy::MultipleDispatch);
            Ok("B")
        }
    }

    #[test]
    fn dispatch_stage_runs_matching_language_action() {
        let mut pipeline: Pipeline<TestStage> = Pipeline::new();
        let a = pipeline
            .add_stage()
            .stage(TestStage::A(StageInfo::default()))
            .name("a")
            .new();
        let b = pipeline
            .add_stage()
            .stage(TestStage::B(StageInfo::default()))
            .name("b")
            .new();

        let mut action = IdentifyStage;
        assert_eq!(pipeline.dispatch_stage(a, &mut action).unwrap(), Some("A"));
        assert_eq!(pipeline.dispatch_stage(b, &mut action).unwrap(), Some("B"));

        let missing = CompileStage::new(Id(999));
        assert_eq!(pipeline.dispatch_stage(missing, &mut action).unwrap(), None);
    }

    #[test]
    fn dispatch_stage_mut_runs_matching_language_action() {
        let mut pipeline: Pipeline<TestStage> = Pipeline::new();
        let a = pipeline
            .add_stage()
            .stage(TestStage::A(StageInfo::default()))
            .name("a")
            .new();
        let b = pipeline
            .add_stage()
            .stage(TestStage::B(StageInfo::default()))
            .name("b")
            .new();

        let mut action = SetPolicy;
        assert_eq!(
            pipeline.dispatch_stage_mut(a, &mut action).unwrap(),
            Some("A")
        );
        assert_eq!(
            pipeline.dispatch_stage_mut(b, &mut action).unwrap(),
            Some("B")
        );

        let a_policy = match pipeline.stage(a).unwrap() {
            TestStage::A(stage) => stage.staged_name_policy(),
            TestStage::B(_) => panic!("expected stage A"),
        };
        let b_policy = match pipeline.stage(b).unwrap() {
            TestStage::A(_) => panic!("expected stage B"),
            TestStage::B(stage) => stage.staged_name_policy(),
        };

        assert_eq!(a_policy, StagedNamePolicy::MultipleDispatch);
        assert_eq!(b_policy, StagedNamePolicy::MultipleDispatch);

        let missing = CompileStage::new(Id(999));
        assert_eq!(
            pipeline.dispatch_stage_mut(missing, &mut action).unwrap(),
            None
        );
    }
}
