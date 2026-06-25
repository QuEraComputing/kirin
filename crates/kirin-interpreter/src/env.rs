use std::collections::HashMap;

use kirin_ir::{Product, SSAValue};

use crate::InterpreterError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnvIndex(usize);

impl EnvIndex {
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn raw(self) -> usize {
        self.0
    }
}

pub trait Store<V> {
    type Error;

    fn alloc(&mut self) -> EnvIndex;
    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error>;
    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<V, Self::Error>;
    fn write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), Self::Error>;

    fn read_many(&self, index: EnvIndex, values: &[SSAValue]) -> Result<Product<V>, Self::Error> {
        values
            .iter()
            .map(|value| self.read(index, *value))
            .collect()
    }

    fn write_product(
        &mut self,
        index: EnvIndex,
        values: &[SSAValue],
        data: Product<V>,
    ) -> Result<(), Self::Error>
    where
        Self::Error: From<InterpreterError>,
    {
        if data.len() != values.len() {
            return Err(Self::Error::from(InterpreterError::ProductArityMismatch {
                expected: values.len(),
                actual: data.len(),
            }));
        }

        for (value, data) in values.iter().copied().zip(data) {
            self.write(index, value, data)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct EnvStackStore<V> {
    stores: Vec<Option<HashMap<SSAValue, V>>>,
    stack: Vec<EnvIndex>,
}

impl<V> Default for EnvStackStore<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> EnvStackStore<V> {
    pub fn new() -> Self {
        Self {
            stores: Vec::new(),
            stack: Vec::new(),
        }
    }

    pub fn push(&mut self) -> EnvIndex {
        let index = self.alloc_store();
        self.stack.push(index);
        index
    }

    pub fn pop(&mut self) -> Result<EnvIndex, InterpreterError> {
        let index = self.stack.pop().ok_or(InterpreterError::EmptyEnvStack)?;
        self.free_store(index)?;
        Ok(index)
    }

    pub fn current(&self) -> Result<EnvIndex, InterpreterError> {
        self.stack
            .last()
            .copied()
            .ok_or(InterpreterError::EmptyEnvStack)
    }

    fn store(&self, index: EnvIndex) -> Result<&HashMap<SSAValue, V>, InterpreterError> {
        self.stores
            .get(index.raw())
            .and_then(Option::as_ref)
            .ok_or(InterpreterError::InvalidEnvIndex(index))
    }

    fn store_mut(
        &mut self,
        index: EnvIndex,
    ) -> Result<&mut HashMap<SSAValue, V>, InterpreterError> {
        self.stores
            .get_mut(index.raw())
            .and_then(Option::as_mut)
            .ok_or(InterpreterError::InvalidEnvIndex(index))
    }

    fn alloc_store(&mut self) -> EnvIndex {
        let index = EnvIndex::new(self.stores.len());
        self.stores.push(Some(HashMap::new()));
        index
    }

    fn free_store(&mut self, index: EnvIndex) -> Result<(), InterpreterError> {
        let store = self
            .stores
            .get_mut(index.raw())
            .ok_or(InterpreterError::InvalidEnvIndex(index))?;
        if store.take().is_some() {
            Ok(())
        } else {
            Err(InterpreterError::InvalidEnvIndex(index))
        }
    }
}

impl<V: Clone> Store<V> for EnvStackStore<V> {
    type Error = InterpreterError;

    fn alloc(&mut self) -> EnvIndex {
        self.alloc_store()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.free_store(index)
    }

    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<V, Self::Error> {
        self.store(index)?
            .get(&value)
            .cloned()
            .ok_or(InterpreterError::UnboundValue { index, value })
    }

    fn write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), Self::Error> {
        self.store_mut(index)?.insert(value, data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use kirin_ir::TestSSAValue;

    use super::*;

    #[test]
    fn stack_store_reads_and_writes_live_envs() {
        let mut env = EnvStackStore::new();
        let index = env.push();
        let value = SSAValue::from(TestSSAValue(0));

        env.write(index, value, 42).unwrap();

        assert_eq!(env.read(index, value).unwrap(), 42);
        assert_eq!(env.current().unwrap(), index);
    }

    #[test]
    fn popped_env_is_no_longer_live() {
        let mut env = EnvStackStore::<i64>::new();
        let index = env.push();

        assert_eq!(env.pop().unwrap(), index);
        assert_eq!(
            env.read(index, SSAValue::from(TestSSAValue(0))),
            Err(InterpreterError::InvalidEnvIndex(index))
        );
    }
}
