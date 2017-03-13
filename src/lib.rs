#![feature(optin_builtin_traits, box_patterns, box_syntax)]

use std::path::PathBuf;

mod symbol;
mod entity;

mod token;
mod ast;
mod ssa;

mod lexer;
mod parser;
mod codegen;

pub struct SourceFile {
    pub name: PathBuf,
    pub source: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Span {
    pub low: usize,
    pub high: usize,
}

pub use codegen::Codegen;
pub use parser::Parser;
pub use lexer::Reader;

pub struct ErrorHandler;

impl ErrorHandler {
    pub fn error(&self, span: Span, message: &str) {
        println!("{}-{}: error: {}", span.low, span.high, message);
    }
}
