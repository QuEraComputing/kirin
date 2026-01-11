use crate::ir::Layout;

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

    fn scan_arguments(
        &mut self,
        arguments: &'ir crate::ir::fields::Arguments,
    ) -> darling::Result<()> {
        scan_arguments(self, arguments)
    }

    fn scan_results(&mut self, results: &'ir crate::ir::fields::Results) -> darling::Result<()> {
        scan_results(self, results)
    }

    fn scan_successors(
        &mut self,
        successors: &'ir crate::ir::fields::Successors,
    ) -> darling::Result<()> {
        scan_successors(self, successors)
    }

    fn scan_blocks(&mut self, blocks: &'ir crate::ir::fields::Blocks) -> darling::Result<()> {
        scan_blocks(self, blocks)
    }

    fn scan_regions(&mut self, regions: &'ir crate::ir::fields::Regions) -> darling::Result<()> {
        scan_regions(self, regions)
    }

    fn scan_comptime_values(
        &mut self,
        comptime_values: &'ir crate::ir::fields::CompileTimeValues<L>,
    ) -> darling::Result<()> {
        scan_comptime_values(self, comptime_values)
    }

    fn scan_wrapper(&mut self, wrapper: &'ir crate::ir::fields::Wrapper) -> darling::Result<()> {
        scan_wrapper(self, wrapper)
    }

    fn scan_result(&mut self, result: &'ir crate::ir::fields::Result) -> darling::Result<()> {
        scan_result(self, result)
    }

    fn scan_argument(&mut self, argument: &'ir crate::ir::fields::Argument) -> darling::Result<()> {
        scan_argument(self, argument)
    }

    fn scan_value(&mut self, value: &'ir crate::ir::fields::Value) -> darling::Result<()> {
        scan_value(self, value)
    }

    fn scan_successor(
        &mut self,
        successor: &'ir crate::ir::fields::Successor,
    ) -> darling::Result<()> {
        scan_successor(self, successor)
    }

    fn scan_block(&mut self, block: &'ir crate::ir::fields::Block) -> darling::Result<()> {
        scan_block(self, block)
    }

    fn scan_region(&mut self, region: &'ir crate::ir::fields::Region) -> darling::Result<()> {
        scan_region(self, region)
    }

    fn scan_comptime_value(
        &mut self,
        comptime_value: &'ir crate::ir::fields::CompileTimeValue<L>,
    ) -> darling::Result<()> {
        scan_comptime_value(self, comptime_value)
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
    let mut errors = darling::Error::accumulator();
    if let Some(wrapper) = &statement.wraps {
        errors.handle_in(|| visitor.scan_wrapper(wrapper));
    }
    for argument in statement.arguments.iter() {
        errors.handle_in(|| visitor.scan_argument(argument));
    }
    for result in statement.results.iter() {
        errors.handle_in(|| visitor.scan_result(result));
    }
    for successor in statement.successors.iter() {
        errors.handle_in(|| visitor.scan_successor(successor));
    }
    for block in statement.blocks.iter() {
        errors.handle_in(|| visitor.scan_block(block));
    }
    for region in statement.regions.iter() {
        errors.handle_in(|| visitor.scan_region(region));
    }
    for comptime_value in statement.values.iter() {
        errors.handle_in(|| visitor.scan_comptime_value(comptime_value));
    }
    errors.finish()
}

pub fn scan_arguments<'ir, V, L>(
    visitor: &mut V,
    arguments: &'ir crate::ir::fields::Arguments,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for arg in arguments.iter() {
        errors.handle_in(|| visitor.scan_argument(arg));
    }
    errors.finish()
}

pub fn scan_results<'ir, V, L>(
    visitor: &mut V,
    results: &'ir crate::ir::fields::Results,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for res in results.iter() {
        errors.handle_in(|| visitor.scan_result(res));
    }
    errors.finish()
}

pub fn scan_successors<'ir, V, L>(
    visitor: &mut V,
    successors: &'ir crate::ir::fields::Successors,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for succ in successors.iter() {
        errors.handle_in(|| visitor.scan_successor(succ));
    }
    errors.finish()
}

pub fn scan_blocks<'ir, V, L>(
    visitor: &mut V,
    blocks: &'ir crate::ir::fields::Blocks,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for block in blocks.iter() {
        errors.handle_in(|| visitor.scan_block(block));
    }
    errors.finish()
}

pub fn scan_regions<'ir, V, L>(
    visitor: &mut V,
    regions: &'ir crate::ir::fields::Regions,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for region in regions.iter() {
        errors.handle_in(|| visitor.scan_region(region));
    }
    errors.finish()
}

pub fn scan_comptime_values<'ir, V, L>(
    visitor: &mut V,
    comptime_values: &'ir crate::ir::fields::CompileTimeValues<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    let mut errors = darling::Error::accumulator();
    for cv in comptime_values.iter() {
        errors.handle_in(|| visitor.scan_comptime_value(cv));
    }
    errors.finish()
}

pub fn scan_argument<'ir, V, L>(
    visitor: &mut V,
    argument: &'ir crate::ir::fields::Argument,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    visitor.scan_value(&argument.0)
}

pub fn scan_result<'ir, V, L>(
    visitor: &mut V,
    result: &'ir crate::ir::fields::Result,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    visitor.scan_value(&result.0)
}

pub fn scan_value<'ir, V, L>(
    _visitor: &mut V,
    _value: &'ir crate::ir::fields::Value,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
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

pub fn scan_successor<'ir, V, L>(
    _visitor: &mut V,
    _successor: &'ir crate::ir::fields::Successor,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_block<'ir, V, L>(
    _visitor: &mut V,
    _block: &'ir crate::ir::fields::Block,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_region<'ir, V, L>(
    _visitor: &mut V,
    _region: &'ir crate::ir::fields::Region,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}

pub fn scan_comptime_value<'ir, V, L>(
    _visitor: &mut V,
    _comptime_value: &'ir crate::ir::fields::CompileTimeValue<L>,
) -> darling::Result<()>
where
    V: Scan<'ir, L> + ?Sized,
    L: Layout,
{
    Ok(())
}
