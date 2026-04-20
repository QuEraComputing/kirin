use crate::control::Control;
use crate::env::Env;

pub trait Interpretable<E: Env>: Sized {
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>;
}
