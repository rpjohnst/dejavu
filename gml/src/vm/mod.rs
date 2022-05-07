use std::collections::HashMap;
use std::ops::Range;

use crate::symbol::Symbol;
use crate::{Function, front::Lines};

pub use crate::vm::interpreter::{Thread, Result, Error, ErrorFrame};
pub use crate::vm::interpreter::{SELF, OTHER, ALL, NOONE, GLOBAL, LOCAL, PUSH_ARRAY, PUSH_ANY};
pub use crate::vm::world::World;
pub use crate::vm::bind::{Bind, FnBind, GetBind, SetBind, Project};
pub use crate::vm::entity_map::{Entity, EntityAllocator, EntityMap};
pub use crate::vm::instance_map::InstanceMap;
pub use crate::vm::value::{Value, ValueRef, Data, to_i32, to_u32, to_bool};
pub use crate::vm::array::{Array, ArrayRef};

pub mod code;
pub mod world;
pub mod bind;
mod entity_map;
mod instance_map;
mod interpreter;
//mod serialize;
mod value;
mod array;
mod debug;

pub struct Assets<W: ?Sized> {
    pub code: HashMap<Function, code::Function>,
    pub api: HashMap<Symbol, ApiFunction<W>>,
    pub get: HashMap<Symbol, GetFunction<W>>,
    pub set: HashMap<Symbol, SetFunction<W>>,
    pub constants: i32,
}

#[derive(Default)]
pub struct Debug {
    pub locations: HashMap<Function, Locations>,
    pub scripts: Vec<Symbol>,
    pub objects: Vec<Symbol>,
    pub rooms: Vec<Symbol>,
    pub instances: HashMap<i32, i32>,
    pub constants: Vec<Symbol>,
}

pub struct Locations {
    pub locations: code::Locations,
    pub lines: Lines,
}

pub type ApiFunction<W> = unsafe fn(&mut W, &mut Thread, Range<usize>) -> Result<Value>;
pub type GetFunction<W> = fn(&mut W, Entity, usize) -> Value;
pub type SetFunction<W> = fn(&mut W, Entity, usize, ValueRef<'_>);

impl<W: ?Sized> Default for Assets<W> {
    fn default() -> Assets<W> {
        Assets {
            code: HashMap::default(),
            api: HashMap::default(),
            get: HashMap::default(),
            set: HashMap::default(),
            constants: 0,
        }
    }
}
