//! Code generation visitor for producing `TokenStream` output from IR.
//!
//! [`Emit`] mirrors [`Scan`](crate::scan::Scan) but returns `TokenStream`
//! instead of `()`. Override hooks at each level to generate code; the
//! default implementations concatenate children's output.
//!
//! Typically used after a [`Scan`](crate::scan::Scan) pass that collected
//! the metadata needed for code generation.
//!
//! # Example: Generating Match Arms
//!
//! ```ignore
//! impl<'ir> Emit<'ir, StandardLayout> for MyEmitter {
//!     fn emit_statement(
//!         &mut self,
//!         stmt: &'ir Statement<StandardLayout>,
//!     ) -> darling::Result<TokenStream> {
//!         let name = &stmt.name;
//!         Ok(quote! { Self::#name { .. } => todo!() })
//!     }
//! }
//! ```

use crate::ir::{Data, Layout, fields::FieldInfo};
use proc_macro2::TokenStream;

/// Visitor trait for generating `TokenStream` output from IR.
///
/// Override specific methods to emit code for nodes of interest.
/// The default implementations concatenate children's output.
/// Call the corresponding free function (e.g., [`emit_statement`])
/// from your override to include children's output in yours.
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

    fn emit_wrapper(
        &mut self,
        wrapper: &'ir crate::ir::fields::Wrapper,
    ) -> darling::Result<TokenStream> {
        emit_wrapper(self, wrapper)
    }

    fn emit_field(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_field(self, field)
    }

    fn emit_argument(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_argument(self, field)
    }

    fn emit_result(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_result(self, field)
    }

    fn emit_block(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_block(self, field)
    }

    fn emit_successor(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_successor(self, field)
    }

    fn emit_region(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_region(self, field)
    }

    fn emit_symbol(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_symbol(self, field)
    }

    fn emit_value(&mut self, field: &'ir FieldInfo<L>) -> darling::Result<TokenStream> {
        emit_value(self, field)
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
            tokens.extend(emitter.emit_statement(variant)?);
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
    Ok(TokenStream::new())
}

pub fn emit_wrapper<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _wrapper: &'ir crate::ir::fields::Wrapper,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_field<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_argument<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_result<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_block<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_successor<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_region<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_symbol<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}

pub fn emit_value<'ir, L: Layout, E: Emit<'ir, L> + ?Sized>(
    _emitter: &mut E,
    _field: &'ir FieldInfo<L>,
) -> darling::Result<TokenStream> {
    Ok(TokenStream::new())
}
