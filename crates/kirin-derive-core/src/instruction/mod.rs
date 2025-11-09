mod attribute;

use crate::{
    FieldAccessor, Generate,
    accessor::Config,
    instruction::attribute::{AttributeInfo, DeriveAttribute},
};

pub struct DeriveInstruction {
    attribute: DeriveAttribute,
    arguments: FieldAccessor,
    results: FieldAccessor,
    successors: FieldAccessor,
    regions: FieldAccessor,
}

impl DeriveInstruction {
    pub fn new(ctx: &crate::DeriveContext<AttributeInfo>) -> Self {
        Self {
            attribute: DeriveAttribute,
            arguments: FieldAccessor::new(
                is_argument,
                Config::new("arguments", "SSAValue", "::kirin_ir::Instruction"),
                ctx,
            ),
            results: FieldAccessor::new(
                is_result,
                Config::new("results", "ResultValue", "::kirin_ir::Instruction"),
                ctx,
            ),
            successors: FieldAccessor::new(
                is_block,
                Config::new("successors", "Block", "::kirin_ir::Instruction"),
                ctx,
            ),
            regions: FieldAccessor::new(
                is_region,
                Config::new("regions", "Region", "::kirin_ir::Instruction"),
                ctx,
            ),
        }
    }
}

impl Generate<AttributeInfo> for DeriveInstruction {
    fn generate(&mut self, ctx: &mut crate::DeriveContext<AttributeInfo>) -> eyre::Result<()> {
        self.attribute.generate(ctx)?;
        self.arguments.generate(ctx)?;
        self.results.generate(ctx)?;
        self.successors.generate(ctx)?;
        self.regions.generate(ctx)?;
        Ok(())
    }
}

fn is_argument(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident("SSAValue"))
}

fn is_result(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident("ResultValue"))
}

fn is_block(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident("Block"))
}

fn is_region(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident("Region"))
}

#[cfg(test)]
mod tests;
