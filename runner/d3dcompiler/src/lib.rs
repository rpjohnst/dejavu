#![cfg(windows)]

use std::{ptr, slice, iter, fmt, env, ffi::CStr, path::Path};
use std::iter::FromIterator;
use std::error::Error;
use std::os::windows::ffi::OsStrExt;

use winapi::shared::minwindef::{BOOL, UINT};
use winapi::shared::winerror::FAILED;
use winapi::um::d3dcommon::ID3DBlob;
use winapi::um::d3dcompiler::*;

use win32::{HResult, Com};

pub struct Shader<'a> {
    pub source: &'a Path,
    pub target: &'a Path,

    pub entry: &'a CStr,
    pub model: &'a CStr,
}

pub fn compile(shaders: &[Shader]) -> Result<(), Box<dyn Error>> {
    let out_dir = env::var("OUT_DIR").unwrap();
    let opt_level = env::var("OPT_LEVEL").unwrap();
    let debug = env::var("DEBUG").unwrap();

    let mut flags =
        D3DCOMPILE_ENABLE_STRICTNESS |
        D3DCOMPILE_WARNINGS_ARE_ERRORS;
    match &opt_level[..] {
        "0" => flags |= D3DCOMPILE_OPTIMIZATION_LEVEL0,
        "z" | "s" | "1" => flags |= D3DCOMPILE_OPTIMIZATION_LEVEL1,
        "2" => flags |= D3DCOMPILE_OPTIMIZATION_LEVEL2,
        "3" => flags |= D3DCOMPILE_OPTIMIZATION_LEVEL3,
        _ => {}
    }
    if debug != "false" {
        flags |= D3DCOMPILE_DEBUG;
    }

    for &Shader { source, entry, model, target } in shaders {
        println!("cargo:rerun-if-changed={}", source.display());
        let shader = compile_from_file(source, entry, model, flags)
            .map_err(|(_hr, errors)| ErrorBlob { errors })?;
        write_blob_to_file(&shader, &Path::new(&out_dir).join(target), true)?;
    }

    Ok(())
}

fn compile_from_file(
    source: &Path, entry: &CStr, model: &CStr, flags: UINT
) -> Result<Com<ID3DBlob>, (HResult, Com<ID3DBlob>)> {
    let source = Vec::from_iter(Iterator::chain(source.as_os_str().encode_wide(), iter::once(0)));

    let mut shader = ptr::null_mut();
    let mut errors = ptr::null_mut();
    unsafe {
        match D3DCompileFromFile(
            source.as_ptr(), ptr::null_mut(), ptr::null_mut(), entry.as_ptr(),
            model.as_ptr(), flags, 0,
            &mut shader, &mut errors
        ) {
            hr if FAILED(hr) => Err((HResult(hr), Com::from_raw(errors))),
            _ => Ok(Com::from_raw(shader)),
        }
    }
}

fn write_blob_to_file(blob: &ID3DBlob, target: &Path, overwrite: bool) -> Result<(), HResult> {
    let target = Vec::from_iter(Iterator::chain(target.as_os_str().encode_wide(), iter::once(0)));

   
        match unsafe {D3DWriteBlobToFile(blob as *const _ as *mut _, target.as_ptr(), overwrite as BOOL)} {
            hr if FAILED(hr) => Err(HResult(hr)),
            _ => Ok(()),
        }
    
}

#[derive(Debug)]
struct ErrorBlob {
    errors: Com<ID3DBlob>,
}

fn blob_as_slice(blob: &ID3DBlob) -> &[u8] {
    unsafe {
        let ptr = blob.GetBufferPointer() as *const u8;
        let len = blob.GetBufferSize();
        slice::from_raw_parts(ptr, len)
    }
}

impl Error for ErrorBlob {}

impl fmt::Display for ErrorBlob {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let errors = CStr::from_bytes_with_nul(blob_as_slice(&self.errors)).unwrap();
        write!(f, "{}", errors.to_string_lossy())
    }
}

