use std::convert::Infallible;

use super::*;
use crate::{
    Block, CFG, CompileStage, DiGraph, Dialect, GlobalSymbol, HasArguments, HasArgumentsMut,
    HasBlocks, HasBlocksMut, HasCFG, HasCFGMut, HasDigraphs, HasDigraphsMut, HasResults,
    HasResultsMut, HasStageInfo, HasSuccessors, HasSuccessorsMut, HasUngraphs, HasUngraphsMut, Id,
    IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator, Pipeline, ResultValue, SSAValue,
    StageInfo, StageMeta, StagedNamePolicy, Successor, UnGraph,
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

        impl<'a> HasCFG<'a> for $dialect {
            type Iter = std::iter::Empty<&'a CFG>;

            fn cfgs(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }

        impl<'a> HasCFGMut<'a> for $dialect {
            type IterMut = std::iter::Empty<&'a mut CFG>;

            fn cfgs_mut(&'a mut self) -> Self::IterMut {
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

/// `Quarantined::from_stage` renders from the stage info inside the stage, not
/// from the stage itself — a pipeline slot holds the whole stage enum, so that
/// is what must be taken custody of, but only a `StageInfo` can be dumped.
#[test]
fn from_stage_renders_the_stage_info_inside_the_stage() {
    let stage = TestStage::A(StageInfo::<LangA>::default());

    let quarantined = crate::Quarantined::from_stage::<LangA>(
        stage,
        crate::QuarantineCause::Panic("boom".to_string()),
        Vec::new(),
    );

    assert!(
        quarantined.report().contains("== statements"),
        "expected an arena dump, got:\n{}",
        quarantined.report()
    );
}

/// A stage that holds no stage info for the dialect asked for still produces an
/// artifact. Unreachable through the pass wrapper, which only quarantines a
/// stage whose stage info it just rewrote, but the constructor must not unwind
/// on a path that is already handling a failure.
#[test]
fn from_stage_falls_back_when_the_dialect_does_not_match() {
    // `TestStage::B` holds `LangB` stage info, so `HasStageInfo<LangA>` misses.
    let stage = TestStage::B(StageInfo::<LangB>::default());

    let quarantined = crate::Quarantined::from_stage::<LangA>(
        stage,
        crate::QuarantineCause::Panic("boom".to_string()),
        Vec::new(),
    );

    assert!(
        quarantined.report().contains("== no stage info =="),
        "expected the stage-info-less fallback, got:\n{}",
        quarantined.report()
    );
    assert!(matches!(
        quarantined.cause(),
        crate::QuarantineCause::Panic(_)
    ));
}

// ---------------------------------------------------------------------------
// `Pipeline::run_pass`
//
// The wrapper's own job is the slot state machine around the pass: which
// outcomes leave the pipeline untouched, and which one kills a position. The
// pass mechanics it delegates to are covered by `tests/pass.rs`.
// ---------------------------------------------------------------------------

use crate::{RewriteError, StagePassError};

/// A pipeline whose only position holds a `LangA` stage.
fn lang_a_pipeline() -> (Pipeline<TestStage>, CompileStage) {
    let mut pipeline: Pipeline<TestStage> = Pipeline::new();
    let id = pipeline
        .add_stage()
        .stage(TestStage::A(StageInfo::default()))
        .name("source")
        .new();
    (pipeline, id)
}

#[test]
fn run_pass_returns_the_pass_value_and_leaves_the_stage_usable() {
    let (mut pipeline, id) = lang_a_pipeline();

    let output = pipeline
        .run_pass::<LangA, _, _, RewriteError>(id, |_rewriter| Ok(7))
        .expect("a pass that succeeds hands its stage back");

    assert_eq!(output, 7);
    assert!(
        pipeline.stage(id).is_some(),
        "the stage returned to its slot"
    );
    assert!(pipeline.quarantined(id).is_none());
}

#[test]
fn run_pass_poisons_the_position_when_the_pass_fails() {
    let (mut pipeline, id) = lang_a_pipeline();

    let error = pipeline
        .run_pass::<LangA, _, (), _>(id, |_rewriter| Err(RewriteError::CannotInsertTerminator))
        .expect_err("a failing pass must not hand back a usable stage");

    assert_eq!(error, StagePassError::PassFailed);
    assert!(
        pipeline.stage(id).is_none(),
        "a poisoned position must not lend its stage out again"
    );
    assert!(
        pipeline.quarantined(id).is_some(),
        "the failure artifact stays readable at that position"
    );
}

#[test]
fn a_poisoned_position_keeps_its_name_and_id() {
    let (mut pipeline, id) = lang_a_pipeline();
    let _ = pipeline.run_pass::<LangA, _, (), _>(id, |_| Err(RewriteError::CannotInsertTerminator));

    // `CompileStage` is a raw index, so poisoning must not renumber anything.
    assert_eq!(pipeline.stage_by_name("source"), Some(id));
}

#[test]
fn run_pass_restores_the_stage_when_the_dialect_does_not_match() {
    let (mut pipeline, id) = lang_a_pipeline();

    // The position holds a `LangA` stage, so a `LangB` pass cannot run on it.
    let error = pipeline
        .run_pass::<LangB, _, (), RewriteError>(id, |_rewriter| Ok(()))
        .expect_err("a pass for the wrong dialect cannot run");

    assert_eq!(error, StagePassError::DialectMismatch);
    assert!(
        pipeline.stage(id).is_some(),
        "nothing ran, so the stage must go back rather than strand the slot"
    );
    assert!(pipeline.quarantined(id).is_none());
}

#[test]
fn run_pass_refuses_an_unknown_position() {
    let (mut pipeline, _) = lang_a_pipeline();
    let missing = CompileStage::new(Id(999));

    let error = pipeline
        .run_pass::<LangA, _, (), RewriteError>(missing, |_| Ok(()))
        .expect_err("there is no such position");

    assert_eq!(error, StagePassError::UnknownStage);
}

#[test]
fn run_pass_refuses_a_position_an_earlier_pass_poisoned() {
    let (mut pipeline, id) = lang_a_pipeline();
    let _ = pipeline.run_pass::<LangA, _, (), _>(id, |_| Err(RewriteError::CannotInsertTerminator));

    let error = pipeline
        .run_pass::<LangA, _, (), RewriteError>(id, |_| Ok(()))
        .expect_err("a dead stage is refused, not rewritten again");

    assert_eq!(error, StagePassError::StagePoisoned);
}

#[test]
fn poisoning_one_position_leaves_the_others_usable() {
    let mut pipeline: Pipeline<TestStage> = Pipeline::new();
    let first = pipeline
        .add_stage()
        .stage(TestStage::A(StageInfo::default()))
        .name("first")
        .new();
    let second = pipeline
        .add_stage()
        .stage(TestStage::A(StageInfo::default()))
        .name("second")
        .new();

    let _ =
        pipeline.run_pass::<LangA, _, (), _>(first, |_| Err(RewriteError::CannotInsertTerminator));

    assert!(pipeline.stage(first).is_none(), "only `first` is dead");
    assert!(pipeline.stage(second).is_some());
    assert_eq!(
        pipeline.run_pass::<LangA, _, _, RewriteError>(second, |_| Ok(1)),
        Ok(1)
    );
}

#[test]
fn stage_status_names_all_three_states() {
    let (mut pipeline, id) = lang_a_pipeline();
    assert_eq!(pipeline.stage_status(id), Some(StageStatus::Present));

    let _ = pipeline.run_pass::<LangA, _, (), _>(id, |_| Err(RewriteError::CannotInsertTerminator));
    assert_eq!(pipeline.stage_status(id), Some(StageStatus::Poisoned));

    let missing = CompileStage::new(Id(999));
    assert_eq!(pipeline.stage_status(missing), None);
}

#[test]
fn dispatch_distinguishes_a_poisoned_position_from_a_missing_one() {
    let mut pipeline: Pipeline<AOnlyStage> = Pipeline::new();
    let a = pipeline
        .add_stage()
        .stage(AOnlyStage::A(StageInfo::default()))
        .name("a")
        .new();

    let _ = pipeline.run_pass::<LangA, _, (), _>(a, |_| Err(RewriteError::CannotInsertTerminator));

    // The position is real and still named; only its stage is gone. Reporting
    // `MissingStage` would send someone looking for a bad stage id instead of
    // the pass that killed this one.
    let mut action = IdentifyAOnly;
    assert_eq!(
        pipeline.dispatch_stage_or_else(a, &mut action, |miss| miss),
        Err(StageDispatchMiss::StageUnavailable)
    );
    assert_eq!(
        pipeline.dispatch_stage_required(a, &mut action),
        Err(StageDispatchRequiredError::Miss(
            StageDispatchMiss::StageUnavailable
        ))
    );

    let missing = CompileStage::new(Id(999));
    assert_eq!(
        pipeline.dispatch_stage_or_else(missing, &mut action, |miss| miss),
        Err(StageDispatchMiss::MissingStage)
    );
}

#[test]
fn stage_names_lists_every_position_including_poisoned_ones() {
    let mut pipeline: Pipeline<TestStage> = Pipeline::new();
    let first = pipeline
        .add_stage()
        .stage(TestStage::A(StageInfo::default()))
        .name("first")
        .new();
    let second = pipeline
        .add_stage()
        .stage(TestStage::A(StageInfo::default()))
        .name("second")
        .new();

    let _ =
        pipeline.run_pass::<LangA, _, (), _>(first, |_| Err(RewriteError::CannotInsertTerminator));

    // `stages()` skips the poisoned position; `stage_names()` must not, or
    // every id derived from its iteration order is off by one.
    assert_eq!(pipeline.stages().count(), 1);
    let listed: Vec<CompileStage> = pipeline.stage_names().map(|(id, _)| id).collect();
    assert_eq!(listed, vec![first, second]);
}

// ---------------------------------------------------------------------------
// `StageInfo::with_builder` panic safety
// ---------------------------------------------------------------------------

#[test]
fn a_panic_inside_with_builder_keeps_what_was_built() {
    let mut stage: StageInfo<LangA> = StageInfo::default();
    stage.with_builder(|b| {
        b.statement().definition(LangA).new();
        b.statement().definition(LangA).new();
    });
    assert_eq!(stage.statement_arena().len(), 2);

    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stage.with_builder(|b| {
            b.statement().definition(LangA).new();
            panic!("construction failed halfway");
        })
    }));

    // The panic still reaches the caller, payload intact.
    let payload = caught.expect_err("the panic must not be swallowed");
    assert_eq!(
        payload.downcast_ref::<&'static str>(),
        Some(&"construction failed halfway")
    );

    // The outward conversion leaves a `Default` behind. Keeping it would make
    // the stage look like a program that compiled to nothing. Instead the
    // builder's contents are reverted.
    assert_eq!(
        stage.statement_arena().len(),
        3,
        "a panic must not empty the stage"
    );
}
