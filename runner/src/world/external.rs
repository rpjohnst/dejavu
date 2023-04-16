use std::{error, fmt, mem};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::{CStr, c_char};
use gml::symbol::Symbol;
use gml::vm;
use crate::platform;

#[derive(Default)]
pub struct State {
    libraries: HashMap<Symbol, platform::Library>,
    functions: Vec<(platform::Proc, Thunk)>,
}

type Thunk = unsafe fn(platform::Proc, &[vm::Value]) -> vm::Result<vm::Value>;

enum Cc { Cdecl, Stdcall }

enum Type { Real, String }

#[derive(Debug)]
pub enum Error {
    Load,
    Symbol,
    CallingConvention,
    Type,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Load => write!(f, "could not load external library")?,
            Error::Symbol => write!(f, "could find symbol in library")?,
            Error::CallingConvention => write!(f, "unknown calling convention")?,
            Error::Type => write!(f, "unknown type")?,
        }
        Ok(())
    }
}

impl error::Error for Error {}

#[gml::bind]
impl State {
    #[gml::get(dll_cdecl)]
    pub fn get_dll_cdecl() -> u32 { Cc::Cdecl as u32 }
    #[gml::get(dll_stdcall)]
    pub fn get_dll_stdcall() -> u32 { Cc::Stdcall as u32 }

    #[gml::get(ty_real)]
    pub fn get_ty_real() -> u32 { Type::Real as u32 }
    #[gml::get(ty_string)]
    pub fn get_ty_string() -> u32 { Type::String as u32 }

    #[gml::api]
    pub fn external_define(
        &mut self, dll: Symbol, name: Symbol, calltype: u32, restype: u32, argnumb: u32,
        argtypes: &[vm::Value]
    ) -> vm::Result<u32> {
        let dll = match self.libraries.entry(dll) {
            Entry::Occupied(entry) => { entry.into_mut() }
            Entry::Vacant(entry) => {
                let dll = platform::Library::load(dll).ok_or(Error::Load)?;
                entry.insert(dll)
            }
        };
        let proc = dll.symbol(name.as_cstr()).ok_or(Error::Symbol)?;

        let calltype = match calltype {
            0 => { Cc::Cdecl }
            1 => { Cc::Stdcall }
            _ => { return Err(Error::CallingConvention)?; }
        };
        let restype = match restype {
            0 => { Type::Real }
            1 => { Type::String }
            _ => { return Err(Error::Type)?; }
        };

        if argnumb as usize != argtypes.len() || argnumb > 16 {
            return Err(vm::Error::arity(5 + argtypes.len()));
        }
        let argtypes = match argnumb {
            0..=4 => {
                // Number of signatures of length less than `argnumb`:
                // 2^0 + 2^1 + ... + 2^(argnumb-1) + 1 = 2^n
                let base = (1 << argnumb) - 1;

                let mut index = 0;
                for argtype in argtypes {
                    let argtype = match argtype.borrow().try_into() {
                        Ok(0) => { Type::Real }
                        Ok(1) => { Type::String }
                        _ => { return Err(Error::Type)?; }
                    };
                    index = index * 2 + argtype as u32;
                }

                base + index
            }
            5..=16 => {
                let base = (1 << 5) - 1;

                for argtype in argtypes {
                    let _argtype = match argtype.borrow().try_into() {
                        Ok(0) => { Type::Real }
                        _ => { return Err(Error::Type)?; }
                    };
                }

                base + argnumb - 5
            }
            _ => unreachable!()
        };

        let signature = ((calltype as u32 * 2) + restype as u32) * 28 + argtypes;

        let len = self.functions.len() as u32;
        self.functions.push((proc, THUNKS[signature as usize]));

        Ok(len)
    }

    #[gml::api]
    pub fn external_call(&mut self, id: u32, args: &[vm::Value]) -> vm::Result<vm::Value> {
        let (proc, thunk) = self.functions[id as usize];
        unsafe { thunk(proc, args) }
    }

    #[gml::api]
    pub fn external_free(&mut self, dll: Symbol) {
        self.libraries.remove(&dll);
    }
}

trait FnExtern {
    unsafe fn call(proc: platform::Proc, args: &[vm::Value]) -> vm::Result<vm::Value>;
}

unsafe trait Param {
    fn from(value: vm::ValueRef<'_>) -> Self;
    fn into(self) -> vm::Value;
}

unsafe impl Param for f64 {
    fn from(value: vm::ValueRef<'_>) -> Self {
        <f64 as TryFrom<_>>::try_from(value).unwrap_or_default()
    }

    fn into(self) -> vm::Value { vm::Value::from(self) }
}

unsafe impl Param for *const c_char {
    fn from(value: vm::ValueRef<'_>) -> Self {
        match value.decode() {
            vm::Data::String(s) => s.as_cstr(),
            _ => [0].as_ptr(),
        }
    }

    fn into(self) -> vm::Value {
        let str = unsafe { CStr::from_ptr(self) };
        vm::Value::from(Symbol::intern(str.to_bytes()))
    }
}

macro_rules! generate_thunks { [$(($($t:ty),*))*] => { [
    $(<unsafe extern "C" fn($($t),*) -> f64 as FnExtern>::call,)*
    $(<unsafe extern "C" fn($($t),*) -> *const c_char as FnExtern>::call,)*
    $(<unsafe extern "stdcall" fn($($t),*) -> f64 as FnExtern>::call,)*
    $(<unsafe extern "stdcall" fn($($t),*) -> *const c_char as FnExtern>::call,)*
] } }
static THUNKS: [Thunk; 172] = generate_thunks![
    ()

    (f64)
    (*const c_char)

    (f64, f64)
    (f64, *const c_char)
    (*const c_char, f64)
    (*const c_char, *const c_char)

    (f64, f64, f64)
    (f64, f64, *const c_char)
    (f64, *const c_char, f64)
    (f64, *const c_char, *const c_char)
    (*const c_char, f64, f64)
    (*const c_char, f64, *const c_char)
    (*const c_char, *const c_char, f64)
    (*const c_char, *const c_char, *const c_char)

    (f64, f64, f64, f64)
    (f64, f64, f64, *const c_char)
    (f64, f64, *const c_char, f64)
    (f64, f64, *const c_char, *const c_char)
    (f64, *const c_char, f64, f64)
    (f64, *const c_char, f64, *const c_char)
    (f64, *const c_char, *const c_char, f64)
    (f64, *const c_char, *const c_char, *const c_char)
    (*const c_char, f64, f64, f64)
    (*const c_char, f64, f64, *const c_char)
    (*const c_char, f64, *const c_char, f64)
    (*const c_char, f64, *const c_char, *const c_char)
    (*const c_char, *const c_char, f64, f64)
    (*const c_char, *const c_char, f64, *const c_char)
    (*const c_char, *const c_char, *const c_char, f64)
    (*const c_char, *const c_char, *const c_char, *const c_char)

    (f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
    (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64)
];

macro_rules! impl_fn_extern { ($cc:literal $($p:ident)*) => {
    impl<$($p,)* B> FnExtern for unsafe extern $cc fn($($p),*) -> B where
        $($p: Param,)*
        B: Param,
    {
        #[allow(nonstandard_style)]
        unsafe fn call(proc: platform::Proc, args: &[vm::Value]) -> vm::Result<vm::Value> {
            let ($($p,)*) = match *args {
                [$(ref $p,)*] => ($($p::from($p.borrow()),)*),
                _ => return Err(vm::Error::arity(args.len())),
            };
            let proc: Self = mem::transmute(proc);
            Ok(proc($($p,)*).into())
        }
    }
} }

macro_rules! impl_fn_extern_params { ($cc:literal) => {
    impl_fn_extern!($cc);
    impl_fn_extern!($cc P0);
    impl_fn_extern!($cc P0 P1);
    impl_fn_extern!($cc P0 P1 P2);
    impl_fn_extern!($cc P0 P1 P2 P3);
    impl_fn_extern!($cc P0 P1 P2 P3 P4);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13 P14);
    impl_fn_extern!($cc P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13 P14 P15);
} }

impl_fn_extern_params!("C");
impl_fn_extern_params!("stdcall");
