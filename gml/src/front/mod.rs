use std::path::PathBuf;

pub use crate::front::lexer::Lexer;
pub use crate::front::parser::Parser;
pub use crate::front::codegen::Codegen;

pub mod token;
pub mod ast;

mod lexer;
mod parser;
mod ssa;
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

pub struct ErrorHandler;

impl ErrorHandler {
    pub fn error(&self, span: Span, message: &str) {
        println!("{}-{}: error: {}", span.low, span.high, message);
    }
}
