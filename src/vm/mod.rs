use std::collections::HashMap;

use symbol::Symbol;

pub use vm::interpreter::{State, Error};
pub use vm::value::{Type, Value, Data};
pub use vm::array::{Array, Row};

pub mod code;
pub mod debug;
mod value;
mod array;
mod interpreter;

pub struct Resources<C> {
    pub scripts: HashMap<Symbol, code::Function>,
    pub functions: HashMap<Symbol, NativeFunction<C>>,
}

// we can't #[derive(Default)] because of rust-lang/rust#26935
impl<C> Default for Resources<C> {
    fn default() -> Self {
        Resources {
            scripts: HashMap::default(),
            functions: HashMap::default(),
        }
    }
}

pub type NativeFunction<C> = fn(
    &mut C, &mut State, &Resources<C>, Arguments
) -> Result<Value, Error>;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Arguments {
    base: usize,
    limit: usize,
}
