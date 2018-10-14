use std::{mem, fmt};
use std::convert::TryFrom;

use symbol::Symbol;
use vm;

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
///
/// 3-bit tag values:
/// 000 - string
/// 001 - array
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Value(u64);

pub enum Data {
    Real(f64),
    String(Symbol),
    Array(vm::Array),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Type {
    Real,
    String,
    Array,
}

impl Value {
    pub fn data(self) -> Data {
        let Value(value) = self;
        let tag = value >> 48;
        let payload = value & ((1 << 48) - 1);

        if tag & !0x7 != 0xfff8 {
            return Data::Real(unsafe { mem::transmute::<_, f64>(value) });
        }

        match tag & 0x7 {
            0x00 => Data::String(Symbol::from_index(payload as u32)),
            0x01 => Data::Array(unsafe { vm::Array::clone_from_raw(payload as *const _) }),
            _ => unreachable!("corrupt value"),
        }
    }

    pub unsafe fn release(self) {
        let Value(value) = self;
        let tag = value >> 48;
        let payload = value & ((1 << 48) - 1);

        if tag & !0x7 != 0xfff8 {
            return;
        }

        match tag & 0x7 {
            0x0 => (),
            0x1 => { vm::Array::from_raw(payload as *const _); },
            _ => unreachable!("corrupt value"),
        }
    }
}

impl Data {
    pub fn ty(&self) -> Type {
        match *self {
            Data::Real(_) => Type::Real,
            Data::String(_) => Type::String,
            Data::Array(_) => Type::Array,
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Value {
        // TODO: check for non-canonical NaNs
        let value = unsafe { mem::transmute::<_, u64>(value) };

        Value(value)
    }
}

impl From<Symbol> for Value {
    fn from(value: Symbol) -> Value {
        let tag = 0xfff8 | 0x0;
        let value = value.into_index() as u64;

        Value((tag << 48) | value)
    }
}

impl From<vm::Array> for Value {
    fn from(value: vm::Array) -> Value {
        let tag = 0xfff8 | 0x1;
        let value = value.into_raw() as u64;

        Value((tag << 48) | value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Value {
        Value::from(value as f64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Value {
        Value::from(value as i32)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let visited = Default::default();
        vm::debug::Value(*self, &visited).fmt(f)
    }
}

pub struct TryFromValueError(());

impl TryFrom<Value> for f64 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<f64, Self::Error> {
        match value.data() {
            vm::Data::Real(i) => Ok(i),
            _ => Err(TryFromValueError(())),
        }
    }
}

impl TryFrom<Value> for Symbol {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<Symbol, Self::Error> {
        match value.data() {
            vm::Data::String(s) => Ok(s),
            _ => Err(TryFromValueError(())),
        }
    }
}

impl TryFrom<Value> for i32 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<i32, Self::Error> {
        match value.data() {
            vm::Data::Real(i) => Ok(vm::Thread::to_i32(i)),
            _ => Err(TryFromValueError(())),
        }
    }
}
