use std::{hint, mem, cmp, fmt};
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::num::NonZeroUsize;

use crate::symbol::Symbol;
use crate::vm;

/// A GML value.
///
/// Values are NaN-boxed, representing either an `f64` or some tagged value. This encoding favors
/// `f64`s, assuming that GML will use them most frequently. Other types are stored as NaN
/// payloads.
///
/// To avoid ambiguity, NaNs are canonicalized. The hardware seems to use positive qNaN with a zero
/// payload (0x7ff8_0000_0000_0000), so other types are encoded as negative qNaNs, leaving 51 bits
/// for tag and value. This means we can check for a NaN-boxed value with an integer comparison.
///
/// By limiting ourselves to 48-bit pointers (the current limit on x86_64 and AArch64, and a nice
/// round number for sign extension), we get 3 bits for a tag. Pointer values could additionally
/// use their lower bits by aligning allocations, if 3 bits is not enough.
///
/// 3-bit tag values:
/// 000 - string
/// 001 - array
#[derive(Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Value(u64);

/// An un-owned GML value.
///
/// A `ValueRef` has the capabilities of a `&Value` with the representation of a `Value`. This
/// sacrifices pointer identity (two `ValueRef`s cannot tell whether they borrow from the same
/// `Value`) and some conveniences (&/* syntax and auto-(de)ref) for a more direct and efficient
/// calling convention.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ValueRef<'a> {
    value: u64,
    _marker: PhantomData<&'a Value>,
}

/// A convenient, unpacked version of a `ValueRef`.
pub enum Data<'a> {
    Real(f64),
    String(Symbol),
    Array(vm::ArrayRef<'a>),
}

impl Default for Value {
    fn default() -> Self { Self::from(0.0) }
}

impl From<f64> for Value {
    fn from(value: f64) -> Value {
        // Canonicalize any NaNs that overlap our tagged value space.
        let value = cmp::min(f64::to_bits(value), 0xfff8_0000_0000_0000);
        Value(value)
    }
}

impl From<Symbol> for Value {
    fn from(value: Symbol) -> Value {
        let tag = 0xfff8 | 0b000;
        Value((tag << 48) | value.into_index().get() as u64)
    }
}

impl From<vm::Array> for Value {
    fn from(value: vm::Array) -> Value {
        let tag = 0xfff8 | 0b001;
        Value((tag << 48) | value.into_raw() as u64)
    }
}

impl Clone for Value {
    fn clone(&self) -> Value { self.borrow().clone() }
}

impl Drop for Value {
    fn drop(&mut self) {
        match self.borrow().decode() {
            // Safety: `self` was constructed from a full `Array`.
            Data::Real(_) | Data::String(_) => {}
            Data::Array(array) => unsafe { let _ = vm::Array::from_raw(array.as_raw()); }
        }
    }
}

impl Value {
    /// Convert a `&Value` into a `ValueRef`.
    pub fn borrow(&self) -> ValueRef<'_> {
        let Value(value) = *self;
        ValueRef { value, _marker: PhantomData }
    }

    /// Convert a `Value` into a `ValueRef<'static>`.
    pub fn leak(self) -> ValueRef<'static> {
        let Value(value) = self;
        ValueRef { value, _marker: PhantomData }
    }

    pub fn into_raw(self) -> u64 { let Value(value) = self; value }

    pub unsafe fn from_raw(raw: u64) -> Value { Value(raw) }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.borrow().fmt(f) }
}

impl Default for ValueRef<'_> {
    fn default() -> Self { Value::default().leak() }
}

impl<'a> ValueRef<'a> {
    /// Convert this borrowed value into an owned value.
    pub fn clone(self) -> Value {
        match self.decode() {
            Data::Real(_) | Data::String(_) => Value(self.value),
            Data::Array(array) => Value::from(array.clone()),
        }
    }

    /// Unpack a `ValueRef` into a type Rust code can work with.
    pub fn decode(self) -> Data<'a> {
        let ValueRef { value, .. } = self;

        // This is our canonical NaN. All tagged values are above it; it is the highest real.
        if value <= 0xfff8_0000_0000_0000 {
            return Data::Real(f64::from_bits(value));
        }

        let tag = value >> 48;
        let payload = value & ((1 << 48) - 1);
        match tag & 0b111 {
            // Safety: String values are always constructed from non-zero `Symbol`s.
            0b000 => unsafe {
                let payload = NonZeroUsize::new_unchecked(payload as usize);
                Data::String(Symbol::from_index(payload))
            }

            // Safety: The returned `ArrayRef` borrows from `self`.
            0b001 => unsafe { Data::Array(vm::ArrayRef::from_raw(payload as *const _)) }

            // Safety: A `Value` cannot be constructed with any other tag value.
            _ => unsafe { hint::unreachable_unchecked() }
        }
    }
}

impl AsRef<Value> for ValueRef<'_> {
    /// Convert a `ValueRef` into a `&Value`.
    ///
    /// Note that we cannot offer the more-permissive reborrow-style variant that converts a
    /// `ValueRef<'a>` to a `&'a Value`, because we do not actually have the address of a real
    /// `Value`.
    fn as_ref(&self) -> &Value {
        // Safety: `Value` and `ValueRef` are `#[repr(transparent)]` and contain a single `u64`.
        //
        // The return value borrows from `self`, so even though it does not actually point to the
        // original `Value` it will still be prevented from outliving it.
        unsafe { mem::transmute::<&ValueRef<'_>, &Value>(self) }
    }
}

impl fmt::Debug for ValueRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let visited = Default::default();
        vm::debug::Value { value: *self, visited: &visited }.fmt(f)
    }
}

// Common type conversions that need special handling to match GM:

// TODO: round-to-nearest instead of truncate
pub fn to_i32(value: f64) -> i32 { value as i32 }

// TODO: round-to-nearest instead of truncate
pub fn to_u32(value: f64) -> u32 { value as u32 }

pub fn to_bool(value: f64) -> bool { to_i32(value) > 0 }

// `From` and `TryFrom` impls to marshal values in and out of API bindings:

impl From<()> for Value { fn from(_: ()) -> Value { Value::from(0.0) } }

impl From<f32> for Value { fn from(value: f32) -> Value { Value::from(value as f64) } }

impl From<i32> for Value { fn from(value: i32) -> Value { Value::from(value as f64) } }

impl From<u32> for Value { fn from(value: u32) -> Value { Value::from(value as f64) } }

impl From<bool> for Value { fn from(value: bool) -> Value { Value::from(value as i32) } }

pub struct TryFromValueError;

impl TryFrom<ValueRef<'_>> for f64 {
    type Error = TryFromValueError;
    fn try_from(value: ValueRef<'_>) -> Result<f64, Self::Error> {
        match value.decode() {
            vm::Data::Real(i) => Ok(i),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<ValueRef<'_>> for Symbol {
    type Error = TryFromValueError;
    fn try_from(value: ValueRef<'_>) -> Result<Symbol, Self::Error> {
        match value.decode() {
            vm::Data::String(s) => Ok(s),
            _ => Err(TryFromValueError)
        }
    }
}

impl TryFrom<ValueRef<'_>> for f32 {
    type Error = TryFromValueError;
    fn try_from(value: ValueRef<'_>) -> Result<f32, Self::Error> {
        match value.decode() {
            vm::Data::Real(i) => Ok(i as f32),
            _ => Err(TryFromValueError)
        }
    }
}

impl TryFrom<ValueRef<'_>> for i32 {
    type Error = TryFromValueError;
    fn try_from(value: ValueRef<'_>) -> Result<i32, Self::Error> {
        match value.decode() {
            vm::Data::Real(i) => Ok(vm::to_i32(i)),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<ValueRef<'_>> for u32 {
    type Error = TryFromValueError;
    fn try_from(value: ValueRef<'_>) -> Result<u32, Self::Error> {
        match value.decode() {
            vm::Data::Real(i) => Ok(vm::to_u32(i)),
            _ => Err(TryFromValueError),
        }
    }
}

impl TryFrom<ValueRef<'_>> for bool {
    type Error = TryFromValueError;
    fn try_from(value: ValueRef<'_>) -> Result<bool, Self::Error> {
        match value.decode() {
            vm::Data::Real(i) => Ok(vm::to_bool(i)),
            _ => Err(TryFromValueError),
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
        assert!(matches!(value.borrow().decode(), vm::Data::Real(x) if x == 0.0));

        let value = vm::Value::from(3.5);
        assert!(matches!(value.borrow().decode(), vm::Data::Real(x) if x == 3.5));

        let value = vm::Value::from(f64::INFINITY);
        assert!(matches!(value.borrow().decode(), vm::Data::Real(x) if x == f64::INFINITY));

        let value = vm::Value::from(f64::NEG_INFINITY);
        assert!(matches!(value.borrow().decode(), vm::Data::Real(x) if x == f64::NEG_INFINITY));

        let value = vm::Value::from(f64::NAN);
        assert!(matches!(value.borrow().decode(), vm::Data::Real(x) if f64::is_nan(x)));
    }

    #[test]
    fn strings() {
        let value = vm::Value::from(Symbol::intern("true"));
        assert!(matches!(value.borrow().decode(), vm::Data::String(keyword::True)));

        let value = vm::Value::from(Symbol::intern("argument0"));
        assert!(matches!(
            value.borrow().decode(),
            vm::Data::String(x) if x == Symbol::from_argument(0)
        ));

        let symbol = Symbol::intern("foo");
        let value = vm::Value::from(symbol);
        assert!(matches!(value.borrow().decode(), vm::Data::String(x) if x == symbol));
    }
}
