use super::{Attrs, definition::*};
use quote::ToTokens;

pub trait ScanExtra<'src, Input, Output>: Sized {
    fn scan_extra(&self, node: &'src Input) -> syn::Result<Output>;
}

impl<'src, L, Input> ScanExtra<'src, Input, ()> for L {
    fn scan_extra(&self, _node: &'src Input) -> syn::Result<()> {
        Ok(())
    }
}

pub trait ScanInto:
    Layout
    + Sized
    + for<'a> ScanExtra<'a, syn::DeriveInput, Self::StatementExtra>
    + for<'a> ScanExtra<'a, syn::Variant, Self::StatementExtra>
    + for<'a> ScanExtra<'a, syn::Field, Self::FieldExtra>
{
    fn scan<'src>(&self, node: &'src syn::DeriveInput) -> syn::Result<Input<'src, Self>> {
        match &node.data {
            syn::Data::Struct(data) => Ok(Input::Struct(Struct {
                input: node,
                src: data,
                definition: self.scan_into_struct(node)?,
            })),
            syn::Data::Enum(data) => Ok(Input::Enum(Enum {
                input: node,
                src: data,
                definition: self.scan_into_enum(node)?,
            })),
            syn::Data::Union(_) => Err(syn::Error::new_spanned(
                node,
                "Unions are not supported by Kirin",
            )),
        }
    }

    fn scan_into_struct<'src>(
        &self,
        node: &'src syn::DeriveInput,
    ) -> syn::Result<DefinitionStruct<Self>> {
        Ok(DefinitionStruct(self.scan_into_statement(node)?))
    }

    fn scan_into_enum<'src>(
        &self,
        node: &'src syn::DeriveInput,
    ) -> syn::Result<DefinitionEnum<Self>> {
        let syn::Data::Enum(data) = &node.data else {
            return Err(syn::Error::new_spanned(
                node,
                "Expected an enum for DefinitionEnum",
            ));
        };

        let marked_wraps = node.attrs.iter().any(|attr| attr.path().is_ident("wraps"));
        let mut variants = data
            .variants
            .iter()
            .map(|v| self.scan_into_variant(v))
            .collect::<syn::Result<Vec<_>>>()?;

        if marked_wraps {
            variants
                .iter_mut()
                .zip(data.variants.iter())
                // filter only non-explicitly wrapped variants
                .filter(|(variant, _)| !variant.wraps)
                .fold(Ok(()), |acc, (variant, src)| {
                    // if acc is Err combine it with the new error
                    if variant.fields.len() != 1 {
                        acc.map_err(|mut e: syn::Error| {
                            e.combine(
                                syn::Error::new_spanned(
                            src,
                            "This enum is marked as a wrapper, so all variants must have exactly \
                                    one field, or mark a field explicitly as wrapper using #[wraps]",
                                )
                            );
                            e
                        })
                    } else {
                        variant.wraps = true;
                        variant.fields[0].wraps = true;
                        Ok(())
                    }
                })?;
        }

        Ok(DefinitionEnum {
            attrs: node.scan_into_attrs()?,
            marked_wraps,
            variants,
        })
    }

    fn scan_into_variant<'src>(
        &self,
        node: &'src syn::Variant,
    ) -> syn::Result<DefinitionVariant<Self>> {
        Ok(DefinitionVariant(self.scan_into_statement(node)?))
    }

    fn scan_into_statement<'src, Attr, N>(
        &self,
        node: &'src N,
    ) -> syn::Result<DefinitionStatement<Attr, Self>>
    where
        Self: ScanExtra<'src, N, Self::StatementExtra>,
        N: Attrs<Output = Vec<syn::Attribute>> + SynFields + ToTokens + ScanAttr<Attr>,
    {
        let mut wraps = node
            .attrs()
            .iter()
            .any(|attr| attr.path().is_ident("wraps"));
        let mut fields = node
            .fields()
            .iter()
            .map(|f| self.scan_into_field(f))
            .collect::<syn::Result<Vec<_>>>()?;

        if wraps && fields.iter().all(|f| !f.wraps) {
            if fields.len() != 1 {
                return Err(syn::Error::new_spanned(
                    node,
                    "A wrapper can only have one field, or mark a field explicitly as wrapper using #[wraps]",
                ));
            }
            fields[0].wraps = true;
        } else {
            wraps = fields.iter().any(|f| f.wraps);
        }

        Ok(DefinitionStatement {
            attrs: node.scan_into_attrs()?,
            wraps,
            fields,
            extra: self.scan_extra(node)?,
        })
    }

    fn scan_into_field<'src>(&self, node: &'src syn::Field) -> syn::Result<DefinitionField<Self>> {
        let wraps = node.attrs.iter().any(|attr| attr.path().is_ident("wraps"));
        Ok(DefinitionField {
            wraps,
            attrs: node.scan_into_attrs()?,
            extra: self.scan_extra(node)?,
        })
    }
}

impl<L: Layout> ScanInto for L where
    L: Layout
        + Sized
        + for<'a> ScanExtra<'a, syn::DeriveInput, Self::StatementExtra>
        + for<'a> ScanExtra<'a, syn::Variant, Self::StatementExtra>
        + for<'a> ScanExtra<'a, syn::Field, Self::FieldExtra>
{
}

pub trait ScanAttr<T> {
    fn scan_into_attrs(&self) -> syn::Result<T>;
}

impl<T> ScanAttr<T> for syn::DeriveInput
where
    T: darling::FromDeriveInput,
{
    fn scan_into_attrs(&self) -> syn::Result<T> {
        T::from_derive_input(self).map_err(|e| syn::Error::new_spanned(self, e))
    }
}

impl<T> ScanAttr<T> for syn::Variant
where
    T: darling::FromVariant,
{
    fn scan_into_attrs(&self) -> syn::Result<T> {
        T::from_variant(self).map_err(|e| syn::Error::new_spanned(self, e))
    }
}

impl<T> ScanAttr<T> for syn::Field
where
    T: darling::FromField,
{
    fn scan_into_attrs(&self) -> syn::Result<T> {
        T::from_field(self).map_err(|e| syn::Error::new_spanned(self, e))
    }
}

pub trait SynFields {
    fn fields(&self) -> &'_ syn::Fields;
}

impl SynFields for syn::DeriveInput {
    fn fields(&self) -> &'_ syn::Fields {
        match &self.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Called fields() on non-struct DeriveInput"),
        }
    }
}

impl SynFields for syn::Variant {
    fn fields(&self) -> &'_ syn::Fields {
        &self.fields
    }
}
