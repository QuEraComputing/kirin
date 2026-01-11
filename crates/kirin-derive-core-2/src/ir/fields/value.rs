use darling::FromField;
use std::ops::{Deref, DerefMut};

use crate::ir::attrs::KirinFieldOptions;

use super::{collection::Collection, index::FieldIndex};

/// Represents all the `SSAValue` arguments of a statement.
#[derive(Debug, Clone, Default)]
pub struct Arguments {
    data: Vec<Argument>,
}

#[derive(Debug, Clone, Default)]
pub struct Results {
    data: Vec<Result>,
}

impl Arguments {
    pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
        let opts = KirinFieldOptions::from_field(f)?;
        let Some(coll) = Collection::from_type(&f.ty, "SSAValue") else {
            return Ok(false);
        };

        self.data.push(Argument::new(
            FieldIndex::new(f.ident.clone(), index),
            opts.ssa_ty
                .unwrap_or_else(|| syn::parse_quote! { Default::default() }),
            coll,
        ));
        Ok(true)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Argument> {
        self.data.iter()
    }
}

impl Results {
    pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
        let opts = KirinFieldOptions::from_field(f)?;
        let Some(coll) = Collection::from_type(&f.ty, "ResultValue") else {
            return Ok(false);
        };
        self.data.push(Result::new(
            FieldIndex::new(f.ident.clone(), index),
            opts.ssa_ty
                .unwrap_or_else(|| syn::parse_quote! { Default::default() }),
            coll,
        ));
        Ok(true)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Result> {
        self.data.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Argument(pub Value);

impl Argument {
    pub fn new(field: FieldIndex, ty: syn::Expr, collection: Collection) -> Self {
        Argument(Value {
            field,
            ty,
            collection,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Result(pub(crate) Value);

impl Result {
    pub fn new(field: FieldIndex, ty: syn::Expr, collection: Collection) -> Self {
        Result(Value {
            field,
            ty,
            collection,
        })
    }
}

impl Deref for Argument {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for Result {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Result {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for Argument {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Represents a field declared as `SSAValue` or `ResultValue`, requires
/// the helper attribute to specify the type or it will be `Default::default()`.
#[derive(Debug, Clone)]
pub struct Value {
    pub field: FieldIndex,
    pub ty: syn::Expr,
    pub collection: Collection,
}
