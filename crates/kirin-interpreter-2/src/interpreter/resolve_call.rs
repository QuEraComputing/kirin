use std::marker::PhantomData;

use kirin_ir::{CompileStage, Function, SpecializedFunction, StagedFunction, Symbol};

use super::Interpreter;

pub mod callee {
    use super::{
        CompileStage, Function, Interpreter, PhantomData, ResolveCallee, SpecializedFunction,
        StagedFunction, Symbol,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Target {
        Specialized(SpecializedFunction),
        Staged(StagedFunction),
        Function(Function),
        Symbol(Symbol),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Stage {
        #[default]
        Current,
        Exact(CompileStage),
    }

    /// Policy for selecting a [`StagedFunction`] when callee resolution starts
    /// from a [`Function`] or [`Symbol`].
    ///
    /// This axis answers "which staged view of this function should this call
    /// use?" before specialization selection runs.
    ///
    /// The enum is intentionally broader than the current implementation. It
    /// documents the policy vocabulary that call-like dialect operations may
    /// want to express even if a particular shell only implements a subset.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum StagedPolicy {
        /// Resolve the function in exactly one compile stage.
        ///
        /// With the default builder flow this means:
        /// - use the current activation stage when `.stage(...)` is omitted
        /// - use the explicitly requested stage when `.stage(...)` is present
        ///
        /// Resolution fails when the function has no staged definition for that
        /// stage.
        #[default]
        ExactStage,
    }

    /// Policy for selecting a [`SpecializedFunction`] from a resolved
    /// [`StagedFunction`].
    ///
    /// This axis answers "which specialization of this staged function should
    /// this call invoke?" after target and stage selection are complete.
    ///
    /// The enum defines the public policy language for call conventions. Not
    /// every shell must implement every variant immediately; unsupported
    /// variants should fail explicitly rather than silently falling back to a
    /// different rule.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum SpecializationPolicy {
        /// Require exactly one live specialization to exist.
        ///
        /// Resolution succeeds only when the staged function has a single
        /// non-invalidated specialization. It fails when there are none or when
        /// multiple live specializations remain.
        #[default]
        UniqueLive,
        /// Choose an existing specialization that matches the current call
        /// arguments exactly.
        ///
        /// This is stricter than "best match": coercions, fallback dispatch,
        /// or on-demand specialization materialization are not implied.
        ExactMatch,
        /// Choose the most specific applicable existing specialization for the
        /// current call arguments.
        ///
        /// This is the natural hook for overload resolution, subtype-aware
        /// dispatch, or staged specializations ordered by specificity.
        BestMatch,
        /// Reuse an exact specialization when present, otherwise materialize a
        /// new exact specialization for the current call arguments.
        ///
        /// This is useful for shells that own specialization creation and want
        /// calls to behave like "lookup or instantiate".
        MaterializeExact,
        /// Perform full multiple-dispatch resolution using the call arguments.
        ///
        /// Unlike [`BestMatch`], this variant implies that dispatch may depend
        /// on multiple argument positions as part of the semantic rule.
        MultipleDispatch,
    }

    /// Marker for [`StagedPolicy::ExactStage`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct ExactStage;

    /// Marker for [`SpecializationPolicy::UniqueLive`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct UniqueLive;

    /// Marker for [`SpecializationPolicy::ExactMatch`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct ExactMatch;

    /// Marker for [`SpecializationPolicy::BestMatch`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct BestMatch;

    /// Marker for [`SpecializationPolicy::MaterializeExact`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MaterializeExact;

    /// Marker for [`SpecializationPolicy::MultipleDispatch`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MultipleDispatch;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Query {
        target: Target,
        stage: Stage,
        staged: StagedPolicy,
        specialization: SpecializationPolicy,
    }

    impl Query {
        pub fn target(&self) -> Target {
            self.target
        }

        pub fn stage(&self) -> Stage {
            self.stage
        }

        pub fn staged(&self) -> StagedPolicy {
            self.staged
        }

        pub fn specialization(&self) -> SpecializationPolicy {
            self.specialization
        }
    }

    impl From<ExactStage> for StagedPolicy {
        fn from(_: ExactStage) -> Self {
            Self::ExactStage
        }
    }

    impl From<UniqueLive> for SpecializationPolicy {
        fn from(_: UniqueLive) -> Self {
            Self::UniqueLive
        }
    }

    impl From<ExactMatch> for SpecializationPolicy {
        fn from(_: ExactMatch) -> Self {
            Self::ExactMatch
        }
    }

    impl From<BestMatch> for SpecializationPolicy {
        fn from(_: BestMatch) -> Self {
            Self::BestMatch
        }
    }

    impl From<MaterializeExact> for SpecializationPolicy {
        fn from(_: MaterializeExact) -> Self {
            Self::MaterializeExact
        }
    }

    impl From<MultipleDispatch> for SpecializationPolicy {
        fn from(_: MultipleDispatch) -> Self {
            Self::MultipleDispatch
        }
    }

    pub struct Builder<'a, 'ir, I> {
        interp: &'a I,
        _ir: PhantomData<&'ir ()>,
    }

    impl<'a, 'ir, I> Builder<'a, 'ir, I>
    where
        I: ResolveCallee<'ir>,
    {
        pub(crate) fn new(interp: &'a I) -> Self {
            Self {
                interp,
                _ir: PhantomData,
            }
        }

        pub fn specialized(self, callee: SpecializedFunction) -> SpecializedFunction {
            callee
        }

        pub fn staged(self, staged: StagedFunction) -> QueryBuilder<'a, 'ir, I> {
            QueryBuilder {
                interp: self.interp,
                query: Query {
                    target: Target::Staged(staged),
                    stage: Stage::Current,
                    staged: StagedPolicy::ExactStage,
                    specialization: SpecializationPolicy::UniqueLive,
                },
                _ir: PhantomData,
            }
        }

        pub fn function(self, function: Function) -> QueryBuilder<'a, 'ir, I> {
            QueryBuilder {
                interp: self.interp,
                query: Query {
                    target: Target::Function(function),
                    stage: Stage::Current,
                    staged: StagedPolicy::ExactStage,
                    specialization: SpecializationPolicy::UniqueLive,
                },
                _ir: PhantomData,
            }
        }

        pub fn symbol(self, target: Symbol) -> QueryBuilder<'a, 'ir, I> {
            QueryBuilder {
                interp: self.interp,
                query: Query {
                    target: Target::Symbol(target),
                    stage: Stage::Current,
                    staged: StagedPolicy::ExactStage,
                    specialization: SpecializationPolicy::UniqueLive,
                },
                _ir: PhantomData,
            }
        }
    }

    pub struct QueryBuilder<'a, 'ir, I> {
        interp: &'a I,
        query: Query,
        _ir: PhantomData<&'ir ()>,
    }

    impl<'a, 'ir, I> QueryBuilder<'a, 'ir, I>
    where
        I: ResolveCallee<'ir>,
    {
        pub fn stage(mut self, stage: CompileStage) -> Self {
            self.query.stage = Stage::Exact(stage);
            self
        }

        pub fn staged_by(mut self, policy: impl Into<StagedPolicy>) -> Self {
            self.query.staged = policy.into();
            self
        }

        pub fn specialization(mut self, policy: impl Into<SpecializationPolicy>) -> Self {
            self.query.specialization = policy.into();
            self
        }

        pub fn args(
            self,
            args: &[I::Value],
        ) -> Result<SpecializedFunction, <I as Interpreter<'ir>>::Error> {
            self.interp.resolve_query(self.query, args)
        }
    }
}

/// Request-side call resolution for call-like statements.
pub trait ResolveCall<'ir, I: Interpreter<'ir>> {
    fn resolve_call(
        &self,
        interp: &I,
        args: &[I::Value],
    ) -> Result<SpecializedFunction, <I as Interpreter<'ir>>::Error>;
}

/// Interpreter-side resolution for function callees.
pub trait ResolveCallee<'ir>: Interpreter<'ir> {
    fn callee(&self) -> callee::Builder<'_, 'ir, Self>
    where
        Self: Sized,
    {
        callee::Builder::new(self)
    }

    fn resolve_query(
        &self,
        query: callee::Query,
        args: &[Self::Value],
    ) -> Result<SpecializedFunction, <Self as Interpreter<'ir>>::Error>;
}
