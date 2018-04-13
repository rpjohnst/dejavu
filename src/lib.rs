#![feature(optin_builtin_traits)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(slice_patterns)]
#![feature(from_ref)]
#![feature(clone_closures)]
#![feature(range_contains)]

pub mod front;
pub mod back;
pub mod vm;

pub mod symbol;
mod handle_map;
mod bit_vec;
mod index_map;
