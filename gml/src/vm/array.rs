use std::mem;
use std::rc::Rc;
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use crate::vm;

#[derive(Clone)]
pub struct Array {
    pub(in crate::vm) data: Rc<UnsafeCell<Vec<Vec<vm::Value>>>>,
}

#[derive(Copy, Clone)]
pub struct Row {
    pub(in crate::vm) data: NonNull<Vec<vm::Value>>,
}

#[derive(Copy, Clone, Debug)]
pub struct BoundsError;

impl Array {
    pub fn from_scalar(value: vm::Value) -> Array {
        Array { data: Rc::new(UnsafeCell::new(vec![vec![value]])) }
    }

    pub fn load(&self, i: i32, j: i32) -> Result<vm::Value, BoundsError> {
        let row = self.load_row(i)?;
        let value = unsafe { row.load(j)? };

        Ok(value)
    }

    pub fn store(&self, i: i32, j: i32, value: vm::Value) -> Result<(), BoundsError> {
        let row = self.store_row(i)?;
        unsafe { row.store(j, value)? };

        Ok(())
    }

    pub fn load_row(&self, i: i32) -> Result<Row, BoundsError> {
        let array = unsafe { &*self.data.get() };
        let row = array.get(i as usize).ok_or(BoundsError)?;

        unsafe { Ok(mem::transmute::<*const Vec<vm::Value>, Row>(row)) }
    }

    pub fn store_row(&self, i: i32) -> Result<Row, BoundsError> {
        let array = unsafe { &mut *self.data.get() };

        if i < 0 {
            return Err(BoundsError);
        }

        let i = i as usize;
        if i >= array.len() {
            array.resize(i + 1, vec![]);
        }
        let row = &mut array[i];

        unsafe { Ok(mem::transmute::<*mut Vec<vm::Value>, Row>(row)) }
    }

    pub fn into_raw(self) -> *const UnsafeCell<Vec<Vec<vm::Value>>> {
        Rc::into_raw(self.data)
    }

    pub fn as_ref(&self) -> &UnsafeCell<Vec<Vec<vm::Value>>> {
        &self.data
    }

    pub unsafe fn from_raw(ptr: *const UnsafeCell<Vec<Vec<vm::Value>>>) -> Array {
        let data = Rc::from_raw(ptr);
        Array { data }
    }

    pub unsafe fn clone_from_raw(ptr: *const UnsafeCell<Vec<Vec<vm::Value>>>) -> Array {
        let raw = Rc::from_raw(ptr);
        let data = raw.clone();
        Rc::into_raw(raw);
        Array { data }
    }
}

impl Row {
    pub unsafe fn load(&self, j: i32) -> Result<vm::Value, BoundsError> {
        let row = self.data.as_ref();
        let value = row.get(j as usize).ok_or(BoundsError)?;

        Ok(*value)
    }

    pub unsafe fn store(&self, j: i32, value: vm::Value) -> Result<(), BoundsError> {
        let row = &mut *self.data.as_ptr();

        if j < 0 {
            return Err(BoundsError);
        }

        let j = j as usize;
        if j >= row.len() {
            row.resize(j + 1, vm::Value::from(0.0));
        }
        row[j] = value;

        Ok(())
    }
}
