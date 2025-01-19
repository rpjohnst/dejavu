use std::{mem, error, fmt};
use std::ffi::{CStr, c_char};
use crate::symbol::Symbol;
use crate::vm;

pub type Proc = *mut ();
pub type Thunk = unsafe fn(Proc, &[vm::Value]) -> vm::Result<vm::Value>;

pub enum Cc { Cdecl, Stdcall }

#[derive(Copy, Clone)]
pub enum Type { Real, String }

#[derive(Debug)]
pub enum Error {
    Arity,
    Type,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Arity => write!(f, "too many parameters")?,
            Error::Type => write!(f, "all parameters must be of type real")?,
        }
        Ok(())
    }
}

impl error::Error for Error {}

pub fn thunk(calltype: Cc, restype: Type, argtypes: &[Type]) -> Result<Thunk, Error> {
    if argtypes.len() > 16 {
        return Err(Error::Arity);
    }
    let argnumb = argtypes.len() as u32;
    let argtypes = match argnumb {
        0..=4 => {
            // Number of signatures of length less than `argnumb`:
            // 2^0 + 2^1 + ... + 2^(argnumb-1) + 1 = 2^n
            let base = (1 << argnumb) - 1;

            let mut index = 0;
            for &argtype in argtypes {
                index = index * 2 + argtype as u32;
            }

            base + index
        }
        5..=16 => {
            let base = (1 << 5) - 1;

            for &argtype in argtypes {
                match argtype {
                    Type::Real => {}
                    _ => { return Err(Error::Type); }
                };
            }

            base + argnumb - 5
        }
        _ => unreachable!()
    };

    let signature = ((calltype as u32 * 2) + restype as u32) * 28 + argtypes;
    Ok(THUNKS[signature as usize])
}

macro_rules! generate_thunks { [$(($($t:ty),*))*] => { [
    $(<unsafe extern "C" fn($($t),*) -> f64 as FnExtern>::call,)*
    $(<unsafe extern "C" fn($($t),*) -> *const c_char as FnExtern>::call,)*
    $(#[cfg(windows)] <unsafe extern "stdcall" fn($($t),*) -> f64 as FnExtern>::call,)*
    $(#[cfg(windows)] <unsafe extern "stdcall" fn($($t),*) -> *const c_char as FnExtern>::call,)*
] } }
#[cfg(not(windows))]
const NUM_THUNKS: usize = 86;
#[cfg(windows)]
const NUM_THUNKS: usize = 86 + 86;
static THUNKS: [Thunk; NUM_THUNKS] = generate_thunks![
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

trait FnExtern {
    unsafe fn call(proc: Proc, args: &[vm::Value]) -> vm::Result<vm::Value>;
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

macro_rules! impl_fn_extern { ($cc:literal $($p:ident)*) => {
    impl<$($p,)* B> FnExtern for unsafe extern $cc fn($($p),*) -> B where
        $($p: Param,)*
        B: Param,
    {
        #[allow(nonstandard_style)]
        unsafe fn call(proc: Proc, args: &[vm::Value]) -> vm::Result<vm::Value> {
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

#[cfg(windows)]
impl_fn_extern_params!("stdcall");
