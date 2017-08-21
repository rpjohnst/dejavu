use std::{u8, mem};

use vm;

pub struct Function {
    pub params: u32,
    pub locals: u32,
    pub constants: Vec<vm::Value>,
    pub instructions: Vec<Inst>,
}

impl Function {
    pub fn new() -> Function {
        Function {
            params: 0,
            locals: 0,
            constants: vec![],
            instructions: vec![],
        }
    }
}

/// An encoded instruction.
///
/// Fields use this structure, stored in little-endian order:
/// | op: 8 | dst: 8 | a: 8 | b: 8 |
#[derive(Copy, Clone)]
pub struct Inst(u32);

impl Inst {
    pub fn encode(op: Op, dst: usize, a: usize, b: usize) -> Self {
        assert!(dst <= u8::MAX as usize);
        assert!(a <= u8::MAX as usize);
        assert!(b <= u8::MAX as usize);

        Inst((op as usize | dst << 8 | a << 16 | b << 24) as u32)
    }

    pub fn decode(&self) -> (Op, usize, usize, usize) {
        let Inst(bits) = *self;
        let op = unsafe { mem::transmute::<_, Op>((bits & 0xff) as u8) };
        let dst = (bits >> 8) as u8;
        let a = (bits >> 16) as u8;
        let b = (bits >> 24) as u8;

        (op, dst as usize, a as usize, b as usize)
    }
}

#[repr(u8)]
pub enum Op {
    Imm,
    Move,
    Args,

    Neg,
    Not,
    BitNot,

    Lt,
    Le,
    Eq,
    Ne,
    Ge,
    Gt,

    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,

    And,
    Or,
    Xor,

    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,

    DeclareGlobal,
    Lookup,

    LoadField,
    LoadIndex,

    StoreField,
    StoreIndex,

    With,
    Next,

    Call,
    Ret,
    Exit,

    Jump,
    BranchFalse,
}
