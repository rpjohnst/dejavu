#![feature(optin_builtin_traits, box_patterns, box_syntax, slice_patterns)]

use std::path::PathBuf;

mod symbol;
mod entity;
mod bitvec;

pub mod front;
pub mod back;
pub mod vm;

pub struct SourceFile {
    pub name: PathBuf,
    pub source: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Span {
    pub low: usize,
    pub high: usize,
}

pub struct ErrorHandler;

impl ErrorHandler {
    pub fn error(&self, span: Span, message: &str) {
        println!("{}-{}: error: {}", span.low, span.high, message);
    }
}
