use std::{mem, ops, cmp, fmt};
use std::hash::{Hash, Hasher};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashSet;

/// A symbol is an index into a thread-local interner
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(u32);

impl !Send for Symbol {}
impl !Sync for Symbol {}

impl Symbol {
    /// Map a string to its interned symbol
    pub fn intern(string: &str) -> Self {
        Interner::with(|interner| interner.intern(string))
    }

    pub fn into_index(self) -> u32 {
        self.0
    }

    pub fn from_index(index: u32) -> Symbol {
        Symbol(index)
    }
}

impl ops::Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &str {
        Interner::with(|interner| unsafe { mem::transmute(interner.get(*self)) })
    }
}

impl Borrow<str> for Symbol {
    fn borrow(&self) -> &str {
        self
    }
}

impl cmp::PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        let a: &str = self;
        let b: &str = other;
        a.partial_cmp(b)
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}({})", self, self.0)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self)
    }
}

#[derive(Default)]
struct Interner {
    strings: HashSet<Entry>,
    ids: Vec<*const str>,
}

struct Entry {
    string: Box<str>,
    id: u32,
}

impl cmp::PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        let a: &str = self.borrow();
        let b: &str = other.borrow();
        a == b
    }
}

impl cmp::Eq for Entry {}

impl Hash for Entry {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        let a: &str = self.borrow();
        a.hash(state)
    }
}

impl Borrow<str> for Entry {
    fn borrow(&self) -> &str {
        &self.string
    }
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
        if let Some(entry) = self.strings.get(string) {
            return Symbol(entry.id);
        }

        let string = String::from(string).into_boxed_str();
        let data = &*string as *const str;
        let id = self.ids.len() as u32;
        self.strings.insert(Entry { string, id });
        self.ids.push(data);

        Symbol(id)
    }

    fn get(&self, name: Symbol) -> &str {
        unsafe { &*self.ids[name.0 as usize] }
    }

    fn with<T, F: FnOnce(&mut Interner) -> T>(f: F) -> T {
        thread_local!(static INTERNER: RefCell<Interner> = {
            RefCell::new(Interner::new())
        });
        INTERNER.with(|interner| f(&mut *interner.borrow_mut()))
    }
}

macro_rules! declare_symbols {(
    keywords: $(($index: expr, $name: ident, $string: expr))*
    arguments: $(($symbol_index: expr, $argument_index: expr))*
) => {
    #[allow(non_upper_case_globals)]
    pub mod keyword {
        use super::Symbol;

        $(pub const $name: Symbol = Symbol($index);)*
    }

    impl Interner {
        fn new() -> Self {
            Interner::fill(&[
                $($string,)*
                $(concat!("argument", $argument_index),)*
            ])
        }
    }
}}

declare_symbols! {
keywords:
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

arguments:
    (34, 0)
    (35, 1)
    (36, 2)
    (37, 3)
    (38, 4)
    (39, 5)
    (40, 6)
    (41, 7)
    (42, 8)
    (43, 9)
    (44, 10)
    (45, 11)
    (46, 12)
    (47, 13)
    (48, 14)
    (49, 15)
}

impl Symbol {
    pub fn is_keyword(&self) -> bool {
        self.0 < 34
    }

    pub fn is_argument(&self) -> bool {
        34 <= self.0 && self.0 < 50
    }

    pub fn as_argument(&self) -> Option<u32> {
        if self.is_argument() {
            Some(self.0 - 34)
        } else {
            None
        }
    }

    pub fn from_argument(argument: u32) -> Symbol {
        assert!(argument < 16);
        Symbol::from_index(34 + argument)
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
