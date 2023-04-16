use std::{ops, cmp, fmt};
use std::num::NonZeroUsize;
use std::marker::PhantomData;
use std::hash::{Hash, Hasher};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_char;
use bstr::ByteSlice;

/// A symbol is an index into a thread-local interner.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    // Symbols must be non-zero for use in `vm::Value`.
    index: NonZeroUsize,

    // Symbols must be `!Send` and `!Sync` to avoid crossing interners.
    _marker: PhantomData<*const [u8]>,
}

struct Interner {
    strings: HashSet<Entry>,
    indices: Vec<*const [u8]>,
}

struct Entry {
    string: Box<[u8]>,
    index: NonZeroUsize,
}

impl Symbol {
    /// Map a string to its interned symbol.
    pub fn intern(bytes: &[u8]) -> Self {
        Interner::with(|interner| Symbol { index: interner.intern(bytes), _marker: PhantomData })
    }

    pub fn into_index(self) -> NonZeroUsize { self.index }

    pub fn from_index(index: NonZeroUsize) -> Symbol { Symbol { index, _marker: PhantomData } }

    pub fn as_cstr(self) -> *const c_char { self.as_ptr() as *const c_char }
}

impl Default for Symbol {
    fn default() -> Self { Symbol::intern(b"") }
}

impl ops::Deref for Symbol {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // Safety: `Symbol` is not `Send` or `Sync`, and is always allocated from a thread-local
        // `Interner`. This ensures the string will not be freed until the thread dies and takes
        // all associated `Symbol`s with it.
        unsafe { &*Interner::with(|interner| interner.get(self.index)) }
    }
}

impl Borrow<[u8]> for Symbol {
    fn borrow(&self) -> &[u8] { self }
}

impl cmp::Ord for Symbol {
    fn cmp(&self, other: &Self) -> cmp::Ordering { <[u8]>::cmp(self, other) }
}

impl cmp::PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(Self::cmp(self, other)) }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_bstr(), f)?;
        f.write_str("@")?;
        fmt::Debug::fmt(&self.index, f)?;
        Ok(())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_bstr(), f)?;
        Ok(())
    }
}

impl Interner {
    fn intern(&mut self, string: &[u8]) -> NonZeroUsize {
        if let Some(entry) = self.strings.get(string) {
            return entry.index;
        }

        let mut buffer = Vec::with_capacity(string.len() + 1);
        buffer.extend_from_slice(string);
        buffer.push(b'\0');
        let string = buffer.into_boxed_slice();
        let data = &string[..string.len() - 1] as *const [u8];
        // Safety: `self.indices` always has at least one entry.
        let index = unsafe { NonZeroUsize::new_unchecked(self.indices.len()) };
        self.strings.insert(Entry { string, index });
        self.indices.push(data);

        index
    }

    fn get(&self, index: NonZeroUsize) -> *const [u8] {
        self.indices[index.get()]
    }

    fn with<T, F: FnOnce(&mut Interner) -> T>(f: F) -> T {
        thread_local!(static INTERNER: RefCell<Interner> = RefCell::new(Interner::with_keywords()));
        INTERNER.with(|interner| f(&mut *interner.borrow_mut()))
    }
}

impl Default for Interner {
    fn default() -> Self {
        Interner { strings: HashSet::default(), indices: vec![b"UNUSED\0"] }
    }
}

impl Borrow<[u8]> for Entry {
    fn borrow(&self) -> &[u8] { &self.string[..self.string.len() - 1] }
}

impl cmp::Eq for Entry {}

impl cmp::PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool { <[u8] as PartialEq>::eq(self.borrow(), other.borrow()) }
}

impl Hash for Entry {
    fn hash<H: Hasher>(&self, state: &mut H) { <[u8]>::hash(self.borrow(), state) }
}

macro_rules! declare_symbols {(
    keywords: $(($index: expr, $name: ident, $string: expr))*
    arguments: $(($symbol_index: expr, $argument_index: expr))*
) => {
    #[allow(non_upper_case_globals)]
    pub mod keyword {
        use std::num::NonZeroUsize;
        use std::marker::PhantomData;
        use super::Symbol;

        // Safety: The indices below are all non-zero.
        $(pub const $name: Symbol = unsafe {
            let index = NonZeroUsize::new_unchecked($index);
            Symbol { index, _marker: PhantomData }
        };)*
    }

    impl Interner {
        fn with_keywords() -> Self {
            let mut interner = Self::default();

            $(interner.intern($string.as_bytes());)*
            $(interner.intern(concat!("argument", $argument_index).as_bytes());)*

            interner
        }
    }
}}

declare_symbols! {
keywords:
    (1, True, "true")
    (2, False, "false")

    (3, Self_, "self")
    (4, Other, "other")
    (5, All, "all")
    (6, NoOne, "noone")
    (7, Global, "global")
    (8, Local, "local")

    (9, Var, "var")
    (10, GlobalVar, "globalvar")

    (11, If, "if")
    (12, Then, "then")
    (13, Else, "else")
    (14, Repeat, "repeat")
    (15, While, "while")
    (16, Do, "do")
    (17, Until, "until")
    (18, For, "for")
    (19, With, "with")
    (20, Switch, "switch")
    (21, Case, "case")
    (22, Default, "default")
    (23, Break, "break")
    (24, Continue, "continue")
    (25, Exit, "exit")
    (26, Return, "return")

    (27, Begin, "begin")
    (28, End, "end")

    (29, Not, "not")
    (30, Div, "div")
    (31, Mod, "mod")
    (32, And, "and")
    (33, Or, "or")
    (34, Xor, "xor")

arguments:
    (35, 0)
    (36, 1)
    (37, 2)
    (38, 3)
    (39, 4)
    (40, 5)
    (41, 6)
    (42, 7)
    (43, 8)
    (44, 9)
    (45, 10)
    (46, 11)
    (47, 12)
    (48, 13)
    (49, 14)
    (50, 15)
}

impl Symbol {
    pub fn is_keyword(&self) -> bool { self.index.get() < 35 }

    pub fn is_argument(&self) -> bool { 35 <= self.index.get() && self.index.get() < 51 }

    pub fn as_argument(&self) -> Option<u32> {
        if self.is_argument() { Some(self.index.get() as u32 - 35) } else { None }
    }

    pub fn from_argument(argument: u32) -> Symbol {
        assert!(argument < 16);
        let index = NonZeroUsize::new(35 + argument as usize).unwrap();
        Symbol::from_index(index)
    }
}

#[cfg(test)]
mod tests {
    use super::{Symbol, keyword};

    #[test]
    fn keywords() {
        let empty = Symbol::default();
        assert_eq!(empty, empty);

        let keyword = Symbol::intern(b"other");
        assert_eq!(keyword, keyword::Other);

        let arg = Symbol::intern(b"argument3");
        assert_eq!(arg, Symbol::from_argument(3));
    }

    #[test]
    fn alloc() {
        let dog1 = Symbol::intern(b"dog");
        assert_eq!(&*dog1, b"dog");

        let dog2 = Symbol::intern(b"dog");
        assert_eq!(&*dog2, b"dog");
        assert_eq!(dog1, dog2);

        let cat1 = Symbol::intern(b"cat");
        assert_eq!(&*cat1, b"cat");

        let cat2 = Symbol::intern(b"cat");
        assert_eq!(&*cat2, b"cat");
        assert_eq!(cat1, cat2);

        assert_ne!(dog1, cat1);
    }
}
