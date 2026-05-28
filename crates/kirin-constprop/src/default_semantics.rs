//! Default `OwnerSemantics` implementation for [`ConstPropOwner`].
//!
//! `DefaultConstPropSemantics` saves users from re-implementing the standard
//! constant propagation owner protocol: function owners build a
//! [`FunctionInvocation`], scf-for location owners build an scf-for body
//! block frame from the location summary, and completions advance either a
//! function-return summary or a loop-carried summary.
//!
//! Users provide per-owner entry arguments and let the framework do the rest.
//! Visit counts are exposed via [`DefaultConstPropSemantics::visits`] for
//! tests and instrumentation.

use core::convert::Infallible;
use std::collections::HashMap;

use kirin_interpreter::{
    Env, FrameDispatch, FunctionInvocation, InterpreterError, OwnerSemantics, ProjectOrSelf,
    StandardCompletion, SummaryEffect,
};
use kirin_ir::{HasBottom, HasTop, Product};

use crate::{ConstPropFunctionOwner, ConstPropLocationSummary, ConstPropOwner, ConstPropSummary};

/// Marker projection trait for completions consumed by const-prop owners.
///
/// `Completion` must project to a [`StandardCompletion::FunctionReturned`]
/// for function owners. Location owners additionally require a projection
/// into the dialect-specific yield completion; that bound is added by the
/// per-summary impl.
pub trait DefaultConstPropCompletion<V>:
    ProjectOrSelf<StandardCompletion<V>, Error = Infallible>
{
}

impl<C, V> DefaultConstPropCompletion<V> for C where
    C: ProjectOrSelf<StandardCompletion<V>, Error = Infallible>
{
}

/// Location summary that knows how to enter its body and advance with a
/// yielded carry product.
///
/// The associated [`Completion`](AdvanceableLocationSummary::Completion) is
/// the dialect-specific completion type that this summary projects from
/// (e.g. `ScfCompletion<V>` for scf-for). Users carrying location summaries
/// must ensure their total completion projects into it.
pub trait AdvanceableLocationSummary<V>: ConstPropLocationSummary<V> + Sized {
    /// Dialect-specific completion projected when the body block yields.
    type Completion;

    /// The body block to execute when entering this location.
    fn body(&self) -> kirin_ir::Block;

    /// Initial arguments for the body block.
    fn body_args(&self) -> Product<V>
    where
        V: Clone;

    /// Advance the summary using the carry product yielded by the last
    /// body execution. Returns `None` if the loop step cannot be performed.
    fn advance_with_carried(self, carried: Product<V>) -> Option<Self>;

    /// Extract the carry product from a yield completion.
    fn carry_from_completion(completion: Self::Completion) -> Option<Product<V>>;
}

/// Default semantics for [`ConstPropOwner`].
///
/// Holds a per-owner argument map keyed by [`ConstPropFunctionOwner`] and a
/// running visit counter so that callers can inspect how many times each
/// owner was analyzed. The entry summary is stashed between `entry_frame`
/// and `complete_owner` so that location-summary advancement can read the
/// summary that was active when the frame was entered.
#[derive(Clone, Debug)]
pub struct DefaultConstPropSemantics<V, Loc = ()> {
    args: HashMap<ConstPropFunctionOwner, Product<V>>,
    visits: HashMap<ConstPropOwner, usize>,
    entry_summary: Option<ConstPropSummary<V, Loc>>,
}

impl<V, Loc> Default for DefaultConstPropSemantics<V, Loc> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<V, Loc> DefaultConstPropSemantics<V, Loc> {
    /// Build empty semantics. Use [`with_args`](Self::with_args) to register
    /// per-owner entry arguments before calling
    /// [`StandardFixpointInterpreter::solve`](kirin_interpreter::StandardFixpointInterpreter::solve).
    pub fn empty() -> Self {
        Self {
            args: HashMap::new(),
            visits: HashMap::new(),
            entry_summary: None,
        }
    }

    /// Build semantics for a single entry-function owner.
    pub fn new(owner: ConstPropFunctionOwner, args: impl IntoIterator<Item = V>) -> Self {
        Self::empty().with_args(owner, args)
    }

    /// Register entry arguments for an additional function owner. When the
    /// fixpoint driver visits that owner for the first time, these arguments
    /// are used to build its entry [`FunctionInvocation`].
    pub fn with_args(
        mut self,
        owner: ConstPropFunctionOwner,
        args: impl IntoIterator<Item = V>,
    ) -> Self {
        self.args.insert(owner, args.into_iter().collect());
        self
    }

    /// Read the visit count for an owner. Returns 0 if the owner was never
    /// analyzed.
    pub fn visits(&self, owner: &ConstPropOwner) -> usize {
        self.visits.get(owner).copied().unwrap_or(0)
    }
}

impl<I, F, C, E, V, Loc> OwnerSemantics<I, ConstPropOwner, ConstPropSummary<V, Loc>, F, C, E>
    for DefaultConstPropSemantics<V, Loc>
where
    I: FrameDispatch<F, V, E> + Env<V, Error = E>,
    C: DefaultConstPropCompletion<V> + ProjectOrSelf<Loc::Completion, Error = Infallible>,
    Loc: AdvanceableLocationSummary<V>,
    V: HasBottom + HasTop + Clone + PartialEq,
    E: From<InterpreterError>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut I,
        owner: &ConstPropOwner,
    ) -> Result<ConstPropSummary<V, Loc>, E> {
        Ok(match owner {
            ConstPropOwner::Function(_) => ConstPropSummary::function_bottom(),
            ConstPropOwner::Location(_) => ConstPropSummary::location_bottom(),
        })
    }

    fn entry_frame(
        &mut self,
        interp: &mut I,
        owner: &ConstPropOwner,
        summary: &ConstPropSummary<V, Loc>,
    ) -> Result<F, E> {
        *self.visits.entry(*owner).or_default() += 1;
        self.entry_summary = Some(summary.clone());
        match owner {
            ConstPropOwner::Function(function_owner) => {
                let args = self.args.get(function_owner).cloned().ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "missing entry args for const-prop function owner",
                    ))
                })?;
                interp.dispatch_function_invocation(FunctionInvocation::new(
                    function_owner.stage,
                    function_owner.target,
                    args,
                ))
            }
            ConstPropOwner::Location(location) => {
                let state = summary.location_state().ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "missing location summary for const-prop location owner",
                    ))
                })?;
                let env = interp.alloc();
                interp.dispatch_block(location.stage, state.body(), env, state.body_args())
            }
        }
    }

    fn complete_owner(
        &mut self,
        _interp: &mut I,
        owner: ConstPropOwner,
        completion: C,
    ) -> Result<SummaryEffect<ConstPropOwner, ConstPropSummary<V, Loc>>, E> {
        let entry = self.entry_summary.take();
        match owner {
            ConstPropOwner::Function(_) => {
                let value = expect_function_return::<C, V, E>(completion)?;
                Ok(SummaryEffect::Update {
                    owner,
                    candidate: ConstPropSummary::function(value),
                })
            }
            ConstPropOwner::Location(_) => {
                let current = entry
                    .and_then(|summary| match summary {
                        ConstPropSummary::Location(state) => state,
                        ConstPropSummary::Function(_) => None,
                    })
                    .ok_or_else(|| {
                        E::from(InterpreterError::Custom(
                            "missing const-prop location summary during completion",
                        ))
                    })?;
                let inner = match completion.project_or_self() {
                    Ok(inner) => inner,
                    Err(_) => {
                        return Err(E::from(InterpreterError::Custom(
                            "expected location yield completion for const-prop location owner",
                        )));
                    }
                };
                let carried = Loc::carry_from_completion(inner).ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "expected location yield completion for const-prop location owner",
                    ))
                })?;
                let next = current
                    .advance_with_carried(carried)
                    .ok_or_else(|| E::from(InterpreterError::LoopStepOverflow))?;
                Ok(SummaryEffect::Update {
                    owner,
                    candidate: ConstPropSummary::location(next),
                })
            }
        }
    }
}

/// Project a completion into the value produced by a function return.
///
/// Errors with [`InterpreterError::ProductArityMismatch`] if the return product
/// has more than one value, or [`InterpreterError::Custom`] if the completion
/// is not a function return.
pub fn expect_function_return<C, V, E>(completion: C) -> Result<V, E>
where
    C: ProjectOrSelf<StandardCompletion<V>, Error = Infallible>,
    E: From<InterpreterError>,
{
    let standard = match completion.project_or_self() {
        Ok(standard) => standard,
        Err(_) => {
            return Err(E::from(InterpreterError::Custom(
                "expected function return completion",
            )));
        }
    };
    match standard {
        StandardCompletion::FunctionReturned(product) => {
            if product.len() != 1 {
                return Err(E::from(InterpreterError::ProductArityMismatch {
                    expected: 1,
                    actual: product.len(),
                }));
            }
            Ok(product.into_iter().next().unwrap())
        }
        _ => Err(E::from(InterpreterError::Custom(
            "expected function return completion",
        ))),
    }
}

/// Trivial location summary used when an analysis has no location-scoped
/// state. With this, [`DefaultConstPropSemantics`] only handles function
/// owners; encountering a location owner is reported as a missing summary.
impl<V> AdvanceableLocationSummary<V> for () {
    type Completion = core::convert::Infallible;

    fn body(&self) -> kirin_ir::Block {
        unreachable!("trivial location summary has no body")
    }

    fn body_args(&self) -> Product<V>
    where
        V: Clone,
    {
        Product::new()
    }

    fn advance_with_carried(self, _carried: Product<V>) -> Option<Self> {
        None
    }

    fn carry_from_completion(completion: Self::Completion) -> Option<Product<V>> {
        match completion {}
    }
}
