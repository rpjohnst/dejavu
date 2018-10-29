use std::collections::HashMap;

use symbol::Symbol;

pub use vm::interpreter::{Thread, Error, ErrorKind, SELF, OTHER, ALL, NOONE, GLOBAL};
pub use vm::world::World;
pub use vm::entity_map::{Entity, EntityAllocator, EntityMap};
pub use vm::value::{Type, Value, Data};
pub use vm::array::{Array, Row};

pub mod code;
pub mod debug;
pub mod world;
mod interpreter;
mod entity_map;
mod value;
mod array;

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
