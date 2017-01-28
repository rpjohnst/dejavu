#![feature(optin_builtin_traits)]

use std::path::PathBuf;

mod symbol;
mod token;
mod lexer;
mod ast;
mod parser;

pub struct SourceFile {
    pub name: PathBuf,
    pub source: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Span {
    pub low: usize,
    pub high: usize,
}

pub use parser::Parser;
pub use lexer::Reader;

pub struct ErrorHandler;

impl ErrorHandler {
    pub fn error(&self, span: Span, message: &str) {
        println!("{}-{}: error: {}", span.low, span.high, message);
    }
}
