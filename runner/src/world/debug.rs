use std::io::{self, Write};
use gml::{vm, front::Span, ErrorPrinter};

pub struct State {
    pub debug: vm::Debug,
    pub error: fn(&Self, &vm::Error),
    pub write: Box<dyn Write>,
}

impl Default for State {
    fn default() -> State {
        State {
            debug: vm::Debug::default(),
            error: |state, error| state.show_vm_error_write(error, io::stderr()),
            write: Box::new(io::stdout()),
        }
    }
}

#[gml::bind]
impl State {
    pub fn show_vm_error(&self, error: &vm::Error) { (self.error)(self, error); panic!() }
    pub fn show_vm_error_write<W: Write>(&self, error: &vm::Error, write: W) {
        if let [ref frame, ref stack @ ..] = error.frames[..] {
            let mut errors = ErrorPrinter::from_debug(&self.debug, frame.function, write);
            let span = Span::from_debug(&self.debug, frame);
            ErrorPrinter::error(&mut errors, span, format_args!("{}", error.kind));
            ErrorPrinter::stack_from_debug(&mut errors, &self.debug, stack);
        }
    }

    #[gml::api]
    pub fn show_debug_message(&mut self, arguments: &[vm::Value]) {
        for argument in arguments {
            let _ = write!(&mut *self.write, "{:?} ", argument);
        }
        let _ = writeln!(&mut *self.write);
    }
}
