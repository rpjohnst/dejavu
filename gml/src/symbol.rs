use std::{ptr, slice, str};
use std::ops::Deref;
use std::cmp::{self, Eq, PartialEq, Ord, PartialOrd};
use std::fmt::{self, Debug, Display};
use std::hash::{Hash, Hasher};
use std::borrow::Borrow;
use std::alloc::Layout;
use std::cell::RefCell;
use std::collections::HashSet;

/// A string in a thread-local interner.
///
/// Equality and hash are based on pointer identity.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol { entry: *const Entry }

/// A set of unique strings.
#[derive(Default)]
struct Interner {
    /// Pointers to entries allocated in `arena`.
    ///
    /// These references point into `self.arena`, and are not really `'static`, but this lets
    /// `HashSet` pick up the right `Hash` and `Eq` impls.
    entries: RefCell<HashSet<&'static Entry>>,

    /// Actual storage for `entries`.
    arena: quickdry::Arena,
}

/// An interned string and its metadata.
///
/// The length is stored at the start of the allocation, to keep `Entry: Sized`.
/// Equality and hash are based only on string content.
#[repr(C)]
struct Entry { len: usize, kind: Kind, data: [u8; 0] }

/// A symbol equivalence class.
#[repr(u32)]
#[derive(Copy, Clone)]
enum Kind { None, Keyword, Argument(u32) }

impl Symbol {
    /// Intern a string in the current thread's interner.
    pub fn intern(string: &str) -> Self { Self::with_kind(string, Kind::None) }

    fn with_kind(string: &str, kind: Kind) -> Self {
        thread_local! { static INTERNER: Interner = Interner::with_keywords(); }
        INTERNER.with(|interner| Symbol { entry: interner.intern(string, kind) })
    }

    /// Return the wrapped raw pointer.
    pub fn into_raw(self) -> *const u8 { self.entry as *const _ }

    /// Construct a `Symbol` from a raw pointer, obtained from `Symbol::into_raw`.
    pub unsafe fn from_raw(raw: *const u8) -> Self { Symbol { entry: raw as *const _ } }

    fn entry(&self) -> &Entry {
        // Safety: `Symbol` is not `Send` or `Sync`, and is always allocated from a thread-local
        // `Interner`. This ensures the associated `Entry` will not be freed until the thread dies
        // and takes all associated `Symbol`s with it.
        unsafe { &*self.entry }
    }
}

impl Default for Symbol {
    fn default() -> Self { EMPTY }
}

impl Deref for Symbol {
    type Target = str;
    fn deref(&self) -> &str { self.entry().borrow() }
}

impl Borrow<str> for Symbol {
    fn borrow(&self) -> &str { self }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> cmp::Ordering { str::cmp(self, other) }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(Self::cmp(self, other)) }
}

impl Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <str as Debug>::fmt(self, f)?;
        write!(f, "@")?;
        <*const Entry as Debug>::fmt(&self.entry, f)?;
        Ok(())
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { <str as Display>::fmt(self, f) }
}

impl Interner {
    /// Look up a string and insert it if it's new.
    fn intern<'a>(&'a self, string: &str, kind: Kind) -> &'a Entry {
        if let Some(&entry) = self.entries.borrow_mut().get(string) {
            return entry;
        }

        let len = string.len();
        let layout = Layout::new::<Entry>();

        // Safety:
        // * `Entry::data` is carefully aligned to match the end of `Entry`, with no subsequent
        //   padding, so we can use it elsewhere to compute the offset of the string.
        // * The entry is allocated in `self.arena` and only escapes into client code as
        //   `&'a Entry`, so it can go into `self.entries` as `&'static Entry`.
        let entry = unsafe {
            let layout = Layout::from_size_align_unchecked(layout.size() + len, layout.align());
            let entry = self.arena.alloc(layout) as *mut Entry;
            ptr::write(entry, Entry { len, kind, data: [] });
            ptr::copy_nonoverlapping(string.as_ptr(), entry.add(1) as *mut u8, len);
            &*entry
        };

        self.entries.borrow_mut().insert(entry);
        entry
    }

    /// Insert a statically-allocated `Entry` into the interner.
    fn insert(&self, entry: &'static Entry) {
        assert_eq!(self.entries.borrow_mut().insert(entry), true);
    }
}

impl Borrow<str> for Entry {
    fn borrow(&self) -> &str {
        // Safety: `Entry` is always allocated with a following `str` of length `self.len`.
        unsafe {
            let slice = slice::from_raw_parts(self.data.as_ptr(), self.len);
            str::from_utf8_unchecked(slice)
        }
    }
}

// Shim impl to let `Interner::intern` call `HashSet<&Entry>::get(&str)`.
impl Borrow<str> for &'_ Entry {
    fn borrow(&self) -> &str { <Entry as Borrow<str>>::borrow(*self) }
}

impl Eq for Entry {}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        <str as PartialEq<str>>::eq(self.borrow(), other.borrow())
    }
}

impl Hash for Entry {
    fn hash<H: Hasher>(&self, state: &mut H) { str::hash(self.borrow(), state) }
}

/// An `Entry` wrapper to be allocated statically.
struct StaticEntry<T: ?Sized> { entry: Entry, _data: T }

macro_rules! static_entry { ($name: ident, $string: expr, $kind: expr) => {
    static $name: &StaticEntry<[u8]> = &StaticEntry {
        entry: Entry { len: $string.len(), kind: $kind, data: [] },
        _data: *$string,
    };
}}

const EMPTY: Symbol = {
    static_entry! { EMPTY, b"", Kind::None }
    Symbol { entry: &EMPTY.entry }
};

macro_rules! declare_symbols {(
    keywords: $(($name: ident, $string: expr))*
    arguments: $(($index: expr, $argument: expr))*
) => {
    #[allow(non_upper_case_globals)]
    pub mod keyword {
        use super::{Symbol, Entry, Kind, StaticEntry};

        // Safety: See `Interner::intern`; this time the `&'static Entry` is not even a lie.

        $(pub const $name: Symbol = {
            static_entry! { ENTRY, $string, Kind::Keyword }
            Symbol { entry: &ENTRY.entry }
        };)*

        pub const ARGUMENT: [Symbol; 16] = [
            $({
                static_entry! { ENTRY, $argument, Kind::Argument($index) }
                Symbol { entry: &ENTRY.entry }
            },)*
        ];
    }

    impl Interner {
        fn with_keywords() -> Self {
            let interner = Self::default();

            interner.insert(EMPTY.entry());
            $(interner.insert(keyword::$name.entry());)*
            for argument in &keyword::ARGUMENT {
                interner.insert(argument.entry());
            }

            interner
        }
    }
}}

declare_symbols! {
keywords:
    (True, b"true")
    (False, b"false")

    (Self_, b"self")
    (Other, b"other")
    (All, b"all")
    (NoOne, b"noone")
    (Global, b"global")
    (Local, b"local")

    (Var, b"var")
    (GlobalVar, b"globalvar")

    (If, b"if")
    (Then, b"then")
    (Else, b"else")
    (Repeat, b"repeat")
    (While, b"while")
    (Do, b"do")
    (Until, b"until")
    (For, b"for")
    (With, b"with")
    (Switch, b"switch")
    (Case, b"case")
    (Default, b"default")
    (Break, b"break")
    (Continue, b"continue")
    (Exit, b"exit")
    (Return, b"return")

    (Begin, b"begin")
    (End, b"end")

    (Not, b"not")
    (Div, b"div")
    (Mod, b"mod")
    (And, b"and")
    (Or, b"or")
    (Xor, b"xor")

arguments:
    (0, b"argument0")
    (1, b"argument1")
    (2, b"argument2")
    (3, b"argument3")
    (4, b"argument4")
    (5, b"argument5")
    (6, b"argument6")
    (7, b"argument7")
    (8, b"argument8")
    (9, b"argument9")
    (10, b"argument10")
    (11, b"argument11")
    (12, b"argument12")
    (13, b"argument13")
    (14, b"argument14")
    (15, b"argument15")
}

impl Symbol {
    pub fn is_keyword(&self) -> bool {
        match self.entry().kind { Kind::Keyword => true, _ => false, }
    }

    pub fn is_argument(&self) -> bool {
        match self.entry().kind { Kind::Argument(_) => true, _ => false }
    }

    pub fn as_argument(&self) -> Option<u32> {
        match self.entry().kind { Kind::Argument(index) => Some(index), _ => None }
    }

    pub fn from_argument(argument: u32) -> Symbol {
        keyword::ARGUMENT[argument as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::Symbol;

    #[test]
    fn keywords() {
        let empty = Symbol::default();
        assert_eq!(empty, super::EMPTY);

        let keyword = Symbol::intern("other");
        assert_eq!(keyword, super::keyword::Other);

        let arg = Symbol::intern("argument3");
        assert_eq!(arg, Symbol::from_argument(3));
    }

    #[test]
    fn alloc() {
        let dog1 = Symbol::intern("dog");
        assert_eq!(&*dog1, "dog");

        let dog2 = Symbol::intern("dog");
        assert_eq!(&*dog2, "dog");
        assert_eq!(dog1, dog2);

        let cat1 = Symbol::intern("cat");
        assert_eq!(&*cat1, "cat");

        let cat2 = Symbol::intern("cat");
        assert_eq!(&*cat2, "cat");
        assert_eq!(cat1, cat2);

        assert_ne!(cat1, dog1);
    }
}
