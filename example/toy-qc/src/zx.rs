use crate::types::QubitType;
use kirin::prelude::*;

/// Function body holding an UnGraph for ZX-stage programs.
/// ZX calculus diagrams are undirected graphs: wires are edges
/// and spiders/boxes are nodes connected by those edges.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "{:signature} {body}")]
pub struct ZXFunction {
    pub body: UnGraph,
}

// TODO(RFC-0004): Replace with `sig: Signature<QubitType>` field
impl HasSignature<ZX> for ZXFunction {
    fn signature(&self) -> Option<Signature<QubitType>> {
        None
    }
}

/// Wire edge operation — creates an edge SSAValue.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(edge, builders, type = QubitType)]
#[chumsky(format = "$wire -> {res:type}")]
pub struct Wire {
    pub res: ResultValue,
}

/// Z-spider node (green spider in ZX calculus).
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$z_spider({angle}) {legs}")]
pub struct ZSpider {
    pub angle: f64,
    pub legs: Vec<SSAValue>,
}

/// X-spider node (red spider in ZX calculus).
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$x_spider({angle}) {legs}")]
pub struct XSpider {
    pub angle: f64,
    pub legs: Vec<SSAValue>,
}

/// Hadamard box node.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "$h_box {src}, {dst}")]
pub struct HBox {
    pub src: SSAValue,
    pub dst: SSAValue,
}

/// ZX calculus dialect language enum.
#[derive(Clone, Debug, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
pub enum ZX {
    #[wraps]
    Wire(Wire),
    #[wraps]
    ZSpider(ZSpider),
    #[wraps]
    XSpider(XSpider),
    #[wraps]
    HBox(HBox),
    #[wraps]
    ZXFunction(ZXFunction),
}
