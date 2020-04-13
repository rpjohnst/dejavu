use std::{mem, fmt, f64};
use std::num::NonZeroUsize;
use std::convert::TryFrom;

use crate::symbol::Symbol;
use crate::vm;

/// A GML value.
///
/// Values are NaN-boxed, representing either an `f64` or a tagged value. The encoding favors
/// `f64`s, assuming that GML will use them most frequently. Other types are stored as NaN
/// payloads.
///
/// To avoid ambiguity, NaNs are canonicalized. The hardware seems to use positive qNaN with a zero
/// payload (0x7ff8_0000_0000_0000), so other types are encoded as negative NaNs, leaving 52 bits
/// for tag and value (including the quiet bit- we ensure the payload is non-zero elsewhere).
///
/// By limiting ourselves to 48-bit pointers (the current limit on x86_64 and AArch64, and a nice
/// round number for sign extension), we get 4 bits for a tag. This could be expanded to 5 bits by
/// exploiting the fact that typical kernels only give positive addresses to user space. For
/// pointer values only, another 3-5 bits could be taken from the LSB end by aligning allocations.
///
/// 4-bit tag values:
/// 0000 - string
/// 0001 - array
///
/// 1000 - canonical NaN
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

        // This check covers finite, infinite, and positive NaN reals.
        // Negative NaN is handled below with tagged values.
        if value <= 0xfff0_0000_0000_0000 {
            return Data::Real(f64::from_bits(value));
        }

        let tag = value >> 48;
        let payload = value & ((1 << 48) - 1);
        match tag & 0xf {
            // Safety: String values are always constructed from non-zero `Symbol`s.
            // (The check above also excludes cases where both `tag & 0xf` and `payload` are zero.)
            0x0 => Data::String(Symbol::from_index(unsafe {
                NonZeroUsize::new_unchecked(payload as usize)
            })),

            0x1 => Data::Array(unsafe { vm::Array::clone_from_raw(payload as *const _) }),

            0x8 => Data::Real(f64::from_bits(value)),
            _ => unreachable!("corrupt value"),
        }
    }

    pub unsafe fn release(self) {
        let Value(value) = self;
        let tag = value >> 48;
        let payload = value & ((1 << 48) - 1);

        if tag & !0xf != 0xfff0 {
            return;
        }

        match tag & 0xf {
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
        let tag = 0xfff0 | 0x0;
        let value = value.into_index().get() as u64;

        Value((tag << 48) | value)
    }
}

impl From<vm::Array> for Value {
    fn from(value: vm::Array) -> Value {
        let tag = 0xfff0 | 0x1;
        let value = value.into_raw() as u64;

        Value((tag << 48) | value)
    }
}

impl From<()> for Value {
    fn from(_: ()) -> Value {
        Value::from(0.0)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Value {
        Value::from(value as f64)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Value {
        Value::from(value as f64)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Value {
        Value::from(value as f64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Value {
        Value::from(value as i32)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let visited = Default::default();
        vm::debug::Value { value: *self, visited: &visited }.fmt(f)
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

impl TryFrom<Value> for f32 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<f32, Self::Error> {
        match value.data() {
            vm::Data::Real(i) => Ok(i as f32),
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

impl TryFrom<Value> for u32 {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<u32, Self::Error> {
        match value.data() {
            vm::Data::Real(i) => Ok(vm::Thread::to_u32(i)),
            _ => Err(TryFromValueError(())),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = TryFromValueError;

    fn try_from(value: Value) -> Result<bool, Self::Error> {
        match value.data() {
            vm::Data::Real(i) => Ok(vm::Thread::to_bool(i)),
            _ => Err(TryFromValueError(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::symbol::{keyword, Symbol};
    use crate::vm;

    #[test]
    fn reals() {
        let value = vm::Value::from(0.0);
        assert!(match value.data() { vm::Data::Real(x) if x == 0.0 => true, _ => false });

        let value = vm::Value::from(3.5);
        assert!(match value.data() { vm::Data::Real(x) if x == 3.5 => true, _ => false });

        let value = vm::Value::from(f64::INFINITY);
        assert!(match value.data() {
            vm::Data::Real(x) if x == f64::INFINITY => true,
            _ => false
        });

        let value = vm::Value::from(f64::NEG_INFINITY);
        assert!(match value.data() {
            vm::Data::Real(x) if x == f64::NEG_INFINITY => true,
            _ => false
        });

        let value = vm::Value::from(f64::NAN);
        assert!(match value.data() { vm::Data::Real(x) if f64::is_nan(x) => true, _ => false });
    }

    #[test]
    fn strings() {
        let value = vm::Value::from(Symbol::intern("true"));
        assert!(match value.data() { vm::Data::String(keyword::True) => true, _ => false });

        let value = vm::Value::from(Symbol::intern("argument0"));
        assert!(match value.data() {
            vm::Data::String(x) if x == Symbol::from_argument(0) => true,
            _ => false
        });

        let symbol = Symbol::intern("foo");
        let value = vm::Value::from(symbol);
        assert!(match value.data() { vm::Data::String(x) if x == symbol => true, _ => false });
    }
}
