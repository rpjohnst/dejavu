use std::collections::HashMap;
use std::ops::Range;

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

pub struct Assets<W: ?Sized, A: ?Sized> {
    pub scripts: HashMap<i32, code::Function>,
    pub events: HashMap<Event, code::Function>,
    pub debug: HashMap<Function, code::Debug>,

    pub api: HashMap<Symbol, ApiFunction<W, A>>,
    pub get: HashMap<Symbol, GetFunction<W, A>>,
    pub set: HashMap<Symbol, SetFunction<W, A>>,
}

pub type ApiFunction<W, A> = unsafe fn(
    &mut W, &mut A, &mut Thread, Range<usize>
) -> Result<Value, ErrorKind>;
pub type GetFunction<W, A> = fn(&mut W, &mut A, Entity, usize) -> Value;
pub type SetFunction<W, A> = fn(&mut W, &mut A, Entity, usize, ValueRef);

impl<W: ?Sized, A: ?Sized> Default for Assets<W, A> {
    fn default() -> Self {
        Assets {
            scripts: HashMap::default(),
            events: HashMap::default(),
            debug: HashMap::default(),

            api: HashMap::default(),
            get: HashMap::default(),
            set: HashMap::default(),
        }
    }
}

pub trait Api<'a, A: 'a> {
    fn fields<'r>(&'r mut self, assets: &'r mut A) -> (&'r mut World, &'r mut Assets<Self, A>);
}
