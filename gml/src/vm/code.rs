use std::{u8, mem, fmt};

use crate::symbol::Symbol;
use crate::vm;

pub struct Function {
    pub params: u32,
    pub locals: u32,
    pub symbols: Vec<Symbol>,
    pub constants: Vec<vm::Value>,
    pub instructions: Vec<Inst>,
}

impl Function {
    pub fn new() -> Function {
        Function {
            params: 0,
            locals: 0,
            symbols: vec![],
            constants: vec![],
            instructions: vec![],
        }
    }
}

#[derive(Default)]
pub struct Locations {
    pub mappings: Vec<SourceMap>,
}

pub struct SourceMap {
    pub offset: u32,
    pub location: u32,
}

impl Locations {
    pub fn get_location(&self, offset: u32) -> u32 {
        let i = match self.mappings.binary_search_by_key(&offset, |map| map.offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        self.mappings[i].location
    }
}

/// An encoded instruction.
///
/// Fields use this structure, stored in little-endian order:
/// | op: 8 | dst: 8 | a: 8 | b: 8 |
#[derive(Copy, Clone)]
pub struct Inst(pub(crate) u32);

impl Inst {
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
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Op {
    Const,
    GlobalConst,
    Move,

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
    LoadScope,
    StoreScope,

    With,
    ReleaseWith,
    LoadPointer,
    NextPointer,
    NePointer,
    ExistsEntity,

    Read,
    Write,
    ScopeError,

    ToArray,
    ToScalar,
    ReleaseOwned,

    LoadField,
    LoadFieldDefault,
    LoadRow,
    LoadIndex,

    StoreField,
    StoreRow,
    StoreIndex,

    Call,
    Ret,
    CallApi,
    CallDll,
    CallGet,
    CallSet,

    Jump,
    BranchFalse,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for param in 0..self.params {
            write!(f, "%{:?}, ", param)?;
        }
        writeln!(f, ")[{:?}]", self.locals)?;

        for &inst in &self.instructions {
            let (op, a, b, c) = inst.decode();
            match op {
                Op::Const => writeln!(f, "  %{:?} = {:?} {:?}", a, op, self.constants[b])?,
                Op::GlobalConst => writeln!(f, "  %{:?} = {:?} {:?}", a, op, b)?,
                Op::Move => writeln!(f, "  %{:?} = %{:?}", a, b)?,
                Op::Neg | Op::Not | Op::BitNot | Op::ToArray | Op::ToScalar |
                Op::LoadPointer | Op::NextPointer | Op::ExistsEntity |
                Op::ScopeError =>
                    writeln!(f, "  %{:?} = {:?} %{:?}", a, op, b)?,
                Op::DeclareGlobal => writeln!(f, "  {:?} {:?}", op, self.symbols[a])?,
                Op::Lookup => writeln!(f, "  %{:?} = {:?} {:?}", a, op, self.symbols[b])?,
                Op::LoadScope => writeln!(f, "  %{:?} = {:?} {:?}", a, op, b as i8)?,
                Op::StoreScope => writeln!(f, "  {:?} %{:?}, {:?}", op, a, b as i32)?,
                Op::With => writeln!(f, "  %{:?}, %{:?} = {:?} %{:?}", a, b, op, c)?,
                Op::ReleaseWith | Op::ReleaseOwned | Op::Ret =>
                    writeln!(f, "  {:?}", op)?,
                Op::Read => writeln!(f, "  {:?} %{:?}, {:?}", op, a, self.symbols[b])?,
                Op::LoadField | Op::LoadFieldDefault =>
                    writeln!(f, "  %{:?} = {:?} %{:?}.{:?}", a, op, b, self.symbols[c])?,
                Op::LoadRow | Op::LoadIndex | Op::StoreRow =>
                    writeln!(f, "  %{:?} = {:?} %{:?}[%{:?}]", a, op, b, c)?,
                Op::StoreField =>
                    writeln!(f, "  {:?} %{:?}, %{:?}.{:?}", op, a, b, self.symbols[c])?,
                Op::StoreIndex => writeln!(f, "  {:?} %{:?}, %{:?}[%{:?}]", op, a, b, c)?,
                Op::Call =>
                    writeln!(f, "  %{:?} = {:?} {:?}(%{:?} +{:?})", b, op, self.constants[a], b, c)?,
                Op::CallApi | Op::CallGet =>
                    writeln!(f, "  %{:?} = {:?} {:?}(%{:?} +{:?})", b, op, self.symbols[a], b, c)?,
                Op::CallSet =>
                    writeln!(f, "  {:?} {:?}(%{:?} +{:?})", op, self.symbols[a], b, c)?,
                Op::Jump => writeln!(f, "  {:?} {:?}", op, a | (b << 8))?,
                Op::BranchFalse => writeln!(f, "  {:?} %{:?}, {:?}", op, a, b | (c << 8))?,
                _ => writeln!(f, "  %{:?} = {:?} %{:?}, %{:?}", a, op, b, c)?,
            }
        }

        Ok(())
    }
}
