use crate::ir::{Data, Layout};
use proc_macro2::TokenStream;

pub trait Emit<'ir, L: Layout> {
    fn emit_input(&mut self, input: &'ir crate::ir::Input<L>) -> darling::Result<TokenStream> {
        emit_input(self, input)
    }

    fn emit_struct(&mut self, data: &'ir crate::ir::DataStruct<L>) -> darling::Result<TokenStream> {
        emit_struct(self, data)
    }

    fn emit_enum(&mut self, data: &'ir crate::ir::DataEnum<L>) -> darling::Result<TokenStream> {
        emit_enum(self, data)
    }

    fn emit_statement(
        &mut self,
        statement: &'ir crate::ir::Statement<L>,
    ) -> darling::Result<TokenStream> {
        emit_statement(self, statement)
    }

    fn emit_arguments(
        &mut self,
        arguments: &'ir crate::ir::fields::Arguments,
    ) -> darling::Result<TokenStream> {
        emit_arguments(self, arguments)
    }

    fn emit_results(
        &mut self,
        results: &'ir crate::ir::fields::Results,
    ) -> darling::Result<TokenStream> {
        emit_results(self, results)
    }

    fn emit_successors(
        &mut self,
        successors: &'ir crate::ir::fields::Successors,
    ) -> darling::Result<TokenStream> {
        emit_successors(self, successors)
    }

    fn emit_blocks(
        &mut self,
        blocks: &'ir crate::ir::fields::Blocks,
    ) -> darling::Result<TokenStream> {
        emit_blocks(self, blocks)
    }

    fn emit_regions(
        &mut self,
        regions: &'ir crate::ir::fields::Regions,
    ) -> darling::Result<TokenStream> {
        emit_regions(self, regions)
    }

    fn emit_comptime_values(
        &mut self,
        comptime_values: &'ir crate::ir::fields::CompileTimeValues<L>,
    ) -> darling::Result<TokenStream> {
        emit_comptime_values(self, comptime_values)
    }

    fn emit_wrapper(
        &mut self,
        wrapper: &'ir crate::ir::fields::Wrapper,
    ) -> darling::Result<TokenStream> {
        emit_wrapper(self, wrapper)
    }

    fn emit_result(
        &mut self,
        result: &'ir crate::ir::fields::Result,
    ) -> darling::Result<TokenStream> {
        emit_result(self, result)
    }

    fn emit_argument(
        &mut self,
        argument: &'ir crate::ir::fields::Argument,
    ) -> darling::Result<TokenStream> {
        emit_argument(self, argument)
    }

    fn emit_value(&mut self, value: &'ir crate::ir::fields::Value) -> darling::Result<TokenStream> {
        emit_value(self, value)
    }

    fn emit_successor(
        &mut self,
        successor: &'ir crate::ir::fields::Successor,
    ) -> darling::Result<TokenStream> {
        emit_successor(self, successor)
    }
}

pub fn emit_input<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    emitter: &mut E,
    input: &'ir crate::ir::Input<L>,
) -> darling::Result<TokenStream> {
    match &input.data {
        Data::Struct(data) => emitter.emit_struct(data),
        Data::Enum(data) => emitter.emit_enum(data),
    }
}

pub fn emit_struct<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    emitter: &mut E,
    data: &'ir crate::ir::DataStruct<L>,
) -> darling::Result<TokenStream> {
    emitter.emit_statement(&data.0)
}

pub fn emit_enum<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    emitter: &mut E,
    data: &'ir crate::ir::DataEnum<L>,
) -> darling::Result<TokenStream> {
    let mut tokens = TokenStream::new();
    let mut errors = darling::Error::accumulator();

    for variant in &data.variants {
        errors.handle_in(|| {
            tokens.extend(emitter.emit_statement(&variant)?);
            Ok(())
        });
    }
    errors.finish()?;
    Ok(tokens)
}

pub fn emit_statement<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _statement: &'ir crate::ir::Statement<L>,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_arguments<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _arguments: &'ir crate::ir::fields::Arguments,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_results<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _results: &'ir crate::ir::fields::Results,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_successors<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _successors: &'ir crate::ir::fields::Successors,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_blocks<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _blocks: &'ir crate::ir::fields::Blocks,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_regions<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _regions: &'ir crate::ir::fields::Regions,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_comptime_values<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _comptime_values: &'ir crate::ir::fields::CompileTimeValues<L>,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_wrapper<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _wrapper: &'ir crate::ir::fields::Wrapper,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_result<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _result: &'ir crate::ir::fields::Result,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_argument<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _argument: &'ir crate::ir::fields::Argument,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_value<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _value: &'ir crate::ir::fields::Value,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_successor<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _successor: &'ir crate::ir::fields::Successor,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_block<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _block: &'ir crate::ir::fields::Block,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_comptime_value<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _comptime_value: &'ir crate::ir::fields::CompileTimeValue<L>,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}

pub fn emit_region<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _region: &'ir crate::ir::fields::Region,
) -> darling::Result<TokenStream> {
    // Default implementation produces no tokens.
    Ok(TokenStream::new())
}
