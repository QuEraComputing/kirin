use kirin_ir::{Block, CompileStage, ResultValue, SpecializedFunction};

pub enum Control<V, Ext> {
    Advance,
    Jump(Block, Vec<V>),
    Fork(Vec<(Block, Vec<V>)>),
    Call {
        callee: SpecializedFunction,
        stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    },
    Return(V),
    Yield(V),
    Ext(Ext),
}

impl<V, Ext> Control<V, Ext> {
    pub fn map_ext<Ext2>(self, f: impl FnOnce(Ext) -> Ext2) -> Control<V, Ext2> {
        match self {
            Control::Advance => Control::Advance,
            Control::Jump(b, a) => Control::Jump(b, a),
            Control::Fork(branches) => Control::Fork(branches),
            Control::Call {
                callee,
                stage,
                args,
                results,
            } => Control::Call {
                callee,
                stage,
                args,
                results,
            },
            Control::Return(v) => Control::Return(v),
            Control::Yield(v) => Control::Yield(v),
            Control::Ext(e) => Control::Ext(f(e)),
        }
    }
}

pub enum CursorExt<C> {
    Push(C),
    Pop,
}

impl<V, Ext> From<()> for Control<V, Ext> {
    fn from(_: ()) -> Self {
        Control::Advance
    }
}
