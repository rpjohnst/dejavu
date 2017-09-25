use std::slice;

pub fn ref_slice<T>(s: &T) -> &[T] {
    unsafe { slice::from_raw_parts(s, 1) }
}

pub fn ref_slice_mut<T>(s: &mut T) -> &mut [T] {
    unsafe { slice::from_raw_parts_mut(s, 1) }
}
