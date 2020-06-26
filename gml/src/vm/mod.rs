use std::collections::HashMap;

use crate::symbol::Symbol;
use crate::{Function, Event};

pub use crate::vm::interpreter::{Thread, Error, ErrorKind, SELF, OTHER, ALL, NOONE, GLOBAL};
pub use crate::vm::world::World;
pub use crate::vm::entity_map::{Entity, EntityAllocator, EntityMap};
pub use crate::vm::instance_map::InstanceMap;
pub use crate::vm::value::{Value, ValueRef, Data, to_i32, to_u32, to_bool};
pub use crate::vm::array::{Array, ArrayRef};

pub mod code;
pub mod world;
mod entity_map;
mod instance_map;
mod interpreter;
mod value;
mod array;
mod debug;

pub struct Resources<E: ?Sized> {
    pub scripts: HashMap<i32, code::Function>,
    pub events: HashMap<Event, code::Function>,

    pub debug: HashMap<Function, code::Debug>,

    pub api: HashMap<Symbol, ApiFunction<E>>,
    pub get: HashMap<Symbol, GetFunction<E>>,
    pub set: HashMap<Symbol, SetFunction<E>>,
}

pub type ApiFunction<E> = fn(&mut E, &Resources<E>, Entity, &[Value]) -> Result<Value, ErrorKind>;
pub type GetFunction<E> = fn(&mut E, Entity, usize) -> Value;
pub type SetFunction<E> = fn(&mut E, Entity, usize, ValueRef);

impl<E: ?Sized> Default for Resources<E> {
    fn default() -> Self {
        Resources {
            scripts: Default::default(),
            events: Default::default(),
            debug: Default::default(),

            api: Default::default(),
            get: Default::default(),
            set: Default::default(),
        }
    }
}
