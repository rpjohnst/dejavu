use std::collections::HashMap;

use symbol::Symbol;

pub use vm::interpreter::{State, Error};
pub use vm::value::{Type, Value, Data};
pub use vm::array::{Array, Row};
pub use vm::world::{Entity, Instance, Hash, World};

pub mod code;
pub mod debug;
mod value;
mod array;
mod world;
mod interpreter;

#[derive(Default)]
pub struct Resources {
    pub scripts: HashMap<Symbol, code::Function>,
    pub functions: HashMap<Symbol, ApiFunction>,

    pub get: HashMap<Symbol, GetFunction>,
    pub set: HashMap<Symbol, SetFunction>,
}

pub type ApiFunction = fn(&mut State, &Resources, Arguments) -> Result<Value, Error>;
pub type GetFunction = fn(&Instance) -> Value;
pub type SetFunction = fn(&mut Instance, Value);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Arguments {
    base: usize,
    limit: usize,
}
