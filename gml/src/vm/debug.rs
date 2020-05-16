use std::fmt;
use std::collections::HashSet;
use std::cell::UnsafeCell;

use crate::vm;

pub(in crate::vm) struct Value<'a, 'b> {
    pub(in crate::vm) value: vm::ValueRef<'a>,
    pub(in crate::vm) visited: &'b UnsafeCell<HashSet<usize>>,
}

impl<'a, 'b> fmt::Debug for Value<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Value { value, visited } = *self;
        match value.decode() {
            vm::Data::Real(value) => write!(f, "{:?}", value),
            vm::Data::String(value) => write!(f, "{}", value),
            vm::Data::Array(array) => write!(f, "{:?}", Array { array, visited }),
        }
    }
}

struct Array<'a, 'b> {
    array: vm::ArrayRef<'a>,
    visited: &'b UnsafeCell<HashSet<usize>>,
}

impl<'a, 'b> fmt::Debug for Array<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Array { array, visited } = *self;
        let address = array.as_raw() as usize;

        // Safety: `HashSet::insert` cannot see the `UnsafeCell`.
        if unsafe {
            let visited = &mut *visited.get();
            !visited.insert(address)
        } {
            write!(f, "{{...}}")?;
            return Ok(());
        }

        // Safety: `ArrayRef::as_raw` is always non-null, and while this iterator creates immutable
        // references into the array's `UnsafeCell`, no other code can see it.
        unsafe {
            let data = &*array.as_raw();
            let vec = &*data.get();
            let entries = vec.iter().map(|value| Value { value: value.borrow(), visited });
            f.debug_set().entries(entries).finish()?;
        }

        // Safety: `HashSet::remove` cannot see the `UnsafeCell`.
        unsafe {
            let visited = &mut *visited.get();
            visited.remove(&address);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::vm;

    #[test]
    fn recursive() {
        let array = vm::Array::from_scalar(vm::Value::from(1.0));

        let a = array.borrow();
        a.set_jagged(0, 1, vm::Value::from(a.clone())).unwrap();
        a.set_jagged(0, 2, vm::Value::from(3.0)).unwrap();

        let array = vm::Value::from(array);
        assert_eq!(format!("{:?}", array), "{{1.0, {...}, 3.0}}");
    }
}
