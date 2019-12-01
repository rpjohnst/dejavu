use std::fmt;
use crate::symbol::Symbol;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Token {
    Eof,
    Unexpected(u8),

    Ident(Symbol),
    Keyword(Symbol),
    Real(Symbol),
    String(Symbol),

    OpenDelim(Delim),
    CloseDelim(Delim),

    Eq,
    ColonEq,

    Lt,
    Le,
    EqEq,
    Ne,
    LtGt,
    Ge,
    Gt,

    BinOp(BinOp),
    BinOpEq(BinOp),
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Bang,
    Tilde,

    Dot,
    Comma,
    Semicolon,
    Colon,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Delim {
    Paren,
    Bracket,
    Brace,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BinOp {
    Plus,
    Minus,
    Star,
    Slash,

    Ampersand,
    Pipe,
    Caret,
}

impl fmt::Display for Token {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Token::Eof => write!(fmt, "end of file")?,
            Token::Unexpected(c) => write!(fmt, "character '{}'", c)?,

            Token::Ident(symbol) => write!(fmt, "identifier '{}'", symbol)?,
            Token::Keyword(symbol) => write!(fmt, "keyword {}", symbol)?,
            Token::Real(symbol) => write!(fmt, "real {}", symbol)?,
            Token::String(symbol) => write!(fmt, "string \"{}\"", symbol)?,

            Token::OpenDelim(delim) => match delim {
                Delim::Paren => write!(fmt, "(")?,
                Delim::Bracket => write!(fmt, "[")?,
                Delim::Brace => write!(fmt, "{{")?,
            },
            Token::CloseDelim(delim) => match delim {
                Delim::Paren => write!(fmt, ")")?,
                Delim::Bracket => write!(fmt, "]")?,
                Delim::Brace => write!(fmt, "}}")?,
            },

            Token::Eq => write!(fmt, "=")?,
            Token::ColonEq => write!(fmt, ":=")?,

            Token::Lt => write!(fmt, "<")?,
            Token::Le => write!(fmt, "<=")?,
            Token::EqEq => write!(fmt, "==")?,
            Token::Ne => write!(fmt, "!=")?,
            Token::LtGt => write!(fmt, "<>")?,
            Token::Ge => write!(fmt, ">=")?,
            Token::Gt => write!(fmt, "<")?,

            Token::BinOp(op) => match op {
                BinOp::Plus => write!(fmt, "+")?,
                BinOp::Minus => write!(fmt, "-")?,
                BinOp::Star => write!(fmt, "*")?,
                BinOp::Slash => write!(fmt, "/")?,

                BinOp::Ampersand => write!(fmt, "&")?,
                BinOp::Pipe => write!(fmt, "|")?,
                BinOp::Caret => write!(fmt, "^")?,
            },
            Token::BinOpEq(op) => match op {
                BinOp::Plus => write!(fmt, "+=")?,
                BinOp::Minus => write!(fmt, "-=")?,
                BinOp::Star => write!(fmt, "*=")?,
                BinOp::Slash => write!(fmt, "/=")?,

                BinOp::Ampersand => write!(fmt, "&=")?,
                BinOp::Pipe => write!(fmt, "|=")?,
                BinOp::Caret => write!(fmt, "^=")?,
            },
            Token::And => write!(fmt, "&&")?,
            Token::Or => write!(fmt, "||")?,
            Token::Xor => write!(fmt, "^^")?,
            Token::Shl => write!(fmt, "<<")?,
            Token::Shr => write!(fmt, ">>")?,
            Token::Bang => write!(fmt, "!")?,
            Token::Tilde => write!(fmt, "~")?,

            Token::Dot => write!(fmt, ".")?,
            Token::Comma => write!(fmt, ",")?,
            Token::Semicolon => write!(fmt, ";")?,
            Token::Colon => write!(fmt, ":")?,
        }

        Ok(())
    }
}
