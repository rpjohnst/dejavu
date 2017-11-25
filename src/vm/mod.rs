pub use vm::interpreter::State;
pub use vm::interpreter::Error;
pub use vm::value::{Type, Value, Data};
pub use vm::array::{Array, Row};

pub mod code;
pub mod debug;
mod value;
mod array;
mod interpreter;
