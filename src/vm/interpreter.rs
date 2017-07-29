use vm::{self, code};

pub struct State {
    stack: Vec<vm::Value>,
}

impl State {
    pub fn new() -> State {
        State {
            stack: vec![],
        }
    }

    pub fn execute(&mut self, function: &code::Function) {
    }
}
