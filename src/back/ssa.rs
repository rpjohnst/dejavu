use std::u32;
use entity::{Entity, EntityMap};
use symbol::Symbol;

pub struct Function {
    blocks: EntityMap<Block, Body>,
    values: EntityMap<Value, Instruction>,
}

impl Function {
    pub fn new() -> Self {
        let entry = Body {
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

    pub fn emit_constant(&mut self, value: Constant) -> Value {
        self.values.push(Instruction::Immediate(value))
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
        let block = Body {
            arguments: vec![],
            instructions: vec![],
        };

        self.blocks.push(block)
    }
}

pub struct Body {
    arguments: Vec<Value>,
    instructions: Vec<Value>,
}

#[derive(PartialEq, Debug)]
pub enum Instruction {
    Immediate(Constant),
    Unary(Unary, Value),
    Binary(Binary, Value, Value),

    Declare(Value, Symbol),
    Load(Value, Symbol, [Value; 2]),
    Store(Value, Symbol, [Value; 2], Value),

    Argument,
    Call(Value, Box<[Value]>),
    Jump(Block, Box<[Value]>),
    Branch(Value, Block, Box<[Value]>, Block, Box<[Value]>),
    Return(Value),
    Exit,

    With(Value),
    Next(Value),
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
