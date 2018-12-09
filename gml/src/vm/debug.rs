use std::fmt;
use std::collections::HashSet;
use std::cell::Cell;

use crate::vm;

pub(in crate::vm) struct Value<'b> {
    pub(in crate::vm) value: vm::Value,
    pub(in crate::vm) visited: &'b Cell<HashSet<usize>>,
}

impl<'b> fmt::Debug for Value<'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Value { value, visited } = *self;
        match value.data() {
            vm::Data::Real(value) => write!(f, "{:?}", value),
            vm::Data::String(value) => write!(f, "{}", value),
            vm::Data::Array(value) => write!(f, "{:?}", Array(&value, visited)),
        }
    }
}

struct Array<'a, 'b>(&'a vm::Array, &'b Cell<HashSet<usize>>);

impl<'a, 'b> fmt::Debug for Array<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Array(array, visited) = *self;
        let raw = &*array.data as *const _ as usize;
        if begin(visited, raw) {
            let array = unsafe { &*array.data.get() };
            f.debug_set().entries(array.iter().map(|row| Row(row, visited))).finish()?;
        } else {
            write!(f, "{{...}}")?;
        }
        end(visited, raw);

        fn begin(visited: &Cell<HashSet<usize>>, address: usize) -> bool {
            let mut v = visited.take();
            let i = v.insert(address);
            visited.set(v);
            i
        }

        fn end(visited: &Cell<HashSet<usize>>, address: usize) {
            let mut v = visited.take();
            v.remove(&address);
            visited.set(v);
        }

        Ok(())
    }
}

struct Row<'a, 'b>(&'a Vec<vm::Value>, &'b Cell<HashSet<usize>>);

impl<'a, 'b> fmt::Debug for Row<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Row(row, visited) = *self;
        f.debug_set().entries(row.iter().map(|&value| Value { value, visited })).finish()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::vm;

    #[test]
    fn recursive() {
        let array = vm::Array::from_scalar(vm::Value::from(1.0));
        let raw = array.into_raw();

        let array = unsafe { vm::Array::clone_from_raw(raw) };
        array.store(0, 1, vm::Value::from(unsafe { vm::Array::clone_from_raw(raw) }));
        array.store(0, 2, vm::Value::from(3.0));

        let array = vm::Value::from(unsafe { vm::Array::from_raw(raw) });
        assert_eq!(format!("{:?}", array), "{{1.0, {...}, 3.0}}");
    }
}
