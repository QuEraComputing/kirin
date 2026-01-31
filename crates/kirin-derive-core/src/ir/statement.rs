use super::{attrs::StatementOptions, fields::*, layout::Layout};
use darling::{FromDeriveInput, FromVariant};

#[derive(Debug, Clone)]
pub struct Statement<L: Layout> {
    pub name: syn::Ident,
    pub attrs: StatementOptions,
    pub arguments: Arguments,
    pub results: Results,
    pub blocks: Blocks,
    pub successors: Successors,
    pub regions: Regions,
    /// compile-time values of the statement
    pub values: CompileTimeValues<L>,
    pub wraps: Option<Wrapper>,
    pub extra: L::StatementExtra,
    pub extra_attrs: L::ExtraStatementAttrs,
}

impl<L: Layout> Statement<L> {
    pub fn new(
        name: syn::Ident,
        attrs: StatementOptions,
        extra: L::StatementExtra,
        extra_attrs: L::ExtraStatementAttrs,
    ) -> Self {
        Self {
            name,
            attrs,
            arguments: Default::default(),
            results: Default::default(),
            blocks: Default::default(),
            successors: Default::default(),
            regions: Default::default(),
            values: Default::default(),
            wraps: None,
            extra,
            extra_attrs,
        }
    }

    pub fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let syn::Data::Struct(data) = &input.data else {
            return Err(
                darling::Error::custom("Kirin statements can only be derived for structs")
                    .with_span(input),
            );
        };
        let attrs = StatementOptions::from_derive_input(input)?;
        let extra = L::StatementExtra::from_derive_input(input)?;
        let extra_attrs = L::ExtraStatementAttrs::from_derive_input(input)?;
        Statement::new(input.ident.clone(), attrs, extra, extra_attrs).update_fields(
            input.attrs.iter().any(|attr| attr.path().is_ident("wraps")),
            &data.fields,
        )
    }

    pub fn from_variant(wraps: bool, variant: &syn::Variant) -> darling::Result<Self> {
        let attrs = StatementOptions::from_variant(variant)?;
        let extra = L::StatementExtra::from_variant(variant)?;
        let extra_attrs = L::ExtraStatementAttrs::from_variant(variant)?;
        Statement::new(variant.ident.clone(), attrs, extra, extra_attrs).update_fields(
            wraps
                || variant
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("wraps")),
            &variant.fields,
        )
    }

    fn update_fields(mut self, wraps: bool, fields: &syn::Fields) -> darling::Result<Self> {
        let mut errors = darling::Error::accumulator();
        if wraps
            || fields
                .iter()
                .any(|f| f.attrs.iter().any(|attr| attr.path().is_ident("wraps")))
        {
            if fields.len() == 1 {
                self.wraps = Some(Wrapper::new(0, fields.iter().next().unwrap()));
            } else {
                for (i, f) in fields.iter().enumerate() {
                    errors.handle_in(|| {
                        if f.attrs.iter().any(|attr| attr.path().is_ident("wraps")) {
                            self.wraps = Some(Wrapper::new(i, f));
                            Ok(())
                        } else {
                            self.values.add(i, f)?;
                            Ok(())
                        }
                    });
                }
            }

            if self.wraps.is_none() {
                errors.push(
                    darling::Error::custom("No field marked with #[wraps] attribute")
                        .with_span(fields),
                );
            }
            errors.finish()?;
            return Ok(self);
        }

        for (i, f) in fields.iter().enumerate() {
            errors.handle_in(|| {
                if self.arguments.add(i, f)?
                    || self.results.add(i, f)?
                    || self.blocks.add(i, f)?
                    || self.successors.add(i, f)?
                    || self.regions.add(i, f)?
                    || self.values.add(i, f)?
                {
                    return Ok(true);
                }
                Ok(true)
            });
        }
        errors.finish()?;
        Ok(self)
    }
}
