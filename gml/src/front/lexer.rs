use std::str;
use crate::symbol::Symbol;
use crate::front::Span;
use crate::front::token::{Token, BinOp, Delim};

pub struct Lexer<'s> {
    source: &'s [u8],
    position: usize,
}

impl<'s> Lexer<'s> {
    pub fn new(source: &'s [u8], position: usize) -> Lexer<'s> {
        Lexer { source, position }
    }

    pub fn read_token(&mut self) -> (Token, Span) {
        self.scan_whitespace_or_comment();

        let low = self.position;
        let token = if is_ident_start(self.current()) {
            self.scan_ident_or_keyword()
        } else if
            is_digit(self.current()) ||
            self.current() == Some(b'$') ||
            (self.current() == Some(b'.') && is_digit(self.next_char()))
        {
            self.scan_real()
        } else if [Some(b'"'), Some(b'\'')].contains(&self.current()) {
            self.scan_string()
        } else if is_operator(self.current()) {
            self.scan_operator()
        } else if let Some(c) = self.current() {
            self.advance_byte();
            Token::Unexpected(c)
        } else {
            Token::Eof
        };
        let high = self.position;

        (token, Span { low: low, high: high })
    }

    fn scan_whitespace_or_comment(&mut self) {
        loop {
            match self.current() {
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => (),

                Some(b'/') if self.next_char() == Some(b'/') => {
                    self.advance_byte();
                    self.advance_byte();

                    loop {
                        if let Some(b'\n') | None = self.current() {
                            break;
                        }
                        self.advance_byte();
                    }
                }

                Some(b'/') if self.next_char() == Some(b'*') => {
                    self.advance_byte();
                    self.advance_byte();

                    loop {
                        match self.current() {
                            None => break,
                            Some(b'*') if self.next_char() == Some(b'/') => {
                                self.advance_byte();
                                break;
                            }
                            _ => (),
                        }
                        self.advance_byte();
                    }
                }

                _ => break,
            }

            self.advance_byte();
        }
    }

    fn scan_ident_or_keyword(&mut self) -> Token {
        let source = &self.source[..];
        let low = self.position;
        self.advance_byte();
        while is_ident_continue(self.current()) {
            self.advance_byte();
        }
        let high = self.position;

        // Identifiers and keywords are UTF-8 by construction.
        let ident = str::from_utf8(&source[..high - low]).unwrap();
        let symbol = Symbol::intern(ident);
        if symbol.is_keyword() {
            Token::Keyword(symbol)
        } else {
            Token::Ident(symbol)
        }
    }

    fn scan_real(&mut self) -> Token {
        let source = &self.source[..];
        let low = self.position;

        let radix = match self.current() {
            Some(b'$') => {
                self.advance_byte();
                16
            }

            _ => 10,
        };

        while self.current().map(|c| (c as char).is_digit(radix)).unwrap_or(false) {
            self.advance_byte();
        }

        if
            radix == 10 &&
            self.current() == Some(b'.') &&
            is_digit(self.next_char())
        {
            self.advance_byte();
            while self.current().map(|c| (c as char).is_digit(radix)).unwrap_or(false) {
                self.advance_byte();
            }
        }

        let high = self.position;

        // Real literals are UTF-8 by construction.
        let real = str::from_utf8(&source[..high - low]).unwrap();
        let symbol = Symbol::intern(real);
        Token::Real(symbol)
    }

    fn scan_string(&mut self) -> Token {
        let delim = self.current();

        let source = &self.source[..];
        let low = self.position;
        self.advance_byte();

        while self.current() != delim && self.current() != None {
            self.advance_byte();
        }

        self.advance_byte();
        let high = self.position;

        // String literals may contain invalid UTF-8.
        let string = String::from_utf8_lossy(&source[..high - low]);
        let symbol = Symbol::intern(&string);
        Token::String(symbol)
    }

    fn scan_operator(&mut self) -> Token {
        match self.advance_byte() {
            Some(b'(') => Token::OpenDelim(Delim::Paren),
            Some(b')') => Token::CloseDelim(Delim::Paren),
            Some(b'[') => Token::OpenDelim(Delim::Bracket),
            Some(b']') => Token::CloseDelim(Delim::Bracket),
            Some(b'{') => Token::OpenDelim(Delim::Brace),
            Some(b'}') => Token::CloseDelim(Delim::Brace),

            Some(b'<') => match self.current() {
                Some(b'=') => { self.advance_byte(); Token::Le }
                Some(b'<') => { self.advance_byte(); Token::Shl }
                Some(b'>') => { self.advance_byte(); Token::LtGt }
                _ => Token::Lt
            }
            Some(b'=') => match self.current() {
                Some(b'=') => { self.advance_byte(); Token::EqEq }
                _ => Token::Eq
            },
            Some(b'!') => match self.current() {
                Some(b'=') => { self.advance_byte(); Token::Ne }
                _ => Token::Bang
            },
            Some(b'>') => match self.current() {
                Some(b'=') => { self.advance_byte(); Token::Ge }
                Some(b'>') => { self.advance_byte(); Token::Shr }
                _ => Token::Gt
            },

            Some(b'+') => self.scan_binop(BinOp::Plus),
            Some(b'-') => self.scan_binop(BinOp::Minus),
            Some(b'*') => self.scan_binop(BinOp::Star),
            Some(b'/') => self.scan_binop(BinOp::Slash),

            Some(b'&') => match self.current() {
                Some(b'&') => { self.advance_byte(); Token::And }
                _ => self.scan_binop(BinOp::Ampersand)
            },
            Some(b'|') => match self.current() {
                Some(b'|') => { self.advance_byte(); Token::Or }
                _ => self.scan_binop(BinOp::Pipe)
            },
            Some(b'^') => match self.current() {
                Some(b'^') => { self.advance_byte(); Token::Xor }
                _ => self.scan_binop(BinOp::Caret)
            },

            Some(b'~') => Token::Tilde,

            Some(b'.') => Token::Dot,
            Some(b',') => Token::Comma,
            Some(b';') => Token::Semicolon,
            Some(b':') => match self.current() {
                Some(b'=') => { self.advance_byte(); Token::ColonEq }
                _ => Token::Colon
            },

            Some(c) => Token::Unexpected(c),
            None => Token::Eof,
        }
    }

    fn scan_binop(&mut self, op: BinOp) -> Token {
        if self.current() == Some(b'=') {
            self.advance_byte();
            Token::BinOpEq(op)
        } else {
            Token::BinOp(op)
        }
    }

    fn advance_byte(&mut self) -> Option<u8> {
        if let Some((&current, rest)) = self.source.split_first() {
            self.source = rest;
            self.position += 1;

            Some(current)
        } else {
            None
        }
    }

    fn current(&self) -> Option<u8> {
        self.source.get(0).copied()
    }

    fn next_char(&self) -> Option<u8> {
        self.source.get(1).copied()
    }
}

fn is_ident_start(c: Option<u8>) -> bool {
    (Some(b'a') <= c && c <= Some(b'z')) ||
    (Some(b'A') <= c && c <= Some(b'Z')) ||
    c == Some(b'_')
}

fn is_ident_continue(c: Option<u8>) -> bool {
    is_ident_start(c) || is_digit(c)
}

fn is_digit(c: Option<u8>) -> bool {
    Some(b'0') <= c && c <= Some(b'9')
}

fn is_operator(c: Option<u8>) -> bool {
    [
        Some(b'{'), Some(b'}'), Some(b'('), Some(b')'), Some(b'['), Some(b']'),
        Some(b'.'), Some(b','), Some(b':'), Some(b';'),
        Some(b'+'), Some(b'-'), Some(b'*'), Some(b'/'),
        Some(b'|'), Some(b'&'), Some(b'^'), Some(b'~'),
        Some(b'='), Some(b'<'), Some(b'>'),
        Some(b'!'),
    ].contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(id: &str) -> Token {
        Token::Ident(Symbol::intern(id))
    }

    fn keyword(id: &str) -> Token {
        let symbol = Symbol::intern(id);
        assert!(symbol.is_keyword());
        Token::Keyword(symbol)
    }

    fn real(real: &str) -> Token {
        Token::Real(Symbol::intern(real))
    }

    fn span(low: usize, high: usize) -> Span {
        Span { low: low, high: high }
    }

    #[test]
    fn spans() {
        let mut lexer = Lexer::new(b"/* comment */ var foo; foo = 3", 0);

        assert_eq!(lexer.read_token(), (keyword("var"), span(14, 17)));
        assert_eq!(lexer.read_token(), (ident("foo"), span(18, 21)));
        assert_eq!(lexer.read_token(), (Token::Semicolon, span(21, 22)));
        assert_eq!(lexer.read_token(), (ident("foo"), span(23, 26)));
        assert_eq!(lexer.read_token(), (Token::Eq, span(27, 28)));
        assert_eq!(lexer.read_token(), (real("3"), span(29, 30)));
        assert_eq!(lexer.read_token(), (Token::Eof, span(30, 30)));
    }
}
