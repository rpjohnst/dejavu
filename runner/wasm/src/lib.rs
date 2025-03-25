#![cfg(target_arch = "wasm32")]

extern crate self as wasm;

use core::mem;
use bstr::BStr;

pub use wasm_meta::Reflect;

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct JsValue(usize);

impl Default for JsValue {
    fn default() -> JsValue { JsValue::UNDEFINED }
}

impl JsValue {
    pub const UNDEFINED: JsValue = JsValue(0);
}

pub trait Reflect {
    const LAYOUT: Layout;
}

#[repr(u8)]
pub enum Layout {
    Bool,
    Integer { signed: bool, size: usize },
    Float { size: usize },
    Array { item: &'static Layout, stride: usize, len: usize },
    Struct { fields: &'static [Field] },
    BStr,
    Slice {
        item: &'static Layout,
        stride: usize,
        ptr: unsafe extern "C" fn(*const ()) -> *const (),
        len: unsafe extern "C" fn(*const ()) -> usize,
        store: unsafe extern "C" fn(*mut (), *const (), usize),
    },
    Vec {
        item: &'static Layout,
        stride: usize,
        ptr: unsafe extern "C" fn(*const ()) -> *const (),
        len: unsafe extern "C" fn(*const ()) -> usize,
        resize: unsafe extern "C" fn(*mut (), usize) -> *mut (),
    },
}

#[repr(C)]
pub struct Field {
    pub name: &'static [u8],
    pub offset: usize,
    pub layout: &'static Layout
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fields_ptr(fields: *const &[Field]) -> *const Field {
    unsafe { (*fields).as_ptr() }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fields_len(fields: *const &[Field]) -> usize {
    unsafe { (*fields).len() }
}

impl Reflect for bool {
    const LAYOUT: Layout = Layout::Bool;
}
impl Reflect for u8 {
    const LAYOUT: Layout = Layout::Integer { signed: false, size: mem::size_of::<u8>() };
}
impl Reflect for i32 {
    const LAYOUT: Layout = Layout::Integer { signed: true, size: mem::size_of::<i32>() };
}
impl Reflect for u32 {
    const LAYOUT: Layout = Layout::Integer { signed: false, size: mem::size_of::<u32>() };
}
impl Reflect for usize {
    const LAYOUT: Layout = Layout::Integer { signed: false, size: mem::size_of::<usize>() };
}
impl Reflect for f64 {
    const LAYOUT: Layout = Layout::Float { size: mem::size_of::<f64>() };
}

impl<'a> Reflect for &'a BStr {
    const LAYOUT: Layout = Layout::BStr;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bstr_ptr(ptr: *const &BStr) -> *const () {
    unsafe { (*ptr).as_ptr() as *const () }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bstr_len(ptr: *const &BStr) -> usize {
    unsafe { (*ptr).len() }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bstr_store(ptr: *mut &BStr, data: *const (), len: usize) {
    let ptr = ptr as *mut &BStr;
    unsafe { *ptr = BStr::new(std::slice::from_raw_parts(data as _, len)); }
}

impl<T: Reflect, const N: usize> Reflect for [T; N] {
    const LAYOUT: Layout = Layout::Array {
        item: &T::LAYOUT,
        stride: core::mem::size_of::<T>(),
        len: N,
    };
}

impl<'a, T: Reflect + Default> Reflect for &'a [T] {
    const LAYOUT: Layout = Layout::Slice {
        item: &T::LAYOUT,
        stride: core::mem::size_of::<T>(),
        ptr: slice_ptr::<T>,
        len: slice_len::<T>,
        store: slice_store::<T>,
    };
}
unsafe extern "C" fn slice_ptr<T>(ptr: *const ()) -> *const () {
    let ptr = ptr as *const &[T];
    unsafe { (*ptr).as_ptr() as *const () }
}
unsafe extern "C" fn slice_len<T>(ptr: *const ()) -> usize {
    let ptr = ptr as *const &[T];
    unsafe { (*ptr).len() }
}
unsafe extern "C" fn slice_store<T: Default>(ptr: *mut (), data: *const (), len: usize) {
    let ptr = ptr as *mut &[T];
    unsafe { *ptr = std::slice::from_raw_parts(data as _, len); }
}

impl<T: Reflect + Default> Reflect for Vec<T> {
    const LAYOUT: Layout = Layout::Vec {
        item: &T::LAYOUT,
        stride: core::mem::size_of::<T>(),
        ptr: vec_ptr::<T>,
        len: vec_len::<T>,
        resize: vec_resize::<T>,
    };
}
unsafe extern "C" fn vec_ptr<T>(ptr: *const ()) -> *const () {
    let ptr = ptr as *const Vec<T>;
    unsafe { (*ptr).as_ptr() as *const () }
}
unsafe extern "C" fn vec_len<T>(ptr: *const ()) -> usize {
    let ptr = ptr as *const Vec<T>;
    unsafe { (*ptr).len() }
}
unsafe extern "C" fn vec_resize<T: Default>(ptr: *mut (), len: usize) -> *mut () {
    let ptr = ptr as *mut Vec<T>;
    unsafe {
        (*ptr).resize_with(len, T::default);
        vec_ptr::<T>(ptr as _) as _
    }
}

impl<T1: Reflect, T2: Reflect> Reflect for (T1, T2) {
    const LAYOUT: Layout = Layout::Struct { fields: &[
        Field { name: b"0", offset: core::mem::offset_of!((T1, T2), 0), layout: &T1::LAYOUT },
        Field { name: b"1", offset: core::mem::offset_of!((T1, T2), 1), layout: &T1::LAYOUT },
    ] };
}
