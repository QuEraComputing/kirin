#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PartialStruct<S, F, V> {
    shape: S,
    fields: Vec<(F, V)>,
}

impl<S, F, V> PartialStruct<S, F, V> {
    pub fn new(shape: S, fields: Vec<(F, V)>) -> Self {
        Self { shape, fields }
    }

    pub fn shape(&self) -> &S {
        &self.shape
    }

    pub fn fields(&self) -> &[(F, V)] {
        &self.fields
    }

    pub fn into_parts(self) -> (S, Vec<(F, V)>) {
        (self.shape, self.fields)
    }

    pub fn field(&self, field: &F) -> Option<&V>
    where
        F: PartialEq,
    {
        self.fields
            .iter()
            .find_map(|(key, value)| (key == field).then_some(value))
    }

    pub fn same_shape(&self, other: &Self) -> bool
    where
        S: PartialEq,
        F: PartialEq,
    {
        self.shape == other.shape
            && self.fields.len() == other.fields.len()
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|((lhs, _), (rhs, _))| lhs == rhs)
    }

    pub fn zip_map(&self, other: &Self, map: impl Fn(&V, &V) -> V) -> Option<Self>
    where
        S: Clone + PartialEq,
        F: Clone + PartialEq,
    {
        self.same_shape(other).then(|| {
            Self::new(
                self.shape.clone(),
                self.fields
                    .iter()
                    .zip(other.fields.iter())
                    .map(|((field, lhs), (_, rhs))| (field.clone(), map(lhs, rhs)))
                    .collect(),
            )
        })
    }
}
