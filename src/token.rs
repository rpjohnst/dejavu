use symbol::Symbol;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Token {
    Eof,
    Unexpected(char),

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
