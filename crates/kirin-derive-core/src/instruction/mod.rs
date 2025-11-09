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
                Config::new("arguments", "SSAValue", "::kirin_ir::Instruction"),
                ctx,
            ),
            results: FieldAccessor::new(
                Config::new("results", "ResultValue", "::kirin_ir::Instruction"),
                ctx,
            ),
            successors: FieldAccessor::new(
                Config::new("successors", "Block", "::kirin_ir::Instruction"),
                ctx,
            ),
            regions: FieldAccessor::new(
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

#[cfg(test)]
mod tests;
