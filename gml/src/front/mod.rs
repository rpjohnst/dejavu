pub use crate::front::lexer::Lexer;
pub use crate::front::parser::Parser;
pub use crate::front::codegen::Codegen;

use crate::symbol::Symbol;

pub mod token;
pub mod ast;

mod lexer;
mod parser;
mod ssa;
mod codegen;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Span {
    pub low: usize,
    pub high: usize,
}

pub fn compute_lines(source: &str) -> Vec<usize> {
    let mut lines = vec![0];
    lines.extend(source.bytes()
        .enumerate()
        .filter(|&(_, b)| b == b'\n')
        .map(|(i, _)| i + 1));
    lines
}

pub fn get_position(lines: &[usize], pos: usize) -> (usize, usize) {
    let line = match lines.binary_search(&pos) {
        Ok(line) => line,
        Err(line) => line - 1,
    };
    let column = pos - lines[line];
    (line + 1, column + 1)
}

pub trait ErrorHandler {
    fn error(&mut self, span: Span, message: &str);
}

pub struct ErrorPrinter {
    name: Symbol,
    lines: Vec<usize>,
}

impl ErrorPrinter {
    pub fn new(name: Symbol, source: &str) -> Self {
        ErrorPrinter {
            name,
            lines: compute_lines(source),
        }
    }
}

impl ErrorHandler for ErrorPrinter {
    fn error(&mut self, span: Span, message: &str) {
        let (line, column) = get_position(&self.lines, span.low);
        eprintln!("error: {}:{}:{}: {}", self.name, line, column, message);
    }
}
