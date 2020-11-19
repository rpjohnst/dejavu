use std::{ops, cmp, iter, mem, ptr, slice};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::rc::Rc;

/// A copy-on-write `Vec`.
///
/// Invoking `clone` on an `RcVec` produces a new `RcVec` using the same array.
///
/// Attempts to mutate an `RcVec` will first ensure it is unique, invoking `clone` on its elements
/// if there are other `RcVec` objects sharing its array.
pub struct RcVec<T> {
    buf: Rc<[MaybeUninit<T>]>,
    len: usize,

    _marker: PhantomData<[T]>,
}

impl<T> Default for RcVec<T> {
    /// Create a new, empty, unshared `RcVec`.
    fn default() -> Self {
        RcVec { buf: Rc::new([]), len: 0, _marker: PhantomData }
    }
}

impl<T> Clone for RcVec<T> {
    /// Create a new `RcVec` with the same array, increasing its reference count.
    fn clone(&self) -> RcVec<T> {
        RcVec { buf: Rc::clone(&self.buf), len: self.len, _marker: PhantomData }
    }
}

impl<T> Drop for RcVec<T> {
    fn drop(&mut self) {
        // If this is the last reference to the array, drop its elements.
        if Rc::strong_count(&self.buf) == 1 {
            unsafe {
                let buf = slice::from_raw_parts_mut(self.as_mut_ptr(), self.len);
                ptr::drop_in_place(buf);
            }
        }
    }
}

impl<T> ops::Deref for RcVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        // Safety: `self.len` represents the number of initialized elements.
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
    }
}

impl<T> RcVec<T> {
    /// Construct a new `RcVec` with elements initialized to those of `iter` and capacity `cap`.
    ///
    /// If `cap` is less than the number of elements yielded by `iter`, the rest are discarded.
    pub fn from_iter_with_capacity<I>(iter: I, cap: usize) -> Self where
        I: IntoIterator<Item = T>
    {
        let mut len = 0;
        let buf = iter.into_iter()
            .inspect(|_| len += 1)
            .map(MaybeUninit::new)
            .chain(iter::repeat_with(MaybeUninit::uninit))
            .take(cap)
            .collect();

        // Safety: The soundness of `Deref` depends on the value of `len` here.
        RcVec { buf, len, _marker: PhantomData }
    }

    pub fn len(&self) -> usize { self.len }

    pub fn as_ptr(&self) -> *const T {
        // Safety: This immediately "forgets" the duplicate `Rc`.
        let ptr = unsafe { Rc::into_raw(ptr::read(&self.buf)) };
        ptr as *const T
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        // Safety: This immediately "forgets" the duplicate `Rc`.
        let ptr = unsafe { Rc::into_raw(ptr::read(&self.buf)) };
        ptr as *mut T
    }
}

impl<T> RcVec<T> where T: Clone {
    /// Reserve capacity for at least `extra` more elements to be inserted.
    ///
    /// If the array must be reallocated and there are other `RcVec`s using the same allocation,
    /// existing elements will be cloned.
    pub fn reserve(&mut self, extra: usize) {
        if self.buf.len().wrapping_sub(self.len) >= extra {
            return;
        }

        // Mimic `Vec`'s behavior: limiting `size` to `isize::MAX` ensures that subsequent
        // evaluations of `2 * self.buf.len()` cannot overflow.
        let buf_len = self.len.checked_add(extra).expect("capacity overflow");
        let buf_len = cmp::max(buf_len, 2 * self.buf.len());
        let size = buf_len.checked_mul(mem::size_of::<T>()).expect("capacity overflow");
        if mem::size_of::<usize>() < 8 && size > isize::MAX as usize {
            panic!("capacity overflow");
        }

        // If this is the only reference to the array, move its elements. Otherwise, clone them.
        if Rc::strong_count(&self.buf) == 1 {
            // Safety: The moved from values are immediately freed without being dropped.
            let iter = self.buf.iter().take(self.len).map(|value| unsafe { value.assume_init_read() });
            *self = Self::from_iter_with_capacity(iter, buf_len);
        } else {
            *self = Self::from_iter_with_capacity(self.iter().cloned(), buf_len);
        }
    }

    /// Make a mutable reference into the array.
    ///
    /// If there are other `RcVec`s sharing the array, `clone` the elements to ensure uniqueness.
    pub fn make_mut(&mut self) -> &mut [T] {
        // If this is not the only reference to the array, clone it.
        if Rc::strong_count(&self.buf) != 1 {
            *self = Self::from_iter_with_capacity(self.iter().cloned(), self.buf.len());
        }

        // Safety: The strong count of `self.buf` is now 1.
        // `self.len` represents the number of initialized elements.
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    /// Append `value` to the end of the array.
    pub fn push(&mut self, value: T) {
        if self.len == self.buf.len() {
            self.reserve(1);
        }

        // Safety: If there was excess capacity, then no references have been formed to that memory.
        // Otherwise, the strong count of `self.buf` is now 1 and there are no references at all.
        unsafe {
            let end = self.as_mut_ptr().add(self.len);
            ptr::write(end, value);
            self.len += 1;
        }
    }

    /// Remove and return the element at position `index`, shifting later elements down.
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.len();
        assert!(index < len);

        let ret;
        self.make_mut();

        // Safety: The strong count of `self.buf` is now 1, and `index` is in bounds.
        unsafe {
            let ptr = self.as_mut_ptr().add(index);
            ret = ptr::read(ptr);
            ptr::copy(ptr.offset(1), ptr, len - index - 1);
        }

        self.len -= 1;
        ret
    }
}
