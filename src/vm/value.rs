use std::mem;

use symbol::Symbol;

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
