#![cfg(windows)]

pub use com::Com;

mod com;

use std::{ptr, ops::Deref, slice, fmt, error::Error, ffi::OsString};
use std::os::windows::ffi::OsStringExt;
use winapi::shared::minwindef::DWORD;
use winapi::shared::winerror::HRESULT;
use winapi::um::winbase::FormatMessageW;
use winapi::um::winbase::{FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM};

#[derive(Copy, Clone)]
pub struct HResult(pub HRESULT);

impl Error for HResult {}

impl fmt::Debug for HResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let HResult(hr) = *self;
        write!(f, "{:#010x}", hr as u32)
    }
}

impl fmt::Display for HResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            let HResult(hr) = *self;

            let mut ptr = ptr::null_mut();
            let len = FormatMessageW(
                FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM,
                ptr::null_mut(), hr as DWORD, 0, &mut ptr as *mut _ as *mut _, 0, ptr::null_mut()
            );
            let slice = LocalBox::from_raw(slice::from_raw_parts_mut(ptr, len as usize));

            write!(f, "{}", OsString::from_wide(&slice).to_string_lossy())
        }
    }
}

use winapi::um::winbase::LocalFree;

struct LocalBox<T: ?Sized> {
    ptr: ptr::NonNull<T>,
}

impl<T: ?Sized> LocalBox<T> {
    unsafe fn from_raw(ptr: *mut T) -> LocalBox<T> {
        let ptr = ptr::NonNull::new_unchecked(ptr);
        LocalBox { ptr }
    }

    fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

impl<T: ?Sized> Deref for LocalBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.as_ptr() }
    }
}

impl<T: ?Sized> Drop for LocalBox<T> {
    fn drop(&mut self) {
        unsafe { LocalFree(self.as_ptr() as *mut _); }
    }
}
