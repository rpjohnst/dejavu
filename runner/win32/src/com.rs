use std::{ptr, ops::Deref, marker::PhantomData, fmt};
use winapi::shared::winerror::{HRESULT, FAILED};
use winapi::um::unknwnbase::IUnknown;

/// A strong reference to a COM object.
#[repr(transparent)]
pub struct Com<T> {
    ptr: ptr::NonNull<IUnknown>,
    _marker: PhantomData<T>,
}

impl<T> Com<T> {
    /// Construct a `Com` from a raw pointer.
    ///
    /// Takes ownership of the object; `AddRef` should already have been called.
    /// `T` must implement `IUnknown`.
    pub unsafe fn from_raw(ptr: *mut T) -> Com<T> where T: winapi::Interface {
        let ptr = ptr::NonNull::new_unchecked(ptr);

        Com {
            ptr: ptr.cast(),
            _marker: PhantomData,
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr() as *mut T
    }

    pub fn query_interface<U>(&self) -> Result<Com<U>, HRESULT> where U: winapi::Interface {
        
            let mut ptr = ptr::null_mut();
            match unsafe {(*self.ptr.as_ptr()).QueryInterface(&U::uuidof(), &mut ptr)} {
                hr if FAILED(hr) => Err(hr),
                _ => Ok(unsafe {Com::from_raw(ptr as *mut U)})
            }
        
    }
}

impl<T> Deref for Com<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.as_ptr() }
    }
}

impl<T> Clone for Com<T> where T: winapi::Interface {
    fn clone(&self) -> Self {
        unsafe {
            (*self.ptr.as_ptr()).AddRef();
        }
            Com {
                ptr: self.ptr,
                _marker: PhantomData,
            }
    }
}

impl<T> Drop for Com<T> {
    fn drop(&mut self) {
        unsafe {
            (*self.ptr.as_ptr()).Release();
        }
    }
}

impl<T> fmt::Debug for Com<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.ptr)
    }
}

impl<T> PartialEq<Com<T>> for Com<T> where T: winapi::Interface {
    fn eq(&self, other: &Com<T>) -> bool {
        self.ptr == other.ptr
    }
}

impl<T> Eq for Com<T> where T: winapi::Interface {}
