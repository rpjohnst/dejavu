use std::u32;

use entity::{Entity, EntityMap};
use symbol::Symbol;
use slice::{ref_slice, ref_slice_mut};

pub struct Function {
    pub blocks: EntityMap<Block, BlockBody>,
    pub values: EntityMap<Value, Inst>,
}

impl Function {
    pub fn new() -> Self {
        let entry = BlockBody {
            arguments: vec![],
            instructions: vec![],
        };

        let mut blocks = EntityMap::new();
        blocks.push(entry);

        Function {
            blocks: blocks,
            values: EntityMap::new(),
        }
    }

    pub fn entry(&self) -> Block {
        Block(0)
    }

    pub fn terminator(&self, block: Block) -> Value {
        *self.blocks[block].instructions.last()
            .expect("empty block")
    }

    pub fn successors(&self, block: Block) -> &[Block] {
        let value = self.terminator(block);
        match self.values[value] {
            Inst::Jump { ref target, .. } => ref_slice(target),
            Inst::Branch { ref targets, .. } => targets,
            Inst::Return { .. } => &[],

            _ => panic!("corrupt block"),
        }
    }

    pub fn defs(&self, value: Value) -> Option<Value> {
        use self::Inst::*;
        match self.values[value] {
            Immediate { .. } | Unary { .. } | Binary { .. } |
            Argument | Lookup { .. } |
            Write { .. } |
            LoadField { .. } | LoadIndex { .. } |
            WriteField { .. } | ToArrayField { .. } |
            Call { .. } => Some(value),

            Undef | Alias(_) |
            DeclareGlobal { .. } |
            Read { .. } |
            StoreField { .. } | StoreIndex { .. } |
            Return { .. } |
            Jump { .. } | Branch { .. } => None,
        }
    }

    pub fn uses(&self, value: Value) -> &[Value] {
        self.values[value].arguments()
    }

    pub fn emit_instruction(&mut self, block: Block, inst: Inst) -> Value {
        let value = self.values.push(inst);
        self.blocks[block].instructions.push(value);
        value
    }

    pub fn emit_argument(&mut self, block: Block) -> Value {
        let value = self.values.push(Inst::Argument);
        self.blocks[block].arguments.push(value);
        value
    }

    pub fn make_block(&mut self) -> Block {
        let block = BlockBody {
            arguments: vec![],
            instructions: vec![],
        };

        self.blocks.push(block)
    }
}

pub struct BlockBody {
    pub arguments: Vec<Value>,
    pub instructions: Vec<Value>,
}

/// An SSA instruction.
///
/// Some of these instructions have less-than-ideal field grouping- this is so that all "used
/// values" are stored in contiguous arrays, which enables more uniform interfaces elsewhere.
#[derive(PartialEq, Debug)]
pub enum Inst {
    /// A placeholder for an undefined value.
    Undef,
    /// A placeholder for a value that has been replaced.
    ///
    /// Aliases must not exist in blocks, and must be removed before codegen. They should also be
    /// removed between or as part of optimization passes that generate them.
    Alias(Value),

    Immediate { value: Constant },
    Unary { op: Unary, arg: Value },
    Binary { op: Binary, args: [Value; 2] },

    /// A placeholder for an argument to a basic block.
    Argument,
    DeclareGlobal { symbol: Symbol },
    Lookup { symbol: Symbol },

    /// Mark a value as read at this point, error on `Undef`.
    Read { symbol: Symbol, arg: Value },
    /// `args` contains `[value, array]`. If array is a scalar, return `value`.
    Write { args: [Value; 2] },

    LoadField { scope: Value, field: Symbol },
    /// `args` contains `[array, i, j]`
    LoadIndex { args: [Value; 3] },

    /// `args` contains `[value, scope]`
    StoreField { args: [Value; 2], field: Symbol },
    /// `args` contains `[value, array, i, j]`
    StoreIndex { args: [Value; 4] },

    /// `args` contains `[value, scope]`
    WriteField { args: [Value; 2], field: Symbol },
    ToArrayField { scope: Value, field: Symbol },

    Call { symbol: Symbol, args: Vec<Value> },
    Return { arg: Value },

    Jump { target: Block, args: Vec<Value> },
    /// `args` contains `[condition, arg_lens[0].., arg_lens[1]..]`
    Branch { targets: [Block; 2], arg_lens: [usize; 2], args: Vec<Value> },
}

impl Inst {
    pub fn arguments(&self) -> &[Value] {
        use self::Inst::*;
        match *self {
            Unary { ref arg, .. } => ref_slice(arg),
            Binary { ref args, .. } => args,

            Read { ref arg, .. } => ref_slice(arg),
            Write { ref args } => args,

            LoadField { ref scope, .. } => ref_slice(scope),
            LoadIndex { ref args, .. } => args,

            StoreField { ref args, .. } => args,
            StoreIndex { ref args, .. } => args,

            WriteField { ref args, .. } => args,
            ToArrayField { ref scope, .. } => ref_slice(scope),

            Call { ref args, .. } => &args[..],
            Return { ref arg, .. } => ref_slice(arg),

            Jump { ref args, .. } => &args[..],
            Branch { ref args, .. } => &args[..],

            Undef | Alias(..) |
            Immediate { .. } |
            Argument | DeclareGlobal { .. } | Lookup { .. } => &[],
        }
    }

    pub fn arguments_mut(&mut self) -> &mut [Value] {
        use self::Inst::*;
        match *self {
            Unary { ref mut arg, .. } => ref_slice_mut(arg),
            Binary { ref mut args, .. } => args,

            Read { ref mut arg, .. } => ref_slice_mut(arg),
            Write { ref mut args, .. } => args,

            LoadField { ref mut scope, .. } => ref_slice_mut(scope),
            LoadIndex { ref mut args, .. } => args,

            StoreField { ref mut args, .. } => args,
            StoreIndex { ref mut args, .. } => args,

            WriteField { ref mut args, .. } => args,
            ToArrayField { ref mut scope, .. } => ref_slice_mut(scope),

            Call { ref mut args, .. } => &mut args[..],
            Return { ref mut arg, .. } => ref_slice_mut(arg),

            Jump { ref mut args, .. } => &mut args[..],
            Branch { ref mut args, .. } => &mut args[..],

            Undef | Alias(..) |
            Immediate { .. } |
            Argument | DeclareGlobal { .. } | Lookup { .. } => &mut [],
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Constant {
    Real(f64),
    String(Symbol),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Unary {
    Negate,
    Invert,
    BitInvert,

    With,
    Next,

    ToArray,
    ToScalar,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Binary {
    Lt,
    Le,
    Eq,
    Ne,
    Ge,
    Gt,

    Add,
    Subtract,
    Multiply,
    Divide,
    Div,
    Mod,

    And,
    Or,
    Xor,

    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

/// Implement Entity for a tuple struct containing a u32
macro_rules! derive_entity_ref {
    ($entity: ident) => {
        impl Entity for $entity {
            fn new(index: usize) -> Self {
                assert!(index < u32::MAX as usize);
                $entity(index as u32)
            }

            fn index(self) -> usize {
                self.0 as usize
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Block(u32);
derive_entity_ref!(Block);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Value(u32);
derive_entity_ref!(Value);
