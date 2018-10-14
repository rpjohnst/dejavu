use std::collections::HashMap;

use symbol::Symbol;

pub use vm::interpreter::{Thread, Error, ErrorKind};
pub use vm::value::{Type, Value, Data};
pub use vm::array::{Array, Row};
pub use vm::world::{World, Entity, Hash};

pub mod code;
pub mod debug;
pub mod world;
mod value;
mod array;
mod interpreter;

#[derive(Default)]
pub struct Resources<E: ?Sized> {
    pub scripts: HashMap<Symbol, code::Function>,

    pub api: HashMap<Symbol, ApiFunction<E>>,
    pub get: HashMap<Symbol, GetFunction<E>>,
    pub set: HashMap<Symbol, SetFunction<E>>,
}

pub type ApiFunction<E> = fn(&mut E, &[Value]) -> Result<Value, ErrorKind>;
pub type GetFunction<E> = fn(&E, Entity, usize) -> Value;
pub type SetFunction<E> = fn(&mut E, Entity, usize, Value);
