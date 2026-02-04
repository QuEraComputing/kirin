use crate::ir::{fields::FieldInfo, Layout};

pub trait Scan<'ir, L: Layout> {
    fn scan_input(&mut self, input: &'ir crate::ir::Input<L>) -> darling::Result<()> {
        scan_input(self, input)
    }

    fn scan_struct(&mut self, data: &'ir crate::ir::DataStruct<L>) -> darling::Result<()> {
        scan_struct(self, data)
    }

    fn scan_enum(&mut self, data: &'ir crate::ir::DataEnum<L>) -> darling::Result<()> {
        scan_enum(self, data)
    }

    fn scan_statement(&mut self, statement: &'ir crate::ir::Statement<L>) -> darling::Result<()> {
        scan_statement(self, statement)
    }

    fn scan_wrapper(&mut self, wrapper: &'ir crate::ir::fields::Wrapper) -> darling::Result<()> {
        scan_wrapper(self, wrapper)
    }

    /// Scan a field of any category.
    fn scan_field(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_field(self, field)
    }

    /// Scan an argument field (SSAValue).
    fn scan_argument(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_argument(self, field)
    }

    /// Scan a result field (ResultValue).
    fn scan_result(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_result(self, field)
    }

    /// Scan a block field.
    fn scan_block(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_block(self, field)
    }

    /// Scan a successor field.
    fn scan_successor(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_successor(self, field)
    }

    /// Scan a region field.
    fn scan_region(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_region(self, field)
    }

    /// Scan a symbol field.
    fn scan_symbol(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_symbol(self, field)
    }

    /// Scan a compile-time value field.
    fn scan_value(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<()> {
        scan_value(self, field)
    }
}

pub fn scan_input<'ir, V, L>(
    visitor: &mut V,
    input: &'ir crate::ir::Input<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    match &input.data {
        crate::ir::Data::Struct(data) => visitor.scan_struct(data),
        crate::ir::Data::Enum(data) => visitor.scan_enum(data),
    }
}

pub fn scan_struct<'ir, V, L>(
    visitor: &mut V,
    data: &'ir crate::ir::DataStruct<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    visitor.scan_statement(&data.0)
}

pub fn scan_enum<'ir, V, L>(
    visitor: &mut V,
    data: &'ir crate::ir::DataEnum<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for variant in &data.variants {
        errors.handle_in(|| visitor.scan_statement(variant));
    }
    errors.finish()
}

pub fn scan_statement<'ir, V, L>(
    visitor: &mut V,
    statement: &'ir crate::ir::Statement<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    use crate::ir::fields::FieldCategory;

    let mut errors = darling::Error::accumulator();
    if let Some(wrapper) = &statement.wraps {
        errors.handle_in(|| visitor.scan_wrapper(wrapper));
    }
    for field in statement.iter_all_fields() {
        errors.handle_in(|| {
            // Call the general scan_field, then category-specific method
            visitor.scan_field(field)?;
            match field.category() {
                FieldCategory::Argument => visitor.scan_argument(field),
                FieldCategory::Result => visitor.scan_result(field),
                FieldCategory::Block => visitor.scan_block(field),
                FieldCategory::Successor => visitor.scan_successor(field),
                FieldCategory::Region => visitor.scan_region(field),
                FieldCategory::Symbol => visitor.scan_symbol(field),
                FieldCategory::Value => visitor.scan_value(field),
            }
        });
    }
    errors.finish()
}

pub fn scan_wrapper<'ir, V, L>(
    _visitor: &mut V,
    _wrapper: &'ir crate::ir::fields::Wrapper,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_field<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_argument<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_result<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_block<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_successor<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_region<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_symbol<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_value<'ir, V, L>(
    _visitor: &mut V,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}
