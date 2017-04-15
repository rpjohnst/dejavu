pub struct Function {
    params: u32,
    locals: u32,
    instructions: Vec<u32>,
}

impl Function {
    pub fn new() -> Function {
        Function {
            params: 0,
            locals: 0,
            instructions: vec![],
        }
    }
}
