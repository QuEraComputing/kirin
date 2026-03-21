use std::convert::Infallible;

use super::*;
use crate::{
    Block, CompileStage, DiGraph, Dialect, GlobalSymbol, HasArguments, HasArgumentsMut, HasBlocks,
    HasBlocksMut, HasDigraphs, HasDigraphsMut, HasRegions, HasRegionsMut, HasResults,
    HasResultsMut, HasStageInfo, HasSuccessors, HasSuccessorsMut, HasUngraphs, HasUngraphsMut, Id,
    IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator, Pipeline, Region, ResultValue,
    SSAValue, StageInfo, StageMeta, StagedNamePolicy, Successor, UnGraph,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
enum TestType {
    #[default]
    Any,
}

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestType::Any => write!(f, "Any"),
        }
    }
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

        impl<'a> HasDigraphs<'a> for $dialect {
            type Iter = std::iter::Empty<&'a DiGraph>;
            fn digraphs(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }

        impl<'a> HasDigraphsMut<'a> for $dialect {
            type IterMut = std::iter::Empty<&'a mut DiGraph>;
            fn digraphs_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }

        impl<'a> HasUngraphs<'a> for $dialect {
            type Iter = std::iter::Empty<&'a UnGraph>;
            fn ungraphs(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }

        impl<'a> HasUngraphsMut<'a> for $dialect {
            type IterMut = std::iter::Empty<&'a mut UnGraph>;
            fn ungraphs_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }

        impl IsEdge for $dialect {
            fn is_edge(&self) -> bool {
                false
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

#[derive(Debug)]
enum AOnlyStage {
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

impl HasStageInfo<LangA> for AOnlyStage {
    fn try_stage_info(&self) -> Option<&StageInfo<LangA>> {
        match self {
            AOnlyStage::A(stage) => Some(stage),
            AOnlyStage::B(_) => None,
        }
    }

    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<LangA>> {
        match self {
            AOnlyStage::A(stage) => Some(stage),
            AOnlyStage::B(_) => None,
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

impl HasStageInfo<LangB> for AOnlyStage {
    fn try_stage_info(&self) -> Option<&StageInfo<LangB>> {
        match self {
            AOnlyStage::A(_) => None,
            AOnlyStage::B(stage) => Some(stage),
        }
    }

    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<LangB>> {
        match self {
            AOnlyStage::A(_) => None,
            AOnlyStage::B(stage) => Some(stage),
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

impl StageMeta for AOnlyStage {
    type Languages = (LangA, ());

    fn stage_name(&self) -> Option<GlobalSymbol> {
        match self {
            AOnlyStage::A(stage) => stage.name(),
            AOnlyStage::B(stage) => stage.name(),
        }
    }

    fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
        match self {
            AOnlyStage::A(stage) => stage.set_name(name),
            AOnlyStage::B(stage) => stage.set_name(name),
        }
    }

    fn stage_id(&self) -> Option<CompileStage> {
        match self {
            AOnlyStage::A(stage) => stage.stage_id(),
            AOnlyStage::B(stage) => stage.stage_id(),
        }
    }

    fn set_stage_id(&mut self, id: Option<CompileStage>) {
        match self {
            AOnlyStage::A(stage) => stage.set_stage_id(id),
            AOnlyStage::B(stage) => stage.set_stage_id(id),
        }
    }

    fn from_stage_name(stage_name: &str) -> Result<Self, String> {
        match stage_name {
            "a" => Ok(AOnlyStage::A(StageInfo::<LangA>::default())),
            "b" => Ok(AOnlyStage::B(StageInfo::<LangB>::default())),
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

struct IdentifyAOnly;

impl StageAction<AOnlyStage, LangA> for IdentifyAOnly {
    type Output = &'static str;
    type Error = StageDispatchMiss;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &StageInfo<LangA>,
    ) -> Result<Self::Output, Self::Error> {
        assert_eq!(stage.stage_id(), Some(stage_id));
        Ok("A")
    }
}

struct SetPolicyAOnly;

impl StageActionMut<AOnlyStage, LangA> for SetPolicyAOnly {
    type Output = &'static str;
    type Error = StageDispatchMiss;

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

#[test]
fn dispatch_stage_or_else_reports_miss_kind() {
    let mut pipeline: Pipeline<AOnlyStage> = Pipeline::new();
    let a = pipeline
        .add_stage()
        .stage(AOnlyStage::A(StageInfo::default()))
        .name("a")
        .new();
    let b = pipeline
        .add_stage()
        .stage(AOnlyStage::B(StageInfo::default()))
        .name("b")
        .new();

    let mut action = IdentifyAOnly;
    assert_eq!(
        pipeline.dispatch_stage_or_else(a, &mut action, |miss| miss),
        Ok("A")
    );
    assert_eq!(
        pipeline.dispatch_stage_or_else(b, &mut action, |miss| miss),
        Err(StageDispatchMiss::MissingDialect)
    );

    let missing = CompileStage::new(Id(999));
    assert_eq!(
        pipeline.dispatch_stage_or_else(missing, &mut action, |miss| miss),
        Err(StageDispatchMiss::MissingStage)
    );
}

#[test]
fn dispatch_stage_mut_or_else_reports_miss_kind() {
    let mut pipeline: Pipeline<AOnlyStage> = Pipeline::new();
    let a = pipeline
        .add_stage()
        .stage(AOnlyStage::A(StageInfo::default()))
        .name("a")
        .new();
    let b = pipeline
        .add_stage()
        .stage(AOnlyStage::B(StageInfo::default()))
        .name("b")
        .new();

    let mut action = SetPolicyAOnly;
    assert_eq!(
        pipeline.dispatch_stage_mut_or_else(a, &mut action, |miss| miss),
        Ok("A")
    );
    assert_eq!(
        pipeline.dispatch_stage_mut_or_else(b, &mut action, |miss| miss),
        Err(StageDispatchMiss::MissingDialect)
    );

    let missing = CompileStage::new(Id(999));
    assert_eq!(
        pipeline.dispatch_stage_mut_or_else(missing, &mut action, |miss| miss),
        Err(StageDispatchMiss::MissingStage)
    );
}

#[test]
fn dispatch_stage_required_reports_miss_kind() {
    let mut pipeline: Pipeline<AOnlyStage> = Pipeline::new();
    let a = pipeline
        .add_stage()
        .stage(AOnlyStage::A(StageInfo::default()))
        .name("a")
        .new();
    let b = pipeline
        .add_stage()
        .stage(AOnlyStage::B(StageInfo::default()))
        .name("b")
        .new();

    let mut action = IdentifyAOnly;
    assert_eq!(pipeline.dispatch_stage_required(a, &mut action), Ok("A"));
    assert!(matches!(
        pipeline.dispatch_stage_required(b, &mut action),
        Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::MissingDialect
        ))
    ));

    let missing = CompileStage::new(Id(999));
    assert!(matches!(
        pipeline.dispatch_stage_required(missing, &mut action),
        Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::MissingStage
        ))
    ));
}

#[test]
fn dispatch_stage_mut_required_reports_miss_kind() {
    let mut pipeline: Pipeline<AOnlyStage> = Pipeline::new();
    let a = pipeline
        .add_stage()
        .stage(AOnlyStage::A(StageInfo::default()))
        .name("a")
        .new();
    let b = pipeline
        .add_stage()
        .stage(AOnlyStage::B(StageInfo::default()))
        .name("b")
        .new();

    let mut action = SetPolicyAOnly;
    assert_eq!(
        pipeline.dispatch_stage_mut_required(a, &mut action),
        Ok("A")
    );
    assert!(matches!(
        pipeline.dispatch_stage_mut_required(b, &mut action),
        Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::MissingDialect
        ))
    ));

    let missing = CompileStage::new(Id(999));
    assert!(matches!(
        pipeline.dispatch_stage_mut_required(missing, &mut action),
        Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::MissingStage
        ))
    ));
}
