use std::collections::HashMap;

use kirin_ir::SSAValue;

use crate::{Env, EnvIndex, ForkEnv, InterpreterError};

use super::AbstractValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbstractEnv<V> {
    pub(super) values: HashMap<SSAValue, V>,
}

impl<V> Default for AbstractEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> AbstractEnv<V> {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn values(&self) -> &HashMap<SSAValue, V> {
        &self.values
    }

    pub fn into_values(self) -> HashMap<SSAValue, V> {
        self.values
    }

    pub fn read(&self, value: SSAValue) -> V
    where
        V: AbstractValue,
    {
        self.values.get(&value).cloned().unwrap_or_else(V::bottom)
    }

    pub fn write(&mut self, value: SSAValue, data: V) {
        self.values.insert(value, data);
    }
}

#[derive(Clone, Debug)]
pub struct AbstractEnvStore<V> {
    stores: Vec<Option<AbstractEnv<V>>>,
    stack: Vec<EnvIndex>,
}

impl<V> Default for AbstractEnvStore<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> AbstractEnvStore<V> {
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

    pub fn clone_store_from(&mut self, index: EnvIndex) -> Result<EnvIndex, InterpreterError>
    where
        V: Clone,
    {
        let env = self.snapshot(index)?;
        Ok(self.alloc_from(env))
    }

    pub fn alloc_from(&mut self, env: AbstractEnv<V>) -> EnvIndex {
        let index = EnvIndex::new(self.stores.len());
        self.stores.push(Some(env));
        index
    }

    pub fn snapshot(&self, index: EnvIndex) -> Result<AbstractEnv<V>, InterpreterError>
    where
        V: Clone,
    {
        self.store(index).cloned()
    }

    fn store(&self, index: EnvIndex) -> Result<&AbstractEnv<V>, InterpreterError> {
        self.stores
            .get(index.raw())
            .and_then(Option::as_ref)
            .ok_or(InterpreterError::InvalidEnvIndex(index))
    }

    fn store_mut(&mut self, index: EnvIndex) -> Result<&mut AbstractEnv<V>, InterpreterError> {
        self.stores
            .get_mut(index.raw())
            .and_then(Option::as_mut)
            .ok_or(InterpreterError::InvalidEnvIndex(index))
    }

    fn alloc_store(&mut self) -> EnvIndex {
        let index = EnvIndex::new(self.stores.len());
        self.stores.push(Some(AbstractEnv::new()));
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

impl<V> Env<V> for AbstractEnvStore<V>
where
    V: AbstractValue,
{
    type Error = InterpreterError;

    fn alloc(&mut self) -> EnvIndex {
        self.alloc_store()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.free_store(index)
    }

    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<V, Self::Error> {
        Ok(self.store(index)?.read(value))
    }

    fn write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), Self::Error> {
        self.store_mut(index)?.write(value, data);
        Ok(())
    }
}

impl<V> ForkEnv<V> for AbstractEnvStore<V>
where
    V: AbstractValue,
{
    fn fork_env(&mut self, index: EnvIndex) -> Result<EnvIndex, Self::Error> {
        self.clone_store_from(index)
    }
}
