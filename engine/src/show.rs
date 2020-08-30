use std::io::{self, Write};
use gml::vm;

pub struct State {
    write: Box<dyn Write>,
}

impl Default for State {
    fn default() -> State {
        State { write: Box::new(io::stdout()) }
    }
}

#[gml::bind]
impl State {
    pub fn set_write(&mut self, write: Box<dyn Write>) {
        self.write = write;
    }

    #[gml::api]
    pub fn show_debug_message(&mut self, arguments: &[vm::Value]) {
        for argument in arguments {
            let _ = write!(&mut *self.write, "{:?} ", argument);
        }
        let _ = writeln!(&mut *self.write);
    }
}
