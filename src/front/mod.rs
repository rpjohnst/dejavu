use std::path::PathBuf;

pub use front::lexer::Lexer;
pub use front::parser::Parser;
pub use front::codegen::Codegen;

mod lexer;
mod parser;
mod codegen;

mod token;
mod ast;

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
