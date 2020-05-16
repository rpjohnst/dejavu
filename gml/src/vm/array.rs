use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use crate::vm;

/// A GML array value.
///
/// This implementation is shared across all versions of GML, so it has to deal with a few
/// conflicting requirements. The baseline on top of which all other array behaviors are
/// implemented is a 1D, ref-counted, mutable vector of `vm::Value`s.
///
/// On top of this, earlier GML versions can implement nested "jagged" arrays and various flavors
/// of copy-on-write behavior.
#[derive(Clone, Default)]
#[repr(transparent)]
pub struct Array { data: Rc<Data> }

/// A borrowed reference to a GML array value.
///
/// This type is guaranteed to point to an `array::Data` managed by a `vm::Array`, which lets
/// clients manipulate arrays with less ref-counting traffic while retaining the ability to
/// "upgrade" to a full `vm::Array` value.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct ArrayRef<'a> { data: &'a Data }

/// The on-heap portion of a GML array.
pub type Data = UnsafeCell<Vec<vm::Value>>;

impl Array {
    /// Construct a jagged array with `value` at `[0, 0]`.
    pub fn from_scalar(value: vm::Value) -> Array {
        let row = vm::Value::from(Array { data: Rc::new(UnsafeCell::new(vec![value])) });
        Array { data: Rc::new(UnsafeCell::new(vec![row])) }
    }

    /// Convert a `&Array` into an `ArrayRef`.
    pub fn borrow(&self) -> ArrayRef<'_> { ArrayRef { data: &self.data } }

    pub fn into_raw(self) -> *const Data { Rc::into_raw(self.data) }

    pub unsafe fn from_raw(ptr: *const Data) -> Array { Array { data: Rc::from_raw(ptr) } }
}

impl<'a> ArrayRef<'a> {
    /// Read an element from a jagged array.
    pub fn get_jagged(self, i: i32, j: i32) -> Option<vm::Value> {
        // Safety: `value` does not alias `self`. In the presence of cycles, `row` may, but both
        // are shared references to `*self.data`.
        unsafe {
            let value = &*self.get_raw(i)?;
            let row = match value.borrow().decode() { vm::Data::Array(r) => r, _ => return None };
            row.get_flat(j)
        }
    }

    /// Read an element from a 1D array.
    pub fn get_flat(self, j: i32) -> Option<vm::Value> {
        // Safety: `value` does not alias `self`. In the presence of cycles, cloning its referent
        // may create a reference that does, but both are shared references to `*self.data.`
        unsafe {
            let value = &*self.get_raw(j)?;
            Some(value.clone())
        }
    }

    /// Construct a pointer to an element in a 1D array.
    pub(in crate::vm) fn get_raw(self, j: i32) -> Option<*const vm::Value> {
        // Safety: Shared references into `*self.data` are discarded before `self` is usable again.
        unsafe {
            let vec = &*self.data.get();
            Some(vec.get(j as usize)?)
        }
    }

    /// Write an element in a jagged array, growing it if necessary.
    ///
    /// Initialize new elements of the outer array with empty arrays, and new elements of the inner
    /// array with `0.0`.
    pub fn set_jagged(self, i: i32, j: i32, val: vm::Value) -> Option<()> {
        // Safety: `value` does not alias `self`. In the presence of cycles, `row` may, but both
        // are shared references to `*self.data`.
        //
        // Calling `set_flat` invalidates `value`, but `value` is no longer live. It would seem
        // that it could also free `row`'s referent, because `row` only borrows from `value`, but
        // in that case `self` keeps `row` alive precisely because they do alias.
        unsafe {
            let value = &*self.set_raw_outer(i)?;
            let row = match value.borrow().decode() { vm::Data::Array(r) => r, _ => return None };
            row.set_flat(j, val)
        }
    }

    /// Construct a pointer to a row in a jagged array, growing the outer array if necessary.
    pub(in crate::vm) fn set_raw_outer(self, j: i32) -> Option<*mut vm::Value> {
        unsafe { self.set_raw_with(j, || vm::Value::from(Array::default())) }
    }

    /// Write an element in a 1D array, growing it if necessary.
    ///
    /// Initialize new elements with `0.0`.
    pub fn set_flat(self, j: i32, val: vm::Value) -> Option<()> {
        // Safety: `value` does not alias `self`. In the presence of cycles, dropping its referent
        // may create a reference that does, but both are shared references to `*self.data`.
        //
        // That drop may decrement the refcount of `*self.data`, but `self` cannot borrow from the
        // same overwritten element.
        unsafe {
            let value = &mut *self.set_raw_with(j, vm::Value::default)?;
            Some(*value = val)
        }
    }

    /// Construct a pointer to an element to an array, initializing new elements with `f()`.
    ///
    /// Safety: `f` must not access the interior of `*self.data`.
    unsafe fn set_raw_with<F>(self, j: i32, f: F) -> Option<*mut vm::Value> where
        F: FnMut() -> vm::Value
    {
        let j = if j < 0 { return None } else { j as usize };
        // Safety: Unique references into `*self.data` are discarded before `self` is usable again.
        // The call to `resize_with` ensures that `get_unchecked_mut(j)` is in-bounds.
        #[allow(unused_unsafe)] unsafe {
            let vec = &mut *self.data.get();
            if j >= vec.len() { vec.resize_with(j + 1, f) }
            Some(vec.get_unchecked_mut(j))
        }
    }

    /// Convert this borrowed array into an owned array.
    pub fn clone(self) -> Array {
        // Safety: `self.data` is a reference obtained from `Rc::into_raw`,
        // and `ManuallyDrop` prevents the extra drop.
        let data = unsafe { ManuallyDrop::new(Rc::from_raw(self.data)) };
        Array { data: Rc::clone(&data) }
    }

    pub fn as_raw(self) -> *const Data { self.data }

    pub unsafe fn from_raw(ptr: *const Data) -> ArrayRef<'a> { ArrayRef { data: &*ptr } }
}

#[cfg(test)]
mod tests {
    use crate::vm;

    #[test]
    fn jagged() {
        let array = vm::Array::from_scalar(vm::Value::from(3.0));
        let a = array.borrow();

        assert_eq!(a.get_jagged(-1, 0), None);
        assert_eq!(a.get_jagged(0, -1), None);
        assert_eq!(a.get_jagged(0, 0), Some(vm::Value::from(3.0)));
        assert_eq!(a.get_jagged(0, 1), None);
        assert_eq!(a.get_jagged(1, 0), None);

        assert_eq!(a.set_jagged(0, 3, vm::Value::from(5.0)), Some(()));
        assert_eq!(a.get_jagged(-1, 0), None);
        assert_eq!(a.get_jagged(0, -1), None);
        assert_eq!(a.get_jagged(0, 0), Some(vm::Value::from(3.0)));
        assert_eq!(a.get_jagged(0, 1), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(0, 2), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(0, 3), Some(vm::Value::from(5.0)));
        assert_eq!(a.get_jagged(0, 4), None);
        assert_eq!(a.get_jagged(1, 0), None);

        assert_eq!(a.set_jagged(3, 5, vm::Value::from(8.0)), Some(()));
        assert_eq!(a.get_jagged(-1, 0), None);
        assert_eq!(a.get_jagged(0, -1), None);
        assert_eq!(a.get_jagged(0, 0), Some(vm::Value::from(3.0)));
        assert_eq!(a.get_jagged(0, 1), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(0, 2), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(0, 3), Some(vm::Value::from(5.0)));
        assert_eq!(a.get_jagged(0, 4), None);
        assert_eq!(a.get_jagged(1, 0), None);
        assert_eq!(a.get_jagged(2, 0), None);
        assert_eq!(a.get_jagged(3, -1), None);
        assert_eq!(a.get_jagged(3, 0), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(3, 1), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(3, 2), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(3, 3), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(3, 4), Some(vm::Value::from(0.0)));
        assert_eq!(a.get_jagged(3, 5), Some(vm::Value::from(8.0)));
        assert_eq!(a.get_jagged(3, 6), None);
        assert_eq!(a.get_jagged(4, 0), None);
    }
}
