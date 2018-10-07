use std::collections::HashMap;

use symbol::Symbol;

pub use vm::interpreter::{State, Error};
pub use vm::value::{Type, Value, Data};
pub use vm::array::{Array, Row};
pub use vm::world::{Entity, Hash};
pub(crate) use vm::world::World;

pub mod code;
pub mod debug;
mod value;
mod array;
mod world;
mod interpreter;

#[derive(Default)]
pub struct Resources<E: ?Sized> {
    pub scripts: HashMap<Symbol, code::Function>,

    pub api: HashMap<Symbol, ApiFunction<E>>,
    pub get: HashMap<Symbol, GetFunction<E>>,
    pub set: HashMap<Symbol, SetFunction<E>>,
}

pub type ApiFunction<E> = fn(&mut E, &[Value]) -> Result<Value, Error>;
pub type GetFunction<E> = fn(&E, Entity, usize) -> Value;
pub type SetFunction<E> = fn(&mut E, Entity, usize, Value);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Arguments {
    base: usize,
    limit: usize,
}
