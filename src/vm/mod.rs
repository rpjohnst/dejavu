pub use vm::interpreter::State;
pub use vm::value::Value;
pub use vm::array::{Array, Row};

pub mod code;
mod value;
mod array;
mod interpreter;
