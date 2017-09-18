use std::slice;

pub fn ref_slice<T>(s: &T) -> &[T] {
    unsafe { slice::from_raw_parts(s, 1) }
}
