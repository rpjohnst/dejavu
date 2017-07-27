use std::{u8, mem};
use symbol::Symbol;

pub struct Function {
    pub params: u32,
    pub locals: u32,
    pub constants: Vec<Value>,
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

/// A GML value.
///
/// Values are NaN-boxed, representing either an `f64` or a tagged value. The encoding favors
/// `f64`s, assuming that GML will use them most frequently. Other types are stored as NaN
/// payloads.
///
/// To avoid ambiguity, NaNs are canonicalized. The hardware seems to use positive qNaN with a zero
/// payload (0x7fff8_0000_0000_0000), so other types are encoded as negative NaNs, leaving 51 bits
/// for tag and value. This could be expanded to positive NaNs at the cost of more complicated type
/// checking.
///
/// By limiting ourselves to 48-bit pointers (the current limit on x86_64 and AArch64, and a nice
/// round number for sign extension), we get 3 bits for a tag. This could be expanded to 4 bits by
/// exploiting the fact that typical kernels only give positive addresses to user space. For
/// pointer values only, another 3-5 bits could be taken from the LSB end by aligning allocations.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Value(u64);

impl From<f64> for Value {
    fn from(value: f64) -> Value {
        // TODO: check for non-canonical NaNs
        let value = unsafe { mem::transmute::<_, u64>(value) };

        Value(value)
    }
}

impl From<Symbol> for Value {
    fn from(value: Symbol) -> Value {
        let value = value.into_index() as u64;

        Value((0xfff8 << 48) | value)
    }
}

/// An encoded instruction.
///
/// Fields use this structure, stored in little-endian order:
/// | op: 8 | dst: 8 | a: 8 | b: 8 |
pub struct Inst(u32);

impl Inst {
    pub fn encode(op: Op, dst: usize, a: usize, b: usize) -> Self {
        assert!(dst <= u8::MAX as usize);
        assert!(a <= u8::MAX as usize);
        assert!(b <= u8::MAX as usize);

        Inst((op as usize | dst << 8 | a << 16 | b << 24) as u32)
    }

    pub fn decode(&self) -> (Op, u8, u8, u8) {
        let Inst(bits) = *self;
        let op = unsafe { mem::transmute::<_, Op>((bits & 0xff) as u8) };
        let dst = (bits >> 8) as u8;
        let a = (bits >> 16) as u8;
        let b = (bits >> 24) as u8;

        (op, dst, a, b)
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

    Declare,

    LoadDynamic,
    LoadField,
    LoadIndex,

    StoreDynamic,
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
