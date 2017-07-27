use std::u32;
use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Entry;
use back::ssa;
use back::analysis::*;
use back::regalloc::*;
use vm::code;
use entity::{Entity, EntityMap};
use symbol::Symbol;
use bitvec::BitVec;

pub struct Codegen {
    function: code::Function,

    registers: EntityMap<ssa::Value, usize>,
    register_count: usize,

    visited: BitVec,
    block_offsets: HashMap<ssa::Block, usize>,
    jump_offsets: HashMap<usize, ssa::Block>,
    edge_block: usize,

    constants: HashMap<code::Value, usize>,
}

impl Codegen {
    pub fn new() -> Codegen {
        Codegen {
            function: code::Function::new(),

            registers: EntityMap::new(),
            register_count: 0,

            visited: BitVec::new(),
            block_offsets: HashMap::new(),
            jump_offsets: HashMap::new(),
            edge_block: 0,

            constants: HashMap::new(),
        }
    }

    pub fn compile(mut self, program: &ssa::Function) -> code::Function {
        let control_flow = ControlFlow::compute(program);
        let liveness = Liveness::compute(program, &control_flow);
        let interference = Interference::build(program, &liveness);
        let (registers, register_count) = interference.color();

        self.registers = registers;
        self.register_count = register_count;

        self.edge_block = program.blocks.len();

        self.function.params = 0;
        self.function.locals = register_count as u32;
        self.emit_blocks(program, program.entry());
        self.fixup_jumps();

        self.function
    }

    fn emit_blocks(&mut self, program: &ssa::Function, block: ssa::Block) {
        self.visited.set(block.index());
        self.block_offsets.insert(block, self.function.instructions.len());

        for &value in &program.blocks[block].instructions {
            use back::ssa::Instruction::*;
            match program.values[value] {
                Immediate(constant) => {
                    let target = self.registers[value];
                    let a = match constant {
                        ssa::Constant::Real(real) => self.emit_real(real),
                        ssa::Constant::String(string) => self.emit_string(string),
                    };

                    let inst = code::Inst::encode(code::Op::Imm, target, a, 0);
                    self.function.instructions.push(inst);
                },

                Unary(op, a) => {
                    let op = code::Op::from(op);
                    let target = self.registers[value];
                    let a = self.registers[a];

                    let inst = code::Inst::encode(op, target, a, 0);
                    self.function.instructions.push(inst);
                }

                Binary(op, a, b) => {
                    let op = code::Op::from(op);
                    let target = self.registers[value];
                    let a = self.registers[a];
                    let b = self.registers[b];

                    let inst = code::Inst::encode(op, target, a, b);
                    self.function.instructions.push(inst);
                }

                Argument => unreachable!("corrupt function"),

                Declare(scope, symbol) => {
                    let a = self.emit_real(scope);
                    let b = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::Declare, a, b, 0);
                    self.function.instructions.push(inst);
                }

                LoadDynamic(symbol) => {
                    let target = self.registers[value];
                    let a = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::LoadDynamic, target, a, 0);
                    self.function.instructions.push(inst);
                }

                LoadField(scope, symbol) => {
                    let target = self.registers[value];
                    let a = self.registers[scope];
                    let b = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::LoadField, target, a, b);
                    self.function.instructions.push(inst);
                }

                LoadIndex(array, box ref indices) => {
                    let target = self.registers[value];
                    let a = self.registers[array];
                    let b = indices.len();

                    let inst = code::Inst::encode(code::Op::LoadIndex, target, a, b);
                    self.function.instructions.push(inst);

                    let i = indices.get(0).map(|&i| self.registers[i]).unwrap_or(0);
                    let j = indices.get(1).map(|&j| self.registers[j]).unwrap_or(0);

                    let inst = code::Inst::encode(code::Op::Args, i, j, 0);
                    self.function.instructions.push(inst);
                }

                StoreDynamic(symbol, value) => {
                    let source = self.registers[value];
                    let a = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::StoreDynamic, source, a, 0);
                    self.function.instructions.push(inst);
                }

                StoreField(scope, symbol, value) => {
                    let source = self.registers[value];
                    let a = self.registers[scope];
                    let b = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::StoreField, source, a, b);
                    self.function.instructions.push(inst);
                }

                StoreIndex(array, box ref indices, value) => {
                    let source = self.registers[value];
                    let a = self.registers[array];
                    let b = indices.len();

                    let inst = code::Inst::encode(code::Op::StoreIndex, source, a, b);
                    self.function.instructions.push(inst);

                    let i = indices.get(0).map(|&i| self.registers[i]).unwrap_or(0);
                    let j = indices.get(1).map(|&j| self.registers[j]).unwrap_or(0);

                    let inst = code::Inst::encode(code::Op::Args, i, j, 0);
                    self.function.instructions.push(inst);
                }

                With(a) => {
                    let target = self.registers[value];
                    let a = self.registers[a];

                    let inst = code::Inst::encode(code::Op::With, target, a, 0);
                    self.function.instructions.push(inst);
                }

                Next(a) => {
                    let target = self.registers[value];
                    let a = self.registers[a];

                    let inst = code::Inst::encode(code::Op::Next, target, a, 0);
                    self.function.instructions.push(inst);
                }

                Call(a, box ref args) => {
                    let target = self.registers[value];
                    let a = self.registers[a];
                    let b = args.len();

                    let inst = code::Inst::encode(code::Op::Call, target, a, b);
                    self.function.instructions.push(inst);

                    for args in args.chunks(3) {
                        let a = args.get(0).map(|&a| self.registers[a]).unwrap_or(0);
                        let b = args.get(1).map(|&b| self.registers[b]).unwrap_or(0);
                        let c = args.get(2).map(|&c| self.registers[c]).unwrap_or(0);

                        let inst = code::Inst::encode(code::Op::Args, a, b, c);
                        self.function.instructions.push(inst);
                    }
                }

                Return(a) => {
                    let a = self.registers[a];

                    let inst = code::Inst::encode(code::Op::Ret, a, 0, 0);
                    self.function.instructions.push(inst);
                }

                Exit => {
                    let inst = code::Inst::encode(code::Op::Exit, 0, 0, 0);
                    self.function.instructions.push(inst);
                }

                Jump(block, box ref arguments) => {
                    self.emit_edge(program, block, arguments);
                }

                // TODO: solve the block scheduling problem more explicitly
                Branch(a, true_block, box ref true_args, false_block, box ref false_args) => {
                    // split the false CFG edge to make room for the phi moves
                    let edge_block = ssa::Block::new(self.edge_block);
                    self.edge_block += 1;

                    let a = self.registers[a];
                    self.jump_offsets.insert(self.function.instructions.len(), edge_block);

                    let inst = code::Inst::encode(code::Op::BranchFalse, a, 0, 0);
                    self.function.instructions.push(inst);

                    self.emit_edge(program, true_block, true_args);

                    self.block_offsets.insert(edge_block, self.function.instructions.len());

                    self.emit_edge(program, false_block, false_args);
                }
            }
        }
    }

    /// Fall through or jump to the unvisited CFG nodes starting with `block`.
    fn emit_edge(&mut self, program: &ssa::Function, block: ssa::Block, arguments: &[ssa::Value]) {
        let parameters = &program.blocks[block].arguments;
        self.emit_phis(parameters, arguments);

        if self.visited.get(block.index()) {
            self.jump_offsets.insert(self.function.instructions.len(), block);

            let inst = code::Inst::encode(code::Op::Jump, 0, 0, 0);
            self.function.instructions.push(inst);

            return;
        }

        self.emit_blocks(program, block);
    }

    /// Move the values in `arguments` into `parameters`.
    ///
    /// SSA form phi nodes (represented here as parameters and arguments to blocks) are
    /// conceptually all evaluated simultaneously. This means they can represent shifts and cycles
    /// between registers, and we can't naively copy them in order.
    ///
    /// For example, moving r0 to r1 first would erase the value in r1:
    ///     block0():
    ///         ...
    ///         jump block1(r0, r1, r2)
    ///     block1(r1, r2, r3):
    ///
    /// In the general case, this can be represented as a graph. Every phi, or move operation, is
    /// a vertex. Phis are uniquely identified by their target registers, but not their source
    /// registers. There is an edge from each phi that *reads* a register to the phi that *writes*
    /// it, forming a dependency graph. This means each vertex has at most one outgoing edge, but
    /// can have many incoming edges.
    ///
    /// A topological sort of this graph produces an ordering that preserves the correct values.
    /// Cycles are broken by introducing an extra register and moving an arbitrary source value
    /// into it.
    ///
    /// TODO: tests
    /// TODO: pass in register indices directly?
    fn emit_phis(&mut self, parameters: &[ssa::Value], arguments: &[ssa::Value]) {
        // the graph representation
        // - `phis` stores the vertices, which are uniquely identified by their targets
        // - `uses` stores only in-degrees; edges are not kept explicitly

        let mut phis: HashMap<_, _> = {
            let targets = parameters.iter().map(|&a| self.registers[a]);
            let sources = arguments.iter().map(|&a| self.registers[a]);
            Iterator::zip(targets, sources).collect()
        };

        let mut uses = HashMap::new();
        for (_, &source) in phis.iter().filter(|&(_, source)| phis.contains_key(&source)) {
            *uses.entry(source).or_insert(0) += 1;
        }

        let mut work: VecDeque<_> = phis.iter()
            .map(|(&target, &source)| (target, source))
            .filter(|&(target, _)| !uses.contains_key(&target))
            .collect();
        loop {
            while let Some((target, source)) = work.pop_front() {
                let inst = code::Inst::encode(code::Op::Move, target, source, 0);
                self.function.instructions.push(inst);

                if let Entry::Occupied(mut entry) = uses.entry(source) {
                    *entry.get_mut() -= 1;
                    if *entry.get() == 0 {
                        entry.remove();
                        work.push_back((source, phis[&source]));
                    }
                }
            }

            if uses.is_empty() {
                break;
            }

            // TODO: find temp registers during register allocation?
            let temp = self.register_count;
            self.register_count += 1;

            // pick an arbitrary phi to break the cycle
            // there should only be one use left - a phi can't be in more than one cycle
            let (&used, &count) = uses.iter().nth(0).unwrap();
            assert_eq!(count, 1);

            let inst = code::Inst::encode(code::Op::Move, temp, used, 0);
            self.function.instructions.push(inst);

            // TODO: track edges to make this quicker? there can only be one use by this point
            uses.remove(&used);
            for (_, source) in phis.iter_mut().filter(|&(_, &mut source)| source == used) {
                *source = temp;
            }

            work.push_back((used, phis[&used]));
        }
    }

    fn emit_real(&mut self, real: f64) -> usize {
        let constant = code::Value::from(real);
        self.emit_constant(constant)
    }

    fn emit_string(&mut self, string: Symbol) -> usize {
        let constant = code::Value::from(string);
        self.emit_constant(constant)
    }

    fn emit_constant(&mut self, value: code::Value) -> usize {
        let Self { ref mut constants, ref mut function, .. } = *self;
        *constants.entry(value).or_insert_with(|| {
            let index = function.constants.len();
            function.constants.push(value);
            index
        })
    }

    fn fixup_jumps(&mut self) {
        for (&offset, &block) in &self.jump_offsets {
            match self.function.instructions[offset].decode() {
                (code::Op::Jump, 0, 0, 0) => {
                    let target = self.block_offsets[&block];

                    let inst = code::Inst::encode(code::Op::Jump, target, 0, 0);
                    self.function.instructions[offset] = inst;
                }

                (code::Op::BranchFalse, cond, 0, 0) => {
                    let target = self.block_offsets[&block];

                    let inst = code::Inst::encode(code::Op::BranchFalse, cond as usize, target, 0);
                    self.function.instructions[offset] = inst;
                }

                _ => unreachable!("corrupt jump instruction"),
            }
        }
    }
}

impl From<ssa::Unary> for code::Op {
    fn from(unary: ssa::Unary) -> code::Op {
        match unary {
            ssa::Unary::Negate => code::Op::Neg,
            ssa::Unary::Invert => code::Op::Not,
            ssa::Unary::BitInvert => code::Op::BitNot,
        }
    }
}

impl From<ssa::Binary> for code::Op {
    fn from(binary: ssa::Binary) -> code::Op {
        match binary {
            ssa::Binary::Lt => code::Op::Lt,
            ssa::Binary::Le => code::Op::Le,
            ssa::Binary::Eq => code::Op::Eq,
            ssa::Binary::Ne => code::Op::Ne,
            ssa::Binary::Ge => code::Op::Ge,
            ssa::Binary::Gt => code::Op::Gt,

            ssa::Binary::Add => code::Op::Add,
            ssa::Binary::Subtract => code::Op::Sub,
            ssa::Binary::Multiply => code::Op::Mul,
            ssa::Binary::Divide => code::Op::Div,
            ssa::Binary::Div => code::Op::IntDiv,
            ssa::Binary::Mod => code::Op::Mod,

            ssa::Binary::And => code::Op::And,
            ssa::Binary::Or => code::Op::Or,
            ssa::Binary::Xor => code::Op::Xor,

            ssa::Binary::BitAnd => code::Op::BitAnd,
            ssa::Binary::BitOr => code::Op::BitOr,
            ssa::Binary::BitXor => code::Op::BitXor,
            ssa::Binary::ShiftLeft => code::Op::ShiftLeft,
            ssa::Binary::ShiftRight => code::Op::ShiftRight,
        }
    }
}
