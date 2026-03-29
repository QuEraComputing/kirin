use kirin_ir::{
    Dialect, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasDigraphs, HasDigraphsMut,
    HasRegions, HasRegionsMut, HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut,
    HasUngraphs, HasUngraphsMut, IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator,
};

use crate::{Interpreter, Lift, Machine};

/// Wrapper for testing total (Cursor-returning) dialects in a SingleStage shell.
///
/// The shell requires `L::Effect = M::Effect`. Total dialects return `Cursor`,
/// which doesn't match `Flow<V>`. `Total<D>` lifts `Cursor` into the machine's
/// effect type via `Lift`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Total<D>(pub D);

// ---------------------------------------------------------------------------
// Dialect delegation — all trait impls forward to the inner `D`
// ---------------------------------------------------------------------------

impl<'a, D: HasArguments<'a>> HasArguments<'a> for Total<D> {
    type Iter = D::Iter;
    fn arguments(&'a self) -> Self::Iter {
        self.0.arguments()
    }
}

impl<'a, D: HasArgumentsMut<'a>> HasArgumentsMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn arguments_mut(&'a mut self) -> Self::IterMut {
        self.0.arguments_mut()
    }
}

impl<'a, D: HasResults<'a>> HasResults<'a> for Total<D> {
    type Iter = D::Iter;
    fn results(&'a self) -> Self::Iter {
        self.0.results()
    }
}

impl<'a, D: HasResultsMut<'a>> HasResultsMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn results_mut(&'a mut self) -> Self::IterMut {
        self.0.results_mut()
    }
}

impl<'a, D: HasBlocks<'a>> HasBlocks<'a> for Total<D> {
    type Iter = D::Iter;
    fn blocks(&'a self) -> Self::Iter {
        self.0.blocks()
    }
}

impl<'a, D: HasBlocksMut<'a>> HasBlocksMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn blocks_mut(&'a mut self) -> Self::IterMut {
        self.0.blocks_mut()
    }
}

impl<'a, D: HasSuccessors<'a>> HasSuccessors<'a> for Total<D> {
    type Iter = D::Iter;
    fn successors(&'a self) -> Self::Iter {
        self.0.successors()
    }
}

impl<'a, D: HasSuccessorsMut<'a>> HasSuccessorsMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn successors_mut(&'a mut self) -> Self::IterMut {
        self.0.successors_mut()
    }
}

impl<'a, D: HasRegions<'a>> HasRegions<'a> for Total<D> {
    type Iter = D::Iter;
    fn regions(&'a self) -> Self::Iter {
        self.0.regions()
    }
}

impl<'a, D: HasRegionsMut<'a>> HasRegionsMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn regions_mut(&'a mut self) -> Self::IterMut {
        self.0.regions_mut()
    }
}

impl<'a, D: HasDigraphs<'a>> HasDigraphs<'a> for Total<D> {
    type Iter = D::Iter;
    fn digraphs(&'a self) -> Self::Iter {
        self.0.digraphs()
    }
}

impl<'a, D: HasDigraphsMut<'a>> HasDigraphsMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn digraphs_mut(&'a mut self) -> Self::IterMut {
        self.0.digraphs_mut()
    }
}

impl<'a, D: HasUngraphs<'a>> HasUngraphs<'a> for Total<D> {
    type Iter = D::Iter;
    fn ungraphs(&'a self) -> Self::Iter {
        self.0.ungraphs()
    }
}

impl<'a, D: HasUngraphsMut<'a>> HasUngraphsMut<'a> for Total<D> {
    type IterMut = D::IterMut;
    fn ungraphs_mut(&'a mut self) -> Self::IterMut {
        self.0.ungraphs_mut()
    }
}

impl<D: IsTerminator> IsTerminator for Total<D> {
    fn is_terminator(&self) -> bool {
        self.0.is_terminator()
    }
}

impl<D: IsConstant> IsConstant for Total<D> {
    fn is_constant(&self) -> bool {
        self.0.is_constant()
    }
}

impl<D: IsPure> IsPure for Total<D> {
    fn is_pure(&self) -> bool {
        self.0.is_pure()
    }
}

impl<D: IsSpeculatable> IsSpeculatable for Total<D> {
    fn is_speculatable(&self) -> bool {
        self.0.is_speculatable()
    }
}

impl<D: IsEdge> IsEdge for Total<D> {
    fn is_edge(&self) -> bool {
        self.0.is_edge()
    }
}

impl<D: Dialect> Dialect for Total<D> {
    type Type = D::Type;
}

// ---------------------------------------------------------------------------
// Interpretable — lift Cursor into the interpreter's machine effect
// ---------------------------------------------------------------------------

impl<'ir, I, D> crate::Interpretable<'ir, I> for Total<D>
where
    I: Interpreter<'ir>,
    D: crate::Interpretable<'ir, I>,
    D::Effect: Lift<<I as Machine<'ir>>::Effect>,
    D::Error: Into<<I as crate::ValueStore>::Error>,
{
    type Effect = <I as Machine<'ir>>::Effect;
    type Error = <I as crate::ValueStore>::Error;

    fn interpret(&self, interp: &mut I) -> Result<<I as Machine<'ir>>::Effect, Self::Error> {
        self.0.interpret(interp).map_err(Into::into).map(Lift::lift)
    }
}
