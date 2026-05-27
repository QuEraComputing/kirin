use kirin_ir::Product;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PartialTuple<V> {
    elements: Product<V>,
}

impl<V> PartialTuple<V> {
    pub fn new(elements: Product<V>) -> Self {
        Self { elements }
    }

    pub fn from_vec(elements: Vec<V>) -> Self {
        Self::new(Product::from_vec(elements))
    }

    pub fn elements(&self) -> &Product<V> {
        &self.elements
    }

    pub fn into_elements(self) -> Product<V> {
        self.elements
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn same_shape(&self, other: &Self) -> bool {
        self.len() == other.len()
    }

    pub fn zip_map(&self, other: &Self, map: impl Fn(&V, &V) -> V) -> Option<Self> {
        self.same_shape(other).then(|| {
            Self::new(
                self.elements()
                    .iter()
                    .zip(other.elements().iter())
                    .map(|(lhs, rhs)| map(lhs, rhs))
                    .collect(),
            )
        })
    }
}
