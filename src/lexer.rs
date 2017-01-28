use symbol::Symbol;
use token::{Token, BinOp, Delim};
use {SourceFile, Span};

pub struct Reader<'a> {
    source_file: &'a SourceFile,

    current: Option<char>,
    position: usize,
    next_position: usize,
}

impl<'a> Reader<'a> {
    pub fn new(source_file: &'a SourceFile) -> Reader<'a> {
        let mut reader = Reader {
            source_file: source_file,

            current: None,
            position: 0,
            next_position: 0,
        };

        reader.advance_char();
        reader
    }

    pub fn read_token(&mut self) -> (Token, Span) {
        self.scan_whitespace_or_comment();

        let low = self.position;
        let token = if self.current.map(is_ident_start).unwrap_or(false) {
            self.scan_ident_or_keyword()
        } else if
            self.current.map(is_digit).unwrap_or(false) ||
            self.current == Some('$') ||
            (self.current == Some('.') && self.next_char().map(is_digit).unwrap_or(false))
        {
            self.scan_real()
        } else if self.current.map(|ref c| ['"', '\''].contains(c)).unwrap_or(false) {
            self.scan_string()
        } else if self.current.map(is_operator).unwrap_or(false) {
            self.scan_operator()
        } else if let Some(c) = self.current {
            Token::Unexpected(c)
        } else {
            Token::Eof
        };
        let high = self.position;

        (token, Span { low: low, high: high })
    }

    fn scan_whitespace_or_comment(&mut self) {
        while let Some(c) = self.current {
            match c {
                '\r' if self.next_char() == Some('\n') => self.advance_char(),
                c if is_whitespace(c) => (),

                '/' if self.next_char() == Some('/') => {
                    self.advance_char();
                    self.advance_char();

                    while let Some(c) = self.current {
                        match c {
                            '\n' => break,
                            '\r' if self.next_char() == Some('\n') => {
                                self.advance_char();
                                break;
                            }
                            _ => (),
                        }
                        self.advance_char();
                    }
                }

                '/' if self.next_char() == Some('*') => {
                    self.advance_char();
                    self.advance_char();

                    while let Some(c) = self.current {
                        match c {
                            '*' if self.next_char() == Some('/') => {
                                self.advance_char();
                                break;
                            }
                            _ => (),
                        }
                        self.advance_char();
                    }
                }

                _ => break,
            }

            self.advance_char();
        }
    }

    fn scan_ident_or_keyword(&mut self) -> Token {
        let ref source = self.source_file.source;

        let low = self.position;
        self.advance_char();
        while self.current.map(is_ident_continue).unwrap_or(false) {
            self.advance_char();
        }
        let high = self.position;

        let symbol = Symbol::intern(&source[low..high]);
        if symbol.is_keyword() {
            Token::Keyword(symbol)
        } else {
            Token::Ident(symbol)
        }
    }

    fn scan_real(&mut self) -> Token {
        let ref source = self.source_file.source;

        let low = self.position;

        let radix = match self.current {
            Some('$') => {
                self.advance_char();
                16
            }

            _ => 10,
        };

        while self.current.map(|c| c.is_digit(radix)).unwrap_or(false) {
            self.advance_char();
        }

        if
            radix == 10 &&
            self.current == Some('.') &&
            self.next_char().map(is_digit).unwrap_or(false)
        {
            self.advance_char();
            while self.current.map(|c| c.is_digit(radix)).unwrap_or(false) {
                self.advance_char();
            }
        }

        let high = self.position;

        let symbol = Symbol::intern(&source[low..high]);
        Token::Real(symbol)
    }

    fn scan_string(&mut self) -> Token {
        let ref source = self.source_file.source;

        let low = self.position;

        let delim = self.current.unwrap();
        self.advance_char();

        while self.current.map(|c| c != delim).unwrap_or(false) {
            self.advance_char();
        }

        let high = self.position;

        let symbol = Symbol::intern(&source[low..high]);
        Token::String(symbol)
    }

    fn scan_operator(&mut self) -> Token {
        let c = match self.current {
            Some(c) => {
                self.advance_char();
                c
            }
            None => return Token::Eof,
        };

        match c {
            '(' => Token::OpenDelim(Delim::Paren),
            ')' => Token::CloseDelim(Delim::Paren),
            '[' => Token::OpenDelim(Delim::Bracket),
            ']' => Token::CloseDelim(Delim::Bracket),
            '{' => Token::OpenDelim(Delim::Brace),
            '}' => Token::CloseDelim(Delim::Brace),

            '<' => match self.current {
                Some('=') => { self.advance_char(); Token::Le }
                Some('<') => { self.advance_char(); Token::Shl }
                _ => Token::Lt
            },
            '=' => match self.current {
                Some('=') => { self.advance_char(); Token::EqEq }
                _ => Token::Eq
            },
            '!' => match self.current {
                Some('=') => { self.advance_char(); Token::Ne }
                _ => Token::Bang
            },
            '>' => match self.current {
                Some('=') => { self.advance_char(); Token::Ge }
                Some('>') => { self.advance_char(); Token::Shr }
                _ => Token::Gt
            },

            '+' => self.scan_binop(BinOp::Plus),
            '-' => self.scan_binop(BinOp::Minus),
            '*' => self.scan_binop(BinOp::Star),
            '/' => self.scan_binop(BinOp::Slash),

            '&' => match self.current {
                Some('&') => { self.advance_char(); Token::And }
                _ => self.scan_binop(BinOp::Ampersand)
            },
            '|' => match self.current {
                Some('|') => { self.advance_char(); Token::Or }
                _ => self.scan_binop(BinOp::Pipe)
            },
            '^' => match self.current {
                Some('^') => { self.advance_char(); Token::Xor }
                _ => self.scan_binop(BinOp::Caret)
            },

            '~' => Token::Tilde,

            '.' => Token::Dot,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            ':' => match self.current {
                Some('=') => Token::ColonEq,
                _ => Token::Colon,
            },

            c => Token::Unexpected(c),
        }
    }

    fn scan_binop(&mut self, op: BinOp) -> Token {
        if self.current == Some('=') {
            self.advance_char();
            Token::BinOpEq(op)
        } else {
            Token::BinOp(op)
        }
    }

    fn advance_char(&mut self) {
        self.current = self.next_char();
        self.position = self.next_position;
        self.next_position = self.next_position + self.current.map(char::len_utf8).unwrap_or(0);
    }

    fn next_char(&self) -> Option<char> {
        let ref source = self.source_file.source;

        source[self.next_position..].chars().next()
    }
}

fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

fn is_ident_start(c: char) -> bool {
    ('a' <= c && c <= 'z') || ('A' <= c && c <= 'Z') || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    is_ident_start(c) || is_digit(c)
}

fn is_digit(c: char) -> bool {
    '0' <= c && c <= '9'
}

fn is_operator(c: char) -> bool {
    [
        '{', '}', '(', ')', '[', ']',
        '.', ',', ':', ';',
        '+', '-', '*', '/',
        '|', '&', '^', '~',
        '=', '<', '>',
        '!',
    ].contains(&c)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use super::*;

    fn setup(source: &str) -> SourceFile {
        SourceFile {
            name: PathBuf::from("<test>"),
            source: String::from(source),
        }
    }

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
        let source = setup("/* comment */ var foo; foo = 3");
        let mut reader = Reader::new(&source);

        assert_eq!(reader.read_token(), (keyword("var"), span(14, 17)));
        assert_eq!(reader.read_token(), (ident("foo"), span(18, 21)));
        assert_eq!(reader.read_token(), (Token::Semicolon, span(21, 22)));
        assert_eq!(reader.read_token(), (ident("foo"), span(23, 26)));
        assert_eq!(reader.read_token(), (Token::Eq, span(27, 28)));
        assert_eq!(reader.read_token(), (real("3"), span(29, 30)));
        assert_eq!(reader.read_token(), (Token::Eof, span(30, 30)));
    }
}
