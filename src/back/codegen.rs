use {ErrorHandler};
use back::ssa;
use vm::code;

pub struct Codegen<'e> {
    function: code::Function,
    errors: &'e ErrorHandler,
}

impl<'e> Codegen<'e> {
    pub fn new(errors: &'e ErrorHandler) -> Codegen<'e> {
        Codegen {
            function: code::Function::new(),
            errors: errors,
        }
    }

    pub fn compile(mut self, program: &ssa::Function) -> code::Function {
        self.function
    }
}
