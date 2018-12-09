use std::{i8, u8, u16, slice};
use std::collections::{hash_map::Entry, HashMap, VecDeque};

use crate::bit_vec::BitVec;
use crate::handle_map::{Handle, HandleMap};
use crate::symbol::Symbol;
use crate::back::{ssa, analysis::*, regalloc::*};
use crate::vm::{self, code};

pub struct Codegen {
    function: code::Function,

    registers: HandleMap<ssa::Value, usize>,
    register_count: usize,

    visited: BitVec,
    block_offsets: HashMap<ssa::Label, usize>,
    jump_offsets: HashMap<usize, ssa::Label>,
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

    fn emit_blocks(&mut self, program: &ssa::Function, block: ssa::Label) {
        self.visited.set(block.index());
        self.block_offsets.insert(block, self.function.instructions.len());

        for &value in &program.blocks[block].instructions {
            use crate::back::ssa::Instruction::*;

            // TODO: move this logic to live range splitting
            if let Unary { op: ssa::Opcode::Return, arg } = program.values[value] {
                self.emit_phis(slice::from_ref(&program.return_def), slice::from_ref(&arg));

                let inst = inst(code::Op::Ret).encode();
                self.function.instructions.push(inst);

                continue;
            }
            if let Call { op, symbol: a, ref args, ref parameters } = program.values[value] {
                self.emit_phis(parameters, args);

                let op = code::Op::from(op);
                let a = self.emit_string(a);
                let b = self.registers[parameters[0]];
                let c = args.len();
                let inst = inst(op).index(a).index(b).index(c).encode();
                self.function.instructions.push(inst);

                self.emit_phis(slice::from_ref(&value), &parameters[..1]);
                continue;
            }

            // TODO: solve the block scheduling problem more explicitly
            if let Jump { op: ssa::Opcode::Jump, target, ref args } = program.values[value] {
                self.emit_edge(program, target, args);
                continue;
            }
            if let Branch {
                op: ssa::Opcode::Branch,
                targets: [true_block, false_block],
                arg_lens: [true_args, false_args],
                ref args,
            } = program.values[value] {
                // split the false CFG edge to make room for the phi moves
                // TODO: make program &mut to replace self.edge_block with program.make_block?
                let edge_block = ssa::Label::new(self.edge_block);
                self.edge_block += 1;

                self.jump_offsets.insert(self.function.instructions.len(), edge_block);

                let a = args[0];
                let inst = inst(code::Op::BranchFalse).index(self.registers[a]).encode();
                self.function.instructions.push(inst);

                let true_start = 1;
                let true_end = true_start + true_args as usize;
                self.emit_edge(program, true_block, &args[true_start..true_end]);

                self.block_offsets.insert(edge_block, self.function.instructions.len());

                let false_start = true_end;
                let false_end = false_start + false_args as usize;
                self.emit_edge(program, false_block, &args[false_start..false_end]);
                continue;
            }

            let op = code::Op::from(program.op(value));
            let mut inst = inst(op);

            for def in program.defs(value) {
                inst.index(self.registers[def]);
            }

            for &arg in program.uses(value) {
                inst.index(self.registers[arg]);
            }

            match program.values[value] {
                Alias { .. } | Project { .. } | Parameter => panic!("compiling non-instruction"),

                // TODO: de-specialize these
                UnaryReal { op: ssa::Opcode::LoadScope, real } => { inst.scope(real); }
                BinaryReal { op: ssa::Opcode::StoreScope, real, .. } => { inst.scope(real); }

                UnaryReal { real, .. } => { inst.index(self.emit_real(real)); }
                UnarySymbol { symbol, .. } => { inst.index(self.emit_string(symbol)); }
                BinaryReal { real, .. } => { inst.index(self.emit_real(real)); }
                BinarySymbol { symbol, .. } => { inst.index(self.emit_string(symbol)); }
                TernarySymbol { symbol, .. } => { inst.index(self.emit_string(symbol)); }

                _ => {}
            }

            self.function.instructions.push(inst.encode());
        }
    }

    /// Fall through or jump to the unvisited CFG nodes starting with `target`.
    fn emit_edge(&mut self, program: &ssa::Function, target: ssa::Label, arguments: &[ssa::Value]) {
        // TODO: move this logic to live range splitting
        let parameters = &program.blocks[target].parameters;
        self.emit_phis(parameters, arguments);

        if self.visited.get(target.index()) {
            self.jump_offsets.insert(self.function.instructions.len(), target);

            let inst = inst(code::Op::Jump).encode();
            self.function.instructions.push(inst);

            return;
        }

        self.emit_blocks(program, target);
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
    // TODO: replace this with live range splitting
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
                let inst = inst(code::Op::Move).index(target).index(source).encode();
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

            // TODO: move this logic to live range splitting
            let temp = self.register_count;
            self.register_count += 1;

            // pick an arbitrary phi to break the cycle
            // there should only be one use left - a phi can't be in more than one cycle
            let (&used, &count) = uses.iter().nth(0).unwrap();
            assert_eq!(count, 1);

            let inst = inst(code::Op::Move).index(temp).index(used).encode();
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

                    let inst = inst(code::Op::Jump).wide_index(target).encode();
                    self.function.instructions[offset] = inst;
                }

                (code::Op::BranchFalse, cond, 0, 0) => {
                    let target = self.block_offsets[&block];

                    let inst = inst(code::Op::BranchFalse).index(cond).wide_index(target).encode();
                    self.function.instructions[offset] = inst;
                }

                _ => unreachable!("corrupt jump instruction"),
            }
        }
    }
}

struct InstBuilder {
    fields: [u8; 4],
    filled: usize,
}

fn inst(op: code::Op) -> InstBuilder {
    InstBuilder {
        fields: [op as u8, 0, 0, 0],
        filled: 1,
    }
}

impl InstBuilder {
    fn index(&mut self, index: usize) -> &mut Self {
        assert!(index <= u8::MAX as usize);
        self.fields[self.filled] = index as u8;
        self.filled += 1;
        self
    }

    fn wide_index(&mut self, index: usize) -> &mut Self {
        assert!(index <= u16::MAX as usize);
        self.fields[self.filled] = index as u8;
        self.filled += 1;
        self.fields[self.filled] = (index >> 8) as u8;
        self.filled += 1;
        self
    }

    fn scope(&mut self, scope: f64) -> &mut Self {
        assert!(scope <= i8::MAX as f64);
        assert!(scope >= i8::MIN as f64);
        self.fields[self.filled] = scope as i8 as u8;
        self.filled += 1;
        self
    }

    fn encode(&mut self) -> code::Inst {
        code::Inst(
            (self.fields[0] as u32) |
            (self.fields[1] as u32) << 8 |
            (self.fields[2] as u32) << 16 |
            (self.fields[3] as u32) << 24
        )
    }
}

impl From<ssa::Opcode> for code::Op {
    fn from(op: ssa::Opcode) -> code::Op {
        match op {
            ssa::Opcode::Constant => code::Op::Imm,

            ssa::Opcode::Negate => code::Op::Neg,
            ssa::Opcode::Invert => code::Op::Not,
            ssa::Opcode::BitInvert => code::Op::BitNot,

            ssa::Opcode::ToArray => code::Op::ToArray,
            ssa::Opcode::ToScalar => code::Op::ToScalar,

            ssa::Opcode::Release => code::Op::Release,
            ssa::Opcode::Return => code::Op::Ret,

            ssa::Opcode::With => code::Op::With,
            ssa::Opcode::ScopeError => code::Op::ScopeError,
            ssa::Opcode::LoadPointer => code::Op::LoadPointer,
            ssa::Opcode::NextPointer => code::Op::NextPointer,
            ssa::Opcode::ExistsEntity => code::Op::ExistsEntity,

            ssa::Opcode::DeclareGlobal => code::Op::DeclareGlobal,
            ssa::Opcode::Lookup => code::Op::Lookup,
            ssa::Opcode::LoadScope => code::Op::LoadScope,

            ssa::Opcode::Lt => code::Op::Lt,
            ssa::Opcode::Le => code::Op::Le,
            ssa::Opcode::Eq => code::Op::Eq,
            ssa::Opcode::Ne => code::Op::Ne,
            ssa::Opcode::Ge => code::Op::Ge,
            ssa::Opcode::Gt => code::Op::Gt,

            ssa::Opcode::NePointer => code::Op::NePointer,

            ssa::Opcode::Add => code::Op::Add,
            ssa::Opcode::Subtract => code::Op::Sub,
            ssa::Opcode::Multiply => code::Op::Mul,
            ssa::Opcode::Divide => code::Op::Div,
            ssa::Opcode::Div => code::Op::IntDiv,
            ssa::Opcode::Mod => code::Op::Mod,

            ssa::Opcode::And => code::Op::And,
            ssa::Opcode::Or => code::Op::Or,
            ssa::Opcode::Xor => code::Op::Xor,

            ssa::Opcode::BitAnd => code::Op::BitAnd,
            ssa::Opcode::BitOr => code::Op::BitOr,
            ssa::Opcode::BitXor => code::Op::BitXor,
            ssa::Opcode::ShiftLeft => code::Op::ShiftLeft,
            ssa::Opcode::ShiftRight => code::Op::ShiftRight,

            ssa::Opcode::Read => code::Op::Read,
            ssa::Opcode::Write => code::Op::Write,

            ssa::Opcode::StoreScope => code::Op::StoreScope,

            ssa::Opcode::LoadField => code::Op::LoadField,
            ssa::Opcode::LoadFieldDefault => code::Op::LoadFieldDefault,

            ssa::Opcode::LoadRow => code::Op::LoadRow,
            ssa::Opcode::StoreRow => code::Op::StoreRow,
            ssa::Opcode::LoadIndex => code::Op::LoadIndex,

            ssa::Opcode::StoreField => code::Op::StoreField,
            ssa::Opcode::StoreIndex => code::Op::StoreIndex,

            ssa::Opcode::Call => code::Op::Call,
            ssa::Opcode::CallApi => code::Op::CallApi,
            ssa::Opcode::CallGet => code::Op::CallGet,
            ssa::Opcode::CallSet => code::Op::CallSet,
            ssa::Opcode::Jump => code::Op::Jump,
            ssa::Opcode::Branch => code::Op::BranchFalse,
        }
    }
}
