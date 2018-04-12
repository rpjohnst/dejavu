use std::{u32, slice};
use std::collections::{hash_map::Entry, HashMap, VecDeque};

use bit_vec::BitVec;
use handle_map::{Handle, HandleMap};
use symbol::Symbol;
use back::{ssa, analysis::*, regalloc::*};
use vm::{self, code};

pub struct Codegen {
    function: code::Function,

    registers: HandleMap<ssa::Value, usize>,
    register_count: usize,

    visited: BitVec,
    block_offsets: HashMap<ssa::Block, usize>,
    jump_offsets: HashMap<usize, ssa::Block>,
    edge_block: usize,

    constants: HashMap<vm::Value, usize>,
}

impl Codegen {
    pub fn new() -> Codegen {
        Codegen {
            function: code::Function::new(),

            registers: HandleMap::new(),
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
        let (registers, param_count, register_count) = interference.color();

        self.registers = registers;
        self.register_count = register_count;

        self.edge_block = program.blocks.len();

        self.emit_blocks(program, ssa::ENTRY);
        self.fixup_jumps();

        self.function.params = param_count as u32;
        self.function.locals = self.register_count as u32;

        self.function
    }

    fn emit_blocks(&mut self, program: &ssa::Function, block: ssa::Block) {
        self.visited.set(block.index());
        self.block_offsets.insert(block, self.function.instructions.len());

        for &value in &program.blocks[block].instructions {
            use back::ssa::Inst::*;
            match program.values[value] {
                // these should not be used as instructions
                Undef | Alias(_) | Argument => unreachable!("corrupt function"),

                Immediate { value: constant } => {
                    let target = self.registers[value];
                    let a = match constant {
                        ssa::Constant::Real(real) => self.emit_real(real),
                        ssa::Constant::String(string) => self.emit_string(string),
                    };

                    let inst = code::Inst::encode(code::Op::Imm, target, a, 0);
                    self.function.instructions.push(inst);
                },

                Unary { op, arg: a } => {
                    let op = code::Op::from(op);
                    let target = self.registers[value];
                    let a = self.registers[a];

                    let inst = code::Inst::encode(op, target, a, 0);
                    self.function.instructions.push(inst);
                }

                Binary { op, args: [a, b] } => {
                    let op = code::Op::from(op);
                    let target = self.registers[value];
                    let a = self.registers[a];
                    let b = self.registers[b];

                    let inst = code::Inst::encode(op, target, a, b);
                    self.function.instructions.push(inst);
                }

                DeclareGlobal { symbol } => {
                    let a = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::DeclareGlobal, a, 0, 0);
                    self.function.instructions.push(inst);
                }

                Lookup { symbol } => {
                    let target = self.registers[value];
                    let a = self.emit_string(symbol);

                    let inst = code::Inst::encode(code::Op::Lookup, target, a, 0);
                    self.function.instructions.push(inst);
                }

                LoadScope { scope } => {
                    let target = self.registers[value];
                    let scope = scope as usize;

                    let inst = code::Inst::encode(code::Op::LoadScope, target, scope, 0);
                    self.function.instructions.push(inst);
                }

                StoreScope { scope, arg } => {
                    let scope = scope as usize;
                    let arg = self.registers[arg];

                    let inst = code::Inst::encode(code::Op::StoreScope, arg, scope, 0);
                    self.function.instructions.push(inst);
                }

                Read { symbol, arg } => {
                    let a = self.emit_string(symbol);
                    let b = self.registers[arg];

                    let inst = code::Inst::encode(code::Op::Read, a, b, 0);
                    self.function.instructions.push(inst);
                }

                Write { args: [source, array] } => {
                    let target = self.registers[value];
                    let a = self.registers[source];
                    let b = self.registers[array];

                    let inst = code::Inst::encode(code::Op::Write, target, a, b);
                    self.function.instructions.push(inst);
                }

                LoadField { scope, field } => {
                    let target = self.registers[value];
                    let a = self.registers[scope];
                    let b = self.emit_string(field);

                    let inst = code::Inst::encode(code::Op::LoadField, target, a, b);
                    self.function.instructions.push(inst);
                }

                LoadFieldDefault { scope, field } => {
                    let target = self.registers[value];
                    let a = self.registers[scope];
                    let b = self.emit_string(field);

                    let inst = code::Inst::encode(code::Op::LoadFieldDefault, target, a, b);
                    self.function.instructions.push(inst);
                }

                StoreField { args: [value, scope], field } => {
                    let source = self.registers[value];
                    let a = self.registers[scope];
                    let b = self.emit_string(field);

                    let inst = code::Inst::encode(code::Op::StoreField, source, a, b);
                    self.function.instructions.push(inst);
                }

                StoreIndex { args: [value, row, j] } => {
                    let source = self.registers[value];
                    let row = self.registers[row];
                    let j = self.registers[j];

                    let inst = code::Inst::encode(code::Op::StoreIndex, source, row, j);
                    self.function.instructions.push(inst);
                }

                Release { arg: a } => {
                    let a = self.registers[a];

                    let inst = code::Inst::encode(code::Op::Release, a, 0, 0);
                    self.function.instructions.push(inst);
                }

                Call { symbol, prototype, ref args, ref parameters } => {
                    self.emit_phis(parameters, args);

                    let symbol = self.emit_string(symbol);
                    let base = self.registers[parameters[0]];
                    let len = args.len();

                    let op = match prototype {
                        ssa::Prototype::Script => code::Op::Call,
                        ssa::Prototype::Function => code::Op::CallNative,
                    };
                    let inst = code::Inst::encode(op, symbol, base, len);
                    self.function.instructions.push(inst);

                    self.emit_phis(slice::from_ref(&value), &parameters[..1]);
                }

                Return { ref arg } => {
                    self.emit_phis(slice::from_ref(&program.return_def), slice::from_ref(arg));

                    let inst = code::Inst::encode(code::Op::Ret, 0, 0, 0);
                    self.function.instructions.push(inst);
                }

                Jump { target, ref args } => {
                    self.emit_edge(program, target, args);
                }

                // TODO: solve the block scheduling problem more explicitly
                Branch {
                    targets: [true_block, false_block],
                    arg_lens: [true_args, false_args],
                    ref args
                } => {
                    // split the false CFG edge to make room for the phi moves
                    let edge_block = ssa::Block::new(self.edge_block);
                    self.edge_block += 1;

                    let a = self.registers[args[0]];
                    self.jump_offsets.insert(self.function.instructions.len(), edge_block);

                    let inst = code::Inst::encode(code::Op::BranchFalse, a, 0, 0);
                    self.function.instructions.push(inst);

                    let true_start = 1;
                    let true_end = true_start + true_args as usize;
                    self.emit_edge(program, true_block, &args[true_start..true_end]);

                    self.block_offsets.insert(edge_block, self.function.instructions.len());

                    let false_start = true_end;
                    let false_end = false_start + false_args as usize;
                    self.emit_edge(program, false_block, &args[false_start..false_end]);
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
    fn emit_phis(&mut self, parameters: &[ssa::Value], arguments: &[ssa::Value]) {
        // the graph representation
        // - `phis` stores the vertices, which are uniquely identified by their targets
        // - `uses` stores only in-degrees; edges are not kept explicitly

        let mut phis: HashMap<_, _> = {
            let targets = parameters.iter().map(|&a| self.registers[a]);
            let sources = arguments.iter().map(|&a| self.registers[a]);

            // Single-vertex cycles are a success by the register allocator (in particular, copy
            // coalescing), so leave them out rather than spilling them later.
            Iterator::zip(targets, sources)
                .filter(|&(target, source)| target != source)
                .collect()
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

            // TODO: find temp registers during register allocation
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
        let constant = vm::Value::from(real);
        self.emit_constant(constant)
    }

    fn emit_string(&mut self, string: Symbol) -> usize {
        let constant = vm::Value::from(string);
        self.emit_constant(constant)
    }

    fn emit_constant(&mut self, value: vm::Value) -> usize {
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

            ssa::Unary::With => code::Op::With,
            ssa::Unary::Next => code::Op::Next,

            ssa::Unary::ToArray => code::Op::ToArray,
            ssa::Unary::ToScalar => code::Op::ToScalar,
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

            ssa::Binary::LoadRow => code::Op::LoadRow,
            ssa::Binary::LoadIndex => code::Op::LoadIndex,

            ssa::Binary::StoreRow => code::Op::StoreRow,
        }
    }
}
