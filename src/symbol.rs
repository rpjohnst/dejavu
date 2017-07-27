use std::{mem, ops, fmt};
use std::cell::RefCell;
use std::collections::HashMap;

/// A symbol is an index into a thread-local interner
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(u32);

impl !Send for Symbol {}

impl Symbol {
    /// Map a string to its interned symbol
    pub fn intern(string: &str) -> Self {
        Interner::with(|interner| interner.intern(string))
    }

    /// Map a symbol to its string
    pub fn as_str(self) -> InternedString {
        Interner::with(|interner| unsafe {
            InternedString(mem::transmute::<_, &str>(interner.get(self)))
        })
    }

    pub fn into_index(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}({})", self, self.0)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.as_str(), f)
    }
}

/// A thread-local interned string
pub struct InternedString(&'static str);

impl !Send for InternedString {}

impl ops::Deref for InternedString {
    type Target = str;
    fn deref(&self) -> &str { self.0 }
}

impl fmt::Debug for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.0, f)
    }
}

impl fmt::Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.0, f)
    }
}

#[derive(Default)]
struct Interner {
    names: HashMap<Box<str>, Symbol>,
    strings: Vec<Box<str>>,
}

impl Interner {
    fn fill(strings: &[&str]) -> Self {
        let mut interner = Interner::default();
        for &string in strings {
            interner.intern(string);
        }
        interner
    }

    fn intern(&mut self, string: &str) -> Symbol {
        if let Some(&name) = self.names.get(string) {
            return name;
        }

        let name = Symbol(self.strings.len() as u32);
        let string = string.to_string().into_boxed_str();
        self.strings.push(string.clone());
        self.names.insert(string, name);
        name
    }

    fn get(&self, name: Symbol) -> &str {
        &self.strings[name.0 as usize]
    }

    fn with<T, F: FnOnce(&mut Interner) -> T>(f: F) -> T {
        thread_local!(static INTERNER: RefCell<Interner> = {
            RefCell::new(Interner::new())
        });
        INTERNER.with(|interner| f(&mut *interner.borrow_mut()))
    }
}

macro_rules! declare_keywords {(
    $( ($index: expr, $name: ident, $string: expr) )*
) => {
    #[allow(non_upper_case_globals)]
    pub mod keyword {
        use super::Symbol;

        $(
            pub const $name: Symbol = Symbol($index);
        )*
    }

    impl Interner {
        fn new() -> Self {
            Interner::fill(&[$($string,)*])
        }
    }
}}

declare_keywords! {
    (0, True, "true")
    (1, False, "false")

    (2, Self_, "self")
    (3, Other, "other")
    (4, All, "all")
    (5, NoOne, "noone")
    (6, Global, "global")
    (7, Local, "local")

    (8, Var, "var")
    (9, GlobalVar, "globalvar")

    (10, If, "if")
    (11, Then, "then")
    (12, Else, "else")
    (13, Repeat, "repeat")
    (14, While, "while")
    (15, Do, "do")
    (16, Until, "until")
    (17, For, "for")
    (18, With, "with")
    (19, Switch, "switch")
    (20, Case, "case")
    (21, Default, "default")
    (22, Break, "break")
    (23, Continue, "continue")
    (24, Exit, "exit")
    (25, Return, "return")

    (26, Begin, "begin")
    (27, End, "end")

    (28, Not, "not")
    (29, Div, "div")
    (30, Mod, "mod")
    (31, And, "and")
    (32, Or, "or")
    (33, Xor, "xor")
}

impl Symbol {
    pub fn is_keyword(&self) -> bool {
        self.0 <= 33
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern() {
        let mut i = Interner::default();

        assert_eq!(i.intern("dog"), Symbol(0));
        assert_eq!(i.intern("dog"), Symbol(0));
        assert_eq!(i.intern("cat"), Symbol(1));
        assert_eq!(i.intern("cat"), Symbol(1));
        assert_eq!(i.intern("dog"), Symbol(0));
    }
}
