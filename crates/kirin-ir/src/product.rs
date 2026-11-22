use smallvec::SmallVec;

/// An ordered product -- used for both type-level products (multi-result
/// type annotations) and value-level products (packed interpreter values).
///
/// Uses SmallVec for compact inline storage: products with <=2 elements
/// avoid heap allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Product<T>(pub SmallVec<[T; 2]>);

impl<T> Product<T> {
    pub fn new() -> Self {
        Self(SmallVec::new())
    }

    pub fn from_vec(values: Vec<T>) -> Self {
        Self(SmallVec::from_vec(values))
    }

    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.0.iter_mut()
    }

    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }
}

impl<T> Default for Product<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> IntoIterator for Product<T> {
    type Item = T;
    type IntoIter = smallvec::IntoIter<[T; 2]>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Product<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T> FromIterator<T> for Product<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Product(iter.into_iter().collect())
    }
}

impl<T> core::ops::Index<usize> for Product<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        &self.0[index]
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Product<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(")?;
        for (i, elem) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{elem}")?;
        }
        write!(f, ")")
    }
}

/// Construct a `Product` inline.
///
/// ```ignore
/// let types = product![I32, F64];       // Product<MyType>
/// let values = product![v1, v2, v3];    // Product<MyValue>
/// let empty: Product<i32> = product![]; // Product<T> (empty)
/// ```
#[macro_export]
macro_rules! product {
    ($($elem:expr),* $(,)?) => {
        $crate::Product(smallvec::smallvec![$($elem),*])
    };
}

/// A language's own tuple type implements this to wrap a [`Product`].
///
/// A `Product` is the framework container for the values at a boundary and
/// is never itself a type. A language that has tuples defines a tuple type
/// in its type lattice and implements `HasProduct` to build it from, and
/// view it as, a `Product` of its component types. Languages without tuples
/// do not implement it. The value-level counterpart is the interpreter's
/// `HasProductValue`; `kirin-tuple` provides the standard tuple statements.
pub trait HasProduct: Sized {
    /// Wrap a product into this type.
    fn from_product(product: Product<Self>) -> Self;

    /// Extract the product if this is a product type. Returns None otherwise.
    fn as_product(&self) -> Option<&Product<Self>>;
}
