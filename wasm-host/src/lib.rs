use std::fmt;
use wasm_bindgen::prelude::*;
use js_sys::Function;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print(format_args!($($arg)*), false, false));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($args:tt)*) => ({
        $crate::print(format_args!($($args)*), true, false);
    })
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => ($crate::print(format_args!($($arg)*), false, true));
}

#[macro_export]
macro_rules! eprintln {
    () => (eprint!("\n"));
    ($($args:tt)*) => ({
        $crate::print(format_args!($($args)*), true, true);
    })
}

#[wasm_bindgen(module = "/src/index.js")]
extern "C" {
    pub fn redirect_print(out: Function, err: Function);
    fn out_print(string: &str);
    fn err_print(string: &str);
}

#[doc(hidden)]
pub fn print(args: fmt::Arguments<'_>, append_newline: bool, error: bool) {
    let mut string = fmt::format(args);
    if append_newline {
        string.push_str("\n");
    }

    match error {
        false => out_print(&string),
        true => err_print(&string),
    }
}
