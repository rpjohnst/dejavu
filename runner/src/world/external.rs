use std::{error, fmt, iter};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use gml::symbol::Symbol;
use gml::vm;
use gml::vm::dll::{Cc, Type};
use crate::platform;

#[derive(Default)]
pub struct State {
    libraries: HashMap<Symbol, platform::Library>,
    functions: Vec<(vm::Proc, vm::Thunk)>,
}

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
            return Err(vm::Error::arity(5 + argnumb as usize));
        }
        let mut types = [Type::Real; 16];
        for (ty, argtype) in iter::zip(&mut types[..], argtypes) {
            *ty = match argtype.borrow().try_into() {
                Ok(0) => { Type::Real }
                Ok(1) => { Type::String }
                _ => { return Err(Error::Type)?; }
            }
        }

        let thunk = vm::dll::thunk(calltype, restype, &types[..argtypes.len()])?;

        let len = self.functions.len() as u32;
        self.functions.push((proc, thunk));

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
