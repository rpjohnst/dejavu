use std::collections::HashMap;

use crate::symbol::Symbol;

pub use crate::vm::interpreter::{Thread, Error, ErrorKind, SELF, OTHER, ALL, NOONE, GLOBAL};
pub use crate::vm::world::World;
pub use crate::vm::entity_map::{Entity, EntityAllocator, EntityMap};
pub use crate::vm::value::{Type, Value, Data};
pub use crate::vm::array::{Array, Row};

pub mod code;
pub mod debug;
pub mod world;
mod interpreter;
mod entity_map;
mod value;
mod array;

pub struct Resources<E: ?Sized> {
    pub scripts: HashMap<Symbol, code::Function>,
    pub debug: HashMap<Symbol, code::Debug>,

    pub api: HashMap<Symbol, ApiFunction<E>>,
    pub get: HashMap<Symbol, GetFunction<E>>,
    pub set: HashMap<Symbol, SetFunction<E>>,
}

pub type ApiFunction<E> = fn(&mut E, &Resources<E>, Entity, &[Value]) -> Result<Value, ErrorKind>;
pub type GetFunction<E> = fn(&mut E, Entity, usize) -> Value;
pub type SetFunction<E> = fn(&mut E, Entity, usize, Value);

impl<E: ?Sized> Default for Resources<E> {
    fn default() -> Self {
        Resources {
            scripts: Default::default(),
            debug: Default::default(),

            api: Default::default(),
            get: Default::default(),
            set: Default::default(),
        }
    }
}
