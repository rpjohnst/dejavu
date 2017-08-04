use std::u32;

use entity::{Entity, EntityMap};
use symbol::Symbol;

pub struct Function {
    pub blocks: EntityMap<Block, BlockBody>,
    pub values: EntityMap<Value, Instruction>,
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

    pub fn successors(&self, block: Block) -> Vec<Block> {
        let value = *self.blocks[block].instructions.last()
            .expect("empty block");

        match self.values[value] {
            Instruction::Jump(block, _) => vec![block],
            Instruction::Branch(_, t, _, f, _) => vec![t, f],
            Instruction::Return(_) | Instruction::Exit => vec![],

            _ => panic!("corrupt block"),
        }
    }

    pub fn defs(&self, value: Value) -> Option<Value> {
        use self::Instruction::*;
        match self.values[value] {
            Immediate(..) | Unary(..) | Binary(..) | Argument |
            LoadDynamic(..) | LoadField(..) | LoadIndex(..) |
            With(..) | Next(..) |
            Call(..) => Some(value),
            _ => None,
        }
    }

    pub fn uses(&self, value: Value) -> Vec<Value> {
        use self::Instruction::*;
        match self.values[value] {
            Unary(_, value) => vec![value],
            Binary(_, left, right) => vec![left, right],

            LoadField(scope, _) => vec![scope],
            LoadIndex(array, box ref indices) => {
                let mut uses = Vec::with_capacity(1 + indices.len());
                uses.push(array);
                uses.extend(indices);
                uses
            }

            StoreDynamic(_, value) => vec![value],
            StoreField(scope, _, value) => vec![scope, value],
            StoreIndex(array, box ref indices, value) => {
                let mut uses = Vec::with_capacity(1 + indices.len() + 1);
                uses.push(array);
                uses.extend(indices);
                uses.push(value);
                uses
            }

            Call(function, box ref arguments) => {
                let mut uses = Vec::with_capacity(1 + arguments.len());
                uses.push(function);
                uses.extend(arguments);
                uses
            }
            Jump(_, box ref arguments) => arguments.iter().cloned().collect(),
            Branch(value, _, box ref t, _, box ref f) => {
                let mut uses = Vec::with_capacity(1 + t.len() + f.len());
                uses.push(value);
                uses.extend(t);
                uses.extend(f);
                uses
            }
            Return(value) => vec![value],

            With(value) => vec![value],
            Next(value) => vec![value],

            _ => vec![],
        }
    }

    pub fn emit_instruction(&mut self, block: Block, inst: Instruction) -> Value {
        let value = self.values.push(inst);
        self.blocks[block].instructions.push(value);
        value
    }

    pub fn emit_argument(&mut self, block: Block) -> Value {
        let value = self.values.push(Instruction::Argument);
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

#[derive(PartialEq, Debug)]
pub enum Instruction {
    Immediate(Constant),
    Unary(Unary, Value),
    Binary(Binary, Value, Value),

    Argument,
    Declare(f64, Symbol),

    LoadDynamic(Symbol),
    LoadField(Value, Symbol),
    LoadIndex(Value, Box<[Value]>),

    StoreDynamic(Symbol, Value),
    StoreField(Value, Symbol, Value),
    StoreIndex(Value, Box<[Value]>, Value),

    With(Value),
    Next(Value),

    Call(Value, Box<[Value]>),
    Return(Value),
    Exit,

    Jump(Block, Box<[Value]>),
    Branch(Value, Block, Box<[Value]>, Block, Box<[Value]>),
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
