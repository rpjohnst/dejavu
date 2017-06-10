use back::ssa;
use back::analysis::*;
use back::regalloc::*;
use vm::code;

pub struct Codegen {
    function: code::Function,
}

impl Codegen {
    pub fn new() -> Codegen {
        Codegen {
            function: code::Function::new(),
        }
    }

    pub fn compile(mut self, program: &ssa::Function) -> code::Function {
        let control_flow = ControlFlow::compute(program);
        let liveness = Liveness::compute(program, &control_flow);
        let interference = Interference::build(program, &liveness);
        let registers = interference.color();

        self.function
    }
}
